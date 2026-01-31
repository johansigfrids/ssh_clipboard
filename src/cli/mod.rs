use clap::{Args, Parser, Subcommand};
use eyre::Result;
#[cfg(target_os = "linux")]
use eyre::WrapErr;
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

use crate::client::ssh::SshConfig;
use crate::client::transport::ClientConfig;
use crate::protocol::{DEFAULT_MAX_SIZE, ErrorCode, Response, ResponseKind};

mod exit;
mod peek;
mod pull;
mod push;

#[cfg(all(
    feature = "agent",
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
mod agent;

#[derive(Parser)]
#[command(name = "ssh_clipboard", version, about = "SSH clipboard tool")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Push(PushArgs),
    Pull(PullArgs),
    Peek(PeekArgs),
    #[cfg(target_os = "linux")]
    Daemon(DaemonArgs),
    #[cfg(target_os = "linux")]
    Proxy(ProxyArgs),
    #[cfg(all(
        feature = "agent",
        any(target_os = "windows", target_os = "macos", target_os = "linux")
    ))]
    Agent(AgentArgs),
    #[cfg(all(
        feature = "agent",
        any(target_os = "windows", target_os = "macos", target_os = "linux")
    ))]
    Config(ConfigArgs),
    #[cfg(all(
        feature = "agent",
        any(target_os = "windows", target_os = "macos", target_os = "linux")
    ))]
    Autostart(AutostartArgs),
}

#[derive(Args, Clone)]
pub struct PushArgs {
    #[arg(long)]
    pub target: Option<String>,
    #[arg(long)]
    pub host: Option<String>,
    #[arg(long)]
    pub user: Option<String>,
    #[arg(long)]
    pub port: Option<u16>,
    #[arg(long)]
    pub identity_file: Option<PathBuf>,
    #[arg(long)]
    pub ssh_option: Vec<String>,
    #[arg(long)]
    pub ssh_bin: Option<PathBuf>,
    #[arg(long, default_value_t = DEFAULT_MAX_SIZE)]
    pub max_size: usize,
    #[arg(long, default_value_t = 7000)]
    pub timeout_ms: u64,
    #[arg(long)]
    pub stdin: bool,
    #[arg(long)]
    pub strict_frames: bool,
    #[arg(long, default_value_t = 8192)]
    pub resync_max_bytes: usize,
}

#[derive(Args, Clone)]
pub struct PullArgs {
    #[arg(long)]
    pub target: Option<String>,
    #[arg(long)]
    pub host: Option<String>,
    #[arg(long)]
    pub user: Option<String>,
    #[arg(long)]
    pub port: Option<u16>,
    #[arg(long)]
    pub identity_file: Option<PathBuf>,
    #[arg(long)]
    pub ssh_option: Vec<String>,
    #[arg(long)]
    pub ssh_bin: Option<PathBuf>,
    #[arg(long, default_value_t = DEFAULT_MAX_SIZE)]
    pub max_size: usize,
    #[arg(long, default_value_t = 7000)]
    pub timeout_ms: u64,
    #[arg(long)]
    pub stdout: bool,
    #[arg(long)]
    pub output: Option<PathBuf>,
    #[arg(long)]
    pub base64: bool,
    #[arg(long)]
    pub peek: bool,
    #[arg(long)]
    pub json: bool,
    #[arg(long)]
    pub strict_frames: bool,
    #[arg(long, default_value_t = 8192)]
    pub resync_max_bytes: usize,
}

#[derive(Args, Clone)]
pub struct PeekArgs {
    #[arg(long)]
    pub target: Option<String>,
    #[arg(long)]
    pub host: Option<String>,
    #[arg(long)]
    pub user: Option<String>,
    #[arg(long)]
    pub port: Option<u16>,
    #[arg(long)]
    pub identity_file: Option<PathBuf>,
    #[arg(long)]
    pub ssh_option: Vec<String>,
    #[arg(long)]
    pub ssh_bin: Option<PathBuf>,
    #[arg(long, default_value_t = DEFAULT_MAX_SIZE)]
    pub max_size: usize,
    #[arg(long, default_value_t = 7000)]
    pub timeout_ms: u64,
    #[arg(long)]
    pub json: bool,
    #[arg(long)]
    pub strict_frames: bool,
    #[arg(long, default_value_t = 8192)]
    pub resync_max_bytes: usize,
}

#[cfg(target_os = "linux")]
#[derive(Args, Clone)]
pub struct DaemonArgs {
    #[arg(long)]
    pub socket_path: Option<PathBuf>,
    #[arg(long, default_value_t = DEFAULT_MAX_SIZE)]
    pub max_size: usize,
    #[arg(long, default_value_t = 7000)]
    pub io_timeout_ms: u64,
}

#[cfg(target_os = "linux")]
#[derive(Args, Clone)]
pub struct ProxyArgs {
    #[arg(long)]
    pub socket_path: Option<PathBuf>,
    #[arg(long, default_value_t = DEFAULT_MAX_SIZE)]
    pub max_size: usize,
    #[arg(long, default_value_t = 7000)]
    pub io_timeout_ms: u64,
    #[arg(long)]
    pub autostart_daemon: bool,
}

#[cfg(all(
    feature = "agent",
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
#[derive(Args, Clone)]
pub struct AgentArgs {
    #[arg(long)]
    pub no_tray: bool,
    #[arg(long)]
    pub no_hotkeys: bool,
}

#[cfg(all(
    feature = "agent",
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
#[derive(Args, Clone)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub command: ConfigCommands,
}

#[cfg(all(
    feature = "agent",
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
#[derive(Subcommand, Clone)]
pub enum ConfigCommands {
    Path,
    Show {
        #[arg(long)]
        json: bool,
    },
    Validate,
    Defaults,
}

#[cfg(all(
    feature = "agent",
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
#[derive(Args, Clone)]
pub struct AutostartArgs {
    #[command(subcommand)]
    pub command: AutostartCommands,
}

#[cfg(all(
    feature = "agent",
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
#[derive(Subcommand, Clone)]
pub enum AutostartCommands {
    Enable,
    Disable,
    Status,
    Refresh,
}

pub async fn run() -> Result<()> {
    let cli = Cli::parse();
    #[cfg(all(
        feature = "agent",
        any(target_os = "windows", target_os = "macos", target_os = "linux")
    ))]
    let agent_mode = matches!(cli.command, Commands::Agent(_));
    #[cfg(not(all(
        feature = "agent",
        any(target_os = "windows", target_os = "macos", target_os = "linux")
    )))]
    let agent_mode = false;

    init_tracing(agent_mode)?;

    match cli.command {
        Commands::Push(args) => push::run(args).await,
        Commands::Pull(args) => pull::run(args).await,
        Commands::Peek(args) => peek::run(args).await,
        #[cfg(target_os = "linux")]
        Commands::Daemon(args) => {
            let socket_path = args
                .socket_path
                .unwrap_or(crate::daemon::default_socket_path()?);
            crate::daemon::run_daemon(socket_path, args.max_size, args.io_timeout_ms)
                .await
                .wrap_err("daemon failed")?;
            Ok(())
        }
        #[cfg(target_os = "linux")]
        Commands::Proxy(args) => {
            let socket_path = args
                .socket_path
                .unwrap_or(crate::daemon::default_socket_path()?);
            let exit_code = crate::proxy::run_proxy(
                socket_path,
                args.max_size,
                args.io_timeout_ms,
                args.autostart_daemon,
            )
            .await
            .wrap_err("proxy failed")?;
            std::process::exit(exit_code);
        }
        #[cfg(all(
            feature = "agent",
            any(target_os = "windows", target_os = "macos", target_os = "linux")
        ))]
        Commands::Agent(args) => agent::run_agent(args),
        #[cfg(all(
            feature = "agent",
            any(target_os = "windows", target_os = "macos", target_os = "linux")
        ))]
        Commands::Config(args) => agent::run_config(args),
        #[cfg(all(
            feature = "agent",
            any(target_os = "windows", target_os = "macos", target_os = "linux")
        ))]
        Commands::Autostart(args) => agent::run_autostart(args),
    }
}

pub(crate) fn handle_response(response: Response, allow_empty: bool) -> Result<()> {
    match response.kind {
        ResponseKind::Ok => Ok(()),
        ResponseKind::Empty if allow_empty => Ok(()),
        ResponseKind::Empty => exit::exit_with_code(2, "no clipboard value set"),
        ResponseKind::Error { code, message } => match code {
            ErrorCode::InvalidRequest | ErrorCode::InvalidUtf8 | ErrorCode::VersionMismatch => {
                exit::exit_with_code(2, &message)
            }
            ErrorCode::PayloadTooLarge => exit::exit_with_code(3, &message),
            ErrorCode::DaemonNotRunning => exit::exit_with_code(4, &message),
            ErrorCode::Internal => exit::exit_with_code(2, &message),
        },
        ResponseKind::Value { .. } | ResponseKind::Meta { .. } => Ok(()),
    }
}

pub(crate) fn handle_peek_response(response: Response, json: bool) -> Result<()> {
    match &response.kind {
        ResponseKind::Meta {
            content_type,
            size,
            created_at,
        } => {
            if json {
                let value = serde_json::json!({
                    "content_type": content_type,
                    "size": size,
                    "created_at": created_at
                });
                println!("{value}");
            } else {
                println!("content_type={content_type} size={size} created_at={created_at}");
            }
            Ok(())
        }
        ResponseKind::Empty => exit::exit_with_code(2, "no clipboard value set"),
        _ => handle_response(response, true),
    }
}

pub(crate) fn build_client_config(
    target: Option<String>,
    host: Option<String>,
    user: Option<String>,
    port: Option<u16>,
    identity_file: Option<PathBuf>,
    ssh_option: Vec<String>,
    ssh_bin: Option<PathBuf>,
    max_size: usize,
    timeout_ms: u64,
    strict_frames: bool,
    resync_max_bytes: usize,
) -> ClientConfig {
    ClientConfig {
        ssh: SshConfig {
            target: target.unwrap_or_default(),
            port,
            user,
            host,
            identity_file,
            ssh_options: ssh_option,
            ssh_bin,
        },
        max_size,
        timeout_ms,
        resync_frames: !strict_frames,
        resync_max_bytes,
    }
}

fn init_tracing(agent_mode: bool) -> Result<()> {
    #[cfg(not(all(
        feature = "agent",
        any(target_os = "windows", target_os = "macos", target_os = "linux")
    )))]
    let _ = agent_mode;

    #[cfg(all(
        feature = "agent",
        any(target_os = "windows", target_os = "macos", target_os = "linux")
    ))]
    if agent_mode {
        let log_dir = crate::agent::config_path()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| ".".into()))
            .join("logs");
        let _ = std::fs::create_dir_all(&log_dir);
        let file_appender = tracing_appender::rolling::daily(log_dir, "agent.log");
        let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
        Box::leak(Box::new(guard));

        tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::from_default_env())
            .with_writer(non_blocking)
            .init();
        return Ok(());
    }

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();
    Ok(())
}
