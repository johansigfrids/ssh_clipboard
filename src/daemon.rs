use crate::framing::{
    FramingError, decode_message, encode_message, read_frame_payload, write_frame_payload,
};
use crate::protocol::{
    CONTENT_TYPE_PNG, CONTENT_TYPE_TEXT, ClipboardValue, ErrorCode, Request, RequestKind, Response,
    ResponseKind,
};
use eyre::{Result, WrapErr};
use std::env;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use thiserror::Error;
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::Mutex;
use tokio::time::{Duration, timeout};
use tracing::{error, info};

#[derive(Debug, Error)]
pub enum DaemonError {
    #[error("invalid content type")]
    InvalidContentType,
    #[error("invalid utf-8")]
    InvalidUtf8,
    #[error("payload too large")]
    PayloadTooLarge,
}

#[derive(Debug, Clone)]
struct ClipboardState {
    value: Option<ClipboardValue>,
}

pub fn default_socket_path() -> Result<PathBuf> {
    if let Ok(dir) = env::var("XDG_RUNTIME_DIR") {
        return Ok(Path::new(&dir).join("ssh_clipboard").join("daemon.sock"));
    }

    if let Ok(dir) = env::var("TMPDIR") {
        return Ok(Path::new(&dir)
            .join(format!("ssh_clipboard-{}", get_uid()))
            .join("daemon.sock"));
    }

    Ok(Path::new("/tmp")
        .join(format!("ssh_clipboard-{}", get_uid()))
        .join("daemon.sock"))
}

fn get_uid() -> u32 {
    unsafe { libc::getuid() }
}

pub async fn run_daemon(socket_path: PathBuf, max_size: usize, io_timeout_ms: u64) -> Result<()> {
    prepare_socket_path(&socket_path)?;
    let old_umask = set_umask();
    let listener = UnixListener::bind(&socket_path);
    unsafe { libc::umask(old_umask) };
    let listener = listener.wrap_err("bind unix socket")?;
    std::fs::set_permissions(&socket_path, std::fs::Permissions::from_mode(0o600))?;
    info!(path = %socket_path.display(), "daemon listening");

    let state = Arc::new(Mutex::new(ClipboardState { value: None }));

    loop {
        let (stream, _) = listener.accept().await?;
        let state = Arc::clone(&state);
        tokio::spawn(async move {
            if let Err(err) = handle_connection(stream, state, max_size, io_timeout_ms).await {
                error!(error = %err, "connection error");
            }
        });
    }
}

fn prepare_socket_path(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
        std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
    }

    if path.exists() {
        std::fs::remove_file(path)?;
    }

    Ok(())
}

fn set_umask() -> libc::mode_t {
    unsafe { libc::umask(0o077) }
}

async fn handle_connection(
    mut stream: UnixStream,
    state: Arc<Mutex<ClipboardState>>,
    max_size: usize,
    io_timeout_ms: u64,
) -> Result<()> {
    let payload = match timeout(
        Duration::from_millis(io_timeout_ms),
        read_frame_payload(&mut stream, max_size),
    )
    .await
    {
        Ok(Ok(payload)) => payload,
        Ok(Err(err)) => {
            let response = framing_error_response(&err, 0);
            let payload = encode_message(&response)?;
            let _ = write_frame_payload(&mut stream, &payload).await;
            return Ok(());
        }
        Err(_) => {
            let response = Response {
                request_id: 0,
                kind: ResponseKind::Error {
                    code: ErrorCode::Internal,
                    message: "read timeout".to_string(),
                },
            };
            let payload = encode_message(&response)?;
            let _ = write_frame_payload(&mut stream, &payload).await;
            return Ok(());
        }
    };
    let response = match decode_message::<Request>(&payload) {
        Ok(request) => handle_request(request, state, max_size).await,
        Err(err) => Response {
            request_id: 0,
            kind: ResponseKind::Error {
                code: ErrorCode::InvalidRequest,
                message: format!("decode error: {err}"),
            },
        },
    };
    let payload = encode_message(&response)?;
    timeout(
        Duration::from_millis(io_timeout_ms),
        write_frame_payload(&mut stream, &payload),
    )
    .await??;
    Ok(())
}

async fn handle_request(
    request: Request,
    state: Arc<Mutex<ClipboardState>>,
    max_size: usize,
) -> Response {
    let request_id = request.request_id;
    let kind = match request.kind {
        RequestKind::Get => {
            let state = state.lock().await;
            match &state.value {
                Some(value) => ResponseKind::Value {
                    value: value.clone(),
                },
                None => ResponseKind::Empty,
            }
        }
        RequestKind::PeekMeta => {
            let state = state.lock().await;
            match &state.value {
                Some(value) => ResponseKind::Meta {
                    content_type: value.content_type.clone(),
                    size: value.data.len() as u64,
                    created_at: value.created_at,
                },
                None => ResponseKind::Empty,
            }
        }
        RequestKind::Set { value } => match validate_set(&value, max_size) {
            Ok(()) => {
                let mut state = state.lock().await;
                state.value = Some(value);
                ResponseKind::Ok
            }
            Err(err) => to_error_response(err),
        },
    };
    Response { request_id, kind }
}

fn validate_set(value: &ClipboardValue, max_size: usize) -> std::result::Result<(), DaemonError> {
    if value.content_type != CONTENT_TYPE_TEXT && value.content_type != CONTENT_TYPE_PNG {
        return Err(DaemonError::InvalidContentType);
    }
    if value.data.len() > max_size {
        return Err(DaemonError::PayloadTooLarge);
    }
    if value.content_type == CONTENT_TYPE_TEXT && std::str::from_utf8(&value.data).is_err() {
        return Err(DaemonError::InvalidUtf8);
    }
    Ok(())
}

fn to_error_response(err: DaemonError) -> ResponseKind {
    match err {
        DaemonError::InvalidContentType => ResponseKind::Error {
            code: ErrorCode::InvalidRequest,
            message: "invalid content type".to_string(),
        },
        DaemonError::InvalidUtf8 => ResponseKind::Error {
            code: ErrorCode::InvalidUtf8,
            message: "invalid utf-8".to_string(),
        },
        DaemonError::PayloadTooLarge => ResponseKind::Error {
            code: ErrorCode::PayloadTooLarge,
            message: "payload too large".to_string(),
        },
    }
}

fn framing_error_response(err: &eyre::Report, request_id: u64) -> Response {
    if let Some(framing) = err.downcast_ref::<FramingError>() {
        match framing {
            FramingError::InvalidMagic | FramingError::UnsupportedVersion(_) => Response {
                request_id,
                kind: ResponseKind::Error {
                    code: ErrorCode::InvalidRequest,
                    message: format!("invalid framing: {framing}"),
                },
            },
            FramingError::PayloadTooLarge(_) => Response {
                request_id,
                kind: ResponseKind::Error {
                    code: ErrorCode::PayloadTooLarge,
                    message: format!("{framing}"),
                },
            },
        }
    } else {
        Response {
            request_id,
            kind: ResponseKind::Error {
                code: ErrorCode::Internal,
                message: format!("framing read error: {err}"),
            },
        }
    }
}
