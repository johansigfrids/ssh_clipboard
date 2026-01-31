use eyre::{Result, WrapErr};

use crate::cli::{AgentArgs, AutostartArgs, AutostartCommands, ConfigArgs, ConfigCommands};

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
