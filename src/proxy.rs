use crate::framing::{decode_message, encode_message, read_frame_payload, write_frame_payload};
use crate::protocol::{ErrorCode, RESPONSE_OVERHEAD, Request, Response, ResponseKind};
use eyre::{Result, WrapErr};
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{stdin, stdout};
use tokio::net::UnixStream;
use tokio::time::{Duration, timeout};

pub const EXIT_OK: i32 = 0;
pub const EXIT_INVALID_REQUEST: i32 = 2;
pub const EXIT_PAYLOAD_TOO_LARGE: i32 = 3;
pub const EXIT_DAEMON_NOT_RUNNING: i32 = 4;
pub const EXIT_INTERNAL: i32 = 5;

pub async fn run_proxy(
    socket_path: PathBuf,
    max_size: usize,
    io_timeout_ms: u64,
    autostart_daemon: bool,
) -> Result<i32> {
    let mut input = stdin();
    let mut output = stdout();

    let request_payload = timeout(
        Duration::from_millis(io_timeout_ms),
        read_frame_payload(&mut input, max_size),
    )
    .await??;

    let mut stream =
        match connect_daemon(&socket_path, io_timeout_ms, autostart_daemon, max_size).await {
            Ok(stream) => stream,
            Err(err) => {
                let (message, code) = match err {
                    ConnectError::Timeout => (
                        "daemon connect timed out".to_string(),
                        EXIT_DAEMON_NOT_RUNNING,
                    ),
                    ConnectError::Failed(message) => (message, EXIT_DAEMON_NOT_RUNNING),
                    ConnectError::AutostartFailed(message) => (message, EXIT_DAEMON_NOT_RUNNING),
                };
                eprintln!("{message}");
                let response = Response {
                    request_id: request_id_from_payload(&request_payload),
                    kind: ResponseKind::Error {
                        code: ErrorCode::DaemonNotRunning,
                        message,
                    },
                };
                let payload = encode_message(&response)?;
                write_frame_payload(&mut output, &payload).await?;
                return Ok(code);
            }
        };

    timeout(
        Duration::from_millis(io_timeout_ms),
        write_frame_payload(&mut stream, &request_payload),
    )
    .await??;
    let response_payload = timeout(
        Duration::from_millis(io_timeout_ms),
        read_frame_payload(&mut stream, max_size + RESPONSE_OVERHEAD),
    )
    .await
    .wrap_err("read response from daemon timed out")??;

    let exit_code = match decode_message::<Response>(&response_payload) {
        Ok(Response {
            kind: ResponseKind::Error { code, message },
            ..
        }) => {
            eprintln!("{message}");
            map_error_code(code)
        }
        Ok(_) => EXIT_OK,
        Err(err) => {
            eprintln!("failed to decode response: {err}");
            EXIT_INTERNAL
        }
    };

    write_frame_payload(&mut output, &response_payload).await?;
    Ok(exit_code)
}

fn map_error_code(code: ErrorCode) -> i32 {
    match code {
        ErrorCode::InvalidRequest => EXIT_INVALID_REQUEST,
        ErrorCode::PayloadTooLarge => EXIT_PAYLOAD_TOO_LARGE,
        ErrorCode::InvalidUtf8 => EXIT_INVALID_REQUEST,
        ErrorCode::DaemonNotRunning => EXIT_DAEMON_NOT_RUNNING,
        ErrorCode::VersionMismatch => EXIT_INVALID_REQUEST,
        ErrorCode::Internal => EXIT_INTERNAL,
    }
}

fn request_id_from_payload(payload: &[u8]) -> u64 {
    decode_message::<Request>(payload)
        .map(|request| request.request_id)
        .unwrap_or(0)
}

enum ConnectError {
    Timeout,
    Failed(String),
    AutostartFailed(String),
}

async fn connect_daemon(
    socket_path: &PathBuf,
    io_timeout_ms: u64,
    autostart: bool,
    max_size: usize,
) -> Result<UnixStream, ConnectError> {
    let mut attempts = 0usize;
    let mut started = false;
    loop {
        attempts += 1;
        match timeout(
            Duration::from_millis(io_timeout_ms),
            UnixStream::connect(socket_path),
        )
        .await
        {
            Ok(Ok(stream)) => return Ok(stream),
            Ok(Err(err)) => {
                if autostart && !started {
                    if let Err(err) = spawn_daemon(socket_path, max_size, io_timeout_ms) {
                        return Err(ConnectError::AutostartFailed(format!(
                            "daemon autostart failed: {err}"
                        )));
                    }
                    started = true;
                } else if !autostart || attempts >= 3 {
                    return Err(ConnectError::Failed(format!(
                        "daemon not running or socket unavailable at {}: {err}",
                        socket_path.display()
                    )));
                }
            }
            Err(_) => {
                if !autostart || attempts >= 3 {
                    return Err(ConnectError::Timeout);
                }
            }
        }

        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

fn spawn_daemon(socket_path: &PathBuf, max_size: usize, io_timeout_ms: u64) -> Result<()> {
    let exe = std::env::current_exe()?;
    let mut cmd = std::process::Command::new(exe);
    cmd.arg("daemon")
        .arg("--socket-path")
        .arg(socket_path)
        .arg("--max-size")
        .arg(max_size.to_string())
        .arg("--io-timeout-ms")
        .arg(io_timeout_ms.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(unix)]
    unsafe {
        cmd.pre_exec(|| {
            if libc::setsid() == -1 {
                return Err(std::io::Error::last_os_error());
            }
            libc::signal(libc::SIGHUP, libc::SIG_IGN);
            Ok(())
        });
    }
    cmd.spawn()?;
    Ok(())
}

#[cfg(test)]
mod connect_tests {
    use super::*;

    #[tokio::test]
    async fn autostart_attempts_on_first_failure() {
        let mut connect_calls = 0usize;
        let mut spawn_calls = 0usize;

        let result: Result<(), ConnectError> = connect_with_autostart_test(
            || {
                connect_calls += 1;
                async { Err(ConnectError::Failed("nope".to_string())) }
            },
            || {
                spawn_calls += 1;
                Ok(())
            },
        )
        .await;

        assert!(result.is_err());
        assert!(spawn_calls >= 1);
        assert!(connect_calls >= 1);
    }

    async fn connect_with_autostart_test<C, CFut, S>(
        mut connect: C,
        mut spawn: S,
    ) -> Result<(), ConnectError>
    where
        C: FnMut() -> CFut,
        CFut: std::future::Future<Output = Result<(), ConnectError>>,
        S: FnMut() -> Result<()>,
    {
        let mut started = false;
        for attempt in 0..3 {
            match connect().await {
                Ok(()) => return Ok(()),
                Err(err) => {
                    if !started {
                        spawn().map_err(|err| ConnectError::AutostartFailed(err.to_string()))?;
                        started = true;
                    } else if attempt >= 2 {
                        return Err(err);
                    }
                }
            }
        }
        Err(ConnectError::Failed("exhausted".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framing::{read_frame_payload, write_frame_payload};
    use tokio::io::duplex;

    #[tokio::test]
    async fn accepts_response_slightly_over_max_size() {
        let max_size = 256usize;
        let payload = vec![0u8; max_size + 1];
        let (mut writer, mut reader) = duplex(1024);

        write_frame_payload(&mut writer, &payload).await.unwrap();
        let received = read_frame_payload(&mut reader, max_size + RESPONSE_OVERHEAD)
            .await
            .unwrap();

        assert_eq!(received.len(), max_size + 1);
    }
}
