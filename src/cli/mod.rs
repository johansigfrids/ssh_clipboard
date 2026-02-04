use clap::{Args, Parser, Subcommand};
use eyre::Result;
#[cfg(target_os = "linux")]
use eyre::WrapErr;
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

use crate::client::ssh::SshConfig;
use crate::client::transport::ClientConfig;
use crate::protocol::{DEFAULT_MAX_SIZE, ErrorCode, Response, ResponseKind};
use time::{Duration, OffsetDateTime};

mod exit;
#[cfg(target_os = "linux")]
mod install_daemon;
mod peek;
mod pull;
mod push;
#[cfg(all(
    feature = "agent",
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
mod setup_agent;

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
    #[cfg(target_os = "linux")]
    InstallDaemon(InstallDaemonArgs),
    #[cfg(target_os = "linux")]
    UninstallDaemon(UninstallDaemonArgs),
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
    #[cfg(all(
        feature = "agent",
        any(target_os = "windows", target_os = "macos", target_os = "linux")
    ))]
    SetupAgent(SetupAgentArgs),
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

#[cfg(target_os = "linux")]
#[derive(Args, Clone)]
pub struct InstallDaemonArgs {
    #[arg(long)]
    pub dry_run: bool,
    #[arg(long)]
    pub force: bool,
    #[arg(long)]
    pub no_sudo: bool,
    #[arg(long, default_value_t = DEFAULT_MAX_SIZE)]
    pub max_size: usize,
    #[arg(long, default_value_t = 7000)]
    pub io_timeout_ms: u64,
    #[arg(long)]
    pub socket_path: Option<PathBuf>,
}

#[cfg(target_os = "linux")]
#[derive(Args, Clone)]
pub struct UninstallDaemonArgs {
    #[arg(long)]
    pub dry_run: bool,
    #[arg(long)]
    pub no_sudo: bool,
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
    #[arg(long, hide = true)]
    pub autostart: bool,
    #[arg(long, hide = true)]
    pub force_exec: bool,
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
    Set(ConfigSetArgs),
}

#[derive(Args, Clone, Default)]
pub struct ConfigSetArgs {
    #[arg(long)]
    pub target: Option<String>,
    #[arg(long)]
    pub port: Option<u16>,
    #[arg(long)]
    pub identity_file: Option<PathBuf>,
    #[arg(long)]
    pub ssh_option: Vec<String>,
    #[arg(long)]
    pub clear_ssh_options: bool,
    #[arg(long)]
    pub max_size: Option<usize>,
    #[arg(long)]
    pub timeout_ms: Option<u64>,
    #[arg(long, value_parser = clap::value_parser!(bool))]
    pub resync_frames: Option<bool>,
    #[arg(long)]
    pub resync_max_bytes: Option<usize>,
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

#[cfg(all(
    feature = "agent",
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
#[derive(Args, Clone)]
pub struct SetupAgentArgs {
    #[arg(long)]
    pub target: String,
    #[arg(long)]
    pub port: Option<u16>,
    #[arg(long)]
    pub identity_file: Option<PathBuf>,
    #[arg(long)]
    pub ssh_option: Vec<String>,
    #[arg(long)]
    pub clear_ssh_options: bool,
    #[arg(long)]
    pub max_size: Option<usize>,
    #[arg(long)]
    pub timeout_ms: Option<u64>,
    #[arg(long, value_parser = clap::value_parser!(bool))]
    pub resync_frames: Option<bool>,
    #[arg(long)]
    pub resync_max_bytes: Option<usize>,
    #[arg(long)]
    pub no_autostart: bool,
    #[arg(long)]
    pub dry_run: bool,
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
        #[cfg(target_os = "linux")]
        Commands::InstallDaemon(args) => install_daemon::run(args).await,
        #[cfg(target_os = "linux")]
        Commands::UninstallDaemon(args) => install_daemon::run_uninstall(args).await,
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
        #[cfg(all(
            feature = "agent",
            any(target_os = "windows", target_os = "macos", target_os = "linux")
        ))]
        Commands::SetupAgent(args) => setup_agent::run(args),
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
                println!("{}", format_peek_output(content_type, *size, *created_at));
            }
            Ok(())
        }
        ResponseKind::Empty => exit::exit_with_code(2, "no clipboard value set"),
        _ => handle_response(response, true),
    }
}

pub(crate) fn format_peek_output(content_type: &str, size: u64, created_at_ms: i64) -> String {
    format!(
        "Content-Type: {content_type}\nSize: {size} bytes ({human_size})\nCreated: {created}",
        human_size = humanize_bytes(size),
        created = format_created_at(created_at_ms)
    )
}

fn format_created_at(created_at_ms: i64) -> String {
    if created_at_ms <= 0 {
        return "unknown".to_string();
    }
    let created_at =
        match OffsetDateTime::from_unix_timestamp_nanos(created_at_ms as i128 * 1_000_000) {
            Ok(value) => value,
            Err(_) => return "unknown".to_string(),
        };

    let now = OffsetDateTime::now_utc();
    let time_part = match time::format_description::parse(
        "[year]-[month]-[day] [hour]:[minute]:[second] UTC",
    ) {
        Ok(format) => created_at
            .format(&format)
            .unwrap_or_else(|_| "unknown".to_string()),
        Err(_) => "unknown".to_string(),
    };
    if created_at > now {
        return format!("{time_part} (in the future)");
    }

    let diff = now - created_at;
    format!("{time_part} ({})", humanize_duration(diff))
}

fn humanize_duration(duration: Duration) -> String {
    let mut seconds = duration.whole_seconds();
    if seconds <= 0 {
        return "just now".to_string();
    }

    let days = seconds / 86_400;
    seconds %= 86_400;
    let hours = seconds / 3_600;
    seconds %= 3_600;
    let minutes = seconds / 60;
    seconds %= 60;

    let mut parts = Vec::new();
    if days > 0 {
        parts.push(format!("{days}d"));
    }
    if hours > 0 && parts.len() < 2 {
        parts.push(format!("{hours}h"));
    }
    if minutes > 0 && parts.len() < 2 {
        parts.push(format!("{minutes}m"));
    }
    if seconds > 0 && parts.len() < 2 {
        parts.push(format!("{seconds}s"));
    }

    if parts.is_empty() {
        "just now".to_string()
    } else {
        format!("{} ago", parts.join(" "))
    }
}

fn humanize_bytes(size: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = 1024.0 * 1024.0;
    const GIB: f64 = 1024.0 * 1024.0 * 1024.0;

    let size_f = size as f64;
    if size_f >= GIB {
        format!("{:.1} GiB", size_f / GIB)
    } else if size_f >= MIB {
        format!("{:.1} MiB", size_f / MIB)
    } else if size_f >= KIB {
        format!("{:.1} KiB", size_f / KIB)
    } else {
        format!("{size} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn humanize_bytes_formats_units() {
        assert_eq!(humanize_bytes(0), "0 B");
        assert_eq!(humanize_bytes(512), "512 B");
        assert_eq!(humanize_bytes(1024), "1.0 KiB");
        assert_eq!(humanize_bytes(1024 * 1024), "1.0 MiB");
    }

    #[test]
    fn humanize_duration_formats_compact() {
        assert_eq!(humanize_duration(Duration::seconds(0)), "just now");
        assert_eq!(humanize_duration(Duration::seconds(61)), "1m 1s ago");
        assert_eq!(humanize_duration(Duration::seconds(3600)), "1h ago");
        assert_eq!(humanize_duration(Duration::seconds(90061)), "1d 1h ago");
    }

    #[test]
    fn format_created_at_handles_invalid_and_future() {
        assert_eq!(format_created_at(0), "unknown");
        let future = OffsetDateTime::now_utc() + Duration::seconds(60);
        let future_ms = future.unix_timestamp() * 1000;
        let formatted = format_created_at(future_ms);
        assert!(formatted.contains("in the future"));
    }
}

pub(crate) struct ClientConfigArgs {
    pub target: Option<String>,
    pub host: Option<String>,
    pub user: Option<String>,
    pub port: Option<u16>,
    pub identity_file: Option<PathBuf>,
    pub ssh_option: Vec<String>,
    pub ssh_bin: Option<PathBuf>,
    pub max_size: usize,
    pub timeout_ms: u64,
    pub strict_frames: bool,
    pub resync_max_bytes: usize,
}

pub(crate) fn build_client_config(args: ClientConfigArgs) -> ClientConfig {
    ClientConfig {
        ssh: SshConfig {
            target: args.target.unwrap_or_default(),
            port: args.port,
            user: args.user,
            host: args.host,
            identity_file: args.identity_file,
            ssh_options: args.ssh_option,
            ssh_bin: args.ssh_bin,
        },
        max_size: args.max_size,
        timeout_ms: args.timeout_ms,
        resync_frames: !args.strict_frames,
        resync_max_bytes: args.resync_max_bytes,
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

pub fn init_tracing_for_agent() -> Result<()> {
    init_tracing(true)
}
