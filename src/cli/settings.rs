use crate::client::ssh::SshConfig;
use crate::configuration::{ConfigStore, Configuration, Profile, TransportSettings};
use clap::{Args, Subcommand};
use eyre::{Result, WrapErr, eyre};
use std::path::{Path, PathBuf};

#[derive(Args)]
pub struct SettingsArgs {
    /// Use a separate settings file (also useful for portable/test configurations).
    #[arg(long, global = true)]
    pub settings_file: Option<PathBuf>,
    #[command(subcommand)]
    pub command: SettingsCommand,
}

#[derive(Subcommand)]
pub enum SettingsCommand {
    /// Print the shared settings location without creating a file.
    Path,
    /// Print saved settings as JSON, or defaults if no file exists.
    Show,
    /// Update the selected profile and preferences without changing other profiles.
    Set(SettingsSetArgs),
    /// Add a named profile. Adding does not change the selected profile.
    AddProfile {
        #[arg(long)]
        id: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        target: String,
        #[arg(long)]
        port: Option<u16>,
        #[arg(long)]
        identity_file: Option<String>,
    },
    /// Select the profile used by the desktop and target-less CLI transfers.
    SelectProfile { id: String },
    /// Remove a local profile. Removing the selected one selects the first remaining profile.
    RemoveProfile { id: String },
    /// Initialize missing shared settings from a backed-up legacy agent file.
    MigrateAgent {
        #[arg(long)]
        legacy_file: Option<PathBuf>,
    },
}

#[derive(Args, Default)]
#[command(arg_required_else_help = true)]
pub struct SettingsSetArgs {
    #[arg(long)]
    target: Option<String>,
    #[arg(long)]
    name: Option<String>,
    #[arg(long, conflicts_with = "clear_port")]
    port: Option<u16>,
    #[arg(long)]
    clear_port: bool,
    #[arg(long, conflicts_with = "clear_identity_file")]
    identity_file: Option<String>,
    #[arg(long)]
    clear_identity_file: bool,
    #[arg(long, value_parser = clap::value_parser!(bool))]
    show_preview: Option<bool>,
    #[arg(long, value_parser = clap::value_parser!(bool))]
    close_after_copy: Option<bool>,
    #[arg(long)]
    ssh_option: Vec<String>,
    #[arg(long)]
    clear_ssh_options: bool,
    #[arg(long, conflicts_with = "clear_ssh_bin")]
    ssh_bin: Option<PathBuf>,
    #[arg(long)]
    clear_ssh_bin: bool,
    #[arg(long)]
    max_size: Option<usize>,
    #[arg(long)]
    timeout_ms: Option<u64>,
    #[arg(long, value_parser = clap::value_parser!(bool))]
    resync_frames: Option<bool>,
    #[arg(long)]
    resync_max_bytes: Option<usize>,
    #[arg(long)]
    device_name: Option<String>,
}

pub fn store_at(path: Option<&Path>) -> Result<ConfigStore> {
    match path {
        Some(path) => Ok(ConfigStore::new(if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()?.join(path)
        })),
        None => Ok(ConfigStore::user()?),
    }
}

pub fn run(args: SettingsArgs) -> Result<()> {
    let store = store_at(args.settings_file.as_deref())?;
    if matches!(args.command, SettingsCommand::Path) {
        println!("{}", store.path().display());
        return Ok(());
    }
    if let SettingsCommand::MigrateAgent { legacy_file } = args.command {
        let config = match legacy_file {
            Some(path) => store.initialize_from_legacy(&path)?,
            None => store.initialize()?,
        }
        .ok_or_else(|| eyre!("no legacy agent settings file was found"))?;
        println!("{}", serde_json::to_string_pretty(&config)?);
        return Ok(());
    }
    let loaded = store
        .load()
        .wrap_err_with(|| format!("could not load {}", store.path().display()))?;
    let mut next = loaded.clone().unwrap_or_default();
    if matches!(args.command, SettingsCommand::Show) {
        println!("{}", serde_json::to_string_pretty(&next)?);
        return Ok(());
    }
    apply(&mut next, args.command)?;
    let saved = store.prepare(loaded.as_ref(), next)?.commit()?;
    println!("{}", serde_json::to_string_pretty(&saved)?);
    Ok(())
}

fn apply(config: &mut Configuration, command: SettingsCommand) -> Result<()> {
    match command {
        SettingsCommand::Set(args) => {
            let profile = config.selected_profile_mut()?;
            if let Some(target) = args.target {
                profile.target = target.trim().into();
            }
            if let Some(name) = args.name {
                profile.name = name;
            }
            if args.clear_port {
                profile.port = None;
            } else if let Some(port) = args.port {
                profile.port = Some(port);
            }
            if args.clear_identity_file {
                profile.identity_file.clear();
            } else if let Some(identity) = args.identity_file {
                profile.identity_file = identity;
            }
            if args.clear_ssh_options {
                profile.transport.ssh_options.clear();
            }
            profile.transport.ssh_options.extend(args.ssh_option);
            if args.clear_ssh_bin {
                profile.transport.ssh_bin = None;
            } else if let Some(bin) = args.ssh_bin {
                profile.transport.ssh_bin = Some(bin);
            }
            if let Some(bytes) = args.max_size {
                profile.transport.max_size = bytes;
            }
            if let Some(ms) = args.timeout_ms {
                profile.transport.timeout_ms = ms;
            }
            if let Some(resync) = args.resync_frames {
                profile.transport.resync_frames = resync;
            }
            if let Some(bytes) = args.resync_max_bytes {
                profile.transport.resync_max_bytes = bytes;
            }
            if let Some(name) = args.device_name {
                config.device.name = name;
            }
            if let Some(show) = args.show_preview {
                config.preferences.show_preview = show;
            }
            if let Some(close) = args.close_after_copy {
                config.preferences.close_after_copy = close;
            }
        }
        SettingsCommand::AddProfile {
            id,
            name,
            target,
            port,
            identity_file,
        } => {
            if config.profiles.iter().any(|p| p.id == id) {
                return Err(eyre!("a profile with ID {id:?} already exists"));
            }
            config.profiles.push(Profile {
                id,
                name,
                target: target.trim().into(),
                port,
                identity_file: identity_file.unwrap_or_default(),
                transport: TransportSettings::default(),
                endpoint: Default::default(),
            });
        }
        SettingsCommand::SelectProfile { id } => {
            if !config.profiles.iter().any(|p| p.id == id) {
                return Err(eyre!("unknown profile ID {id:?}"));
            }
            config.default_profile = id;
        }
        SettingsCommand::RemoveProfile { id } => {
            config
                .remove_profile(&id)
                .map_err(|message| eyre!(message))?;
        }
        SettingsCommand::Show | SettingsCommand::Path | SettingsCommand::MigrateAgent { .. } => {
            unreachable!("read-only commands handled before apply")
        }
    }
    Ok(())
}

/// Explicit destinations are independent of saved connection defaults unless a
/// profile is explicitly requested. Never accidentally reuse another host's key/port.
pub fn apply_connection_defaults(
    ssh: &mut SshConfig,
    path: Option<&Path>,
    profile_id: Option<&str>,
) -> Result<Option<TransportSettings>> {
    let explicit_destination = !ssh.target.is_empty() || ssh.host.is_some();
    if explicit_destination && profile_id.is_none() {
        return Ok(None);
    }
    let store = store_at(path)?;
    let loaded = if path.is_none() {
        store.initialize()
    } else {
        store.load()
    }
    .wrap_err_with(|| format!("could not load {}", store.path().display()))?;
    let Some(config) = loaded else {
        if let Some(id) = profile_id {
            return Err(eyre!("unknown profile ID {id:?}: no shared settings file"));
        }
        return Ok(None);
    };
    let profile = match profile_id {
        Some(id) => config
            .profiles
            .iter()
            .find(|p| p.id == id)
            .ok_or_else(|| eyre!("unknown profile ID {id:?}"))?,
        None => config.selected_profile()?,
    };
    if !explicit_destination {
        ssh.target = profile.target.clone();
    }
    ssh.port = ssh.port.or(profile.port);
    if ssh.identity_file.is_none() && !profile.identity_file.trim().is_empty() {
        ssh.identity_file = Some(profile.identity_file.trim().into());
    }
    ssh.ssh_options
        .extend(profile.transport.ssh_options.iter().cloned());
    if ssh.ssh_bin.is_none() {
        ssh.ssh_bin = profile.transport.ssh_bin.clone();
    }
    Ok(Some(profile.transport.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ssh(target: &str) -> SshConfig {
        SshConfig {
            target: target.into(),
            port: None,
            host: None,
            user: None,
            identity_file: None,
            ssh_options: vec!["BatchMode=yes".into()],
            ssh_bin: None,
        }
    }

    #[test]
    fn defaults_and_explicit_profile_overrides_are_distinct_from_direct_destinations() {
        let directory = tempfile::tempdir().unwrap();
        let store = ConfigStore::new(directory.path().join("settings.json"));
        let mut config = Configuration::default();
        config.profiles[0].target = "saved-hub".into();
        config.profiles[0].port = Some(2222);
        config.profiles[0].identity_file = "saved-key".into();
        store.prepare(None, config).unwrap().commit().unwrap();
        let mut saved = ssh("");
        assert!(
            apply_connection_defaults(&mut saved, Some(store.path()), None)
                .unwrap()
                .is_some()
        );
        assert_eq!(saved.target, "saved-hub");
        assert_eq!(saved.port, Some(2222));
        assert_eq!(saved.identity_file, Some("saved-key".into()));
        let mut direct = ssh("other-host");
        assert!(
            apply_connection_defaults(&mut direct, Some(store.path()), None)
                .unwrap()
                .is_none()
        );
        assert_eq!(direct.port, None);
        assert_eq!(direct.identity_file, None);
        let mut overridden = ssh("explicit-host");
        overridden.port = Some(2200);
        overridden.identity_file = Some("explicit-key".into());
        assert!(
            apply_connection_defaults(&mut overridden, Some(store.path()), Some("default"))
                .unwrap()
                .is_some()
        );
        assert_eq!(overridden.target, "explicit-host");
        assert_eq!(overridden.port, Some(2200));
        assert_eq!(overridden.identity_file, Some("explicit-key".into()));
        assert_eq!(overridden.ssh_options, vec!["BatchMode=yes"]);
    }

    #[test]
    fn selected_profile_and_one_shot_selection_preserve_the_saved_default() {
        let directory = tempfile::tempdir().unwrap();
        let store = ConfigStore::new(directory.path().join("settings.json"));
        let mut config = Configuration::default();
        apply(
            &mut config,
            SettingsCommand::AddProfile {
                id: "second".into(),
                name: "Second".into(),
                target: "second-hub".into(),
                port: None,
                identity_file: None,
            },
        )
        .unwrap();
        config.profiles[0].target = "first-hub".into();
        let saved = store.prepare(None, config).unwrap().commit().unwrap();
        let mut selected = ssh("");
        apply_connection_defaults(&mut selected, Some(store.path()), Some("second")).unwrap();
        assert_eq!(selected.target, "second-hub");
        assert_eq!(store.load().unwrap(), Some(saved));
        assert!(
            apply_connection_defaults(&mut ssh(""), Some(store.path()), Some("missing")).is_err()
        );
    }

    #[test]
    fn clearing_overrides_and_preference_edits_preserve_unrelated_fields() {
        let mut config = Configuration::default();
        config.profiles[0].port = Some(2222);
        config.profiles[0].identity_file = "saved-key".into();
        let shortcut = config.preferences.open_shortcut.clone();
        apply(
            &mut config,
            SettingsCommand::Set(SettingsSetArgs {
                clear_port: true,
                clear_identity_file: true,
                show_preview: Some(false),
                ..Default::default()
            }),
        )
        .unwrap();
        assert_eq!(config.profiles[0].port, None);
        assert!(config.profiles[0].identity_file.is_empty());
        assert!(!config.preferences.show_preview);
        assert_eq!(config.preferences.open_shortcut, shortcut);
    }
}
