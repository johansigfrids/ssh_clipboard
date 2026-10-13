use crate::clipboard::ClipboardWorker;
use crate::model::{ItemPreview, SessionSettings, is_wayland, validate_transfer_size};
use serde::Serialize;
use ssh_clipboard::client::transport::{make_request, send_request};
use ssh_clipboard::configuration::{ConfigStore, Configuration};
use ssh_clipboard::protocol::{ClipboardValue, ErrorCode, RequestKind, ResponseKind};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use tokio::sync::Mutex;

struct Session {
    settings: SessionSettings,
    item: Option<ClipboardValue>,
    clipboard_checked: bool,
    shortcut_active: Option<String>,
    shortcut_status: String,
    tray_status: String,
    connected: bool,
    saved_config: Option<Configuration>,
    settings_notice: Option<String>,
}

struct Desktop {
    session: Mutex<Session>,
    clipboard: ClipboardWorker,
    config_store: Option<ConfigStore>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PanelView {
    settings: SessionSettings,
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
}

impl Session {
    fn view(&self) -> Result<PanelView, String> {
        Ok(PanelView {
            settings: self.settings.clone(),
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
            active_shortcut: self.shortcut_active.clone(),
        })
    }
}

#[tauri::command]
async fn get_panel(state: State<'_, Desktop>) -> Result<PanelView, String> {
    state.session.lock().await.view()
}

fn configure_shortcut(
    app: &AppHandle,
    session: &mut Session,
    next: &SessionSettings,
) -> Result<String, String> {
    if next.shortcut_enabled && is_wayland() {
        // Preserve the saved preference across platforms, while allowing other settings to be edited.
        return Ok("Wayland: use the launcher or tray menu. Portal support is pending.".into());
    }
    let requested = next.shortcut_enabled.then_some(next.open_shortcut.clone());
    if requested == session.shortcut_active {
        return Ok(if requested.is_some() {
            "Registered"
        } else {
            "Disabled"
        }
        .into());
    }
    if let Some(binding) = &requested {
        let shortcut: Shortcut = binding
            .parse()
            .map_err(|_| "Choose a valid shortcut with a modifier and a key.".to_string())?;
        if shortcut.mods.is_empty() {
            return Err("Choose a shortcut with Ctrl/Cmd, Alt, or Shift and a key.".into());
        }
        app.global_shortcut().register(binding.as_str())
            .map_err(|_| "That shortcut could not be registered. It may be in use; choose another or disable it.".to_string())?;
    }
    if let Some(previous) = &session.shortcut_active
        && app.global_shortcut().unregister(previous.as_str()).is_err()
    {
        if let Some(binding) = &requested {
            let _ = app.global_shortcut().unregister(binding.as_str());
        }
        return Err(
            "Could not replace the existing shortcut. Previous settings remain active.".into(),
        );
    }
    session.shortcut_active = requested;
    Ok(if session.shortcut_active.is_some() {
        "Registered"
    } else {
        "Disabled"
    }
    .into())
}

#[tauri::command]
async fn apply_settings(
    app: AppHandle,
    state: State<'_, Desktop>,
    mut settings: SessionSettings,
) -> Result<PanelView, String> {
    settings.target = settings.target.trim().into();
    settings.validate()?;
    let mut session = state
        .session
        .try_lock()
        .map_err(|_| "Wait for the current operation to finish.".to_string())?;
    if let Some(notice) = &session.settings_notice {
        return Err(notice.clone());
    }
    let store = state
        .config_store
        .as_ref()
        .ok_or("The settings location is unavailable.")?;
    let mut next_config = session.saved_config.clone().unwrap_or_default();
    settings.update_configuration(&mut next_config)?;
    // Validate/check for a stale writer and flush the temporary file before changing OS state.
    let pending = store
        .prepare(session.saved_config.as_ref(), next_config)
        .map_err(|e| e.to_string())?;
    let previous_settings = session.settings.clone();
    let status = configure_shortcut(&app, &mut session, &settings)?;
    let saved = match pending.commit() {
        Ok(saved) => saved,
        Err(error) => {
            session.shortcut_status = match configure_shortcut(&app, &mut session, &previous_settings) {
                Ok(status) => status,
                Err(_) => "Could not restore the previous shortcut. Quit and reopen to restore saved settings.".into(),
            };
            return Err(error.to_string());
        }
    };
    let connection_changed = settings.target != session.settings.target
        || settings.port != session.settings.port
        || settings.identity_file != session.settings.identity_file;
    if connection_changed {
        session.item = None;
        session.clipboard_checked = false;
        session.connected = false;
    }
    session.settings = settings;
    session.saved_config = Some(saved);
    session.shortcut_status = status;
    session.view()
}

async fn request(session: &mut Session, kind: RequestKind) -> Result<ResponseKind, String> {
    session.settings.validate_connection()?;
    let request = make_request(kind);
    validate_transfer_size(&request)?;
    let request_id = request.request_id;
    match send_request(&session.settings.client_config(), request).await {
        Ok(response) => {
            if response.request_id != request_id {
                session.connected = false;
                return Err("The server reply did not match this request.".into());
            }
            session.connected = true;
            match response.kind {
                ResponseKind::Error { code: ErrorCode::PayloadTooLarge, .. } => Err(
                    "The server rejected this item because it exceeds its transfer limit. Copy a smaller image or less text and try again.".into()),
                ResponseKind::Error { message, .. } => Err(message),
                kind => Ok(kind),
            }
        }
        Err(_) => {
            session.connected = false;
            // Do not leak clipboard bytes via an unexpected response/debug report.
            Err("Could not reach the clipboard server. Check the SSH alias, unlocked key, trusted host, and running daemon. Use the CLI doctor for detailed diagnostics.".into())
        }
    }
}

#[tauri::command]
async fn test_connection(state: State<'_, Desktop>) -> Result<PanelView, String> {
    let mut session = state
        .session
        .try_lock()
        .map_err(|_| "Another operation is running.".to_string())?;
    match request(&mut session, RequestKind::PeekMeta).await? {
        ResponseKind::Meta { .. } | ResponseKind::Empty => session.view(),
        _ => Err("Unexpected server response to connection test.".into()),
    }
}

#[tauri::command]
async fn refresh_item(state: State<'_, Desktop>) -> Result<PanelView, String> {
    let mut session = state
        .session
        .try_lock()
        .map_err(|_| "Another operation is running.".to_string())?;
    match request(&mut session, RequestKind::Get).await? {
        ResponseKind::Value { value } => {
            ItemPreview::from_value(&value, session.settings.show_preview)?;
            session.item = Some(value);
        }
        ResponseKind::Empty => session.item = None,
        _ => return Err("Unexpected server response to refresh.".into()),
    }
    session.clipboard_checked = true;
    session.view()
}

#[tauri::command]
async fn share_clipboard(state: State<'_, Desktop>) -> Result<PanelView, String> {
    let mut session = state
        .session
        .try_lock()
        .map_err(|_| "Another operation is running.".to_string())?;
    session.settings.validate_connection()?;
    let value = state.clipboard.read().await?;
    ItemPreview::from_value(&value, session.settings.show_preview)?;
    match request(
        &mut session,
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
    session.view()
}

#[tauri::command]
async fn copy_item(app: AppHandle, state: State<'_, Desktop>) -> Result<PanelView, String> {
    let session = state
        .session
        .try_lock()
        .map_err(|_| "Another operation is running.".to_string())?;
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
                .with_handler(|app, _, event| {
                    if event.state() == ShortcutState::Pressed {
                        show_panel(app, false);
                    }
                })
                .build(),
        );
    }
    let app = builder
        .setup(|app| {
            let (config_store, loaded, settings_notice) = match ConfigStore::user() {
                Ok(store) => match store.load() {
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
                shortcut_active: None,
                shortcut_status: "Disabled".into(),
                tray_status: "Available".into(),
                connected: false,
                saved_config: loaded,
                settings_notice,
            };
            session.shortcut_status = if is_wayland() {
                "Wayland: use the launcher or tray menu. Portal support is pending.".into()
            } else {
                configure_shortcut(app.handle(), &mut session, &settings)
                    .unwrap_or_else(|message| message)
            };
            app.manage(Desktop {
                session: Mutex::new(session),
                clipboard: ClipboardWorker::start()?,
                config_store,
            });
            if install_tray(app.handle()).is_err() {
                app.state::<Desktop>().session.blocking_lock().tray_status =
                    "Unavailable: reopen through the app launcher.".into();
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
            apply_settings,
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
