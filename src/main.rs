use clap::{Parser, Subcommand};
use eyre::{Result, WrapErr, eyre};
use ssh_clipboard::client::ssh::SshConfig;
use ssh_clipboard::client::transport::{ClientConfig, send_request};
use ssh_clipboard::protocol::{
    CONTENT_TYPE_TEXT, ClipboardValue, DEFAULT_MAX_SIZE, ErrorCode, Request, Response,
};
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
    },
    #[cfg(target_os = "linux")]
    Daemon {
        #[arg(long)]
        socket_path: Option<PathBuf>,
        #[arg(long, default_value_t = DEFAULT_MAX_SIZE)]
        max_size: usize,
    },
    #[cfg(target_os = "linux")]
    Proxy {
        #[arg(long)]
        socket_path: Option<PathBuf>,
        #[arg(long, default_value_t = DEFAULT_MAX_SIZE)]
        max_size: usize,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
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
            let text = if stdin {
                match read_stdin_text().await {
                    Ok(text) => text,
                    Err(err) => return exit_with_code(2, &err.to_string()),
                }
            } else {
                match ssh_clipboard::client::clipboard::read_text() {
                    Ok(text) => text,
                    Err(err) => return exit_with_code(6, &err.to_string()),
                }
            };

            let bytes = text.into_bytes();
            if bytes.len() > effective_max_size {
                return exit_with_code(3, "payload too large");
            }

            let value = ClipboardValue {
                content_type: CONTENT_TYPE_TEXT.to_string(),
                data: bytes,
                created_at: now_epoch_millis(),
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
                Request::Set { value },
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
                Request::Get,
            )
            .await
            {
                Ok(response) => response,
                Err(err) => return exit_with_code(5, &err.to_string()),
            };
            match response {
                Response::Value { value } => {
                    let text = match String::from_utf8(value.data) {
                        Ok(text) => text,
                        Err(_) => return exit_with_code(2, "response was not valid UTF-8"),
                    };
                    if stdout {
                        println!("{text}");
                    } else if let Err(err) = ssh_clipboard::client::clipboard::write_text(&text) {
                        return exit_with_code(6, &err.to_string());
                    }
                }
                other => handle_response(other, false)?,
            }
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
                Request::PeekMeta,
            )
            .await
            {
                Ok(response) => response,
                Err(err) => return exit_with_code(5, &err.to_string()),
            };
            match response {
                Response::Meta {
                    content_type,
                    size,
                    created_at,
                } => {
                    println!("content_type={content_type} size={size} created_at={created_at}");
                }
                other => handle_response(other, true)?,
            }
        }
        #[cfg(target_os = "linux")]
        Commands::Daemon {
            socket_path,
            max_size,
        } => {
            let socket_path = socket_path.unwrap_or(ssh_clipboard::daemon::default_socket_path()?);
            ssh_clipboard::daemon::run_daemon(socket_path, max_size)
                .await
                .wrap_err("daemon failed")?;
        }
        #[cfg(target_os = "linux")]
        Commands::Proxy {
            socket_path,
            max_size,
        } => {
            let socket_path = socket_path.unwrap_or(ssh_clipboard::daemon::default_socket_path()?);
            let exit_code = ssh_clipboard::proxy::run_proxy(socket_path, max_size)
                .await
                .wrap_err("proxy failed")?;
            std::process::exit(exit_code);
        }
    }
    Ok(())
}

fn handle_response(response: Response, allow_empty: bool) -> Result<()> {
    match response {
        Response::Ok => Ok(()),
        Response::Empty if allow_empty => Ok(()),
        Response::Empty => exit_with_code(2, "no clipboard value set"),
        Response::Error { code, message } => match code {
            ErrorCode::InvalidRequest | ErrorCode::InvalidUtf8 => exit_with_code(2, &message),
            ErrorCode::PayloadTooLarge => exit_with_code(3, &message),
            ErrorCode::DaemonNotRunning => exit_with_code(4, &message),
            ErrorCode::Internal => exit_with_code(2, &message),
        },
        Response::Value { .. } | Response::Meta { .. } => Ok(()),
    }
}

fn exit_with_code(code: i32, message: &str) -> Result<()> {
    eprintln!("{message}");
    std::process::exit(code);
}

fn now_epoch_millis() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
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
