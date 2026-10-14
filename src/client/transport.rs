use crate::client::failure::{FailureKind, TransportFailure, classify_stderr};
use crate::client::ssh::{SshConfig, spawn_ssh_proxy};
use crate::framing::{
    FramingError, decode_message, encode_message, read_frame_payload, read_frame_payload_resync,
    write_frame_payload,
};
use crate::protocol::{
    DEFAULT_MAX_SIZE, ErrorCode, RESPONSE_OVERHEAD, Request, RequestKind, Response, ResponseKind,
};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::atomic::{AtomicU64 as AtomicU64Warn, Ordering as OrderingWarn};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Child;
use tokio::time::{Duration, Instant, timeout, timeout_at};
use tracing::warn;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub ssh: SshConfig,
    pub max_size: usize,
    pub timeout_ms: u64,
    pub resync_frames: bool,
    pub resync_max_bytes: usize,
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

pub async fn send_request(
    config: &ClientConfig,
    request: Request,
) -> Result<Response, TransportFailure> {
    if crate::client::ssh::resolve_target_and_port(&config.ssh)
        .0
        .trim()
        .is_empty()
    {
        return Err(TransportFailure::new(FailureKind::TargetMissing));
    }
    let max_size = config.normalized_max_size();
    let deadline = Instant::now()
        .checked_add(Duration::from_millis(config.timeout_ms))
        .filter(|_| config.timeout_ms > 0)
        .ok_or_else(|| TransportFailure::new(FailureKind::InvalidTimeout))?;
    let payload = encode_message(&request)
        .map_err(|_| TransportFailure::new(FailureKind::RequestEncoding))?;
    if payload.len() > max_size || payload.len() > u32::MAX as usize {
        return Ok(Response {
            request_id: request.request_id,
            kind: ResponseKind::Error {
                code: ErrorCode::PayloadTooLarge,
                message: "payload too large".to_string(),
            },
        });
    }

    let mut child = spawn_ssh_proxy(&config.ssh).map_err(|error| {
        let kind = match error.downcast_ref::<std::io::Error>().map(|e| e.kind()) {
            Some(std::io::ErrorKind::NotFound) => FailureKind::SshMissing,
            _ => FailureKind::SshLaunch,
        };
        TransportFailure::new(kind)
    })?;
    exchange_child(&mut child, config, &request, payload, deadline).await
}

// Keep stderr drainage in the operation future: no detached reader survives cancellation.
async fn exchange_child(
    child: &mut Child,
    config: &ClientConfig,
    request: &Request,
    payload: Vec<u8>,
    deadline: Instant,
) -> Result<Response, TransportFailure> {
    let max_size = config.normalized_max_size();
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| TransportFailure::new(FailureKind::SshLaunch))?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| TransportFailure::new(FailureKind::SshLaunch))?;
    let mut stderr = child
        .stderr
        .take()
        .ok_or_else(|| TransportFailure::new(FailureKind::SshLaunch))?;

    let exchange = async {
        write_frame_payload(&mut stdin, &payload)
            .await
            .map_err(|_| TransportFailure::new(FailureKind::Io))?;
        stdin
            .shutdown()
            .await
            .map_err(|_| TransportFailure::new(FailureKind::Io))?;
        // EOF is part of the one-shot contract; shutdown alone does not release every pipe.
        drop(stdin);
        let response_payload = if config.resync_frames {
            let result = read_frame_payload_resync(
                &mut stdout,
                max_size.saturating_add(RESPONSE_OVERHEAD),
                config.resync_max_bytes,
            )
            .await
            .map_err(frame_failure)?;
            if result.discarded_bytes > 0 {
                warn_noisy_shell(result.discarded_bytes);
            }
            result.payload
        } else {
            read_frame_payload(&mut stdout, max_size.saturating_add(RESPONSE_OVERHEAD))
                .await
                .map_err(frame_failure)?
        };
        let response: Response = decode_message(&response_payload)
            .map_err(|_| TransportFailure::new(FailureKind::InvalidResponse))?;
        if response.request_id != request.request_id {
            return Err(TransportFailure::new(FailureKind::ResponseMismatch));
        }
        if !matches!(
            (&request.kind, &response.kind),
            (_, ResponseKind::Error { .. })
                | (RequestKind::Set { .. }, ResponseKind::Ok)
                | (
                    RequestKind::Get,
                    ResponseKind::Value { .. } | ResponseKind::Empty
                )
                | (
                    RequestKind::PeekMeta,
                    ResponseKind::Meta { .. } | ResponseKind::Empty
                )
        ) {
            return Err(TransportFailure::new(FailureKind::InvalidResponse));
        }
        Ok::<Response, TransportFailure>(response)
    };

    let mut reply = None;
    let mut captured = StderrCapture::default();
    let operation = async {
        let exchange_and_wait = async {
            let result = exchange.await;
            if result.is_err() {
                let _ = child.start_kill();
            }
            reply = Some(result);
            child.wait().await
        };
        let drain = async {
            let mut buffer = [0u8; 4096];
            loop {
                match stderr.read(&mut buffer).await {
                    Ok(0) | Err(_) => break,
                    Ok(count) => captured.observe(&buffer[..count]),
                }
            }
        };
        let (status, ()) = tokio::join!(exchange_and_wait, drain);
        status
    };
    let completion = timeout_at(deadline, operation).await;
    if completion.is_err() {
        // Reap failed/timed-out SSH children, while bounding cleanup independently.
        let _ = timeout(Duration::from_secs(2), child.kill()).await;
    }
    if let Some(Ok(response)) = &reply
        && matches!(response.kind, ResponseKind::Error { .. })
    {
        // The correlated protocol error outranks SSH's nonzero proxy exit status.
        return reply.expect("reply checked");
    }
    let failure = match completion {
        Err(_) => TransportFailure::new(FailureKind::Timeout),
        Ok(status) => match reply.unwrap_or_else(|| Err(TransportFailure::new(FailureKind::Io))) {
            Ok(response) if status.as_ref().is_ok_and(|s| s.success()) => return Ok(response),
            Ok(_) => TransportFailure::new(if status.is_err() {
                FailureKind::Io
            } else {
                FailureKind::SshExit
            }),
            Err(error) => error,
        },
    };
    let specific_protocol_error = matches!(
        failure.kind,
        FailureKind::ProtocolVersion
            | FailureKind::ResponseMismatch
            | FailureKind::InvalidResponse
            | FailureKind::ResponseTooLarge
    );
    let kind = if specific_protocol_error || failure.kind == FailureKind::Timeout {
        failure.kind
    } else {
        classify_stderr(&captured.classification_bytes()).unwrap_or(failure.kind)
    };
    Err(TransportFailure {
        kind,
        outcome_uncertain: matches!(request.kind, RequestKind::Set { .. }),
    })
}

fn frame_failure(error: eyre::Report) -> TransportFailure {
    let kind = match error.downcast_ref::<FramingError>() {
        Some(FramingError::UnsupportedVersion(_)) => FailureKind::ProtocolVersion,
        Some(FramingError::InvalidMagic | FramingError::MagicNotFound) => {
            FailureKind::ProtocolFraming
        }
        Some(FramingError::PayloadTooLarge(_)) => FailureKind::ResponseTooLarge,
        None => FailureKind::Io,
    };
    TransportFailure::new(kind)
}

const STDERR_PART_BYTES: usize = 8192;

#[derive(Default)]
struct StderrCapture {
    head: Vec<u8>,
    tail: Vec<u8>,
}

impl StderrCapture {
    fn observe(&mut self, bytes: &[u8]) {
        let head_count = (STDERR_PART_BYTES - self.head.len()).min(bytes.len());
        self.head.extend_from_slice(&bytes[..head_count]);
        if bytes.len() >= STDERR_PART_BYTES {
            self.tail.clear();
            self.tail
                .extend_from_slice(&bytes[bytes.len() - STDERR_PART_BYTES..]);
        } else {
            let excess = (self.tail.len() + bytes.len()).saturating_sub(STDERR_PART_BYTES);
            self.tail.drain(..excess);
            self.tail.extend_from_slice(bytes);
        }
    }
    fn classification_bytes(&self) -> Vec<u8> {
        let mut bytes = self.head.clone();
        bytes.push(b'\n');
        bytes.extend_from_slice(&self.tail);
        bytes
    }
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

static LAST_NOISY_WARN_MS: AtomicU64Warn = AtomicU64Warn::new(0);

fn warn_noisy_shell(discarded: usize) {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let last = LAST_NOISY_WARN_MS.load(OrderingWarn::Relaxed);
    if now_ms.saturating_sub(last) < 60_000 {
        return;
    }
    if LAST_NOISY_WARN_MS
        .compare_exchange(last, now_ms, OrderingWarn::Relaxed, OrderingWarn::Relaxed)
        .is_ok()
    {
        warn!(
            discarded_bytes = discarded,
            "discarded unexpected bytes before MAGIC (noisy shell/MOTD?)"
        );
    }
}
