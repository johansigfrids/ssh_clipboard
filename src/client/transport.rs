use crate::client::ssh::{spawn_ssh_proxy, SshConfig};
use crate::framing::{decode_message, encode_message, read_frame_payload, write_frame_payload};
use crate::protocol::{
    ErrorCode, Request, RequestKind, Response, ResponseKind, DEFAULT_MAX_SIZE, RESPONSE_OVERHEAD,
};
use eyre::{eyre, Result, WrapErr};
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::{timeout, Duration};

#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub ssh: SshConfig,
    pub max_size: usize,
    pub timeout_ms: u64,
}

impl ClientConfig {
    pub fn normalized_max_size(&self) -> usize {
        if self.max_size == 0 {
            DEFAULT_MAX_SIZE
        } else {
            self.max_size
        }
    }
}

pub async fn send_request(config: &ClientConfig, request: Request) -> Result<Response> {
    let max_size = config.normalized_max_size();
    let payload = encode_message(&request)?;
    if payload.len() > max_size {
        return Ok(Response {
            request_id: request.request_id,
            kind: ResponseKind::Error {
                code: ErrorCode::PayloadTooLarge,
                message: "payload too large".to_string(),
            },
        });
    }

    let mut child = spawn_ssh_proxy(&config.ssh)?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| eyre!("missing ssh stdin"))?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| eyre!("missing ssh stdout"))?;
    let mut stderr = child
        .stderr
        .take()
        .ok_or_else(|| eyre!("missing ssh stderr"))?;

    let send = async {
        write_frame_payload(&mut stdin, &payload).await?;
        stdin.shutdown().await?;
        Ok::<(), eyre::Report>(())
    };
    timeout(Duration::from_millis(config.timeout_ms), send)
        .await
        .wrap_err("ssh send timed out")??;

    let receive = async {
        let response_payload =
            read_frame_payload(&mut stdout, max_size.saturating_add(RESPONSE_OVERHEAD)).await?;
        let response: Response = decode_message(&response_payload)?;
        Ok::<Response, eyre::Report>(response)
    };
    let response = timeout(Duration::from_millis(config.timeout_ms), receive)
        .await
        .wrap_err("ssh receive timed out")??;

    let status = timeout(Duration::from_millis(config.timeout_ms), child.wait())
        .await
        .wrap_err("ssh wait timed out")?
        .wrap_err("ssh wait failed")?;
    if !status.success() {
        let mut stderr_buf = String::new();
        let _ = stderr.read_to_string(&mut stderr_buf).await;
        if let ResponseKind::Error { .. } = &response.kind {
            return Ok(response);
        }
        if stderr_buf.trim().is_empty() {
            return Err(eyre!("ssh exited with status {status}"));
        }
        return Err(eyre!("ssh error: {stderr_buf}"));
    }

    Ok(response)
}

static REQUEST_COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn new_request_id() -> u64 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let counter = REQUEST_COUNTER.fetch_add(1, Ordering::Relaxed) & 0xffff;
    (now << 16) | counter
}

pub fn make_request(kind: RequestKind) -> Request {
    Request {
        request_id: new_request_id(),
        kind,
    }
}
