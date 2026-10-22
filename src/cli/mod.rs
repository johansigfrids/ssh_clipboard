#[cfg(target_os = "linux")]
use clap::Args;
use clap::{Parser, Subcommand};
use eyre::Result;
use std::path::PathBuf;
#[cfg(target_os = "linux")]
mod server_maintenance;
mod settings;
mod shelf;
#[derive(Parser)]
#[command(name = "ssh_clipboard", version, about = "SSH clipboard history tool")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}
#[derive(Subcommand)]
enum Commands {
    /// Inspect and edit shared desktop/CLI settings.
    Settings(settings::SettingsArgs),
    /// Explicit transfers and metadata over the session endpoint.
    Shelf(shelf::ShelfArgs),
    #[cfg(target_os = "linux")]
    ServerMaintenance(server_maintenance::MaintenanceArgs),
    #[cfg(target_os = "linux")]
    ShelfDaemon(ShelfDaemonArgs),
    #[cfg(target_os = "linux")]
    SessionProxy(SessionProxyArgs),
}
#[cfg(target_os = "linux")]
#[derive(Args, Clone, Debug, PartialEq, Eq)]
pub struct ShelfDaemonArgs {
    #[arg(long)]
    pub socket_path: Option<PathBuf>,
    #[arg(long, default_value_t = 20)]
    pub max_items: usize,
    #[arg(long, default_value_t = 3600)]
    pub retention_secs: u64,
    #[arg(long, default_value_t = 64 * 1024 * 1024)]
    pub max_item_bytes: usize,
    #[arg(long, default_value_t = 256 * 1024 * 1024)]
    pub max_stored_bytes: usize,
    #[arg(long, default_value_t = 384 * 1024 * 1024)]
    pub max_resident_bytes: usize,
    #[arg(long, default_value_t = 256 * 1024 * 1024)]
    pub max_decoded_bytes: usize,
    #[arg(long, default_value_t = 16)]
    pub max_connections: usize,
    #[arg(long, default_value_t = 30000)]
    pub io_timeout_ms: u64,
    #[arg(long, default_value_t = 300000)]
    pub idle_timeout_ms: u64,
}

#[cfg(target_os = "linux")]
#[derive(Args, Clone)]
pub struct SessionProxyArgs {
    #[arg(long)]
    pub socket_path: Option<PathBuf>,
    #[arg(long, default_value_t = 30000)]
    pub io_timeout_ms: u64,
    #[arg(long, default_value_t = 300000)]
    pub idle_timeout_ms: u64,
}

pub async fn run() -> Result<()> {
    let cli = Cli::parse();
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();
    match cli.command {
        Commands::Settings(args) => settings::run(args),
        Commands::Shelf(args) => shelf::run(args).await,
        #[cfg(target_os = "linux")]
        Commands::ServerMaintenance(args) => server_maintenance::run(args).await,
        #[cfg(target_os = "linux")]
        Commands::ShelfDaemon(args) => {
            let path = args
                .socket_path
                .unwrap_or(crate::shelf_daemon::default_socket_path()?);
            let limits = crate::shelf::ShelfLimits {
                max_items: args.max_items,
                retention: std::time::Duration::from_secs(args.retention_secs),
                max_item_bytes: args.max_item_bytes,
                max_stored_bytes: args.max_stored_bytes,
                max_resident_bytes: args.max_resident_bytes,
                max_decoded_bytes: args.max_decoded_bytes,
                ..Default::default()
            };
            let policy = crate::shelf_service::ServiceLimits {
                max_connections: args.max_connections,
                operation_timeout: std::time::Duration::from_millis(args.io_timeout_ms),
                idle_timeout: std::time::Duration::from_millis(args.idle_timeout_ms),
                ..Default::default()
            };
            crate::shelf_daemon::run(path, limits, policy).await
        }
        #[cfg(target_os = "linux")]
        Commands::SessionProxy(args) => {
            let path = args
                .socket_path
                .unwrap_or(crate::shelf_daemon::default_socket_path()?);
            let limits = crate::session_bridge::BridgeLimits {
                read_timeout: std::time::Duration::from_millis(args.idle_timeout_ms),
                write_timeout: std::time::Duration::from_millis(args.io_timeout_ms),
                ..Default::default()
            };
            let code = match crate::session_proxy::run(path, limits).await {
                Ok(code) => code,
                Err(_) => {
                    eprintln!(
                        "Shelf session proxy failed. Check the configured endpoint and daemon."
                    );
                    5
                }
            };
            // Tokio's stdio reads are blocking underneath. Exit on both paths so a
            // held-open SSH stdin cannot keep the finished proxy process alive.
            std::process::exit(code);
        }
    }
}

#[derive(Default)]
pub(crate) struct ClientConfigArgs {
    pub profile: Option<String>,
    pub settings_file: Option<PathBuf>,
    pub target: Option<String>,
    pub port: Option<u16>,
    pub identity_file: Option<PathBuf>,
    pub ssh_option: Vec<String>,
    pub ssh_bin: Option<PathBuf>,
    pub timeout_ms: Option<u64>,
    pub max_item_bytes: Option<u32>,
    pub max_decoded_bytes: Option<u32>,
}
pub(crate) fn build_client_config(
    args: ClientConfigArgs,
) -> Result<crate::client::config::ClientConfig> {
    use crate::client::ssh::SshConfig;
    if args
        .target
        .as_ref()
        .is_some_and(|target| target.trim().is_empty())
    {
        eyre::bail!("missing SSH target");
    }
    // A direct destination only uses saved connection settings when a profile
    // was explicitly selected. In particular, do not even read an unrelated file.
    let profile = if args.target.is_some() && args.profile.is_none() {
        None
    } else {
        settings::load_profile(args.settings_file.as_deref(), args.profile.as_deref())?
    };
    let mut config = match profile {
        Some(profile) => profile
            .client_config()
            .map_err(|message| eyre::eyre!(message))?,
        None => crate::client::config::ClientConfig {
            ssh: SshConfig {
                target: String::new(),
                port: None,
                identity_file: None,
                ssh_options: Vec::new(),
                ssh_bin: None,
            },
            policy: Default::default(),
        },
    };
    if let Some(target) = args.target {
        config.ssh.target = target;
    }
    config.ssh.port = args.port.or(config.ssh.port);
    config.ssh.identity_file = args.identity_file.or(config.ssh.identity_file);
    config.ssh.ssh_bin = args.ssh_bin.or(config.ssh.ssh_bin);
    let mut ssh_options = args.ssh_option;
    ssh_options.extend(config.ssh.ssh_options);
    config.ssh.ssh_options = ssh_options;
    if let Some(value) = args.max_item_bytes {
        config.policy.max_item_bytes = value;
    }
    if let Some(value) = args.max_decoded_bytes {
        config.policy.max_decoded_bytes = value;
    }
    if let Some(value) = args.timeout_ms {
        config.policy.operation_timeout = std::time::Duration::from_millis(value);
    }
    crate::configuration::validate_connection(config.ssh.target.trim(), config.ssh.port, "")
        .map_err(|message| eyre::eyre!(message))?;
    if config.ssh.target.trim().is_empty() {
        eyre::bail!("missing SSH target: save settings or use --target or --profile");
    }
    config
        .policy
        .validate()
        .map_err(|_| eyre::eyre!("Session limits or timeout are invalid."))?;
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::configuration::{ConfigStore, Configuration};

    #[test]
    fn session_policy_resolves_flags_then_profile_then_defaults() {
        let directory = tempfile::tempdir().unwrap();
        let store = ConfigStore::new(directory.path().join("settings.json"));
        let mut settings = Configuration::default();
        settings.profiles[0].target = "saved-host".into();
        settings.profiles[0].port = Some(2222);
        settings.profiles[0].identity_file = "saved-key".into();
        settings.profiles[0].transport.ssh_options = vec!["ConnectTimeout=9".into()];
        settings.profiles[0].transport.timeout_ms = 9000;
        settings.profiles[0].limits.max_item_bytes = 1024;
        settings.profiles[0].limits.max_decoded_bytes = 4096;
        store.prepare(None, settings).unwrap().commit().unwrap();
        let resolve = |target, profile, item| {
            build_client_config(ClientConfigArgs {
                settings_file: Some(store.path().into()),
                target,
                profile,
                max_item_bytes: item,
                ..Default::default()
            })
            .unwrap()
        };
        let saved = resolve(None, None, None);
        assert_eq!(saved.policy.max_item_bytes, 1024);
        assert_eq!(saved.policy.max_decoded_bytes, 4096);
        assert_eq!(saved.policy.operation_timeout.as_millis(), 9000);
        let overridden = resolve(None, Some("default".into()), Some(512));
        assert_eq!(overridden.policy.max_item_bytes, 512);
        assert_eq!(overridden.policy.max_decoded_bytes, 4096);
        let direct = resolve(Some("other-host".into()), None, None);
        assert_eq!(
            direct.policy,
            crate::client::session::SessionPolicy::default()
        );
        assert_eq!(direct.ssh.port, None);
        assert_eq!(direct.ssh.identity_file, None);
        let selected = resolve(Some("other-host".into()), Some("default".into()), None);
        assert_eq!(selected.policy, saved.policy);
        assert_eq!(selected.ssh.port, Some(2222));

        let overridden = build_client_config(ClientConfigArgs {
            settings_file: Some(store.path().into()),
            profile: Some("default".into()),
            ssh_option: vec!["ConnectTimeout=2".into()],
            timeout_ms: Some(2000),
            max_item_bytes: Some(512),
            max_decoded_bytes: Some(2048),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(
            overridden.ssh.ssh_options,
            ["ConnectTimeout=2", "ConnectTimeout=9"]
        );
        assert_eq!(overridden.policy.operation_timeout.as_millis(), 2000);
        assert_eq!(overridden.policy.max_item_bytes, 512);
        assert_eq!(overridden.policy.max_decoded_bytes, 2048);
    }

    #[test]
    fn direct_destination_does_not_read_unrelated_settings() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        std::fs::write(&path, b"unsupported settings").unwrap();
        let resolve = |profile| {
            build_client_config(ClientConfigArgs {
                settings_file: Some(path.clone()),
                target: Some("direct-host".into()),
                profile,
                ..Default::default()
            })
        };
        assert_eq!(resolve(None).unwrap().ssh.target, "direct-host");
        assert!(resolve(Some("default".into())).is_err());
        assert_eq!(std::fs::read(path).unwrap(), b"unsupported settings");
    }
}
