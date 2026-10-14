use crate::clipboard::ClipboardWorker;
use crate::model::{
    ConnectionProfile, ItemPreview, SessionSettings, is_wayland, validate_transfer_size,
};
use crate::shortcuts::{Action, Bindings, Registry};
use serde::Serialize;
use ssh_clipboard::client::diagnostics::{ConnectionReport, diagnose, server_problem};
use ssh_clipboard::client::transport::{make_request, send_request};
use ssh_clipboard::configuration::{ConfigStore, Configuration};
use ssh_clipboard::protocol::{ClipboardValue, ErrorCode, RequestKind, ResponseKind};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use tokio::sync::Mutex;

struct Session {
    settings: SessionSettings,
    item: Option<ClipboardValue>,
    clipboard_checked: bool,
    shortcuts: Bindings,
    shortcut_status: String,
    tray_status: String,
    connected: bool,
    saved_config: Option<Configuration>,
    settings_notice: Option<String>,
    connection_report: Option<ConnectionReport>,
}

struct Desktop {
    session: Arc<Mutex<Session>>,
    pressed_shortcuts: StdMutex<HashSet<u32>>,
    recording_shortcut: AtomicBool,
    clipboard: ClipboardWorker,
    config_store: Option<ConfigStore>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PanelView {
    settings: SessionSettings,
    profiles: Vec<ConnectionProfile>,
    item: Option<ItemPreview>,
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
    migration_summary: Option<&'static str>,
    connection_report: Option<ConnectionReport>,
}

impl Session {
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
            connection_report: self.connection_report.clone(),
            profiles: self.saved_config.as_ref().map_or_else(
                || Configuration::default().profiles.iter().map(ConnectionProfile::from).collect(),
                |c| c.profiles.iter().map(ConnectionProfile::from).collect(),
            ),
            clipboard_checked: self.clipboard_checked,
            item: self
                .item
                .as_ref()
                .map(|value| ItemPreview::from_value(value, self.settings.show_preview))
                .transpose()?,
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
            migration_summary: self.saved_config.as_ref().and_then(|c| c.legacy_import.as_ref())
                .map(|_| "Imported your existing connection and preferences and saved a backup. Review retained direct shortcuts before enabling them. Start-at-login is retained for the later desktop setup."),
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
        {
            self.item = None;
            self.clipboard_checked = false;
            self.connected = false;
            self.connection_report = None;
        }
        self.settings = next;
        self.saved_config = loaded;
        self.settings_notice = None;
        self.shortcut_status = shortcut_status;
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
            let Some(store) = state.config_store.as_ref() else {
                continue;
            };
            let loaded = store.load().map_err(|error| format!("{error} Settings were left untouched at {}. Repair the file to reload it automatically.", store.path().display()));
            if synchronize_settings(&mut session, loaded, |session, settings| {
                configure_shortcut(&app, session, settings)
            }) {
                let _ = app.emit("settings-changed", ());
            }
        }
    });
}

#[tauri::command]
async fn get_panel(state: State<'_, Desktop>) -> Result<PanelView, String> {
    state.session.lock().await.view()
}

#[tauri::command]
fn set_shortcut_recording(state: State<'_, Desktop>, active: bool) {
    // Independent of the transfer lock: losing focus while recording must not start a share.
    state.recording_shortcut.store(active, Ordering::SeqCst);
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
    match session.shortcuts.replace(desired, &mut NativeRegistry(app)) {
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
    mut settings: SessionSettings,
    expected_revision: Option<String>,
) -> Result<PanelView, String> {
    settings.target = settings.target.trim().into();
    settings.validate()?;
    let mut session = state
        .session
        .try_lock()
        .map_err(|_| "Wait for the current operation to finish.".to_string())?;
    session.check_revision(expected_revision.as_deref()).map_err(|_| "Saved settings changed while you were editing. Discard your draft to load them before applying changes.".to_string())?;
    if let Some(notice) = &session.settings_notice {
        return Err(notice.clone());
    }
    let store = state
        .config_store
        .as_ref()
        .ok_or("The settings location is unavailable.")?;
    let mut next_config = session.saved_config.clone().unwrap_or_default();
    settings.update_configuration(&mut next_config)?;
    save_configuration(&mut session, store, next_config, |session, settings| {
        configure_shortcut(&app, session, settings)
    })?;
    session.view()
}

/// Both GUI writes and watcher reloads use the same profile-isolation boundary.
fn save_configuration(
    session: &mut Session,
    store: &ConfigStore,
    next_config: Configuration,
    mut configure: impl FnMut(&mut Session, &SessionSettings) -> Result<String, String>,
) -> Result<(), String> {
    let settings = SessionSettings::from_configuration(&next_config)?;
    settings.validate()?;
    // Validate/check for a stale writer and flush the temporary file before changing OS state.
    let pending = store
        .prepare(session.saved_config.as_ref(), next_config)
        .map_err(|e| e.to_string())?;
    let previous_settings = session.settings.clone();
    let status = configure(session, &settings)?;
    let saved = match pending.commit() {
        Ok(saved) => saved,
        Err(error) => {
            session.shortcut_status = match configure(session, &previous_settings) {
                Ok(status) => status,
                Err(_) => {
                    session.shortcuts.suspend_direct();
                    "Could not restore previous shortcuts. Direct actions are paused. Quit and reopen to restore saved settings.".into()
                }
            };
            return Err(error.to_string());
        }
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
    let report = diagnose(&session.client_config()?).await;
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
    fetch_item(&mut session).await?;
    session.view()
}

async fn fetch_item(session: &mut Session) -> Result<(), String> {
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
    share_current(&mut session, &state.clipboard).await?;
    session.view()
}

async fn share_current(session: &mut Session, clipboard: &ClipboardWorker) -> Result<(), String> {
    session.settings.validate_connection()?;
    let value = clipboard.read(session.client_config()?.max_size).await?;
    ItemPreview::from_value(&value, session.settings.show_preview)?;
    match request(
        session,
        RequestKind::Set {
            value: value.clone(),
        },
    )
    .await?
    {
        ResponseKind::Ok => session.item = Some(value),
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
        let result = if session.settings_notice.is_some() {
            Err("Repair the saved settings before using direct shortcuts.".into())
        } else {
            match action {
                Action::Share => share_current(&mut session, &state.clipboard).await,
                Action::Copy => match fetch_item(&mut session).await {
                    Ok(()) => match session.item.clone() {
                        Some(value) => state.clipboard.write(value).await,
                        None => Err(
                            "The shared clipboard is empty. Your clipboard was left unchanged."
                                .into(),
                        ),
                    },
                    Err(error) => Err(error),
                },
                Action::Open => unreachable!(),
            }
        };
        drop(session);
        let _ = app.emit("settings-changed", ());
        // Until native notification controls land, show an explicit result in the panel.
        show_panel(&app, false);
        match result {
            Ok(()) => {
                let _ = app.emit(
                    "operation-complete",
                    if action == Action::Share {
                        "Current clipboard shared."
                    } else {
                        "Shared clipboard copied to this computer."
                    },
                );
            }
            Err(error) => {
                let _ = app.emit("operation-notice", error);
            }
        }
    });
}

#[tauri::command]
async fn copy_item(
    app: AppHandle,
    state: State<'_, Desktop>,
    expected_revision: Option<String>,
) -> Result<PanelView, String> {
    let session = state
        .session
        .try_lock()
        .map_err(|_| "Another operation is running.".to_string())?;
    session.check_revision(expected_revision.as_deref())?;
    // Copy the snapshot being shown, never silently substitute a newer remote value.
    let value = session
        .item
        .clone()
        .ok_or_else(|| "Refresh the shared clipboard first.".to_string())?;
    state.clipboard.write(value).await?;
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
    let mut builder =
        tauri::Builder::default().plugin(tauri_plugin_single_instance::init(|app, _, _| {
            show_panel(app, false)
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
            let (config_store, loaded, settings_notice) = match ConfigStore::user() {
                Ok(store) => match store.initialize() {
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
            let mut session = Session {
                settings: settings.clone(),
                item: None,
                clipboard_checked: false,
                shortcuts: Bindings::default(),
                shortcut_status: "Disabled".into(),
                tray_status: "Available".into(),
                connected: false,
                saved_config: loaded,
                settings_notice,
                connection_report: None,
            };
            session.shortcut_status = if is_wayland() {
                "Wayland: use the launcher or tray menu. Portal support is pending.".into()
            } else {
                configure_shortcut(app.handle(), &mut session, &settings)
                    .unwrap_or_else(|message| message)
            };
            app.manage(Desktop {
                session: Arc::new(Mutex::new(session)),
                pressed_shortcuts: StdMutex::new(HashSet::new()),
                recording_shortcut: AtomicBool::new(false),
                clipboard: ClipboardWorker::start()?,
                config_store,
            });
            if install_tray(app.handle()).is_err() {
                app.state::<Desktop>().session.blocking_lock().tray_status =
                    "Unavailable: reopen through the app launcher.".into();
            }
            watch_settings(app.handle().clone());
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
            set_shortcut_recording,
            apply_settings,
            remove_profile,
            test_connection,
            refresh_item,
            share_clipboard,
            copy_item,
            hide_panel,
            quit_app
        ])
        .build(tauri::generate_context!())
        .expect("could not start desktop prototype");
    app.run(|app, event| {
        if matches!(event, tauri::RunEvent::Exit) {
            app.state::<Desktop>().clipboard.stop();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn persisted_session() -> (tempfile::TempDir, ConfigStore, Session) {
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

    fn session() -> Session {
        let mut config = Configuration {
            revision: 1,
            ..Default::default()
        };
        config.profiles[0].target = "first-hub".into();
        Session {
            settings: SessionSettings::from_configuration(&config).unwrap(),
            saved_config: Some(config),
            settings_notice: None,
            connection_report: None,
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
