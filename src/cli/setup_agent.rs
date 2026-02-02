use crate::cli::{ConfigSetArgs, SetupAgentArgs};
use eyre::{Result, WrapErr, eyre};

pub fn run(args: SetupAgentArgs) -> Result<()> {
    if args.target.trim().is_empty() {
        return Err(eyre!("--target must not be empty"));
    }

    let mut config = match crate::agent::load_config() {
        Ok(config) => config,
        Err(err) => {
            let path = crate::agent::config_path().ok();
            let missing = path.as_ref().is_some_and(|path| !path.exists());
            if missing {
                crate::agent::default_agent_config()
            } else {
                return Err(err);
            }
        }
    };

    let set_args = ConfigSetArgs {
        target: Some(args.target),
        port: args.port,
        identity_file: args.identity_file,
        ssh_option: args.ssh_option,
        clear_ssh_options: args.clear_ssh_options,
        max_size: args.max_size,
        timeout_ms: args.timeout_ms,
        resync_frames: args.resync_frames,
        resync_max_bytes: args.resync_max_bytes,
    };
    crate::cli::agent::apply_config_set(&mut config, &set_args);

    let want_autostart = !args.no_autostart;
    config.autostart_enabled = want_autostart;

    crate::agent::validate_config(&config)?;

    if args.dry_run {
        println!("{}", serde_json::to_string_pretty(&config)?);
        println!();
        if want_autostart {
            println!("dry-run: would run `ssh_clipboard autostart refresh`");
        } else {
            println!("dry-run: would run `ssh_clipboard autostart disable`");
        }
        return Ok(());
    }

    if want_autostart {
        config.autostart_enabled = false;
    }
    crate::agent::store_config(&config)?;

    if want_autostart {
        crate::agent::autostart::refresh().wrap_err("autostart refresh failed")?;
        config.autostart_enabled = true;
        crate::agent::store_config(&config)?;
    } else {
        crate::agent::autostart::disable().wrap_err("autostart disable failed")?;
    }

    println!("{}", serde_json::to_string_pretty(&config)?);
    Ok(())
}
