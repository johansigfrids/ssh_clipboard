use crate::client::ssh::SshConfig;
use crate::client::transport::{ClientConfig, make_request, send_request};
use crate::protocol::{RequestKind, ResponseKind};
use eyre::{Result, WrapErr, eyre};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub config_version: u32,
    pub target: String,
    pub port: Option<u16>,
    pub identity_file: Option<PathBuf>,
    pub ssh_options: Vec<String>,
    pub max_size: usize,
    pub timeout_ms: u64,
    pub hotkeys: HotkeyConfig,
    pub autostart_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeyConfig {
    pub push: String,
    pub pull: String,
}

#[derive(Debug, Clone, Copy)]
pub enum PlatformDefaults {
    Windows,
    Macos,
    Linux,
}

pub fn platform_defaults() -> PlatformDefaults {
    #[cfg(target_os = "macos")]
    {
        return PlatformDefaults::Macos;
    }
    #[cfg(target_os = "linux")]
    {
        return PlatformDefaults::Linux;
    }
    PlatformDefaults::Windows
}

pub fn default_agent_config() -> AgentConfig {
    let (push, pull) = match platform_defaults() {
        PlatformDefaults::Macos => (
            "CmdOrCtrl+Shift+KeyC".to_string(),
            "CmdOrCtrl+Shift+KeyV".to_string(),
        ),
        PlatformDefaults::Windows => (
            "CmdOrCtrl+Shift+KeyC".to_string(),
            "CmdOrCtrl+Shift+KeyV".to_string(),
        ),
        PlatformDefaults::Linux => (
            "Ctrl+Shift+KeyC".to_string(),
            "Ctrl+Shift+KeyV".to_string(),
        ),
    };

    AgentConfig {
        config_version: 1,
        target: String::new(),
        port: None,
        identity_file: None,
        ssh_options: Vec::new(),
        max_size: crate::protocol::DEFAULT_MAX_SIZE,
        timeout_ms: 7000,
        hotkeys: HotkeyConfig { push, pull },
        autostart_enabled: false,
    }
}

impl Default for AgentConfig {
    fn default() -> Self {
        default_agent_config()
    }
}

pub fn load_config() -> Result<AgentConfig> {
    let config: AgentConfig =
        confy::load("ssh_clipboard", Some("agent")).wrap_err("failed to load agent config")?;
    Ok(config)
}

pub fn store_config(config: &AgentConfig) -> Result<()> {
    confy::store("ssh_clipboard", Some("agent"), config)
        .wrap_err("failed to store agent config")?;
    Ok(())
}

pub fn config_path() -> Result<PathBuf> {
    let path = confy::get_configuration_file_path("ssh_clipboard", Some("agent"))?;
    Ok(path)
}

pub fn validate_config(config: &AgentConfig) -> Result<()> {
    if config.target.trim().is_empty() {
        return Err(eyre!("missing target; set config.target (user@host)"));
    }
    if config.max_size == 0 {
        return Err(eyre!("max_size must be > 0"));
    }
    if config.timeout_ms == 0 {
        return Err(eyre!("timeout_ms must be > 0"));
    }
    crate::agent::hotkey::parse_hotkey(&config.hotkeys.push)
        .wrap_err("invalid push hotkey binding")?;
    crate::agent::hotkey::parse_hotkey(&config.hotkeys.pull)
        .wrap_err("invalid pull hotkey binding")?;
    Ok(())
}

pub fn client_config_from_agent(config: &AgentConfig) -> ClientConfig {
    ClientConfig {
        ssh: SshConfig {
            target: config.target.clone(),
            port: config.port,
            user: None,
            host: None,
            identity_file: config.identity_file.clone(),
            ssh_options: config.ssh_options.clone(),
            ssh_bin: None,
        },
        max_size: config.max_size,
        timeout_ms: config.timeout_ms,
    }
}

pub async fn agent_push(config: &AgentConfig) -> Result<()> {
    let value = crate::client_actions::build_clipboard_value(false, config.max_size)
        .await
        .map_err(|err| eyre!(err.message))?;
    let response = send_request(
        &client_config_from_agent(config),
        make_request(RequestKind::Set { value }),
    )
    .await?;
    match response.kind {
        ResponseKind::Ok => Ok(()),
        ResponseKind::Error { code: _, message } => Err(eyre!(message)),
        other => Err(eyre!("unexpected response: {other:?}")),
    }
}

pub async fn agent_pull(config: &AgentConfig) -> Result<()> {
    let response = send_request(
        &client_config_from_agent(config),
        make_request(RequestKind::Get),
    )
    .await?;
    crate::client_actions::apply_pull_response_to_clipboard(response, config.max_size)
        .wrap_err("pull failed")?;
    Ok(())
}

pub async fn agent_peek(config: &AgentConfig) -> Result<String> {
    let response = send_request(
        &client_config_from_agent(config),
        make_request(RequestKind::PeekMeta),
    )
    .await?;
    match response.kind {
        ResponseKind::Meta {
            content_type,
            size,
            created_at,
        } => Ok(format!(
            "content_type={content_type} size={size} created_at={created_at}"
        )),
        ResponseKind::Empty => Ok("no clipboard value set".to_string()),
        ResponseKind::Error { code: _, message } => Err(eyre!(message)),
        other => Err(eyre!("unexpected response: {other:?}")),
    }
}

pub mod autostart;
pub mod hotkey;
pub mod notify;
pub mod run;

pub use hotkey::parse_hotkey;
