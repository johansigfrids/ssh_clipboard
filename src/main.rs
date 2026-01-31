use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use clap::{Parser, Subcommand};
use eyre::{Result, WrapErr, eyre};
use ssh_clipboard::client::ssh::SshConfig;
use ssh_clipboard::client_actions::{ClipboardBuildError, PullApplyErrorKind};
use ssh_clipboard::client::transport::{ClientConfig, make_request, send_request};
use ssh_clipboard::protocol::{
    CONTENT_TYPE_PNG, CONTENT_TYPE_TEXT, ClipboardValue, DEFAULT_MAX_SIZE, ErrorCode, RequestKind,
    Response, ResponseKind,
};
use std::fs;
use std::path::PathBuf;
use tokio::io::{AsyncReadExt, BufReader};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "ssh_clipboard", version, about = "SSH clipboard tool")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Push {
        #[arg(long)]
        target: Option<String>,
        #[arg(long)]
        host: Option<String>,
        #[arg(long)]
        user: Option<String>,
        #[arg(long)]
        port: Option<u16>,
        #[arg(long)]
        identity_file: Option<PathBuf>,
        #[arg(long)]
        ssh_option: Vec<String>,
        #[arg(long)]
        ssh_bin: Option<PathBuf>,
        #[arg(long, default_value_t = DEFAULT_MAX_SIZE)]
        max_size: usize,
        #[arg(long, default_value_t = 7000)]
        timeout_ms: u64,
        #[arg(long)]
        stdin: bool,
    },
    Pull {
        #[arg(long)]
        target: Option<String>,
        #[arg(long)]
        host: Option<String>,
        #[arg(long)]
        user: Option<String>,
        #[arg(long)]
        port: Option<u16>,
        #[arg(long)]
        identity_file: Option<PathBuf>,
        #[arg(long)]
        ssh_option: Vec<String>,
        #[arg(long)]
        ssh_bin: Option<PathBuf>,
        #[arg(long, default_value_t = DEFAULT_MAX_SIZE)]
        max_size: usize,
        #[arg(long, default_value_t = 7000)]
        timeout_ms: u64,
        #[arg(long)]
        stdout: bool,
        #[arg(long)]
        output: Option<PathBuf>,
        #[arg(long)]
        base64: bool,
        #[arg(long)]
        peek: bool,
        #[arg(long)]
        json: bool,
    },
    Peek {
        #[arg(long)]
        target: Option<String>,
        #[arg(long)]
        host: Option<String>,
        #[arg(long)]
        user: Option<String>,
        #[arg(long)]
        port: Option<u16>,
        #[arg(long)]
        identity_file: Option<PathBuf>,
        #[arg(long)]
        ssh_option: Vec<String>,
        #[arg(long)]
        ssh_bin: Option<PathBuf>,
        #[arg(long, default_value_t = DEFAULT_MAX_SIZE)]
        max_size: usize,
        #[arg(long, default_value_t = 7000)]
        timeout_ms: u64,
        #[arg(long)]
        json: bool,
    },
    #[cfg(target_os = "linux")]
    Daemon {
        #[arg(long)]
        socket_path: Option<PathBuf>,
        #[arg(long, default_value_t = DEFAULT_MAX_SIZE)]
        max_size: usize,
        #[arg(long, default_value_t = 7000)]
        io_timeout_ms: u64,
    },
    #[cfg(target_os = "linux")]
    Proxy {
        #[arg(long)]
        socket_path: Option<PathBuf>,
        #[arg(long, default_value_t = DEFAULT_MAX_SIZE)]
        max_size: usize,
        #[arg(long, default_value_t = 7000)]
        io_timeout_ms: u64,
    },

    #[cfg(all(
        feature = "agent",
        any(target_os = "windows", target_os = "macos", target_os = "linux")
    ))]
    Agent {
        #[arg(long)]
        no_tray: bool,
        #[arg(long)]
        no_hotkeys: bool,
    },

    #[cfg(all(
        feature = "agent",
        any(target_os = "windows", target_os = "macos", target_os = "linux")
    ))]
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },

    #[cfg(all(
        feature = "agent",
        any(target_os = "windows", target_os = "macos", target_os = "linux")
    ))]
    Autostart {
        #[command(subcommand)]
        command: AutostartCommands,
    },
}

#[cfg(all(
    feature = "agent",
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
#[derive(Subcommand)]
enum ConfigCommands {
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
#[derive(Subcommand)]
enum AutostartCommands {
    Enable,
    Disable,
    Status,
    Refresh,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    #[cfg(all(
        feature = "agent",
        any(target_os = "windows", target_os = "macos", target_os = "linux")
    ))]
    let agent_mode = matches!(cli.command, Commands::Agent { .. });
    #[cfg(not(all(
        feature = "agent",
        any(target_os = "windows", target_os = "macos", target_os = "linux")
    )))]
    let agent_mode = false;
    init_tracing(agent_mode)?;

    match cli.command {
        Commands::Push {
            target,
            host,
            user,
            port,
            identity_file,
            ssh_option,
            ssh_bin,
            max_size,
            timeout_ms,
            stdin,
        } => {
            let effective_max_size = if max_size == 0 {
                DEFAULT_MAX_SIZE
            } else {
                max_size
            };
            let value = match build_clipboard_value(stdin, effective_max_size).await {
                Ok(value) => value,
                Err(err) => return exit_with_code(err.code, &err.message),
            };
            let response = match send_request(
                &ClientConfig {
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
                },
                make_request(RequestKind::Set { value }),
            )
            .await
            {
                Ok(response) => response,
                Err(err) => return exit_with_code(5, &err.to_string()),
            };
            handle_response(response, false)?;
        }
        Commands::Pull {
            target,
            host,
            user,
            port,
            identity_file,
            ssh_option,
            ssh_bin,
            max_size,
            timeout_ms,
            stdout,
            output,
            base64,
            peek,
            json,
        } => {
            if stdout && output.is_some() {
                return exit_with_code(2, "use either --stdout or --output, not both");
            }
            if base64 && !stdout {
                return exit_with_code(2, "--base64 requires --stdout");
            }

            let effective_max_size = if max_size == 0 {
                DEFAULT_MAX_SIZE
            } else {
                max_size
            };

            if peek {
                let response = match send_request(
                    &ClientConfig {
                        ssh: SshConfig {
                            target: target.unwrap_or_default(),
                            port,
                            user,
                            host,
                            identity_file,
                            ssh_options: ssh_option,
                            ssh_bin,
                        },
                        max_size: effective_max_size,
                        timeout_ms,
                    },
                    make_request(RequestKind::PeekMeta),
                )
                .await
                {
                    Ok(response) => response,
                    Err(err) => return exit_with_code(5, &err.to_string()),
                };
                return handle_peek_response(response, json);
            }

            let response = match send_request(
                &ClientConfig {
                    ssh: SshConfig {
                        target: target.unwrap_or_default(),
                        port,
                        user,
                        host,
                        identity_file,
                        ssh_options: ssh_option,
                        ssh_bin,
                    },
                    max_size: effective_max_size,
                    timeout_ms,
                },
                make_request(RequestKind::Get),
            )
            .await
            {
                Ok(response) => response,
                Err(err) => return exit_with_code(5, &err.to_string()),
            };
            if !stdout && output.is_none() && !base64 {
                return handle_pull_to_clipboard(response, effective_max_size);
            }

            if let ResponseKind::Value { value } = &response.kind {
                if value.content_type == CONTENT_TYPE_TEXT {
                    let text = match String::from_utf8(value.data.clone()) {
                        Ok(text) => text,
                        Err(_) => return exit_with_code(2, "response was not valid UTF-8"),
                    };
                    if stdout {
                        println!("{text}");
                        return Ok(());
                    }
                    if let Some(path) = output {
                        if let Err(err) = fs::write(&path, text.as_bytes()) {
                            return exit_with_code(2, &format!("failed to write output: {err}"));
                        }
                        return Ok(());
                    }
                }

                if value.content_type == CONTENT_TYPE_PNG {
                    if let Some(path) = output {
                        if let Err(err) = fs::write(&path, &value.data) {
                            return exit_with_code(2, &format!("failed to write output: {err}"));
                        }
                        return Ok(());
                    }
                    if base64 && stdout {
                        let encoded = STANDARD.encode(&value.data);
                        println!("{encoded}");
                        return Ok(());
                    }
                    if stdout {
                        return exit_with_code(2, "use --base64 or --output for image data");
                    }
                }

                if let Some(path) = output {
                    if let Err(err) = fs::write(&path, &value.data) {
                        return exit_with_code(2, &format!("failed to write output: {err}"));
                    }
                    return Ok(());
                }
                if base64 && stdout {
                    let encoded = STANDARD.encode(&value.data);
                    println!("{encoded}");
                    return Ok(());
                }
                return exit_with_code(
                    2,
                    &format!("unsupported content type: {}", value.content_type),
                );
            }

            handle_response(response, false)?;
        }
        Commands::Peek {
            target,
            host,
            user,
            port,
            identity_file,
            ssh_option,
            ssh_bin,
            max_size,
            timeout_ms,
            json,
        } => {
            let response = match send_request(
                &ClientConfig {
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
                },
                make_request(RequestKind::PeekMeta),
            )
            .await
            {
                Ok(response) => response,
                Err(err) => return exit_with_code(5, &err.to_string()),
            };
            return handle_peek_response(response, json);
        }
        #[cfg(target_os = "linux")]
        Commands::Daemon {
            socket_path,
            max_size,
            io_timeout_ms,
        } => {
            let socket_path = socket_path.unwrap_or(ssh_clipboard::daemon::default_socket_path()?);
            ssh_clipboard::daemon::run_daemon(socket_path, max_size, io_timeout_ms)
                .await
                .wrap_err("daemon failed")?;
        }
        #[cfg(target_os = "linux")]
        Commands::Proxy {
            socket_path,
            max_size,
            io_timeout_ms,
        } => {
            let socket_path = socket_path.unwrap_or(ssh_clipboard::daemon::default_socket_path()?);
            let exit_code = ssh_clipboard::proxy::run_proxy(socket_path, max_size, io_timeout_ms)
                .await
                .wrap_err("proxy failed")?;
            std::process::exit(exit_code);
        }

        #[cfg(all(
            feature = "agent",
            any(target_os = "windows", target_os = "macos", target_os = "linux")
        ))]
        Commands::Agent {
            no_tray,
            no_hotkeys,
        } => {
            ssh_clipboard::agent::run::run_agent(no_tray, no_hotkeys)?;
        }

        #[cfg(all(
            feature = "agent",
            any(target_os = "windows", target_os = "macos", target_os = "linux")
        ))]
        Commands::Config { command } => match command {
            ConfigCommands::Path => {
                let path = ssh_clipboard::agent::config_path()?;
                println!("{}", path.display());
            }
            ConfigCommands::Show { json } => {
                let config = ssh_clipboard::agent::load_config()
                    .unwrap_or_else(|_| ssh_clipboard::agent::default_agent_config());
                if json {
                    println!("{}", serde_json::to_string_pretty(&config)?);
                } else {
                    println!("{config:#?}");
                }
            }
            ConfigCommands::Validate => {
                let config = ssh_clipboard::agent::load_config()
                    .unwrap_or_else(|_| ssh_clipboard::agent::default_agent_config());
                ssh_clipboard::agent::validate_config(&config)?;
                println!("ok");
            }
            ConfigCommands::Defaults => {
                let config = ssh_clipboard::agent::default_agent_config();
                println!("{}", serde_json::to_string_pretty(&config)?);
            }
        },

        #[cfg(all(
            feature = "agent",
            any(target_os = "windows", target_os = "macos", target_os = "linux")
        ))]
        Commands::Autostart { command } => match command {
            AutostartCommands::Enable => {
                ssh_clipboard::agent::autostart::enable()?;
                println!("enabled");
            }
            AutostartCommands::Disable => {
                ssh_clipboard::agent::autostart::disable()?;
                println!("disabled");
            }
            AutostartCommands::Status => {
                let enabled = ssh_clipboard::agent::autostart::is_enabled()?;
                println!("{}", if enabled { "enabled" } else { "disabled" });
            }
            AutostartCommands::Refresh => {
                ssh_clipboard::agent::autostart::refresh()?;
                println!("refreshed");
            }
        },
    }
    Ok(())
}

fn handle_response(response: Response, allow_empty: bool) -> Result<()> {
    match response.kind {
        ResponseKind::Ok => Ok(()),
        ResponseKind::Empty if allow_empty => Ok(()),
        ResponseKind::Empty => exit_with_code(2, "no clipboard value set"),
        ResponseKind::Error { code, message } => match code {
            ErrorCode::InvalidRequest | ErrorCode::InvalidUtf8 => exit_with_code(2, &message),
            ErrorCode::PayloadTooLarge => exit_with_code(3, &message),
            ErrorCode::DaemonNotRunning => exit_with_code(4, &message),
            ErrorCode::Internal => exit_with_code(2, &message),
        },
        ResponseKind::Value { .. } | ResponseKind::Meta { .. } => Ok(()),
    }
}

fn handle_peek_response(response: Response, json: bool) -> Result<()> {
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
        ResponseKind::Empty => exit_with_code(2, "no clipboard value set"),
        _ => handle_response(response, true),
    }
}

fn handle_pull_to_clipboard(response: Response, max_decoded_bytes: usize) -> Result<()> {
    match ssh_clipboard::client_actions::apply_pull_response_with_system_clipboard(
        response,
        max_decoded_bytes,
    ) {
        Ok(()) => Ok(()),
        Err(err) => match err.kind {
            PullApplyErrorKind::Clipboard => exit_with_code(6, &err.message),
            PullApplyErrorKind::NoValue => exit_with_code(2, &err.message),
            PullApplyErrorKind::InvalidUtf8
            | PullApplyErrorKind::InvalidPayload
            | PullApplyErrorKind::UnsupportedContentType
            | PullApplyErrorKind::Server
            | PullApplyErrorKind::Unexpected => exit_with_code(2, &err.message),
        },
    }
}

fn exit_with_code(code: i32, message: &str) -> Result<()> {
    eprintln!("{message}");
    std::process::exit(code);
}

async fn build_clipboard_value(
    stdin: bool,
    max_size: usize,
) -> Result<ClipboardValue, ClipboardBuildError> {
    if stdin {
        let text = read_stdin_text().await.map_err(|err| ClipboardBuildError {
            code: 2,
            message: err.to_string(),
        })?;
        return ssh_clipboard::client_actions::build_text_value(text, max_size);
    }

    ssh_clipboard::client_actions::build_clipboard_value_from_clipboard(max_size)
}

async fn read_stdin_text() -> Result<String> {
    let mut reader = BufReader::new(tokio::io::stdin());
    let mut buffer = String::new();
    reader
        .read_to_string(&mut buffer)
        .await
        .wrap_err("failed to read stdin")?;
    if buffer.is_empty() {
        return Err(eyre!("stdin was empty"));
    }
    Ok(buffer)
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
        let log_dir = ssh_clipboard::agent::config_path()
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
