use ssh_clipboard_desktop_prototype::startup::{self, Registry, Status};
use tauri::AppHandle;

pub fn available() -> bool {
    cfg!(all(feature = "installed-desktop", not(debug_assertions)))
}

pub struct NativeRegistry<'a>(pub &'a AppHandle);

#[cfg(all(windows, feature = "installed-desktop"))]
fn quoted_executable(path: &std::path::Path) -> Result<String, String> {
    let text = path
        .to_str()
        .filter(|text| !text.contains(['"', '\0']))
        .ok_or("Invalid installed executable path.")?;
    if !path.is_absolute() {
        return Err("Installed executable path must be absolute.".into());
    }
    Ok(format!("\"{text}\""))
}

#[cfg(all(windows, feature = "installed-desktop"))]
fn windows_manager(app: &AppHandle) -> Result<auto_launch::AutoLaunch, String> {
    let executable = std::env::current_exe()
        .map_err(|_| "Could not resolve the installed executable.".to_string())?;
    // auto-launch 0.6 concatenates app_path and arguments verbatim. Tauri's
    // wrapper passes the path without quotes, breaking our spaced install path.
    Ok(auto_launch::AutoLaunch::new(
        app.config()
            .product_name
            .as_deref()
            .unwrap_or("SSH Clipboard"),
        &quoted_executable(&executable)?,
        auto_launch::WindowsEnableMode::CurrentUser,
        &["--background"],
    ))
}

impl Registry for NativeRegistry<'_> {
    fn enabled(&self) -> Result<bool, String> {
        if !available() {
            return Err("Start at login is available in installed release builds.".into());
        }
        #[cfg(all(windows, feature = "installed-desktop"))]
        {
            windows_manager(self.0)?
                .is_enabled()
                .map_err(|_| "Could not read this app's startup entry.".into())
        }
        #[cfg(all(not(windows), feature = "installed-desktop"))]
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
        #[cfg(all(windows, feature = "installed-desktop"))]
        {
            let manager = windows_manager(self.0)?;
            (if enabled {
                manager.enable()
            } else {
                manager.disable()
            })
            .map_err(|_| "Could not change this app's startup entry.".into())
        }
        #[cfg(all(not(windows), feature = "installed-desktop"))]
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

#[cfg(all(test, windows, feature = "installed-desktop"))]
mod tests {
    use super::*;

    #[link(name = "shell32")]
    unsafe extern "system" {
        fn CommandLineToArgvW(command: *const u16, count: *mut i32) -> *mut *mut u16;
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn LocalFree(memory: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    }

    fn windows_arguments(command: &str) -> Vec<String> {
        let wide: Vec<u16> = command.encode_utf16().chain(Some(0)).collect();
        let mut count = 0;
        // Windows owns this argv allocation until LocalFree below; each entry
        // is a NUL-terminated UTF-16 string within that allocation.
        let argv = unsafe { CommandLineToArgvW(wide.as_ptr(), &mut count) };
        assert!(!argv.is_null());
        let arguments = (0..count as usize)
            .map(|index| unsafe {
                let argument = *argv.add(index);
                let mut length = 0;
                while *argument.add(length) != 0 {
                    length += 1;
                }
                String::from_utf16_lossy(std::slice::from_raw_parts(argument, length))
            })
            .collect();
        unsafe { LocalFree(argv.cast()) };
        arguments
    }

    #[test]
    fn startup_command_keeps_spaced_unicode_executable_separate_from_arguments() {
        let path = std::path::Path::new(
            r"C:\Users\Johan Å\AppData\Local\SSH Clipboard\ssh_clipboard_desktop.exe",
        );
        let command = format!("{} --background", quoted_executable(path).unwrap());
        let expected = vec![path.to_str().unwrap().to_owned(), "--background".into()];
        assert_eq!(windows_arguments(&command), expected);
        // The dependency's previous unquoted command really does split the
        // executable into multiple arguments in the Windows parser.
        assert_ne!(
            windows_arguments(&format!("{} --background", path.display())),
            expected
        );
        assert!(quoted_executable(std::path::Path::new("relative.exe")).is_err());
        assert!(quoted_executable(std::path::Path::new("C:\\bad\"path.exe")).is_err());
    }
}
