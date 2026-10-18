use crate::clipboard::ClipboardWorker;
use crate::history::History;
use crate::model::{
    ConnectionProfile, ItemPreview, SessionSettings, is_wayland, validate_transfer_size,
};
use crate::notifications;
use crate::operation::{Controller, Operation};
use crate::pending::PendingShare;
use crate::shortcuts::{Action, Bindings, Registry};
use serde::Serialize;
use ssh_clipboard::client::connection::Connection;
use ssh_clipboard::client::diagnostics::{
    ConnectionReport, diagnose, diagnose_shelf, server_problem,
};
use ssh_clipboard::client::monitor::{self, Target as MonitorTarget};
use ssh_clipboard::client::session::{Action as ShelfAction, SessionPolicy};
use ssh_clipboard::client::transport::{make_request, send_request};
use ssh_clipboard::configuration::{ConfigStore, Configuration};
use ssh_clipboard::protocol::{ClipboardValue, ErrorCode, RequestKind, ResponseKind};
use ssh_clipboard::session_protocol::{
    ContentType as ShelfContent, Origin as ShelfOrigin, Representation as ShelfRepresentation,
    Response as ShelfResponse,
};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use tokio::sync::{Mutex, watch};
use uuid::Uuid;

struct Session {
    settings: SessionSettings,
    item: Option<ClipboardValue>,
    history: History,
    connection: Option<Connection>,
    clipboard_checked: bool,
    shortcuts: Bindings,
    shortcut_status: String,
    tray_status: String,
    connected: bool,
    saved_config: Option<Configuration>,
    settings_notice: Option<String>,
    connection_report: Option<ConnectionReport>,
    operation_notice: Option<OperationNotice>,
    operation_counter: u64,
    notification_status: notifications::Status,
    history_status: Option<monitor::Status>,
    metadata_barrier: tokio::time::Instant,
    pending_share: Option<PendingShare>,
    startup_status: ssh_clipboard_desktop_prototype::startup::Status,
    updates: crate::updates::Updates,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct OperationNotice {
    id: String,
    message: String,
    failed: bool,
}

struct Desktop {
    session: Arc<Mutex<Session>>,
    operations: Controller,
    open_shortcut: AtomicU64,
    pressed_shortcuts: StdMutex<HashSet<u32>>,
    recording_shortcut: AtomicBool,
    clipboard: ClipboardWorker,
    config_store: Option<ConfigStore>,
    monitor_request: watch::Sender<monitor::Request>,
    monitor_tasks: StdMutex<Vec<tauri::async_runtime::JoinHandle<()>>>,
    updates: crate::updates::Updates,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PanelView {
    settings: SessionSettings,
    profiles: Vec<ConnectionProfile>,
    item: Option<ItemPreview>,
    history: Option<crate::history::View>,
    clipboard_checked: bool,
    shortcut_status: String,
    tray_status: String,
    shortcut_available: bool,
    connected: bool,
    platform: &'static str,
    settings_saved: bool,
    settings_writable: bool,
    settings_notice: Option<String>,
    active_shortcut: Option<String>,
    settings_revision: Option<String>,
    migration_summary: Option<crate::migration_review::View>,
    connection_report: Option<ConnectionReport>,
    operation_notice: Option<OperationNotice>,
    notification_status: notifications::Status,
    history_status: Option<monitor::Status>,
    pending_share: Option<crate::pending::View>,
    startup_status: ssh_clipboard_desktop_prototype::startup::Status,
    update_status: crate::updates::View,
}

impl Session {
    fn after_operation(&mut self, operation: &Operation) {
        self.metadata_barrier = tokio::time::Instant::now();
        if operation.was_cancelled() {
            self.connection = None;
            self.connected = false;
            self.item = None;
            self.history = History::default();
            self.connection_report = None;
        }
    }
    fn monitor_target(&self) -> Option<MonitorTarget> {
        if self.settings_notice.is_some() || self.settings.validate_connection().is_err() {
            return None;
        }
        let policy = self.shelf_policy().ok()??;
        let saved = self.saved_config.as_ref()?;
        Some(MonitorTarget {
            profile_id: saved.default_profile.clone(),
            revision: saved.revision.to_string(),
            ssh: self.client_config().ok()?.ssh,
            policy,
        })
    }
    fn apply_metadata(&mut self, update: &monitor::Update) -> bool {
        if self.monitor_target().as_ref() != Some(&update.target) {
            return false;
        }
        self.history_status = Some(update.status.clone());
        self.connected = update.status.phase == monitor::Phase::Live;
        if let Some(snapshot) = &update.snapshot
            && update
                .snapshot_started_at
                .is_some_and(|started| started >= self.metadata_barrier)
        {
            let previous = (self.history.epoch, self.history.selected);
            if self.history.accept_background((**snapshot).clone()) {
                if previous != (self.history.epoch, self.history.selected) {
                    self.item = None;
                }
                if previous.0 != self.history.epoch {
                    self.connection = None;
                }
                self.clipboard_checked = true;
            }
        }
        true
    }
    async fn take_connection(&mut self) -> Result<Connection, String> {
        let policy = self
            .shelf_policy()?
            .ok_or("This connection has no history controls.")?;
        let config = self.client_config()?.ssh;
        if let Some(mut connection) = self.connection.take() {
            if connection.matches(&config, policy) && connection.is_ready() {
                return Ok(connection);
            }
            connection.close().await;
        }
        Connection::connect(config, policy)
            .await
            .map_err(|error| error.to_string())
    }
    fn keep_connection(&mut self, mut connection: Connection) {
        if connection.is_ready() {
            self.connection = Some(connection);
        }
    }
    async fn load_selected(&mut self, id: Uuid) -> Result<ClipboardValue, String> {
        let mut connection = self.take_connection().await?;
        let result = self
            .history
            .fetch(&mut connection, id, self.settings.show_preview)
            .await;
        self.keep_connection(connection);
        result
    }
    fn shelf_policy(&self) -> Result<Option<SessionPolicy>, String> {
        self.settings
            .endpoint
            .session_policy(self.client_config()?.timeout_ms)
    }
    fn max_decoded(&self) -> Result<usize, String> {
        Ok(self
            .shelf_policy()?
            .map_or(crate::model::MAX_DECODED_BYTES, |policy| {
                policy.max_decoded_bytes as usize
            }))
    }
    fn record_operation(&mut self, result: Result<(), String>, sharing: bool) {
        self.operation_counter = self.operation_counter.wrapping_add(1);
        self.operation_notice = Some(OperationNotice {
            id: self.operation_counter.to_string(),
            failed: result.is_err(),
            message: result.err().unwrap_or_else(|| {
                if sharing {
                    "Current clipboard shared.".into()
                } else {
                    "Shared clipboard copied to this computer.".into()
                }
            }),
        });
    }

    fn client_config(&self) -> Result<ssh_clipboard::client::transport::ClientConfig, String> {
        let mut config = if let Some(saved) = &self.saved_config {
            saved
                .selected_profile()
                .map_err(|e| e.to_string())?
                .client_config()
        } else {
            self.settings.client_config()?
        };
        config.ssh.ssh_options.insert(0, "BatchMode=yes".into());
        Ok(config)
    }
    fn check_revision(&self, expected: Option<&str>) -> Result<(), String> {
        if expected
            != self
                .saved_config
                .as_ref()
                .map(|c| c.revision.to_string())
                .as_deref()
        {
            return Err("Saved settings changed. Reload the panel before continuing.".into());
        }
        Ok(())
    }

    fn view(&self) -> Result<PanelView, String> {
        Ok(PanelView {
            settings: self.settings.clone(),
            history: (!matches!(
                self.settings.endpoint,
                ssh_clipboard::configuration::Endpoint::Legacy {}
            ))
            .then(|| self.history.view(self.settings.show_preview)),
            connection_report: self.connection_report.clone(),
            operation_notice: self.operation_notice.clone(),
            notification_status: self.notification_status.clone(),
            startup_status: self.startup_status.clone(),
            update_status: self.updates.view(),
            history_status: self.history_status.clone(),
            pending_share: self.pending_share.as_ref().map(|pending| {
                let mut view = pending.view();
                view.can_retry &= self
                    .history
                    .epoch
                    .is_none_or(|epoch| epoch == pending.epoch);
                view
            }),
            profiles: self.saved_config.as_ref().map_or_else(
                || {
                    Configuration::default()
                        .profiles
                        .iter()
                        .map(ConnectionProfile::from)
                        .collect()
                },
                |c| c.profiles.iter().map(ConnectionProfile::from).collect(),
            ),
            clipboard_checked: self.clipboard_checked,
            item: if matches!(
                self.settings.endpoint,
                ssh_clipboard::configuration::Endpoint::Shelf { .. }
            ) {
                self.history.item_preview(self.settings.show_preview)
            } else {
                self.item
                    .as_ref()
                    .map(|value| ItemPreview::from_value(value, self.settings.show_preview))
                    .transpose()?
            },
            shortcut_status: self.shortcut_status.clone(),
            tray_status: self.tray_status.clone(),
            shortcut_available: !is_wayland(),
            connected: self.connected,
            platform: std::env::consts::OS,
            settings_saved: self.saved_config.is_some(),
            settings_writable: self.settings_notice.is_none(),
            settings_notice: self.settings_notice.clone(),
            active_shortcut: self.shortcuts.open_label(),
            settings_revision: self.saved_config.as_ref().map(|c| c.revision.to_string()),
            migration_summary: self
                .saved_config
                .as_ref()
                .and_then(crate::migration_review::view),
        })
    }

    fn activate_configuration(
        &mut self,
        loaded: Option<Configuration>,
        next: SessionSettings,
        shortcut_status: String,
    ) {
        let profile_changed = loaded.as_ref().map(|c| &c.default_profile)
            != self.saved_config.as_ref().map(|c| &c.default_profile);
        if profile_changed {
            self.pending_share = None;
        }
        let transport_changed = loaded
            .as_ref()
            .and_then(|c| c.selected_profile().ok())
            .map(|p| &p.transport)
            != self
                .saved_config
                .as_ref()
                .and_then(|c| c.selected_profile().ok())
                .map(|p| &p.transport);
        if profile_changed
            || transport_changed
            || next.target != self.settings.target
            || next.port != self.settings.port
            || next.identity_file != self.settings.identity_file
            || next.endpoint != self.settings.endpoint
        {
            self.item = None;
            self.history = History::default();
            self.connection = None;
            self.clipboard_checked = false;
            self.connected = false;
            self.connection_report = None;
            self.operation_notice = None;
        }
        self.settings = next;
        self.saved_config = loaded;
        self.settings_notice = None;
        self.shortcut_status = shortcut_status;
        self.history_status = None;
        self.metadata_barrier = tokio::time::Instant::now();
    }
}

/// Reloading is local configuration work only: never sample, transfer, or replay a clipboard.
fn synchronize_settings(
    session: &mut Session,
    loaded: Result<Option<Configuration>, String>,
    configure: impl FnOnce(&mut Session, &SessionSettings) -> Result<String, String>,
) -> bool {
    let loaded = match loaded {
        Ok(loaded) => loaded,
        Err(notice) => {
            if session.settings_notice.as_ref() == Some(&notice) {
                return false;
            }
            session.settings_notice = Some(notice);
            return true;
        }
    };
    if loaded == session.saved_config {
        return session.settings_notice.take().is_some();
    }
    let next = match loaded
        .as_ref()
        .map(SessionSettings::from_configuration)
        .transpose()
    {
        Ok(settings) => settings.unwrap_or_default(),
        Err(notice) => {
            session.settings_notice = Some(notice);
            return true;
        }
    };
    let status = match configure(session, &next) {
        Ok(status) => status,
        Err(error) => {
            let notice = format!(
                "Saved settings could not be activated: {error} Previous settings remain active. Repair the saved file to retry."
            );
            if session.settings_notice.as_ref() == Some(&notice) {
                return false;
            }
            session.settings_notice = Some(notice);
            return true;
        }
    };
    session.activate_configuration(loaded, next, status);
    true
}

fn watch_settings(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            let Some(state) = app.try_state::<Desktop>() else {
                break;
            };
            // Defer changes while a transfer or settings application holds the operation lock.
            let Ok(mut session) = state.session.try_lock() else {
                continue;
            };
            let startup = crate::startup_native::status(
                &app,
                session
                    .saved_config
                    .as_ref()
                    .is_some_and(|config| config.preferences.start_at_login),
            );
            if startup != session.startup_status {
                session.startup_status = startup;
                let _ = app.emit("settings-changed", ());
            }
            if session
                .pending_share
                .as_mut()
                .is_some_and(PendingShare::expire)
            {
                let _ = app.emit("settings-changed", ());
            }
            let Some(store) = state.config_store.as_ref() else {
                continue;
            };
            let loaded = store.load().map_err(|error| format!("{error} Settings were left untouched at {}. Repair the file to reload it automatically.", store.path().display()));
            if synchronize_settings(&mut session, loaded, |session, settings| {
                configure_shortcut(&app, session, settings)
            }) {
                sync_monitor(&state, &mut session);
                let _ = app.emit("settings-changed", ());
            }
        }
    });
}

fn sync_monitor(state: &Desktop, session: &mut Session) {
    if session.monitor_target().is_none() {
        session.history_status = None;
    }
    if let Err(error) = monitor::configure(&state.monitor_request, session.monitor_target(), false)
    {
        session.history_status = Some(monitor::Status {
            phase: monitor::Phase::NeedsAction,
            detail: error.into(),
            retry_in_ms: None,
        });
    }
}

#[tauri::command]
async fn reconnect_history(
    state: State<'_, Desktop>,
    expected_revision: Option<String>,
) -> Result<PanelView, String> {
    let session = state
        .session
        .try_lock()
        .map_err(|_| "Wait for the current operation to finish.")?;
    session.check_revision(expected_revision.as_deref())?;
    let target = session
        .monitor_target()
        .ok_or("Save a shelf connection before connecting history.")?;
    monitor::configure(&state.monitor_request, Some(target), true)?;
    session.view()
}

#[tauri::command]
async fn get_panel(state: State<'_, Desktop>) -> Result<PanelView, String> {
    state.session.lock().await.view()
}

#[tauri::command]
fn get_operation(state: State<'_, Desktop>) -> Result<crate::operation::View, String> {
    state.operations.view()
}

#[tauri::command]
fn cancel_operation(
    state: State<'_, Desktop>,
    id: String,
) -> Result<crate::operation::View, String> {
    state.operations.cancel(&id)
}

#[tauri::command]
async fn check_updates(
    app: AppHandle,
    state: State<'_, Desktop>,
) -> Result<crate::updates::View, String> {
    let operation = state.operations.begin("Checking for updates")?;
    operation.run(state.updates.check(&app)).await
}

#[tauri::command]
async fn download_update(
    app: AppHandle,
    state: State<'_, Desktop>,
    version: String,
) -> Result<crate::updates::View, String> {
    let operation = state.operations.begin("Downloading update")?;
    operation.run(state.updates.download(&app, &version)).await
}

#[tauri::command]
async fn install_update(
    app: AppHandle,
    state: State<'_, Desktop>,
    version: String,
) -> Result<(), String> {
    let mut session = state
        .session
        .try_lock()
        .map_err(|_| "Wait for the current operation to finish.")?;
    if session.pending_share.is_some() {
        return Err("Resolve or discard the uncertain share before installing an update.".into());
    }
    if state.clipboard.is_busy() {
        return Err("Wait for the native clipboard operation to finish.".into());
    }
    let operation = state.operations.begin("Installing update")?;
    #[cfg(feature = "installed-desktop")]
    {
        use tauri_plugin_updater::UpdaterExt;
        let (mut progress, candidate, bytes) = state.updates.take_verified(&app, &version).await?;
        let result: Result<(), String> = async {
            let bridge = ssh_clipboard_desktop_prototype::update_handoff::Bridge::new(&candidate).await?;
            let cleanup_app = app.clone();
            let builder = app.updater_builder()
                .endpoints(vec![bridge.url.clone()]).map_err(|_| "Could not initialize installer handoff.")?
                .target(crate::updates::platform())
                .no_proxy()
                .timeout(std::time::Duration::from_secs(4))
                .configure_client(|client| client.redirect(reqwest::redirect::Policy::none()))
                // Upstream calls this BEFORE ShellExecute. Keep native app/clipboard owners
                // alive so launch failure remains recoverable; process exit releases them.
                .on_before_exit(move || cleanup_transport(&cleanup_app));
            let installer = operation.run(async {
                builder.build().map_err(|_| "Could not initialize installer.")?
                    .check().await
                    .map_err(|_| "Could not prepare the verified installer. Nothing was installed.")?
                    .ok_or_else(|| "The installer did not accept this update version.".to_string())
            }).await?;
            // The bridge is local and one-use; still bind the framework's selected object to
            // the candidate whose exact bytes have passed our policy, before calling install.
            if installer.version != candidate.version
                || installer.download_url.as_str() != candidate.url
                || installer.signature != candidate.signature
            {
                return Err("Installer metadata did not match the verified package.".into());
            }
            drop(bridge);
            operation.finishing()?;
            if let Some(connection) = session.connection.take() {
                connection.close().await;
            }
            let installed = tauri::async_runtime::spawn_blocking(move || installer.install(bytes)).await
                .map_err(|_| "The installer could not be started. Check the installed app before trying again.")?;
            installed.map_err(|_| "Update installation did not finish. Check the installed app and version before retrying.".to_string())?;
            #[cfg(not(target_os = "windows"))]
            {
                // Cleanup joins async owners through block_on; run it outside the
                // async command to avoid nesting a Tokio runtime during restart.
                let cleanup_app = app.clone();
                crate::updates::cleanup_before_restart(move || cleanup_desktop(&cleanup_app)).await?;
                app.restart();
            }
            #[cfg(target_os = "windows")]
            { Ok::<_, String>(()) }
        }.await;
        if let Err(error) = &result {
            // The framework may have run its pre-exit hook before a failed launch.
            // Restart only our transport tasks; the native app/clipboard never shut down.
            if state
                .monitor_tasks
                .lock()
                .is_ok_and(|tasks| tasks.is_empty())
            {
                start_background(&app);
            }
            progress.fail(error);
        } else {
            progress.finish();
        }
        result
    }
    #[cfg(not(feature = "installed-desktop"))]
    {
        let _ = (&mut session, operation, app, version);
        Err("Updates are unavailable in this build.".into())
    }
}

#[tauri::command]
fn set_shortcut_recording(state: State<'_, Desktop>, active: bool) {
    // Independent of the transfer lock: losing focus while recording must not start a share.
    state.recording_shortcut.store(active, Ordering::SeqCst);
}

#[tauri::command]
async fn test_notification(app: AppHandle, state: State<'_, Desktop>) -> Result<PanelView, String> {
    let mut session = state
        .session
        .try_lock()
        .map_err(|_| "Wait for the current operation to finish.".to_string())?;
    session.notification_status = notifications::test(&app)?;
    session.view()
}

fn configure_shortcut(
    app: &AppHandle,
    session: &mut Session,
    next: &SessionSettings,
) -> Result<String, String> {
    if is_wayland() {
        // Preserve the saved preference across platforms, while allowing other settings to be edited.
        return Ok("Wayland: use the launcher or tray menu. Portal support is pending.".into());
    }
    struct NativeRegistry<'a>(&'a AppHandle);
    impl Registry for NativeRegistry<'_> {
        fn register(&mut self, shortcut: Shortcut) -> Result<(), ()> {
            self.0.global_shortcut().register(shortcut).map_err(|_| ())
        }
        fn unregister(&mut self, shortcut: Shortcut) -> Result<(), ()> {
            self.0
                .global_shortcut()
                .unregister(shortcut)
                .map_err(|_| ())
        }
    }
    let desired = Bindings::requested(next)?;
    let result = session.shortcuts.replace(desired, &mut NativeRegistry(app));
    if let Some(state) = app.try_state::<Desktop>() {
        state.open_shortcut.store(
            session
                .shortcuts
                .open_id()
                .map_or(0, |id| u64::from(id) + 1),
            Ordering::SeqCst,
        );
    }
    match result {
        Ok(status) => Ok(status),
        Err(error) => {
            session.shortcut_status = error.clone();
            Err(error)
        }
    }
}

#[tauri::command]
async fn apply_settings(
    app: AppHandle,
    state: State<'_, Desktop>,
    settings: SessionSettings,
    expected_revision: Option<String>,
    start_at_login: Option<bool>,
) -> Result<PanelView, String> {
    let mut session = state
        .session
        .try_lock()
        .map_err(|_| "Wait for the current operation to finish.".to_string())?;
    let store = state
        .config_store
        .as_ref()
        .ok_or("The settings location is unavailable.")?;
    let result = apply_settings_draft(
        &mut session,
        store,
        settings,
        expected_revision.as_deref(),
        start_at_login,
        StartupAccess {
            registry: &crate::startup_native::NativeRegistry(&app),
            available: crate::startup_native::available(),
        },
        |session, settings| configure_shortcut(&app, session, settings),
    );
    session.startup_status = crate::startup_native::status(
        &app,
        session
            .saved_config
            .as_ref()
            .is_some_and(|config| config.preferences.start_at_login),
    );
    result?;
    sync_monitor(&state, &mut session);
    session.view()
}

fn acknowledge_import(
    session: &mut Session,
    store: &ConfigStore,
    expected_revision: Option<&str>,
) -> Result<(), String> {
    session.check_revision(expected_revision)?;
    if let Some(notice) = &session.settings_notice {
        return Err(notice.clone());
    }
    let expected = session
        .saved_config
        .as_ref()
        .ok_or("No saved import to review.")?;
    // Only the receipt changes: keep snapshots, pending shares and active shortcuts.
    let saved = crate::migration_review::acknowledge(store, expected)?;
    session.saved_config = Some(saved);
    Ok(())
}

#[tauri::command]
async fn acknowledge_migration_summary(
    state: State<'_, Desktop>,
    expected_revision: Option<String>,
) -> Result<PanelView, String> {
    let mut session = state
        .session
        .try_lock()
        .map_err(|_| "Wait for the current operation to finish.")?;
    let store = state
        .config_store
        .as_ref()
        .ok_or("The settings location is unavailable.")?;
    acknowledge_import(&mut session, store, expected_revision.as_deref())?;
    sync_monitor(&state, &mut session);
    session.view()
}

struct StartupAccess<'a> {
    registry: &'a dyn ssh_clipboard_desktop_prototype::startup::Registry,
    available: bool,
}

fn apply_settings_draft(
    session: &mut Session,
    store: &ConfigStore,
    mut settings: SessionSettings,
    expected_revision: Option<&str>,
    start_at_login: Option<bool>,
    startup: StartupAccess<'_>,
    configure: impl FnMut(&mut Session, &SessionSettings) -> Result<String, String>,
) -> Result<(), String> {
    settings.target = settings.target.trim().into();
    settings.validate()?;
    session.check_revision(expected_revision).map_err(|_| "Saved settings changed while you were editing. Discard your draft to load them before applying changes.".to_string())?;
    if let Some(notice) = &session.settings_notice {
        return Err(notice.clone());
    }
    if start_at_login.is_some() && !startup.available {
        return Err("Start at login is available in installed release builds.".into());
    }
    // Parse the whole requested binding set before startup can change. Actual OS
    // conflicts still use the shortcut transaction and startup rollback below.
    if !is_wayland() {
        Bindings::requested(&settings)?;
    }
    let mut next = session.saved_config.clone().unwrap_or_default();
    settings.update_configuration(&mut next)?;
    if let Some(enabled) = start_at_login {
        next.preferences.start_at_login = enabled;
    }
    save_configuration_transaction(
        session,
        store,
        next,
        start_at_login.map(|enabled| (startup.registry, enabled)),
        configure,
    )
}

/// Both GUI writes and watcher reloads use the same profile-isolation boundary.
fn save_configuration(
    session: &mut Session,
    store: &ConfigStore,
    next_config: Configuration,
    configure: impl FnMut(&mut Session, &SessionSettings) -> Result<String, String>,
) -> Result<(), String> {
    save_configuration_transaction(session, store, next_config, None, configure)
}

fn save_configuration_transaction(
    session: &mut Session,
    store: &ConfigStore,
    next_config: Configuration,
    startup: Option<(
        &dyn ssh_clipboard_desktop_prototype::startup::Registry,
        bool,
    )>,
    mut configure: impl FnMut(&mut Session, &SessionSettings) -> Result<String, String>,
) -> Result<(), String> {
    let settings = SessionSettings::from_configuration(&next_config)?;
    settings.validate()?;
    // Validate/check for a stale writer and flush the temporary file before changing OS state.
    let pending = store
        .prepare(session.saved_config.as_ref(), next_config)
        .map_err(|e| e.to_string())?;
    let previous_settings = session.settings.clone();
    let activate_and_commit = || {
        let status = configure(session, &settings)?;
        let saved = match pending.commit() {
            Ok(saved) => saved,
            Err(error) => {
                match configure(session, &previous_settings) {
                    Ok(status) => session.shortcut_status = status,
                    Err(_) => {
                        session.shortcuts.suspend_direct();
                        session.shortcut_status = "Could not restore previous shortcuts. Direct actions are paused. Quit and reopen to restore saved settings.".into();
                        return Err(format!("{error} {}", session.shortcut_status));
                    }
                }
                return Err(error.to_string());
            }
        };
        Ok((saved, status))
    };
    let (saved, status) = match startup {
        Some((registry, enabled)) => ssh_clipboard_desktop_prototype::startup::change(
            registry,
            enabled,
            activate_and_commit,
        )?,
        None => activate_and_commit()?,
    };
    session.activate_configuration(Some(saved), settings, status);
    Ok(())
}

#[tauri::command]
async fn remove_profile(
    app: AppHandle,
    state: State<'_, Desktop>,
    profile_id: String,
    expected_revision: Option<String>,
) -> Result<PanelView, String> {
    let mut session = state
        .session
        .try_lock()
        .map_err(|_| "Wait for the current operation to finish.".to_string())?;
    session.check_revision(expected_revision.as_deref())?;
    if let Some(notice) = &session.settings_notice {
        return Err(notice.clone());
    }
    let store = state
        .config_store
        .as_ref()
        .ok_or("The settings location is unavailable.")?;
    let mut next = session
        .saved_config
        .clone()
        .ok_or("Save a connection before removing it.")?;
    next.remove_profile(&profile_id)?;
    save_configuration(&mut session, store, next, |session, settings| {
        configure_shortcut(&app, session, settings)
    })?;
    sync_monitor(&state, &mut session);
    session.view()
}

async fn request(session: &mut Session, kind: RequestKind) -> Result<ResponseKind, String> {
    session.settings.validate_connection()?;
    let sharing = matches!(kind, RequestKind::Set { .. });
    let request = make_request(kind);
    let config = session.client_config()?;
    validate_transfer_size(&request, config.max_size)?;
    let request_id = request.request_id;
    match send_request(&config, request).await {
        Ok(response) => {
            if response.request_id != request_id {
                session.connected = false;
                return Err("The server reply did not match this request.".into());
            }
            session.connection_report = None;
            session.connected = true;
            match response.kind {
                ResponseKind::Error { code, .. } => {
                    session.connected = !matches!(
                        code,
                        ErrorCode::DaemonNotRunning
                            | ErrorCode::VersionMismatch
                            | ErrorCode::Internal
                    );
                    let (summary, hint) = server_problem(&code);
                    let uncertain = if sharing && matches!(code, ErrorCode::Internal) {
                        " The share may have reached the server. Refresh before sharing it again."
                    } else {
                        ""
                    };
                    Err(format!("{summary} {hint}{uncertain}"))
                }
                kind => Ok(kind),
            }
        }
        Err(failure) => {
            session.connected = false;
            session.connection_report = Some(ConnectionReport::from_failure(&failure));
            Err(failure.to_string())
        }
    }
}

#[tauri::command]
async fn test_connection(
    state: State<'_, Desktop>,
    expected_revision: Option<String>,
) -> Result<PanelView, String> {
    let mut session = state
        .session
        .try_lock()
        .map_err(|_| "Another operation is running.".to_string())?;
    session.check_revision(expected_revision.as_deref())?;
    session.settings.validate_connection()?;
    let config = session.client_config()?;
    let operation = state.operations.begin("Testing connection")?;
    let result = operation
        .run(async {
            Ok(match session.shelf_policy()? {
                Some(policy) => diagnose_shelf(&config, policy).await,
                None => diagnose(&config).await,
            })
        })
        .await;
    session.after_operation(&operation);
    let report = result?;
    session.connected = report.healthy;
    let failure = (!report.healthy).then(|| report.message());
    session.connection_report = Some(report);
    if let Some(failure) = failure {
        return Err(failure);
    }
    session.view()
}

#[tauri::command]
async fn refresh_item(
    state: State<'_, Desktop>,
    expected_revision: Option<String>,
) -> Result<PanelView, String> {
    let mut session = state
        .session
        .try_lock()
        .map_err(|_| "Another operation is running.".to_string())?;
    session.check_revision(expected_revision.as_deref())?;
    let operation = state.operations.begin("Refreshing shared clipboard")?;
    let result = operation.run(fetch_item(&mut session)).await;
    session.after_operation(&operation);
    result?;
    session.view()
}

#[tauri::command]
async fn select_item(
    state: State<'_, Desktop>,
    expected_revision: Option<String>,
    epoch: Uuid,
    item_id: Uuid,
) -> Result<PanelView, String> {
    let mut session = state
        .session
        .try_lock()
        .map_err(|_| "Another operation is running.".to_string())?;
    session.check_revision(expected_revision.as_deref())?;
    session.history.guard(epoch)?;
    session.item = None;
    let operation = state.operations.begin("Loading item preview")?;
    let result = operation.run(session.load_selected(item_id)).await;
    session.after_operation(&operation);
    let value = result?;
    session.item = Some(value);
    session.connected = true;
    session.view()
}

#[tauri::command]
async fn delete_item(
    state: State<'_, Desktop>,
    expected_revision: Option<String>,
    epoch: Uuid,
    item_id: Uuid,
) -> Result<PanelView, String> {
    let mut session = state
        .session
        .try_lock()
        .map_err(|_| "Another operation is running.".to_string())?;
    session.check_revision(expected_revision.as_deref())?;
    if !session.history.items.iter().any(|item| item.id == item_id) {
        return Err("This item is no longer in the loaded history. Refresh first.".into());
    }
    let operation = state.operations.begin("Deleting shared item")?;
    let result = operation
        .run(mutate_history(
            &mut session,
            epoch,
            ShelfAction::Delete { item_id },
            &operation,
        ))
        .await;
    session.after_operation(&operation);
    result?;
    session.view()
}

#[tauri::command]
async fn clear_history(
    state: State<'_, Desktop>,
    expected_revision: Option<String>,
    epoch: Uuid,
) -> Result<PanelView, String> {
    let mut session = state
        .session
        .try_lock()
        .map_err(|_| "Another operation is running.".to_string())?;
    session.check_revision(expected_revision.as_deref())?;
    let operation = state.operations.begin("Clearing shared history")?;
    let result = operation
        .run(mutate_history(
            &mut session,
            epoch,
            ShelfAction::Clear,
            &operation,
        ))
        .await;
    session.after_operation(&operation);
    result?;
    session.view()
}

async fn mutate_history(
    session: &mut Session,
    epoch: Uuid,
    action: ShelfAction,
    operation: &Operation,
) -> Result<(), String> {
    session.history.guard(epoch)?;
    let deleted = match &action {
        ShelfAction::Delete { item_id } => Some(*item_id),
        ShelfAction::Clear => None,
        _ => return Err("Unsupported history action.".into()),
    };
    let mut connection = session.take_connection().await?;
    if connection.epoch() != Some(epoch) {
        session.keep_connection(connection);
        return Err("The server restarted. Refresh history before continuing.".into());
    }
    operation.mutation_started()?;
    let result = connection
        .execute(action, &[])
        .await
        .map_err(|error| error.to_string());
    session.keep_connection(connection);
    let reply = result?;
    if !matches!(
        reply.response,
        ShelfResponse::Deleted { .. } | ShelfResponse::Cleared { .. }
    ) {
        return Err("Unexpected history mutation reply.".into());
    }
    operation.confirmed()?;
    session.history.sequence = match &reply.response {
        ShelfResponse::Deleted { sequence, .. } | ShelfResponse::Cleared { sequence, .. } => {
            Some(*sequence)
        }
        _ => None,
    };
    // A confirmed mutation is final; a subsequent refresh cannot turn it into failure.
    session
        .history
        .items
        .retain(|item| deleted.is_some_and(|id| item.id != id));
    if session
        .history
        .selected
        .is_none_or(|id| !session.history.items.iter().any(|item| item.id == id))
    {
        session.history.selected = session.history.items.first().map(|item| item.id);
    }
    session.history.preview = None;
    session.item = None;
    session.connected = true;
    session.clipboard_checked = true;
    Ok(())
}

async fn fetch_item(session: &mut Session) -> Result<(), String> {
    if session.shelf_policy()?.is_some() {
        session.item = None;
        session.history.preview = None;
        session.connected = false;
        let mut connection = session.take_connection().await?;
        let result = async {
            session.history.refresh(&mut connection).await?;
            session.connected = true;
            session.connection_report = None;
            session.clipboard_checked = true;
            if let Some(id) = session.history.selected {
                session.item = Some(
                    session
                        .history
                        .fetch(&mut connection, id, session.settings.show_preview)
                        .await?,
                );
            }
            Ok(())
        }
        .await;
        session.keep_connection(connection);
        return result;
    }
    match request(session, RequestKind::Get).await? {
        ResponseKind::Value { value } => {
            ItemPreview::from_value(&value, session.settings.show_preview)?;
            session.item = Some(value);
        }
        ResponseKind::Empty => session.item = None,
        _ => return Err("Unexpected server response to refresh.".into()),
    }
    session.clipboard_checked = true;
    Ok(())
}

async fn fetch_latest_item(session: &mut Session) -> Result<(), String> {
    if session.shelf_policy()?.is_some() {
        session.history.selected = None;
    }
    fetch_item(session).await
}

#[tauri::command]
async fn share_clipboard(
    state: State<'_, Desktop>,
    expected_revision: Option<String>,
) -> Result<PanelView, String> {
    let mut session = state
        .session
        .try_lock()
        .map_err(|_| "Another operation is running.".to_string())?;
    session.check_revision(expected_revision.as_deref())?;
    let operation = state.operations.begin("Sharing current clipboard")?;
    let result = operation
        .run(share_current(&mut session, &state.clipboard, &operation))
        .await;
    session.after_operation(&operation);
    session.record_operation(result.clone(), true);
    result?;
    session.view()
}

#[tauri::command]
async fn discard_share(
    state: State<'_, Desktop>,
    expected_revision: Option<String>,
    operation_id: Uuid,
) -> Result<PanelView, String> {
    let mut session = state
        .session
        .try_lock()
        .map_err(|_| "Wait for the current operation to finish.")?;
    session.check_revision(expected_revision.as_deref())?;
    let pending = session
        .pending_share
        .as_ref()
        .ok_or("No share recovery is pending.")?;
    pending.guard(operation_id, pending.epoch)?;
    session.pending_share = None;
    session.view()
}

#[tauri::command]
async fn recover_share(
    state: State<'_, Desktop>,
    expected_revision: Option<String>,
    operation_id: Uuid,
    retry: bool,
) -> Result<PanelView, String> {
    let mut session = state
        .session
        .try_lock()
        .map_err(|_| "Another operation is running.")?;
    session.check_revision(expected_revision.as_deref())?;
    let operation = state.operations.begin(if retry {
        "Retrying captured share"
    } else {
        "Checking earlier share"
    })?;
    let result = operation
        .run(reconcile_share(
            &mut session,
            &operation,
            operation_id,
            retry,
        ))
        .await;
    session.after_operation(&operation);
    result?;
    session.view()
}

async fn reconcile_share(
    session: &mut Session,
    operation: &Operation,
    operation_id: Uuid,
    retry: bool,
) -> Result<(), String> {
    let pending = session
        .pending_share
        .as_ref()
        .ok_or("No share recovery is pending.")?;
    pending.guard(operation_id, pending.epoch)?;
    let mut connection = session.take_connection().await?;
    let result=async {
        let pending=session.pending_share.as_ref().unwrap();
        pending.guard(operation_id,connection.epoch().ok_or("Reconnect before checking an earlier share.")?)?;
        let reply=connection.execute(ShelfAction::OperationStatus {device_id:pending.origin.device_id,operation_id},&[]).await.map_err(|error|error.to_string())?;
        let ShelfResponse::Operation {item,available,epoch,sequence,epoch_time_ms,..}=reply.response else {return Err("Unexpected operation status reply.".into());};
        session.pending_share.as_mut().unwrap().expire_server(epoch_time_ms);
        let (item,available,epoch,sequence)=if let Some(item)=item {(item,available,epoch,sequence)}
        else {
            if !retry {return Err("The server has no retained acknowledgment for this share. That does not prove it failed. You can retry the captured bytes only within the original safe retry window.".into());}
            let pending=session.pending_share.as_mut().unwrap();
            pending.expire();
            let action=pending.action()?;
            operation.mutation_started()?;
            let reply=match connection.execute(action,&pending.value.as_ref().unwrap().data).await {
                Ok(reply)=>reply,
                Err(ssh_clipboard::client::session_transport::RunError::Session(ssh_clipboard::client::session::SessionError::Remote(ssh_clipboard::session_protocol::ErrorCode::RetryExpired)))=>{
                    pending.expire_server(u64::MAX.into());
                    return Err("The safe retry window expired. Check the earlier share's status or discard recovery data.".into());
                }
                Err(error)=>return Err(error.to_string()),
            };
            let ShelfResponse::Shared {item,available,epoch,sequence,..}=reply.response else {return Err("Unexpected captured-share reply.".into());};
            (item,available,epoch,sequence)
        };
        operation.confirmed()?;
        let pending=session.pending_share.take().unwrap();
        let id=item.id;
        session.history.accept_snapshot(ssh_clipboard::client::session::Snapshot {epoch,sequence,items:if available {vec![item]}else{vec![]}});
        let _=session.history.refresh(&mut connection).await;
        session.item=None;
        if available && session.history.selected==Some(id) && let Some(value)=pending.value {
            // A narrower current preview budget cannot turn confirmed acceptance into failure.
            if let Ok(limit)=session.max_decoded() && let Ok(preview)=ItemPreview::from_value_with_limit(&value,session.settings.show_preview,limit) {session.history.preview=Some(preview);session.item=Some(value);}
        }
        session.connected=true;
        session.clipboard_checked=true;
        session.record_operation(Ok(()),true);
        session.operation_notice.as_mut().unwrap().message=if available {"The earlier share was accepted. Your local clipboard was not read or changed."}else{"The earlier share was accepted, but the item has expired or been removed."}.into();
        Ok(())
    }.await;
    session.keep_connection(connection);
    result
}

async fn share_current(
    session: &mut Session,
    clipboard: &ClipboardWorker,
    operation: &Operation,
) -> Result<(), String> {
    if session.pending_share.is_some() {
        return Err(
            "Check or discard the earlier uncertain share before capturing another clipboard."
                .into(),
        );
    }
    session.settings.validate_connection()?;
    if let Some(policy) = session.shelf_policy()? {
        let device = &session
            .saved_config
            .as_ref()
            .ok_or("Save this connection before sharing.")?
            .device;
        let origin = ShelfOrigin {
            device_id: device
                .id
                .as_ref()
                .ok_or("Save settings to initialize this computer's identity.")?
                .parse()
                .map_err(|_| "Invalid device identity.")?,
            name: device.name.clone(),
        };
        // Reject invalid saved identity metadata before sampling the native clipboard.
        ssh_clipboard::session_protocol::Request::Share {request_id:1.into(), epoch:Uuid::from_u128(1), operation_id:Uuid::from_u128(1), origin:origin.clone(), representation:ShelfRepresentation {id:0,content_type:ShelfContent::Text,encoded_bytes:0,dimensions:None,decoded_bytes:None},created_at_ms:None}.validate(0).map_err(|_|"This computer's device identity or name is invalid. Repair shared settings before sharing.".to_string())?;
        // Freeze exactly one local capture before waiting for a connection.
        let value = clipboard
            .read_with_limit(
                policy.max_item_bytes as usize,
                policy.max_decoded_bytes as usize,
            )
            .await?;
        let preview = ItemPreview::from_value_with_limit(
            &value,
            session.settings.show_preview,
            policy.max_decoded_bytes as usize,
        )?;
        let representation = ShelfRepresentation {
            id: 0,
            content_type: if value.content_type == ssh_clipboard::protocol::CONTENT_TYPE_PNG {
                ShelfContent::Png
            } else {
                ShelfContent::Text
            },
            encoded_bytes: value.data.len() as u32,
            dimensions: preview.dimensions,
            // Native capture encodes RGBA8, so its PNG native buffer equals RGBA size.
            decoded_bytes: preview
                .dimensions
                .and_then(|size| size.width.checked_mul(size.height)?.checked_mul(4)),
        };
        let mut connection = session.take_connection().await?;
        let result = async {
            session.connected = false;
            let epoch = connection.epoch().ok_or("Reconnect before sharing.")?;
            let retry_ms = connection
                .limits()
                .ok_or("Reconnect before sharing.")?
                .retry_retention_ms;
            let operation_id = Uuid::new_v4();
            // Anchor the ORIGINAL retry deadline to a fresh server-monotonic sample.
            let server_time = if connection.supports("deadline_share") {
                let reply = connection
                    .execute(
                        ShelfAction::OperationStatus {
                            device_id: origin.device_id,
                            operation_id,
                        },
                        &[],
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                match reply.response {
                    ShelfResponse::Operation { epoch_time_ms, .. } => Some(epoch_time_ms),
                    _ => return Err("Unexpected operation clock reply.".into()),
                }
            } else {
                None
            };
            operation.mutation_started()?;
            session.pending_share = Some(PendingShare::new(
                epoch,
                operation_id,
                origin,
                representation,
                value.clone(),
                retry_ms,
                server_time,
            ));
            let pending = session.pending_share.as_ref().unwrap();
            // Initial sends are permitted even if the negotiated retry window is too
            // short to offer a retry. Retrying later uses PendingShare::action's deadline.
            let action = pending.initial_action();
            let reply = match connection.execute(action, &value.data).await {
                Ok(reply) => reply,
                Err(error) => {
                    if !matches!(
                        &error,
                        ssh_clipboard::client::session_transport::RunError::Uncertain { .. }
                    ) {
                        session.pending_share = None;
                    }
                    return Err(error.to_string());
                }
            };
            let ShelfResponse::Shared {
                epoch,
                sequence,
                item,
                ..
            } = reply.response
            else {
                return Err("Unexpected share reply.".into());
            };
            operation.confirmed()?;
            session.pending_share = None;
            let id = item.id;
            session
                .history
                .accept_snapshot(ssh_clipboard::client::session::Snapshot {
                    epoch,
                    sequence,
                    items: vec![item],
                });
            // The share is confirmed. A later metadata refresh failure cannot report it as failed.
            let _ = session.history.refresh(&mut connection).await;
            if session.history.selected == Some(id) {
                session.item = Some(value);
                session.history.preview = Some(preview);
            } else {
                session.item = None;
            }
            session.connected = true;
            session.clipboard_checked = true;
            session.connection_report = None;
            Ok(())
        }
        .await;
        session.keep_connection(connection);
        return result;
    }
    let value = clipboard.read(session.client_config()?.max_size).await?;
    ItemPreview::from_value(&value, session.settings.show_preview)?;
    operation.mutation_started()?;
    match request(
        session,
        RequestKind::Set {
            value: value.clone(),
        },
    )
    .await?
    {
        ResponseKind::Ok => {
            operation.confirmed()?;
            session.item = Some(value);
        }
        _ => return Err("Unexpected server response to share.".into()),
    }
    session.clipboard_checked = true;
    Ok(())
}

// Direct actions are explicit background gestures. While this window has focus, send
// keys to the recorder instead; never transfer while someone is configuring shortcuts.
fn panel_focused(app: &AppHandle) -> bool {
    app.get_webview_window("main")
        .is_none_or(|window| window.is_focused().unwrap_or(true))
}

async fn copy_latest(
    session: &mut Session,
    clipboard: &ClipboardWorker,
    operation: &Operation,
) -> Result<(), String> {
    fetch_latest_item(session).await?;
    let value = session
        .item
        .clone()
        .ok_or("The shared clipboard is empty. Your clipboard was left unchanged.")?;
    let limit = session.max_decoded()?;
    operation.finishing()?;
    clipboard.write_with_limit(value, limit).await
}

fn handle_shortcut(
    app: &AppHandle,
    shortcut: &Shortcut,
    event: tauri_plugin_global_shortcut::ShortcutEvent,
) {
    let Some(state) = app.try_state::<Desktop>() else {
        return;
    };
    let Ok(mut pressed) = state.pressed_shortcuts.lock() else {
        return;
    };
    if event.state() == ShortcutState::Released {
        pressed.remove(&shortcut.id());
        return;
    }
    if !pressed.insert(shortcut.id()) {
        return;
    }
    drop(pressed);
    // Open must remain available while a transfer owns the session lock. This ID
    // comes from the actual registration map, including rollback reconciliation.
    if state.open_shortcut.load(Ordering::SeqCst) == u64::from(shortcut.id()) + 1
        && !state.recording_shortcut.load(Ordering::SeqCst)
    {
        show_panel(app, false);
        return;
    }
    // Reserve the operation immediately. Never queue captures behind an earlier transfer
    // or a profile change, and dispatch only from the actual committed registration map.
    let Ok(mut session) = state.session.clone().try_lock_owned() else {
        return;
    };
    let Some(action) = session.shortcuts.action(shortcut.id()) else {
        return;
    };
    if state.recording_shortcut.load(Ordering::SeqCst) || panel_focused(app) {
        #[derive(Clone, Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Press {
            binding: String,
            open_panel: bool,
        }
        let _ = app.emit(
            "shortcut-pressed",
            Press {
                binding: shortcut.to_string(),
                open_panel: action == Action::Open,
            },
        );
        return;
    }
    if action == Action::Open {
        drop(session);
        show_panel(app, false);
        return;
    }
    let app = app.clone();
    let Ok(operation) = state.operations.begin(if action == Action::Share {
        "Sharing current clipboard"
    } else {
        "Copying shared clipboard"
    }) else {
        return;
    };
    tauri::async_runtime::spawn(async move {
        if app
            .state::<Desktop>()
            .recording_shortcut
            .load(Ordering::SeqCst)
            || panel_focused(&app)
        {
            return;
        }
        let state = app.state::<Desktop>();
        let result = operation
            .run(async {
                if session.settings_notice.is_some() {
                    Err("Repair the saved settings before using direct shortcuts.".into())
                } else {
                    match action {
                        Action::Share => {
                            share_current(&mut session, &state.clipboard, &operation).await
                        }
                        Action::Copy => {
                            copy_latest(&mut session, &state.clipboard, &operation).await
                        }
                        Action::Open => unreachable!(),
                    }
                }
            })
            .await;
        session.after_operation(&operation);
        let failed = result.is_err();
        let preference = session.settings.notifications;
        session.record_operation(result, action == Action::Share);
        drop(session);
        drop(operation);
        let _ = app.emit("settings-changed", ());
        if !panel_focused(&app)
            && notifications::present(&app, preference, failed, action == Action::Share)
        {
            show_panel(&app, false);
        }
    });
}

#[tauri::command]
async fn copy_item(
    app: AppHandle,
    state: State<'_, Desktop>,
    expected_revision: Option<String>,
) -> Result<PanelView, String> {
    let mut session = state
        .session
        .try_lock()
        .map_err(|_| "Another operation is running.".to_string())?;
    session.check_revision(expected_revision.as_deref())?;
    let operation = state.operations.begin("Copying shared clipboard")?;
    let result = operation
        .run(async {
            if let Some(policy) = session.shelf_policy()? {
                let id = session
                    .history
                    .selected
                    .ok_or("Select an item in history first.")?;
                session.item = None;
                let value = session.load_selected(id).await?;
                operation.finishing()?;
                state
                    .clipboard
                    .write_with_limit(value.clone(), policy.max_decoded_bytes as usize)
                    .await?;
                session.item = Some(value);
                return Ok(());
            }
            // Copy the snapshot being shown, never silently substitute a newer remote value.
            let value = session
                .item
                .clone()
                .ok_or_else(|| "Refresh the shared clipboard first.".to_string())?;
            operation.finishing()?;
            state.clipboard.write(value).await?;
            Ok(())
        })
        .await;
    session.after_operation(&operation);
    session.record_operation(result.clone(), false);
    result?;
    if session.settings.close_after_copy
        && let Some(window) = app.get_webview_window("main")
    {
        // Copy already succeeded. A dismissal problem must not report copy failure.
        let _ = window.hide();
    }
    session.view()
}

#[tauri::command]
fn hide_panel(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window.hide().map_err(|err| err.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn quit_app(app: AppHandle, state: State<'_, Desktop>) -> Result<(), String> {
    let guard = state
        .session
        .try_lock()
        .map_err(|_| "Wait for the current transfer before quitting.".to_string())?;
    drop(guard);
    app.exit(0);
    Ok(())
}

fn show_panel(app: &AppHandle, settings: bool) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
        let _ = app.emit("navigate", if settings { "settings" } else { "panel" });
    }
}

fn install_tray(app: &AppHandle) -> tauri::Result<()> {
    use tauri::menu::{Menu, MenuItem};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
    let open = MenuItem::with_id(app, "open", "Open clipboard", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &settings, &quit])?;
    TrayIconBuilder::with_id("clipboard")
        .icon(tauri::image::Image::from_bytes(include_bytes!(
            "../../../assets/icon.png"
        ))?)
        .tooltip("SSH Clipboard")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_panel(app, false),
            "settings" => show_panel(app, true),
            "quit" => {
                // Do not terminate an acknowledged transfer halfway through.
                if app.state::<Desktop>().session.try_lock().is_ok() {
                    app.exit(0);
                } else {
                    show_panel(app, false);
                    let _ = app.emit(
                        "operation-notice",
                        "Wait for the current transfer before quitting.",
                    );
                }
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) {
                show_panel(tray.app_handle(), false);
            }
        })
        .build(app)?;
    Ok(())
}

pub fn run() {
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_single_instance::init(|app, args, _| {
            if !args.iter().any(|arg| arg == "--background") {
                show_panel(app, false);
            }
        }));
    if !is_wayland() {
        builder = builder.plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(handle_shortcut)
                .build(),
        );
    }
    let app = builder
        .setup(|app| {
            #[cfg(feature = "installed-desktop")]
            {
                app.handle().plugin(tauri_plugin_autostart::Builder::new()
                    .app_name(app.config().product_name.as_deref().unwrap_or("SSH Clipboard"))
                    .arg("--background").build())?;
                app.handle().plugin(tauri_plugin_updater::Builder::new().build())?;
            }
            let (config_store, loaded, settings_notice) = match crate::configuration::store() {
                Ok(store) => match crate::configuration::initialize(&store) {
                    Ok(config) => (Some(store), config, None),
                    Err(error) => {
                        let notice = format!("{error} Settings were left untouched at {}. Repair or move the file, then quit and reopen.", store.path().display());
                        (Some(store), None, Some(notice))
                    }
                },
                Err(error) => (None, None, Some(error.to_string())),
            };
            let settings = loaded.as_ref().map(SessionSettings::from_configuration)
                .transpose().map_err(std::io::Error::other)?.unwrap_or_default();
            let startup_status = crate::startup_native::status(app.handle(), loaded.as_ref().is_some_and(|config| config.preferences.start_at_login));
            let updates=crate::updates::Updates::new(app.package_info().version.to_string());
            let mut session = Session {
                settings: settings.clone(),
                item: None,
                history: History::default(),
                connection: None,
                clipboard_checked: false,
                shortcuts: Bindings::default(),
                shortcut_status: "Disabled".into(),
                tray_status: "Available".into(),
                connected: false,
                saved_config: loaded,
                settings_notice,
                connection_report: None,
                operation_notice: None,
                operation_counter: 0,
                notification_status: notifications::status(app.handle()),
                history_status: None,
                metadata_barrier: tokio::time::Instant::now(),
                pending_share: None,
                startup_status,
                updates: updates.clone(),
            };
            session.shortcut_status = if is_wayland() {
                "Wayland: use the launcher or tray menu. Portal support is pending.".into()
            } else {
                configure_shortcut(app.handle(), &mut session, &settings)
                    .unwrap_or_else(|message| message)
            };
            let open_shortcut = session.shortcuts.open_id().map_or(0, |id| u64::from(id) + 1);
            app.manage(Desktop {
                session: Arc::new(Mutex::new(session)),
                operations: Controller::default(),
                open_shortcut: AtomicU64::new(open_shortcut),
                pressed_shortcuts: StdMutex::new(HashSet::new()),
                recording_shortcut: AtomicBool::new(false),
                clipboard: ClipboardWorker::start()?,
                config_store,
                monitor_request: watch::channel(monitor::Request::default()).0,
                monitor_tasks: StdMutex::new(Vec::new()),
                updates: updates.clone(),
            });
            let tray_available = install_tray(app.handle()).is_ok();
            if !tray_available {
                app.state::<Desktop>().session.blocking_lock().tray_status =
                    "Unavailable: reopen through the app launcher.".into();
            }
            watch_settings(app.handle().clone());
            let monitor_state = app.state::<Desktop>();
            sync_monitor(&monitor_state,&mut monitor_state.session.blocking_lock());
            start_background(app.handle());
            let operations = app.state::<Desktop>().operations.clone();
            let mut changed = operations.subscribe();
            let operation_app = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                while changed.changed().await.is_ok() {
                    if let Ok(view) = operations.view() {
                        let _ = operation_app.emit("operation-state", view);
                    }
                }
            });
            // The configured window starts hidden: hiding it here is too late to
            // prevent a visible flash (and possible focus theft) during creation.
            let background = crate::startup_native::available()
                && std::env::args().any(|arg| arg == "--background");
            if !background || !tray_available {
                show_panel(app.handle(), false);
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // Keep the native clipboard owner alive until explicit Quit.
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_panel,
            reconnect_history,
            get_operation,
            cancel_operation,
            check_updates,
            download_update,
            install_update,
            set_shortcut_recording,
            test_notification,
            apply_settings,
            acknowledge_migration_summary,
            remove_profile,
            test_connection,
            refresh_item,
            select_item,
            delete_item,
            clear_history,
            share_clipboard,
            discard_share,
            recover_share,
            copy_item,
            hide_panel,
            quit_app
        ])
        .build(tauri::generate_context!())
        .expect("could not start desktop prototype");
    app.run(|app, event| {
        if matches!(event, tauri::RunEvent::Exit) {
            cleanup_desktop(app);
        }
    });
}

fn start_background(app: &AppHandle) {
    let state = app.state::<Desktop>();
    let monitor_input = state.monitor_request.subscribe();
    let monitor_generation = state.monitor_request.clone();
    let (monitor_output, mut monitor_updates) = watch::channel(None::<Arc<monitor::Update>>);
    let monitor_task = tauri::async_runtime::spawn(monitor::run(monitor_input, monitor_output));
    let monitor_app = app.clone();
    let apply_task = tauri::async_runtime::spawn(async move {
        while monitor_updates.changed().await.is_ok() {
            let update = monitor_updates.borrow_and_update().clone();
            let Some(update) = update else {
                continue;
            };
            let state = monitor_app.state::<Desktop>();
            let mut session = state.session.lock().await;
            if monitor_generation.borrow().generation == update.generation
                && session.apply_metadata(&update)
            {
                let _ = monitor_app.emit("settings-changed", ());
            }
        }
    });
    state
        .monitor_tasks
        .lock()
        .unwrap()
        .extend([monitor_task, apply_task]);
    let updates = state.updates.clone();
    if updates.view().available {
        let update_app = app.clone();
        let task = tauri::async_runtime::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(15)).await;
            loop {
                if !updates.view().busy && updates.view().phase != "ready" {
                    let _ = updates.check(&update_app).await;
                }
                tokio::time::sleep(std::time::Duration::from_secs(6 * 60 * 60)).await;
            }
        });
        state.monitor_tasks.lock().unwrap().push(task);
    }
}

fn cleanup_desktop(app: &AppHandle) {
    cleanup_transport(app);
    if let Some(state) = app.try_state::<Desktop>() {
        state.clipboard.stop();
    }
}

fn cleanup_transport(app: &AppHandle) {
    let Some(state) = app.try_state::<Desktop>() else {
        return;
    };
    let tasks = state
        .monitor_tasks
        .lock()
        .map(|mut tasks| std::mem::take(&mut *tasks))
        .unwrap_or_default();
    for task in &tasks {
        task.abort();
    }
    let connection = state
        .session
        .try_lock()
        .ok()
        .and_then(|mut session| session.connection.take());
    // Join the aborted owner so its SSH child/reader are dropped before process exit.
    tauri::async_runtime::block_on(async {
        let _ = tokio::time::timeout(std::time::Duration::from_secs(2), async {
            for task in tasks {
                let _ = task.await;
            }
            if let Some(connection) = connection {
                connection.close().await;
            }
        })
        .await;
    });
}

#[cfg(test)]
#[path = "panel/startup_tests.rs"]
mod startup_tests;

#[cfg(test)]
#[path = "panel/ipc_contract_tests.rs"]
mod ipc_contract_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_from_before_a_user_action_or_another_profile_cannot_replace_its_result() {
        let mut session = session();
        session.saved_config.as_mut().unwrap().profiles[0].endpoint =
            ssh_clipboard::configuration::Endpoint::Shelf {
                max_item_bytes: 1024,
                max_decoded_bytes: 1024,
            };
        session.settings =
            SessionSettings::from_configuration(session.saved_config.as_ref().unwrap()).unwrap();
        let started = tokio::time::Instant::now();
        let update = monitor::Update {
            generation: 1,
            target: session.monitor_target().unwrap(),
            status: monitor::Status {
                phase: monitor::Phase::Live,
                detail: "History is up to date.".into(),
                retry_in_ms: None,
            },
            snapshot: Some(Arc::new(ssh_clipboard::client::session::Snapshot {
                epoch: Uuid::from_u128(2),
                sequence: 0.into(),
                items: vec![],
            })),
            snapshot_started_at: Some(started),
        };
        session.metadata_barrier = started + std::time::Duration::from_secs(1);
        assert!(session.apply_metadata(&update));
        assert_eq!(session.item.as_ref().unwrap().data, b"retained snapshot");
        assert!(session.history.epoch.is_none());
        session.metadata_barrier = started;
        assert!(session.apply_metadata(&update));
        assert!(session.item.is_none());
        assert_eq!(session.history.epoch, Some(Uuid::from_u128(2)));
        session.saved_config.as_mut().unwrap().revision += 1;
        assert!(!session.apply_metadata(&update));
    }

    pub(super) fn persisted_session() -> (tempfile::TempDir, ConfigStore, Session) {
        let directory = tempfile::tempdir().unwrap();
        let store = ConfigStore::new(directory.path().join("settings.json"));
        let mut session = session();
        session.saved_config.as_mut().unwrap().profiles[0]
            .transport
            .timeout_ms = 9000;
        session
            .saved_config
            .as_mut()
            .unwrap()
            .preferences
            .start_at_login = true;
        let saved = store
            .prepare(None, session.saved_config.clone().unwrap())
            .unwrap()
            .commit()
            .unwrap();
        session.settings = SessionSettings::from_configuration(&saved).unwrap();
        session.saved_config = Some(saved);
        (directory, store, session)
    }

    #[test]
    fn reviewing_an_import_preserves_active_setup_and_clipboard_state() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("agent.toml");
        std::fs::write(
            &source,
            include_bytes!("../../../tests/fixtures/legacy-agent.toml"),
        )
        .unwrap();
        let store = ConfigStore::new(directory.path().join("settings.json"));
        let config = store.initialize_from_legacy(&source).unwrap().unwrap();
        let mut session = session();
        session.settings = SessionSettings::from_configuration(&config).unwrap();
        session.saved_config = Some(config.clone());
        let revision = config.revision.to_string();
        let settings = serde_json::to_value(&session.settings).unwrap();
        let shortcuts = session.shortcuts.open_label();
        let barrier = session.metadata_barrier;
        assert!(session.view().unwrap().migration_summary.is_some());
        assert!(acknowledge_import(&mut session, &store, Some("999")).is_err());
        assert!(session.view().unwrap().migration_summary.is_some());
        session.settings_notice = Some("Repair saved settings first.".into());
        assert!(acknowledge_import(&mut session, &store, Some(&revision)).is_err());
        assert_eq!(store.load().unwrap(), Some(config.clone()));
        session.settings_notice = None;
        acknowledge_import(&mut session, &store, Some(&revision)).unwrap();
        assert!(session.view().unwrap().migration_summary.is_none());
        assert_eq!(serde_json::to_value(&session.settings).unwrap(), settings);
        assert_eq!(session.shortcuts.open_label(), shortcuts);
        assert_eq!(session.metadata_barrier, barrier);
        assert_eq!(session.item.as_ref().unwrap().data, b"retained snapshot");
        assert!(session.connected);
        assert_eq!(
            session.saved_config.as_ref().unwrap().preferences,
            config.preferences
        );
        assert!(session.check_revision(Some(&revision)).is_err());
    }

    #[test]
    fn advanced_gui_edits_persist_per_profile_and_drive_the_actual_transport() {
        let (_directory, store, mut session) = persisted_session();
        let mut next = session.saved_config.clone().unwrap();
        let mut other = next.profiles[0].clone();
        other.id = "other".into();
        other.transport.timeout_ms = 16000;
        other.transport.ssh_bin = Some("other/ssh".into());
        next.profiles.push(other.clone());
        save_configuration(&mut session, &store, next, |_, _| Ok("Registered".into())).unwrap();
        assert!(session.item.is_some());

        let mut draft = session.settings.clone();
        draft.transport.ssh_options = vec!["BatchMode=no".into(), "ServerAliveInterval=20".into()];
        draft.transport.ssh_binary = "path with spaces/ssh".into();
        draft.transport.max_size = "1048576".into();
        draft.transport.timeout_ms = "12000".into();
        draft.transport.resync_frames = false;
        draft.transport.resync_max_bytes = "4096".into();
        let mut next = session.saved_config.clone().unwrap();
        draft.update_configuration(&mut next).unwrap();
        save_configuration(&mut session, &store, next, |_, _| Ok("Registered".into())).unwrap();

        let saved = store.load().unwrap().unwrap();
        assert_eq!(saved.profiles[1], other);
        assert_eq!(
            saved.selected_profile().unwrap().transport,
            draft.transport.to_shared().unwrap()
        );
        let client = session.client_config().unwrap();
        assert_eq!(client.timeout_ms, 12000);
        assert_eq!(client.max_size, 1048576);
        assert_eq!(client.resync_max_bytes, 4096);
        assert!(!client.resync_frames);
        assert_eq!(
            client.ssh.ssh_options,
            ["BatchMode=yes", "BatchMode=no", "ServerAliveInterval=20"]
        );
        assert_eq!(
            client.ssh.ssh_bin.unwrap(),
            std::path::PathBuf::from("path with spaces/ssh")
        );
        assert!(session.item.is_none());
        assert!(!session.connected);
        let view = session.view().unwrap();
        assert_eq!(view.settings.transport.timeout_ms, "12000");
        assert_eq!(view.profiles[1].transport.timeout_ms, "16000");
    }

    #[test]
    fn invalid_advanced_drafts_leave_the_configuration_and_snapshot_untouched() {
        let (_directory, store, session) = persisted_session();
        let original = session.saved_config.clone().unwrap();
        let mut draft = session.settings.clone();
        draft.transport.timeout_ms = "0".into();
        let mut next = original.clone();
        assert!(
            draft
                .update_configuration(&mut next)
                .unwrap_err()
                .contains("Timeout")
        );
        assert_eq!(next, original);
        assert_eq!(store.load().unwrap(), Some(original));
        assert!(session.item.is_some());
    }

    #[test]
    fn desktop_profile_lifecycle_preserves_identity_policy_and_global_preferences() {
        let (_directory, store, mut session) = persisted_session();
        let original = session.saved_config.clone().unwrap();
        let draft = SessionSettings {
            profile_id: None,
            profile_name: " Second server ".into(),
            target: "second-hub".into(),
            port: Some(2222),
            identity_file: "path with spaces/key".into(),
            transport: crate::connection::TransportDraft::default(),
            ..session.settings.clone()
        };
        let mut next = original.clone();
        draft.update_configuration(&mut next).unwrap();
        save_configuration(&mut session, &store, next, |_, _| Ok("Registered".into())).unwrap();
        let saved = store.load().unwrap().unwrap();
        let id = saved.default_profile.clone();
        assert_ne!(id, "default");
        assert_eq!(saved.profiles[0], original.profiles[0]);
        assert_eq!(saved.preferences, original.preferences);
        assert_eq!(saved.device, original.device);
        assert_eq!(saved.selected_profile().unwrap().name, "Second server");
        assert_eq!(saved.selected_profile().unwrap().transport.timeout_ms, 7000);
        assert_eq!(session.settings.profile_id.as_deref(), Some(id.as_str()));
        assert_eq!(session.client_config().unwrap().ssh.port, Some(2222));
        assert!(session.item.is_none());

        // A display-only rename keeps the connection and its current snapshot.
        session.item = Some(ClipboardValue {
            content_type: ssh_clipboard::protocol::CONTENT_TYPE_TEXT.into(),
            data: b"second snapshot".to_vec(),
            created_at: 2,
        });
        session.connected = true;
        let mut renamed = session.settings.clone();
        renamed.profile_name = "Renamed server".into();
        let mut next = saved.clone();
        renamed.update_configuration(&mut next).unwrap();
        save_configuration(&mut session, &store, next, |_, _| Ok("Registered".into())).unwrap();
        assert!(session.item.is_some());
        assert!(session.connected);
        assert_eq!(session.saved_config.as_ref().unwrap().default_profile, id);

        let mut next = session.saved_config.clone().unwrap();
        next.remove_profile(&id).unwrap();
        save_configuration(&mut session, &store, next, |_, _| Ok("Registered".into())).unwrap();
        let remaining = store.load().unwrap().unwrap();
        assert_eq!(remaining.default_profile, "default");
        assert_eq!(remaining.profiles, [original.profiles[0].clone()]);
        assert_eq!(remaining.preferences, original.preferences);
        assert_eq!(remaining.device, original.device);
        assert!(session.item.is_none());
        assert!(!session.connected);
        assert_eq!(session.client_config().unwrap().timeout_ms, 9000);
    }

    #[test]
    fn applying_another_profile_at_the_same_target_clears_the_previous_snapshot() {
        let (_directory, store, mut session) = persisted_session();
        let mut next = session.saved_config.clone().unwrap();
        let mut other = next.profiles[0].clone();
        other.id = "second".into();
        next.profiles.push(other);
        next.default_profile = "second".into();
        save_configuration(&mut session, &store, next, |_, _| Ok("Registered".into())).unwrap();
        assert_eq!(session.settings.target, "first-hub");
        assert_eq!(session.view().unwrap().profiles.len(), 2);
        assert!(session.item.is_none());
        assert!(!session.clipboard_checked);
        assert!(!session.connected);
        assert!(session.check_revision(Some("1")).is_err());
    }

    #[test]
    fn a_stale_profile_save_does_not_activate_or_overwrite_an_external_edit() {
        let (_directory, store, mut session) = persisted_session();
        let original = session.saved_config.clone().unwrap();
        let mut external = original.clone();
        external.device.name = "Changed through CLI".into();
        let external = store
            .prepare(Some(&original), external)
            .unwrap()
            .commit()
            .unwrap();
        let mut next = original.clone();
        next.profiles[0].name = "Stale rename".into();
        assert!(
            save_configuration(&mut session, &store, next, |_, _| {
                panic!("a stale save must fail before OS activation")
            })
            .is_err()
        );
        assert_eq!(store.load().unwrap(), Some(external));
        assert_eq!(session.saved_config, Some(original));
        assert!(session.item.is_some());
    }

    #[test]
    fn rejected_activation_abandons_the_profile_save_and_keeps_the_snapshot() {
        let (_directory, store, mut session) = persisted_session();
        let original = session.saved_config.clone().unwrap();
        let mut next = original.clone();
        next.profiles[0].target = "second-hub".into();
        assert_eq!(
            save_configuration(&mut session, &store, next, |_, _| Err(
                "Shortcut conflict".into()
            ))
            .unwrap_err(),
            "Shortcut conflict"
        );
        assert_eq!(store.load().unwrap(), Some(original.clone()));
        assert_eq!(session.saved_config, Some(original));
        assert!(session.item.is_some());
        assert!(session.connected);
    }

    #[test]
    fn failed_replacement_restores_os_settings_and_keeps_the_previous_profile_active() {
        let (directory, store, mut session) = persisted_session();
        let path = directory.path().join("settings.json");
        let original_bytes = std::fs::read(&path).unwrap();
        let original = session.saved_config.clone().unwrap();
        let mut next = original.clone();
        next.preferences.open_shortcut = "CommandOrControl+Shift+K".into();
        let mut activations = Vec::new();
        let result = save_configuration(&mut session, &store, next, |_, settings| {
            activations.push(settings.open_shortcut.clone());
            if activations.len() == 1 {
                // Inject replacement failure after preparation/activation, within a temp directory.
                std::fs::remove_file(&path).unwrap();
                std::fs::create_dir(&path).unwrap();
            } else {
                std::fs::remove_dir(&path).unwrap();
                std::fs::write(&path, &original_bytes).unwrap();
            }
            Ok("Registered".into())
        });
        assert!(result.is_err());
        assert_eq!(
            activations,
            ["CommandOrControl+Shift+K", "CommandOrControl+Shift+V"]
        );
        assert_eq!(store.load().unwrap(), Some(original.clone()));
        assert_eq!(session.saved_config, Some(original));
        assert!(session.item.is_some());
    }

    #[test]
    fn desktop_lockfile_imports_the_same_legacy_fixture_as_the_cli() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("agent.toml");
        let fixture = include_str!("../../../tests/fixtures/legacy-agent.toml");
        std::fs::write(&source, fixture).unwrap();
        let config = ConfigStore::new(directory.path().join("settings.json"))
            .initialize_from_legacy(&source)
            .unwrap()
            .unwrap();
        assert_eq!(config.profiles[0].target, "legacy-hub");
        assert_eq!(config.profiles[0].transport.timeout_ms, 9000);
        assert_eq!(
            config.profiles[0].transport.ssh_options,
            ["BatchMode=no", "ServerAliveInterval=20"]
        );
        assert!(config.preferences.start_at_login);
        assert_eq!(
            std::fs::read(config.legacy_import.unwrap().backup).unwrap(),
            fixture.as_bytes()
        );
    }

    #[test]
    fn failed_file_and_shortcut_restoration_suspend_direct_dispatch() {
        let (directory, store, mut session) = persisted_session();
        let path = directory.path().join("settings.json");
        let original = session.saved_config.clone().unwrap();
        let mut next = original.clone();
        next.preferences.direct_shortcuts_enabled = true;
        next.preferences.direct_share_shortcut = Some("Ctrl+Alt+C".into());
        let mut first = true;
        assert!(
            save_configuration(&mut session, &store, next, |session, settings| {
                if first {
                    first = false;
                    session.shortcuts = Bindings::requested(settings).unwrap();
                    std::fs::remove_file(&path).unwrap();
                    std::fs::create_dir(&path).unwrap();
                    Ok("Registered".into())
                } else {
                    Err("Injected OS restoration failure".into())
                }
            })
            .is_err()
        );
        assert_eq!(session.saved_config, Some(original));
        let share: Shortcut = "Ctrl+Alt+C".parse().unwrap();
        assert_eq!(session.shortcuts.action(share.id()), None);
        assert!(
            session
                .shortcut_status
                .contains("Direct actions are paused")
        );
    }

    #[test]
    fn imported_transport_is_used_and_gui_batch_mode_precedes_legacy_options() {
        let mut session = session();
        let settings = &mut session.saved_config.as_mut().unwrap().profiles[0].transport;
        settings.ssh_options = vec!["BatchMode=no".into(), "ServerAliveInterval=20".into()];
        settings.timeout_ms = 9000;
        settings.max_size = 123456;
        settings.resync_frames = false;
        settings.resync_max_bytes = 2048;
        let config = session.client_config().unwrap();
        assert_eq!(
            config.ssh.ssh_options,
            ["BatchMode=yes", "BatchMode=no", "ServerAliveInterval=20"]
        );
        assert_eq!(config.timeout_ms, 9000);
        assert_eq!(config.max_size, 123456);
        assert!(!config.resync_frames);
        assert_eq!(config.resync_max_bytes, 2048);
    }

    #[test]
    fn changing_advanced_connection_options_clears_the_previous_snapshot() {
        let mut session = session();
        let mut next = session.saved_config.clone().unwrap();
        next.revision += 1;
        next.profiles[0]
            .transport
            .ssh_options
            .push("ProxyJump=other-gateway".into());
        assert!(synchronize_settings(
            &mut session,
            Ok(Some(next)),
            |_, _| Ok("Registered".into())
        ));
        assert!(session.item.is_none());
        assert!(!session.connected);
    }

    #[test]
    fn endpoint_changes_clear_both_history_and_the_legacy_snapshot_without_io() {
        let mut session = session();
        session.history.epoch = Some(Uuid::from_u128(1));
        session.history.selected = Some(Uuid::from_u128(2));
        let mut next = session.saved_config.clone().unwrap();
        next.revision += 1;
        next.profiles[0].endpoint = ssh_clipboard::configuration::Endpoint::Shelf {
            max_item_bytes: 67108864,
            max_decoded_bytes: 268435456,
        };
        assert!(synchronize_settings(
            &mut session,
            Ok(Some(next)),
            |_, _| Ok("Registered".into())
        ));
        assert!(session.item.is_none());
        assert!(session.history.epoch.is_none());
        assert!(session.history.selected.is_none());
        assert!(!session.clipboard_checked);
        assert!(!session.connected);
    }

    #[tokio::test]
    async fn share_freezes_one_capture_before_an_ssh_failure_and_never_writes_the_clipboard() {
        use std::sync::atomic::AtomicUsize;
        struct Captured {
            reads: Arc<AtomicUsize>,
            writes: Arc<AtomicUsize>,
        }
        impl ssh_clipboard::client_actions::ClipboardAccess for Captured {
            fn read_text(&mut self) -> eyre::Result<String> {
                self.reads.fetch_add(1, Ordering::SeqCst);
                Ok("frozen synthetic text".into())
            }
            fn read_image(
                &mut self,
            ) -> eyre::Result<ssh_clipboard::client::image::ImageData<'static>> {
                Err(eyre::eyre!("unused"))
            }
            fn write_text(&mut self, _: &str) -> eyre::Result<()> {
                self.writes.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
            fn write_image(
                &mut self,
                _: ssh_clipboard::client::image::ImageData<'static>,
            ) -> eyre::Result<()> {
                self.writes.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        }
        let reads = Arc::new(AtomicUsize::new(0));
        let writes = Arc::new(AtomicUsize::new(0));
        let captured_reads = reads.clone();
        let captured_writes = writes.clone();
        let clipboard = ClipboardWorker::start_with(move || {
            Ok(Captured {
                reads: captured_reads.clone(),
                writes: captured_writes.clone(),
            })
        })
        .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let mut session = session();
        let config = session.saved_config.as_mut().unwrap();
        config.device.id = Some(Uuid::from_u128(10).to_string());
        config.profiles[0].endpoint = ssh_clipboard::configuration::Endpoint::Shelf {
            max_item_bytes: 1024,
            max_decoded_bytes: 1024,
        };
        config.profiles[0].transport.ssh_bin = Some(dir.path().join("missing-ssh"));
        session.settings = SessionSettings::from_configuration(config).unwrap();
        session.item = None;
        let operation = Controller::default().begin("Share").unwrap();
        assert!(
            share_current(&mut session, &clipboard, &operation)
                .await
                .is_err()
        );
        assert_eq!(reads.load(Ordering::SeqCst), 1);
        assert_eq!(writes.load(Ordering::SeqCst), 0);
        assert!(session.connection.is_none());
        assert!(session.item.is_none());
        session.pending_share = Some(PendingShare::new(
            Uuid::from_u128(1),
            Uuid::from_u128(2),
            ShelfOrigin {
                device_id: Uuid::from_u128(3),
                name: "Synthetic".into(),
            },
            ShelfRepresentation {
                id: 0,
                content_type: ShelfContent::Text,
                encoded_bytes: 6,
                dimensions: None,
                decoded_bytes: None,
            },
            ClipboardValue {
                content_type: ssh_clipboard::protocol::CONTENT_TYPE_TEXT.into(),
                data: b"frozen".to_vec(),
                created_at: 1,
            },
            10_000,
            Some(0.into()),
        ));
        assert!(
            share_current(&mut session, &clipboard, &operation)
                .await
                .unwrap_err()
                .contains("earlier uncertain share")
        );
        assert_eq!(
            reads.load(Ordering::SeqCst),
            1,
            "pending recovery must block recapturing a newer clipboard"
        );
        clipboard.stop();
    }

    pub(super) fn session() -> Session {
        let mut config = Configuration {
            revision: 1,
            ..Default::default()
        };
        config.profiles[0].target = "first-hub".into();
        Session {
            history: History::default(),
            connection: None,
            settings: SessionSettings::from_configuration(&config).unwrap(),
            saved_config: Some(config),
            settings_notice: None,
            connection_report: None,
            operation_notice: None,
            operation_counter: 0,
            notification_status: notifications::Status {
                available: false,
                detail: "Test fixture",
            },
            history_status: None,
            metadata_barrier: tokio::time::Instant::now(),
            pending_share: None,
            startup_status: ssh_clipboard_desktop_prototype::startup::Status {
                available: false,
                enabled: None,
                desired: false,
                detail: "Test fixture",
            },
            updates: crate::updates::Updates::new("0.0.0".into()),
            item: Some(ClipboardValue {
                content_type: ssh_clipboard::protocol::CONTENT_TYPE_TEXT.into(),
                data: b"retained snapshot".to_vec(),
                created_at: 1,
            }),
            clipboard_checked: true,
            connected: true,
            shortcuts: Bindings::requested(&SessionSettings::default()).unwrap(),
            shortcut_status: "Registered".into(),
            tray_status: "Available".into(),
        }
    }

    #[test]
    fn profile_reload_clears_snapshots_even_when_the_destination_is_identical() {
        let mut session = session();
        session.connection_report = Some(ConnectionReport::from_failure(
            &ssh_clipboard::client::failure::TransportFailure {
                kind: ssh_clipboard::client::failure::FailureKind::Authentication,
                outcome_uncertain: false,
            },
        ));
        let mut next = session.saved_config.clone().unwrap();
        let mut second = next.profiles[0].clone();
        second.id = "second".into();
        next.profiles.push(second);
        next.default_profile = "second".into();
        next.revision = 2;
        assert!(synchronize_settings(
            &mut session,
            Ok(Some(next)),
            |_, _| Ok("Registered".into())
        ));
        assert!(session.item.is_none());
        assert!(!session.clipboard_checked);
        assert!(!session.connected);
        assert!(session.connection_report.is_none());
        assert!(session.check_revision(Some("1")).is_err());
        assert!(session.check_revision(Some("2")).is_ok());
    }

    #[test]
    fn preference_reload_preserves_snapshot_and_unchanged_files_do_not_reconfigure() {
        let mut session = session();
        let mut next = session.saved_config.clone().unwrap();
        next.preferences.show_preview = false;
        next.preferences.close_after_copy = true;
        next.revision = 2;
        assert!(synchronize_settings(
            &mut session,
            Ok(Some(next.clone())),
            |_, _| Ok("Registered".into())
        ));
        assert!(session.item.is_some());
        assert!(session.connected);
        assert!(!session.settings.show_preview);
        assert!(session.settings.close_after_copy);
        assert!(!synchronize_settings(
            &mut session,
            Ok(Some(next)),
            |_, _| panic!("unchanged settings must not register shortcuts")
        ));
    }

    #[test]
    fn transfer_results_are_retained_but_cannot_cross_profiles() {
        let mut session = session();
        session.record_operation(Ok(()), true);
        let first = session.view().unwrap().operation_notice.unwrap();
        assert!(!first.failed);
        assert_eq!(first.message, "Current clipboard shared.");
        session.record_operation(Err("Synthetic failure".into()), false);
        let failed = session.view().unwrap().operation_notice.unwrap();
        assert!(failed.failed);
        assert_ne!(first.id, failed.id);
        let mut next = session.saved_config.clone().unwrap();
        next.preferences.notifications = ssh_clipboard::configuration::NotificationPreference::Off;
        synchronize_settings(&mut session, Ok(Some(next)), |_, _| Ok("Registered".into()));
        assert!(session.operation_notice.is_some());
        let mut next = session.saved_config.clone().unwrap();
        next.profiles[0].target = "other-hub".into();
        synchronize_settings(&mut session, Ok(Some(next)), |_, _| Ok("Registered".into()));
        assert!(session.operation_notice.is_none());
    }

    #[test]
    fn corrupt_reload_preserves_active_settings_until_the_file_recovers() {
        let mut session = session();
        let original = session.saved_config.clone();
        assert!(synchronize_settings(
            &mut session,
            Err("Invalid settings".into()),
            |_, _| panic!("invalid file must not activate")
        ));
        assert_eq!(session.saved_config, original);
        assert_eq!(session.settings.target, "first-hub");
        assert!(session.item.is_some());
        assert!(!session.view().unwrap().settings_writable);
        assert!(!synchronize_settings(
            &mut session,
            Err("Invalid settings".into()),
            |_, _| unreachable!()
        ));
        assert!(synchronize_settings(
            &mut session,
            Ok(original),
            |_, _| unreachable!()
        ));
        assert!(session.view().unwrap().settings_writable);
    }

    #[test]
    fn rejected_os_settings_preserve_the_previous_active_profile_and_report_once() {
        let mut session = session();
        let original = session.saved_config.clone();
        let mut next = original.clone().unwrap();
        next.profiles[0].target = "second-hub".into();
        next.revision = 2;
        assert!(synchronize_settings(
            &mut session,
            Ok(Some(next.clone())),
            |_, _| Err("Shortcut conflict".into())
        ));
        assert_eq!(session.saved_config, original);
        assert_eq!(session.settings.target, "first-hub");
        assert!(session.item.is_some());
        assert!(!synchronize_settings(
            &mut session,
            Ok(Some(next)),
            |_, _| Err("Shortcut conflict".into())
        ));
    }

    #[test]
    fn deleting_saved_settings_returns_to_setup_and_clears_the_previous_item() {
        let mut session = session();
        assert!(synchronize_settings(&mut session, Ok(None), |_, _| Ok(
            "Registered".into()
        )));
        assert!(session.settings.target.is_empty());
        assert!(session.item.is_none());
        assert!(session.saved_config.is_none());
        assert!(session.check_revision(None).is_ok());
        assert!(session.check_revision(Some("1")).is_err());
    }
}
