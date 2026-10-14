use crate::cli::DoctorArgs;
use crate::client::diagnostics::{CheckStatus as DiagnosticStatus, diagnose};
use crate::client::ssh::{SshConfig, resolve_target_and_port};
use crate::client::transport::ClientConfig;
use crate::protocol::DEFAULT_MAX_SIZE;
use eyre::Result;

const DEFAULT_RESYNC_MAX_BYTES: usize = 8192;

#[derive(Clone, Copy, PartialEq, Eq)]
enum CheckStatus {
    Ok,
    Warn,
    Fail,
}

impl CheckStatus {
    fn label(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Warn => "warn",
            Self::Fail => "fail",
        }
    }
}

struct CheckOutcome {
    name: &'static str,
    status: CheckStatus,
    detail: String,
    hint: Option<String>,
}

impl CheckOutcome {
    fn ok(name: &'static str, detail: impl Into<String>) -> Self {
        Self {
            name,
            status: CheckStatus::Ok,
            detail: detail.into(),
            hint: None,
        }
    }

    fn warn(name: &'static str, detail: impl Into<String>, hint: impl Into<String>) -> Self {
        Self {
            name,
            status: CheckStatus::Warn,
            detail: detail.into(),
            hint: Some(hint.into()),
        }
    }

    fn fail(name: &'static str, detail: impl Into<String>, hint: impl Into<String>) -> Self {
        Self {
            name,
            status: CheckStatus::Fail,
            detail: detail.into(),
            hint: Some(hint.into()),
        }
    }
}

#[cfg(target_os = "linux")]
#[derive(Clone, Copy, PartialEq, Eq)]
enum LinuxSession {
    Wayland,
    X11,
    Unknown,
}

pub async fn run(args: DoctorArgs) -> Result<()> {
    let explicit_destination = args.target.is_some() || args.host.is_some();
    let mut ssh = SshConfig {
        target: args.target.unwrap_or_default(),
        port: args.port,
        user: args.user,
        host: args.host,
        identity_file: args.identity_file,
        ssh_options: args.ssh_option,
        ssh_bin: args.ssh_bin,
    };
    let defaults = crate::cli::settings::apply_connection_defaults(
        &mut ssh,
        args.settings_file.as_deref(),
        args.profile.as_deref(),
    )?;
    let timeout_ms = args
        .timeout_ms
        .unwrap_or_else(|| defaults.as_ref().map_or(7000, |s| s.timeout_ms))
        .max(1);
    let (target, port) = resolve_target_and_port(&ssh);
    let mut checks = Vec::new();
    if let Some(check) = maybe_linux_hotkey_check() {
        checks.push(check);
    }
    if target.trim().is_empty() {
        checks.push(CheckOutcome::fail(
            "target",
            "missing SSH target",
            "pass --target user@server or run ssh_clipboard settings set --target ...",
        ));
    } else {
        let source = if defaults.is_some() && !explicit_destination {
            "shared profile"
        } else {
            "CLI args"
        };
        let detail = match port {
            Some(port) => format!("using {target} on port {port} ({source})"),
            None => format!("using {target} ({source})"),
        };
        checks.push(CheckOutcome::ok("target", detail));
        let config = ClientConfig {
            ssh,
            timeout_ms,
            max_size: defaults.as_ref().map_or(DEFAULT_MAX_SIZE, |s| s.max_size),
            resync_frames: defaults.as_ref().is_none_or(|s| s.resync_frames),
            resync_max_bytes: defaults
                .as_ref()
                .map_or(DEFAULT_RESYNC_MAX_BYTES, |s| s.resync_max_bytes),
        };
        let report = diagnose(&config).await;
        for check in report.checks {
            checks.push(match check.status {
                DiagnosticStatus::Passed => CheckOutcome::ok(check.label, check.detail),
                DiagnosticStatus::Failed => CheckOutcome::fail(
                    check.label,
                    check.detail,
                    report.hint.unwrap_or("Check the server setup and retry."),
                ),
                DiagnosticStatus::NotChecked => CheckOutcome::warn(
                    check.label,
                    check.detail,
                    "Resolve the failed check and retry.",
                ),
            });
        }
    }
    print_report(&checks);
    let fail_count = checks
        .iter()
        .filter(|c| c.status == CheckStatus::Fail)
        .count();
    if fail_count > 0 {
        return crate::cli::exit::exit_with_code(
            2,
            &format!("doctor found {fail_count} failing check(s)"),
        );
    }
    Ok(())
}
#[cfg(target_os = "linux")]
fn maybe_linux_hotkey_check() -> Option<CheckOutcome> {
    match detect_linux_session() {
        LinuxSession::Wayland => Some(CheckOutcome::warn(
            "local hotkeys",
            "Wayland session detected; global hotkeys are auto-disabled",
            "use tray actions, or run under X11 to enable global hotkeys",
        )),
        LinuxSession::X11 => Some(CheckOutcome::ok(
            "local hotkeys",
            "X11 session detected; global hotkeys are available",
        )),
        LinuxSession::Unknown => Some(CheckOutcome::warn(
            "local hotkeys",
            "could not determine local display session type",
            "set XDG_SESSION_TYPE/WAYLAND_DISPLAY/DISPLAY, or run under X11 for global hotkeys",
        )),
    }
}

#[cfg(not(target_os = "linux"))]
fn maybe_linux_hotkey_check() -> Option<CheckOutcome> {
    None
}

#[cfg(target_os = "linux")]
fn detect_linux_session() -> LinuxSession {
    detect_linux_session_from_env(|name| std::env::var(name).ok())
}

#[cfg(target_os = "linux")]
fn detect_linux_session_from_env<F>(mut get_var: F) -> LinuxSession
where
    F: FnMut(&str) -> Option<String>,
{
    if let Some(session_type) = get_var("XDG_SESSION_TYPE") {
        let session_type = session_type.trim().to_ascii_lowercase();
        if session_type == "wayland" {
            return LinuxSession::Wayland;
        }
        if session_type == "x11" {
            return LinuxSession::X11;
        }
    }

    if get_var("WAYLAND_DISPLAY").is_some_and(|value| !value.trim().is_empty()) {
        return LinuxSession::Wayland;
    }

    if get_var("DISPLAY").is_some_and(|value| !value.trim().is_empty()) {
        return LinuxSession::X11;
    }

    LinuxSession::Unknown
}

fn print_report(checks: &[CheckOutcome]) {
    println!("ssh_clipboard doctor");
    for check in checks {
        println!(
            "[{}] {}: {}",
            check.status.label(),
            check.name,
            check.detail
        );
        if let Some(hint) = &check.hint {
            println!("      hint: {hint}");
        }
    }
    let ok = checks
        .iter()
        .filter(|check| check.status == CheckStatus::Ok)
        .count();
    let warn = checks
        .iter()
        .filter(|check| check.status == CheckStatus::Warn)
        .count();
    let fail = checks
        .iter()
        .filter(|check| check.status == CheckStatus::Fail)
        .count();
    println!("summary: {ok} ok, {warn} warning(s), {fail} failure(s)");
}
