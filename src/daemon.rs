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
use std::os::unix::io::AsRawFd;
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
    if let Err(err) = verify_peer_credentials(&stream) {
        let response = Response {
            request_id: 0,
            kind: ResponseKind::Error {
                code: ErrorCode::InvalidRequest,
                message: format!("peer credential check failed: {err}"),
            },
        };
        let payload = encode_message(&response)?;
        let _ = write_frame_payload(&mut stream, &payload).await;
        return Ok(());
    }

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
            FramingError::InvalidMagic | FramingError::MagicNotFound => Response {
                request_id,
                kind: ResponseKind::Error {
                    code: ErrorCode::InvalidRequest,
                    message: format!("invalid framing: {framing}"),
                },
            },
            FramingError::UnsupportedVersion(_) => Response {
                request_id,
                kind: ResponseKind::Error {
                    code: ErrorCode::VersionMismatch,
                    message: format!("version mismatch: {framing}"),
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

fn verify_peer_credentials(stream: &UnixStream) -> Result<()> {
    let expected = get_uid();
    let actual = peer_uid(stream)?;
    if !peer_uid_matches(actual, expected) {
        return Err(eyre::eyre!(
            "peer uid mismatch (expected {expected}, got {actual})"
        ));
    }
    Ok(())
}

fn peer_uid_matches(actual: u32, expected: u32) -> bool {
    actual == expected
}

fn peer_uid(stream: &UnixStream) -> Result<u32> {
    let fd = stream.as_raw_fd();
    let mut cred: libc::ucred = unsafe { std::mem::zeroed() };
    let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    let ret = unsafe {
        libc::getsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            &mut cred as *mut _ as *mut libc::c_void,
            &mut len as *mut _,
        )
    };
    if ret != 0 {
        return Err(eyre::eyre!("getsockopt SO_PEERCRED failed"));
    }
    Ok(cred.uid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framing::{decode_message, read_frame_payload};
    use std::os::unix::fs::PermissionsExt;
    use tokio::net::UnixListener;
    use tokio::time::Duration;

    #[tokio::test]
    async fn read_timeout_returns_error_response() {
        let dir = tempfile::tempdir().unwrap();
        let socket_path = dir.path().join("daemon.sock");
        let listener = UnixListener::bind(&socket_path).unwrap();
        let state = Arc::new(Mutex::new(ClipboardState { value: None }));

        let server = tokio::spawn({
            let state = Arc::clone(&state);
            async move {
                let (stream, _) = listener.accept().await.unwrap();
                handle_connection(stream, state, 1024, 10).await.unwrap();
            }
        });

        let mut client = UnixStream::connect(&socket_path).await.unwrap();
        let response_payload = tokio::time::timeout(
            Duration::from_millis(200),
            read_frame_payload(&mut client, 2048),
        )
        .await
        .unwrap()
        .unwrap();

        let response: Response = decode_message(&response_payload).unwrap();
        match response.kind {
            ResponseKind::Error { code, message } => {
                assert!(matches!(code, ErrorCode::Internal));
                assert!(message.contains("read timeout"));
            }
            other => panic!("unexpected response: {other:?}"),
        }

        server.await.unwrap();
    }

    #[test]
    fn prepare_socket_path_sets_directory_permissions() {
        let dir = tempfile::tempdir().unwrap();
        let socket_path = dir.path().join("runtime").join("daemon.sock");

        prepare_socket_path(&socket_path).unwrap();

        let parent = socket_path.parent().unwrap();
        let mode = std::fs::metadata(parent).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o700);
    }

    #[tokio::test]
    async fn validate_set_rejects_invalid_utf8() {
        let value = ClipboardValue {
            content_type: CONTENT_TYPE_TEXT.to_string(),
            data: vec![0xff, 0xfe],
            created_at: 0,
        };
        let err = validate_set(&value, 1024).unwrap_err();
        assert!(matches!(err, DaemonError::InvalidUtf8));
    }

    #[tokio::test]
    async fn validate_set_rejects_invalid_content_type() {
        let value = ClipboardValue {
            content_type: "application/octet-stream".to_string(),
            data: vec![1, 2, 3],
            created_at: 0,
        };
        let err = validate_set(&value, 1024).unwrap_err();
        assert!(matches!(err, DaemonError::InvalidContentType));
    }

    #[tokio::test]
    async fn validate_set_rejects_oversize() {
        let value = ClipboardValue {
            content_type: CONTENT_TYPE_TEXT.to_string(),
            data: vec![b'a'; 5],
            created_at: 0,
        };
        let err = validate_set(&value, 4).unwrap_err();
        assert!(matches!(err, DaemonError::PayloadTooLarge));
    }

    #[tokio::test]
    async fn handle_request_preserves_request_id() {
        let state = Arc::new(Mutex::new(ClipboardState { value: None }));
        let request = Request {
            request_id: 7,
            kind: RequestKind::Get,
        };
        let response = handle_request(request, state, 1024).await;
        assert_eq!(response.request_id, 7);
    }

    #[test]
    fn peer_uid_match_helper() {
        assert!(peer_uid_matches(1000, 1000));
        assert!(!peer_uid_matches(1001, 1000));
    }
}
