use crate::framing::{decode_message, encode_message, read_frame_payload, write_frame_payload};
use crate::protocol::{ErrorCode, RESPONSE_OVERHEAD, Request, Response, ResponseKind};
use eyre::{Result, WrapErr};
use std::path::PathBuf;
use tokio::io::{stdin, stdout};
use tokio::net::UnixStream;
use tokio::time::{Duration, timeout};

pub const EXIT_OK: i32 = 0;
pub const EXIT_INVALID_REQUEST: i32 = 2;
pub const EXIT_PAYLOAD_TOO_LARGE: i32 = 3;
pub const EXIT_DAEMON_NOT_RUNNING: i32 = 4;
pub const EXIT_INTERNAL: i32 = 5;

pub async fn run_proxy(socket_path: PathBuf, max_size: usize, io_timeout_ms: u64) -> Result<i32> {
    let mut input = stdin();
    let mut output = stdout();

    let request_payload = timeout(
        Duration::from_millis(io_timeout_ms),
        read_frame_payload(&mut input, max_size),
    )
    .await??;

    let mut stream = match timeout(
        Duration::from_millis(io_timeout_ms),
        UnixStream::connect(&socket_path),
    )
    .await
    {
        Ok(Ok(stream)) => stream,
        Ok(Err(err)) => {
            let message = format!(
                "daemon not running or socket unavailable at {}: {err}",
                socket_path.display()
            );
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
            return Ok(EXIT_DAEMON_NOT_RUNNING);
        }
        Err(_) => {
            let message = "daemon connect timed out".to_string();
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
            return Ok(EXIT_DAEMON_NOT_RUNNING);
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
        ErrorCode::Internal => EXIT_INTERNAL,
    }
}

fn request_id_from_payload(payload: &[u8]) -> u64 {
    decode_message::<Request>(payload)
        .map(|request| request.request_id)
        .unwrap_or(0)
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
