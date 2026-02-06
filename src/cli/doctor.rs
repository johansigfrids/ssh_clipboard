use crate::cli::DoctorArgs;
use crate::client::ssh::{SshConfig, resolve_target_and_port};
use crate::client::transport::{ClientConfig, make_request, send_request};
use crate::protocol::{DEFAULT_MAX_SIZE, ErrorCode, RequestKind, ResponseKind};
use eyre::Result;
use std::path::PathBuf;
use tokio::process::Command;
use tokio::time::{Duration, timeout};

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

struct AgentConfigInfo {
    used_for_target: bool,
    load_error: Option<String>,
}

pub async fn run(args: DoctorArgs) -> Result<()> {
    let timeout_ms = args.timeout_ms.max(1);
    let mut ssh = SshConfig {
        target: args.target.unwrap_or_default(),
        port: args.port,
        user: args.user,
        host: args.host,
        identity_file: args.identity_file,
        ssh_options: args.ssh_option,
        ssh_bin: args.ssh_bin,
    };
    let agent_info = maybe_apply_agent_config_defaults(&mut ssh);

    let (target, port) = resolve_target_and_port(&ssh);
    let mut checks = Vec::new();

    if let Some(err) = agent_info.load_error {
        checks.push(CheckOutcome::warn(
            "agent config",
            format!("could not load saved agent config: {err}"),
            "if you rely on saved defaults, run `ssh_clipboard config show` to inspect/fix it",
        ));
    }

    match run_local_ssh_version(&ssh, timeout_ms).await {
        Ok(_) => checks.push(CheckOutcome::ok("ssh binary", "found and executable")),
        Err(err) => checks.push(CheckOutcome::fail(
            "ssh binary",
            err.to_string(),
            "install OpenSSH client or pass `--ssh-bin <path>`",
        )),
    }

    if target.trim().is_empty() {
        checks.push(CheckOutcome::fail(
            "target",
            "missing SSH target",
            "pass `--target user@server` (or `--host`/`--user`), or run `ssh_clipboard setup-agent --target ...`",
        ));
    } else {
        let source = if agent_info.used_for_target {
            "agent config"
        } else {
            "CLI args"
        };
        let detail = match port {
            Some(port) => format!("using `{target}` on port {port} ({source})"),
            None => format!("using `{target}` ({source})"),
        };
        checks.push(CheckOutcome::ok("target", detail));
    }

    let mut auth_ok = false;
    let mut proxy_ok = false;
    let has_hard_fail = checks.iter().any(|c| c.status == CheckStatus::Fail);

    if !has_hard_fail {
        match run_ssh_probe(&ssh, timeout_ms, &["true"]).await {
            Ok(_) => {
                auth_ok = true;
                checks.push(CheckOutcome::ok(
                    "ssh auth",
                    "non-interactive SSH command succeeded",
                ));
            }
            Err(err) => checks.push(CheckOutcome::fail(
                "ssh auth",
                err.to_string(),
                format!("try `ssh -T {target} true` and fix keys/known_hosts/auth"),
            )),
        }
    }

    if auth_ok {
        match run_ssh_probe(&ssh, timeout_ms, &["ssh_clipboard", "proxy", "--help"]).await {
            Ok(_) => {
                proxy_ok = true;
                checks.push(CheckOutcome::ok(
                    "remote proxy command",
                    "`ssh_clipboard proxy` is runnable on the server",
                ));
            }
            Err(err) => checks.push(CheckOutcome::fail(
                "remote proxy command",
                err.to_string(),
                format!("ensure `{}` is on PATH for SSH sessions", "ssh_clipboard"),
            )),
        }
    }

    if proxy_ok {
        let client_config = ClientConfig {
            ssh: ssh.clone(),
            max_size: DEFAULT_MAX_SIZE,
            timeout_ms,
            resync_frames: true,
            resync_max_bytes: DEFAULT_RESYNC_MAX_BYTES,
        };
        match send_request(&client_config, make_request(RequestKind::PeekMeta)).await {
            Ok(response) => match response.kind {
                ResponseKind::Meta { .. } | ResponseKind::Empty => {
                    checks.push(CheckOutcome::ok(
                        "protocol roundtrip",
                        "framing/protocol exchange with proxy succeeded",
                    ));
                }
                ResponseKind::Error {
                    code: ErrorCode::DaemonNotRunning,
                    message,
                } => {
                    checks.push(CheckOutcome::warn(
                        "protocol roundtrip",
                        format!("proxy reachable but daemon is not running: {message}"),
                        "start the server daemon (`./ssh_clipboard install-daemon` on the server) and retry",
                    ));
                }
                ResponseKind::Error { message, .. } => {
                    checks.push(CheckOutcome::fail(
                        "protocol roundtrip",
                        format!("proxy returned protocol error: {message}"),
                        "verify client/server versions and server setup",
                    ));
                }
                other => checks.push(CheckOutcome::fail(
                    "protocol roundtrip",
                    format!("unexpected response: {other:?}"),
                    "verify server/proxy binaries are up to date",
                )),
            },
            Err(err) => checks.push(CheckOutcome::fail(
                "protocol roundtrip",
                err.to_string(),
                "verify SSH, proxy, and daemon setup; then retry",
            )),
        }
    }

    print_report(&checks);

    let fail_count = checks
        .iter()
        .filter(|check| check.status == CheckStatus::Fail)
        .count();
    if fail_count > 0 {
        return crate::cli::exit::exit_with_code(
            2,
            &format!("doctor found {fail_count} failing check(s)"),
        );
    }
    Ok(())
}

async fn run_local_ssh_version(config: &SshConfig, timeout_ms: u64) -> Result<()> {
    let ssh_bin = config
        .ssh_bin
        .clone()
        .unwrap_or_else(|| PathBuf::from("ssh"));
    let mut cmd = Command::new(ssh_bin);
    cmd.arg("-V");
    let output = timeout(Duration::from_millis(timeout_ms), cmd.output())
        .await
        .map_err(|_| eyre::eyre!("timed out after {timeout_ms}ms while running `ssh -V`"))?
        .map_err(|err| eyre::eyre!("failed to run ssh: {err}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(eyre::eyre!(
            "`ssh -V` exited with {}: {}",
            output.status,
            summarize_output(&output.stdout, &output.stderr)
        ))
    }
}

async fn run_ssh_probe(config: &SshConfig, timeout_ms: u64, remote_args: &[&str]) -> Result<()> {
    let (target, port) = resolve_target_and_port(config);
    if target.trim().is_empty() {
        return Err(eyre::eyre!("missing target"));
    }

    let ssh_bin = config
        .ssh_bin
        .clone()
        .unwrap_or_else(|| PathBuf::from("ssh"));
    let mut cmd = Command::new(ssh_bin);
    cmd.arg("-T");
    if let Some(port) = port {
        cmd.arg("-p").arg(port.to_string());
    }
    if let Some(identity_file) = &config.identity_file {
        cmd.arg("-i").arg(identity_file);
    }
    for opt in &config.ssh_options {
        cmd.arg("-o").arg(opt);
    }
    cmd.arg(target);
    for arg in remote_args {
        cmd.arg(arg);
    }

    let output = timeout(Duration::from_millis(timeout_ms), cmd.output())
        .await
        .map_err(|_| eyre::eyre!("timed out after {timeout_ms}ms"))?
        .map_err(|err| eyre::eyre!("failed to run ssh: {err}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(eyre::eyre!(
            "ssh exited with {}: {}",
            output.status,
            summarize_output(&output.stdout, &output.stderr)
        ))
    }
}

fn summarize_output(stdout: &[u8], stderr: &[u8]) -> String {
    let stdout = String::from_utf8_lossy(stdout);
    let stderr = String::from_utf8_lossy(stderr);
    let mut combined = if stderr.trim().is_empty() {
        stdout.trim().to_string()
    } else if stdout.trim().is_empty() {
        stderr.trim().to_string()
    } else {
        format!("{} | {}", stderr.trim(), stdout.trim())
    };
    if combined.is_empty() {
        combined = "no output".to_string();
    }
    let max = 240usize;
    if combined.len() > max {
        combined.truncate(max);
        combined.push_str("...");
    }
    combined
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

#[cfg(all(
    feature = "agent",
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
fn maybe_apply_agent_config_defaults(ssh: &mut SshConfig) -> AgentConfigInfo {
    let mut info = AgentConfigInfo {
        used_for_target: false,
        load_error: None,
    };
    let want_target_from_config = resolve_target_and_port(ssh).0.trim().is_empty();

    match crate::agent::load_config() {
        Ok(config) => {
            if want_target_from_config && !config.target.trim().is_empty() {
                ssh.target = config.target.clone();
                info.used_for_target = true;
            }
            if ssh.port.is_none() {
                ssh.port = config.port;
            }
            if ssh.identity_file.is_none() {
                ssh.identity_file = config.identity_file.clone();
            }
            if ssh.ssh_options.is_empty() {
                ssh.ssh_options = config.ssh_options.clone();
            }
        }
        Err(err) => {
            info.load_error = Some(err.to_string());
        }
    }
    info
}

#[cfg(not(all(
    feature = "agent",
    any(target_os = "windows", target_os = "macos", target_os = "linux")
)))]
fn maybe_apply_agent_config_defaults(_ssh: &mut SshConfig) -> AgentConfigInfo {
    AgentConfigInfo {
        used_for_target: false,
        load_error: None,
    }
}
