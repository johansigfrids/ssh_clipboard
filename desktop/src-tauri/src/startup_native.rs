use ssh_clipboard_desktop_prototype::startup::{self, Registry, Status};
use tauri::AppHandle;

pub fn available() -> bool {
    cfg!(all(feature = "installed-desktop", not(debug_assertions)))
}

pub struct NativeRegistry<'a>(pub &'a AppHandle);

impl Registry for NativeRegistry<'_> {
    fn enabled(&self) -> Result<bool, String> {
        if !available() {
            return Err("Start at login is available in installed release builds.".into());
        }
        #[cfg(feature = "installed-desktop")]
        {
            use tauri_plugin_autostart::ManagerExt;
            self.0
                .autolaunch()
                .is_enabled()
                .map_err(|_| "Could not read this app's startup entry.".into())
        }
        #[cfg(not(feature = "installed-desktop"))]
        {
            let _ = self.0;
            Err("Startup support is unavailable.".into())
        }
    }

    fn set_enabled(&self, enabled: bool) -> Result<(), String> {
        if !available() {
            return Err("Start at login is available in installed release builds.".into());
        }
        #[cfg(feature = "installed-desktop")]
        {
            use tauri_plugin_autostart::ManagerExt;
            let manager = self.0.autolaunch();
            (if enabled {
                manager.enable()
            } else {
                manager.disable()
            })
            .map_err(|_| "Could not change this app's startup entry.".into())
        }
        #[cfg(not(feature = "installed-desktop"))]
        {
            let _ = enabled;
            Err("Startup support is unavailable.".into())
        }
    }
}

pub fn status(app: &AppHandle, desired: bool) -> Status {
    startup::status(&NativeRegistry(app), available(), desired)
}
