pub fn notify(summary: &str, body: &str) {
    if try_notify(summary, body).is_err() {
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

    #[cfg(target_os = "macos")]
    {
        notify_rust::Notification::new()
            .summary(summary)
            .body(body)
            .show()
            .map_err(|err| err.to_string())?;
        return Ok(());
    }

    #[allow(unreachable_code)]
    Err("unsupported platform".to_string())
}
