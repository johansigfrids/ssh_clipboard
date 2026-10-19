use eyre::{Result, WrapErr, eyre};
use std::path::PathBuf;
use tokio::process::{Child, Command};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SshConfig {
    pub target: String,
    pub port: Option<u16>,
    pub user: Option<String>,
    pub host: Option<String>,
    pub identity_file: Option<PathBuf>,
    pub ssh_options: Vec<String>,
    pub ssh_bin: Option<PathBuf>,
}

impl SshConfig {
    pub fn resolve_target(&self) -> String {
        if !self.target.is_empty() {
            return self.target.clone();
        }
        match (&self.user, &self.host) {
            (Some(user), Some(host)) => format!("{user}@{host}"),
            (None, Some(host)) => host.clone(),
            _ => String::new(),
        }
    }
}

fn split_target_and_port(target: &str) -> (String, Option<u16>) {
    let target = target.trim();
    if target.is_empty() {
        return (String::new(), None);
    }

    let host_part = target.split('@').next_back().unwrap_or(target);
    let colon_count = host_part.chars().filter(|c| *c == ':').count();
    if colon_count != 1 {
        return (target.to_string(), None);
    }

    let Some(last_colon) = target.rfind(':') else {
        return (target.to_string(), None);
    };
    let port_str = &target[last_colon + 1..];
    if port_str.is_empty() || !port_str.chars().all(|c| c.is_ascii_digit()) {
        return (target.to_string(), None);
    }
    match port_str.parse::<u16>() {
        Ok(port) => (target[..last_colon].to_string(), Some(port)),
        Err(_) => (target.to_string(), None),
    }
}

pub fn resolve_target_and_port(config: &SshConfig) -> (String, Option<u16>) {
    let (target, target_port) = split_target_and_port(&config.resolve_target());
    let port = config.port.or(target_port);
    (target, port)
}

pub fn spawn_ssh_proxy(config: &SshConfig) -> Result<Child> {
    spawn_proxy(config, "proxy")
}

pub fn spawn_ssh_session_proxy(config: &SshConfig) -> Result<Child> {
    spawn_proxy(config, "session-proxy")
}

fn ssh_command(ssh_bin: PathBuf) -> Command {
    let mut command = Command::new(ssh_bin);
    command.kill_on_drop(true);
    // GUI callers have no console to inherit. Piped stdio alone does not stop
    // Windows from opening a console for the SSH child.
    #[cfg(target_os = "windows")]
    command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    command
}

fn spawn_proxy(config: &SshConfig, endpoint: &'static str) -> Result<Child> {
    let (target, port) = resolve_target_and_port(config);
    if target.trim().is_empty() {
        return Err(eyre!("missing SSH target (use --target or --host)"));
    }

    let ssh_bin = config
        .ssh_bin
        .clone()
        .unwrap_or_else(|| PathBuf::from("ssh"));

    let mut command = ssh_command(ssh_bin);
    command.arg("-T");

    if let Some(port) = port {
        command.arg("-p").arg(port.to_string());
    }

    if let Some(identity_file) = &config.identity_file {
        command.arg("-i").arg(identity_file);
    }

    for opt in &config.ssh_options {
        command.arg("-o").arg(opt);
    }

    command.arg(target);
    command.arg("ssh_clipboard");
    command.arg(endpoint);

    command
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .wrap_err("failed to spawn ssh")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_config(target: &str) -> SshConfig {
        SshConfig {
            target: target.to_string(),
            port: None,
            user: None,
            host: None,
            identity_file: None,
            ssh_options: Vec::new(),
            ssh_bin: None,
        }
    }

    #[cfg(target_os = "windows")]
    #[tokio::test]
    async fn ssh_child_has_no_console_and_preserves_piped_stdio() {
        use std::process::Stdio;
        use std::time::Duration;
        use tokio::io::AsyncWriteExt;

        const CONSOLE_PARENT: &str = "SSH_CLIPBOARD_TEST_CONSOLE_PARENT";
        if std::env::var_os(CONSOLE_PARENT).is_none() {
            // Give the isolated test parent a known console. Some test runners
            // already suppress consoles, which can mask a missing launch flag.
            let output = tokio::time::timeout(
                Duration::from_secs(30),
                Command::new(std::env::current_exe().unwrap())
                    .args([
                        "--exact",
                        "client::ssh::tests::ssh_child_has_no_console_and_preserves_piped_stdio",
                        "--nocapture",
                    ])
                    .env(CONSOLE_PARENT, "1")
                    .creation_flags(0x0000_0010) // CREATE_NEW_CONSOLE
                    .kill_on_drop(true)
                    .output(),
            )
            .await
            .expect("console parent timed out")
            .unwrap();
            assert!(
                output.status.success(),
                "console parent failed: {}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }

        // Use a local console executable instead of SSH or a live connection.
        // Query the actual child console, rather than inspecting launch flags.
        let mut command = ssh_command("powershell.exe".into());
        command
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                r#"
                Add-Type -Name NativeConsole -Namespace SshClipboardTest -MemberDefinition '
                    [System.Runtime.InteropServices.DllImport("kernel32.dll")]
                    public static extern System.IntPtr GetConsoleWindow();
                '
                if ([SshClipboardTest.NativeConsole]::GetConsoleWindow() -ne [IntPtr]::Zero) {
                    [Console]::Error.Write('unexpected console');
                    exit 1;
                }
                [Console]::Out.Write([Console]::In.ReadToEnd());
                [Console]::Error.Write('diagnostic');
                "#,
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn().unwrap();
        let payload = b"synthetic clipboard transport\r\n";
        child
            .stdin
            .take()
            .unwrap()
            .write_all(payload)
            .await
            .unwrap();
        let output = tokio::time::timeout(Duration::from_secs(15), child.wait_with_output())
            .await
            .expect("console probe timed out")
            .unwrap();
        assert!(
            output.status.success(),
            "console probe failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, payload);
        assert_eq!(output.stderr, b"diagnostic");
    }

    #[test]
    fn resolve_target_and_port_parses_inline_port() {
        let config = base_config("user@example.com:2222");
        let (target, port) = resolve_target_and_port(&config);
        assert_eq!(target, "user@example.com");
        assert_eq!(port, Some(2222));
    }

    #[test]
    fn resolve_target_and_port_keeps_ipv6_like_target_untouched() {
        let config = base_config("user@[2001:db8::1]");
        let (target, port) = resolve_target_and_port(&config);
        assert_eq!(target, "user@[2001:db8::1]");
        assert_eq!(port, None);
    }

    #[test]
    fn resolve_target_and_port_prefers_explicit_port_flag() {
        let mut config = base_config("user@example.com:2222");
        config.port = Some(2200);
        let (target, port) = resolve_target_and_port(&config);
        assert_eq!(target, "user@example.com");
        assert_eq!(port, Some(2200));
    }
}
