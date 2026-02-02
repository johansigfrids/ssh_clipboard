use eyre::{Result, WrapErr};

use crate::cli::{
    AgentArgs, AutostartArgs, AutostartCommands, ConfigArgs, ConfigCommands, ConfigSetArgs,
};

pub fn run_agent(args: AgentArgs) -> Result<()> {
    crate::agent::run::run_agent(args.no_tray, args.no_hotkeys)
}

pub fn run_config(args: ConfigArgs) -> Result<()> {
    match args.command {
        ConfigCommands::Path => {
            let path = crate::agent::config_path()?;
            println!("{}", path.display());
        }
        ConfigCommands::Show { json } => {
            let config = crate::agent::load_config()
                .unwrap_or_else(|_| crate::agent::default_agent_config());
            if json {
                println!("{}", serde_json::to_string_pretty(&config)?);
            } else {
                println!("{config:#?}");
            }
        }
        ConfigCommands::Validate => {
            let config = crate::agent::load_config()
                .unwrap_or_else(|_| crate::agent::default_agent_config());
            crate::agent::validate_config(&config)?;
            println!("ok");
        }
        ConfigCommands::Defaults => {
            let config = crate::agent::default_agent_config();
            println!("{}", serde_json::to_string_pretty(&config)?);
        }
        ConfigCommands::Set(args) => {
            let mut config = match crate::agent::load_config() {
                Ok(config) => config,
                Err(err) => {
                    let path = crate::agent::config_path().ok();
                    let missing = path
                        .as_ref()
                        .is_some_and(|path| !path.exists());
                    if missing {
                        crate::agent::default_agent_config()
                    } else {
                        return Err(err);
                    }
                }
            };
            apply_config_set(&mut config, &args);
            crate::agent::validate_config(&config)?;
            crate::agent::store_config(&config)?;
            println!("{}", serde_json::to_string_pretty(&config)?);
        }
    }
    Ok(())
}

pub fn run_autostart(args: AutostartArgs) -> Result<()> {
    match args.command {
        AutostartCommands::Enable => {
            crate::agent::autostart::enable().wrap_err("autostart enable failed")?;
            println!("enabled");
        }
        AutostartCommands::Disable => {
            crate::agent::autostart::disable().wrap_err("autostart disable failed")?;
            println!("disabled");
        }
        AutostartCommands::Status => {
            let enabled = crate::agent::autostart::is_enabled()?;
            println!("{}", if enabled { "enabled" } else { "disabled" });
        }
        AutostartCommands::Refresh => {
            crate::agent::autostart::refresh().wrap_err("autostart refresh failed")?;
            println!("refreshed");
        }
    }
    Ok(())
}

fn apply_config_set(config: &mut crate::agent::AgentConfig, args: &ConfigSetArgs) {
    if let Some(target) = &args.target {
        config.target = target.clone();
    }
    if let Some(port) = args.port {
        config.port = Some(port);
    }
    if let Some(identity) = &args.identity_file {
        config.identity_file = Some(identity.clone());
    }
    if let Some(max_size) = args.max_size {
        config.max_size = max_size;
    }
    if let Some(timeout_ms) = args.timeout_ms {
        config.timeout_ms = timeout_ms;
    }
    if let Some(resync_frames) = args.resync_frames {
        config.resync_frames = resync_frames;
    }
    if let Some(resync_max_bytes) = args.resync_max_bytes {
        config.resync_max_bytes = resync_max_bytes;
    }
    if args.clear_ssh_options {
        config.ssh_options.clear();
    }
    if !args.ssh_option.is_empty() {
        config.ssh_options.extend(args.ssh_option.iter().cloned());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_config_set_updates_only_provided_fields() {
        let mut config = crate::agent::default_agent_config();
        config.target = "user@old".to_string();
        config.max_size = 1;
        config.timeout_ms = 2;

        let args = ConfigSetArgs {
            target: Some("user@new".to_string()),
            max_size: Some(42),
            timeout_ms: None,
            ..ConfigSetArgs::default()
        };
        apply_config_set(&mut config, &args);

        assert_eq!(config.target, "user@new");
        assert_eq!(config.max_size, 42);
        assert_eq!(config.timeout_ms, 2);
    }

    #[test]
    fn apply_config_set_appends_ssh_options_by_default() {
        let mut config = crate::agent::default_agent_config();
        config.ssh_options = vec!["A=1".to_string()];
        let args = ConfigSetArgs {
            ssh_option: vec!["B=2".to_string()],
            ..ConfigSetArgs::default()
        };
        apply_config_set(&mut config, &args);
        assert_eq!(config.ssh_options, vec!["A=1", "B=2"]);
    }

    #[test]
    fn apply_config_set_clear_ssh_options() {
        let mut config = crate::agent::default_agent_config();
        config.ssh_options = vec!["A=1".to_string()];
        let args = ConfigSetArgs {
            clear_ssh_options: true,
            ssh_option: vec!["B=2".to_string()],
            ..ConfigSetArgs::default()
        };
        apply_config_set(&mut config, &args);
        assert_eq!(config.ssh_options, vec!["B=2"]);
    }
}
