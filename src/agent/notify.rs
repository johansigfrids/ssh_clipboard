pub fn notify(summary: &str, body: &str) {
    if let Err(err) = try_notify(summary, body) {
        tracing::warn!("notification delivery failed: {err}");
        eprintln!("{summary}: {body}");
    }
}

fn try_notify(summary: &str, body: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use winrt_notification::{Duration, Toast};
        Toast::new(Toast::POWERSHELL_APP_ID)
            .title(summary)
            .text1(body)
            .duration(Duration::Short)
            .show()
            .map_err(|err| err.to_string())?;
        return Ok(());
    }

    #[cfg(target_os = "linux")]
    {
        notify_rust::Notification::new()
            .summary(summary)
            .body(body)
            .show()
            .map_err(|err| err.to_string())?;
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        use std::process::Command;

        let script = format!(
            "display notification {} with title {}",
            apple_script_string(body),
            apple_script_string(summary)
        );
        let output = Command::new("osascript")
            .arg("-e")
            .arg(script)
            .output()
            .map_err(|err| format!("failed to run osascript: {err}"))?;
        if output.status.success() {
            return Ok(());
        }

        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("osascript failed: {}", stderr.trim()));
    }

    #[allow(unreachable_code)]
    Err("unsupported platform".to_string())
}

#[cfg(target_os = "macos")]
fn apple_script_string(value: &str) -> String {
    let escaped = value
        .replace('\\', "\\\\")
        .replace('\"', "\\\"")
        .replace('\n', "\\n");
    format!("\"{escaped}\"")
}
