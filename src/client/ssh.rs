use eyre::{Result, eyre};
use std::path::PathBuf;
use tokio::process::{Child, Command};

#[derive(Debug, Clone)]
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

pub fn spawn_ssh_proxy(config: &SshConfig) -> Result<Child> {
    let (target, target_port) = split_target_and_port(&config.resolve_target());
    if target.trim().is_empty() {
        return Err(eyre!("missing SSH target (use --target or --host)"));
    }

    let ssh_bin = config
        .ssh_bin
        .clone()
        .unwrap_or_else(|| PathBuf::from("ssh"));

    let mut command = Command::new(ssh_bin);
    command.kill_on_drop(true);
    command.arg("-T");

    let port = config.port.or(target_port);
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
    command.arg("proxy");

    command
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|err| eyre!("failed to spawn ssh: {err}"))
}
