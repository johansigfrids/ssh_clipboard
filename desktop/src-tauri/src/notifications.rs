use serde::Serialize;
use ssh_clipboard::configuration::NotificationPreference;
use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub available: bool,
    pub detail: &'static str,
}

pub fn status(_app: &AppHandle) -> Status {
    // Portable Windows builds lack a stable installed notification identity. Do not
    // attribute our messages to PowerShell, as development notification fallbacks do.
    if cfg!(all(
        target_os = "windows",
        any(debug_assertions, not(feature = "installed-desktop"))
    )) {
        return Status {
            available: false,
            detail: "Windows notifications need an installed app. This build shows results in the panel.",
        };
    }
    // The plugin's desktop permission API always returns Granted; it cannot verify OS
    // notification settings or actual delivery. Do not present it as a permission check.
    Status {
        available: true,
        detail: "Notifications use your system settings. Quiet modes or system permissions may hide them. Use Test notification to check.",
    }
}

pub fn test(app: &AppHandle) -> Result<Status, String> {
    if !status(app).available {
        return Err("Notifications require an installed app on this platform.".into());
    }
    app.notification()
        .builder()
        .title("SSH Clipboard")
        .body("Test notification. Clipboard content is never included.")
        .show()
        .map_err(|_| {
            "The notification could not be sent. Check your system notification settings."
                .to_string()
        })?;
    Ok(Status {
        available: true,
        detail: "Test notification submitted to the system. If it did not appear, check system notification settings and quiet modes.",
    })
}

#[derive(Debug, PartialEq, Eq)]
enum Delivery {
    Panel,
    Notify,
    Quiet,
}

fn delivery(preference: NotificationPreference, failed: bool, available: bool) -> Delivery {
    match preference {
        NotificationPreference::Panel => Delivery::Panel,
        NotificationPreference::All | NotificationPreference::Errors if failed => {
            if available {
                Delivery::Notify
            } else {
                Delivery::Panel
            }
        }
        NotificationPreference::All => {
            if available {
                Delivery::Notify
            } else {
                Delivery::Panel
            }
        }
        NotificationPreference::Errors | NotificationPreference::Off => Delivery::Quiet,
    }
}

/// Return whether to open the panel. Delivery failure never changes the transfer result.
pub fn present(
    app: &AppHandle,
    preference: NotificationPreference,
    failed: bool,
    sharing: bool,
) -> bool {
    match delivery(preference, failed, status(app).available) {
        Delivery::Panel => true,
        Delivery::Quiet => false,
        Delivery::Notify => {
            let body = match (failed, sharing) {
                (true, _) => "The clipboard action failed. Open SSH Clipboard for details.",
                (false, true) => "Current clipboard shared.",
                (false, false) => "Shared clipboard copied to this computer.",
            };
            app.notification()
                .builder()
                .title("SSH Clipboard")
                .body(body)
                .show()
                .is_err()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use NotificationPreference::*;

    #[test]
    fn preference_and_delivery_capability_have_explicit_fallbacks() {
        for failed in [false, true] {
            for available in [false, true] {
                assert_eq!(delivery(Panel, failed, available), Delivery::Panel);
                assert_eq!(delivery(Off, failed, available), Delivery::Quiet);
                assert_eq!(
                    delivery(All, failed, available),
                    if available {
                        Delivery::Notify
                    } else {
                        Delivery::Panel
                    }
                );
                assert_eq!(
                    delivery(Errors, failed, available),
                    if !failed {
                        Delivery::Quiet
                    } else if available {
                        Delivery::Notify
                    } else {
                        Delivery::Panel
                    }
                );
            }
        }
    }
}
