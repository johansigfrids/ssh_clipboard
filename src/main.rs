use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use clap::{Parser, Subcommand};
use eyre::{Result, WrapErr, eyre};
use ssh_clipboard::client::ssh::SshConfig;
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
                make_request(RequestKind::Get),
            )
            .await
            {
                Ok(response) => response,
                Err(err) => return exit_with_code(5, &err.to_string()),
            };
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
                    if let Err(err) = ssh_clipboard::client::clipboard::write_text(&text) {
                        return exit_with_code(6, &err.to_string());
                    }
                    return Ok(());
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
                    let image = match ssh_clipboard::client::image::decode_png(&value.data, max_size)
                    {
                        Ok(image) => image,
                        Err(err) => return exit_with_code(2, &err.to_string()),
                    };
                    if let Err(err) = ssh_clipboard::client::clipboard::write_image(image) {
                        return exit_with_code(6, &err.to_string());
                    }
                    return Ok(());
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

struct ClipboardBuildError {
    code: i32,
    message: String,
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
        return build_text_value(text, max_size);
    }

    match ssh_clipboard::client::clipboard::read_text() {
        Ok(text) => return build_text_value(text, max_size),
        Err(text_err) => match ssh_clipboard::client::clipboard::read_image() {
            Ok(image) => {
                let png = ssh_clipboard::client::image::encode_png(image).map_err(|err| {
                    ClipboardBuildError {
                        code: 2,
                        message: err.to_string(),
                    }
                })?;
                if png.len() > max_size {
                    return Err(ClipboardBuildError {
                        code: 3,
                        message: "payload too large".to_string(),
                    });
                }
                return Ok(ClipboardValue {
                    content_type: CONTENT_TYPE_PNG.to_string(),
                    data: png,
                    created_at: now_epoch_millis(),
                });
            }
            Err(image_err) => {
                return Err(ClipboardBuildError {
                    code: 6,
                    message: format!(
                        "clipboard read failed (text: {text_err}; image: {image_err})"
                    ),
                });
            }
        },
    }
}

fn build_text_value(text: String, max_size: usize) -> Result<ClipboardValue, ClipboardBuildError> {
    let bytes = text.into_bytes();
    if bytes.len() > max_size {
        return Err(ClipboardBuildError {
            code: 3,
            message: "payload too large".to_string(),
        });
    }
    Ok(ClipboardValue {
        content_type: CONTENT_TYPE_TEXT.to_string(),
        data: bytes,
        created_at: now_epoch_millis(),
    })
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
