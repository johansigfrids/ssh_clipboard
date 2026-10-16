use super::*;
use crate::protocol::{MAGIC, VERSION};
use std::process::Stdio;
use tokio::process::Command;

fn config() -> ClientConfig {
    ClientConfig {
        ssh: SshConfig {
            target: "fixture-only".into(),
            port: None,
            user: None,
            host: None,
            identity_file: None,
            ssh_options: vec![],
            ssh_bin: None,
        },
        max_size: 1024,
        timeout_ms: 5000,
        resync_frames: true,
        resync_max_bytes: 64,
    }
}

fn frame(kind: ResponseKind, request_id: u64) -> Vec<u8> {
    let payload = encode_message(&Response { request_id, kind }).unwrap();
    let mut bytes = MAGIC.to_vec();
    bytes.extend_from_slice(&VERSION.to_le_bytes());
    bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&payload);
    bytes
}

/// Pipe-only subprocess fixtures; no SSH, network, clipboard, or user files.
#[cfg(windows)]
fn fixture(
    reply: &[u8],
    flood: usize,
    diagnostic: &str,
    exit: u8,
    before_ms: u64,
    after_ms: u64,
) -> Command {
    use base64::{Engine, engine::general_purpose::STANDARD};
    let script = format!(
        r#"
$inputPipe = [Console]::OpenStandardInput()
$buffer = New-Object byte[] 4096
while ($inputPipe.Read($buffer, 0, $buffer.Length) -gt 0) {{}}
$errorPipe = [Console]::OpenStandardError()
for ($i = 0; $i -lt {flood}; $i += $buffer.Length) {{ $errorPipe.Write($buffer, 0, $buffer.Length) }}
$errorBytes = [Convert]::FromBase64String('{}')
$errorPipe.Write($errorBytes, 0, $errorBytes.Length)
Start-Sleep -Milliseconds {before_ms}
$replyBytes = [Convert]::FromBase64String('{}')
$outputPipe = [Console]::OpenStandardOutput()
$outputPipe.Write($replyBytes, 0, $replyBytes.Length)
$outputPipe.Flush()
Start-Sleep -Milliseconds {after_ms}
exit {exit}
"#,
        STANDARD.encode(diagnostic.as_bytes()),
        STANDARD.encode(reply)
    );
    let encoded = STANDARD.encode(
        script
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>(),
    );
    let mut command = Command::new("powershell.exe");
    command.args([
        "-NoLogo",
        "-NoProfile",
        "-NonInteractive",
        "-EncodedCommand",
        &encoded,
    ]);
    command
}

#[cfg(unix)]
fn fixture(
    reply: &[u8],
    flood: usize,
    diagnostic: &str,
    exit: u8,
    before_ms: u64,
    after_ms: u64,
) -> Command {
    fn octal(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("\\{byte:03o}")).collect()
    }
    let script = format!(
        "cat >/dev/null; dd if=/dev/zero bs=4096 count={} >&2 2>/dev/null; printf '{}' >&2; {}; printf '{}'; {}; exit {exit}",
        flood.div_ceil(4096),
        octal(diagnostic.as_bytes()),
        if before_ms == 0 {
            ":".into()
        } else {
            format!("exec sleep {}", before_ms as f64 / 1000.0)
        },
        octal(reply),
        if after_ms == 0 {
            ":".into()
        } else {
            format!("exec sleep {}", after_ms as f64 / 1000.0)
        },
    );
    let mut command = Command::new("/bin/sh");
    command.args(["-c", &script]);
    command
}

async fn run(
    mut command: Command,
    config: &ClientConfig,
    kind: RequestKind,
    deadline_ms: u64,
) -> Result<Response, TransportFailure> {
    let _permit = crate::client::SUBPROCESS_PROBES.acquire().await.unwrap();
    let started = Instant::now();
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let request = Request {
        request_id: 42,
        kind,
    };
    let result = exchange_child(
        &mut child,
        config,
        &request,
        encode_message(&request).unwrap(),
        Instant::now() + Duration::from_millis(deadline_ms),
    )
    .await;
    assert!(
        child.try_wait().unwrap().is_some(),
        "completed/failed operations must reap their child"
    );
    assert!(
        started.elapsed() < Duration::from_millis(deadline_ms + 2500),
        "operation plus cleanup exceeded its bound: {:?}",
        started.elapsed()
    );
    result
}

#[tokio::test]
async fn stderr_flood_and_eof_dependent_proxy_complete_without_deadlock() {
    let result = run(
        fixture(&frame(ResponseKind::Empty, 42), 1024 * 1024, "", 0, 0, 0),
        &config(),
        RequestKind::PeekMeta,
        5000,
    )
    .await
    .unwrap();
    assert!(matches!(result.kind, ResponseKind::Empty));
}

#[tokio::test]
async fn correlated_protocol_error_outranks_nonzero_exit_and_secret_stderr() {
    let result = run(
        fixture(
            &frame(
                ResponseKind::Error {
                    code: ErrorCode::DaemonNotRunning,
                    message: "daemon missing".into(),
                },
                42,
            ),
            1024 * 1024,
            "Permission denied PRIVATE_SENTINEL",
            4,
            0,
            0,
        ),
        &config(),
        RequestKind::PeekMeta,
        5000,
    )
    .await
    .unwrap();
    assert!(matches!(
        result.kind,
        ResponseKind::Error {
            code: ErrorCode::DaemonNotRunning,
            ..
        }
    ));
}

#[tokio::test]
async fn early_ssh_failure_classifies_bounded_stderr_without_returning_its_bytes() {
    let error = run(
        fixture(
            &[],
            1024 * 1024,
            "Permission denied (publickey) PRIVATE_SENTINEL",
            255,
            0,
            0,
        ),
        &config(),
        RequestKind::PeekMeta,
        5000,
    )
    .await
    .unwrap_err();
    assert_eq!(error.kind, FailureKind::Authentication);
    assert!(!error.to_string().contains("PRIVATE_SENTINEL"));
}

#[tokio::test]
async fn mismatched_malformed_and_oversized_replies_fail_before_being_used() {
    let mut oversized = MAGIC.to_vec();
    oversized.extend_from_slice(&VERSION.to_le_bytes());
    oversized.extend_from_slice(&u32::MAX.to_le_bytes());
    let mut version = frame(ResponseKind::Empty, 42);
    version[4..6].copy_from_slice(&(VERSION + 1).to_le_bytes());
    let mut malformed = MAGIC.to_vec();
    malformed.extend_from_slice(&VERSION.to_le_bytes());
    malformed.extend_from_slice(&1u32.to_le_bytes());
    malformed.push(255);
    for (bytes, kind) in [
        (
            frame(ResponseKind::Empty, 43),
            FailureKind::ResponseMismatch,
        ),
        (
            frame(
                ResponseKind::Value {
                    value: crate::protocol::ClipboardValue {
                        content_type: "text/plain".into(),
                        data: b"PRIVATE_SENTINEL".to_vec(),
                        created_at: 0,
                    },
                },
                42,
            ),
            FailureKind::InvalidResponse,
        ),
        (oversized, FailureKind::ResponseTooLarge),
        (version, FailureKind::ProtocolVersion),
        (malformed, FailureKind::InvalidResponse),
    ] {
        let error = run(
            fixture(&bytes, 0, "PRIVATE_SENTINEL", 0, 0, 0),
            &config(),
            RequestKind::PeekMeta,
            5000,
        )
        .await
        .unwrap_err();
        assert_eq!(error.kind, kind);
        assert!(!error.to_string().contains("PRIVATE_SENTINEL"));
    }
}

#[tokio::test]
async fn noisy_stdout_is_bounded_and_strict_framing_rejects_it() {
    let mut bytes = b"banner\n".to_vec();
    bytes.extend_from_slice(&frame(ResponseKind::Empty, 42));
    assert!(
        run(
            fixture(&bytes, 0, "", 0, 0, 0),
            &config(),
            RequestKind::PeekMeta,
            5000
        )
        .await
        .is_ok()
    );
    let mut strict = config();
    strict.resync_frames = false;
    assert_eq!(
        run(
            fixture(&bytes, 0, "", 0, 0, 0),
            &strict,
            RequestKind::PeekMeta,
            5000
        )
        .await
        .unwrap_err()
        .kind,
        FailureKind::ProtocolFraming
    );
    let bytes = vec![b'x'; 128];
    assert_eq!(
        run(
            fixture(&bytes, 0, "", 0, 0, 0),
            &config(),
            RequestKind::PeekMeta,
            5000
        )
        .await
        .unwrap_err()
        .kind,
        FailureKind::ProtocolFraming
    );
}

#[tokio::test]
async fn timeout_reaps_the_process_and_marks_an_unacknowledged_share_as_uncertain() {
    let request = RequestKind::Set {
        value: crate::protocol::ClipboardValue {
            content_type: crate::protocol::CONTENT_TYPE_TEXT.into(),
            data: b"PRIVATE_SENTINEL".to_vec(),
            created_at: 0,
        },
    };
    let error = run(fixture(&[], 0, "", 0, 5000, 0), &config(), request, 600)
        .await
        .unwrap_err();
    assert_eq!(error.kind, FailureKind::Timeout);
    assert!(error.outcome_uncertain);
    assert!(!error.to_string().contains("PRIVATE_SENTINEL"));
}

#[tokio::test]
async fn a_reply_without_process_exit_still_obeys_the_single_operation_deadline() {
    let error = run(
        fixture(&frame(ResponseKind::Empty, 42), 0, "", 0, 0, 5000),
        &config(),
        RequestKind::PeekMeta,
        1500,
    )
    .await
    .unwrap_err();
    assert_eq!(error.kind, FailureKind::Timeout);
    assert!(!error.outcome_uncertain);
}

#[tokio::test]
async fn missing_ssh_and_local_size_rejection_do_not_launch_a_process() {
    let mut config = config();
    config.ssh.ssh_bin = Some("nonexistent-ssh-clipboard-test-binary".into());
    let request = Request {
        request_id: 42,
        kind: RequestKind::PeekMeta,
    };
    assert_eq!(
        send_request(&config, request.clone())
            .await
            .unwrap_err()
            .kind,
        FailureKind::SshMissing
    );
    config.ssh.target.clear();
    assert_eq!(
        send_request(&config, request.clone())
            .await
            .unwrap_err()
            .kind,
        FailureKind::TargetMissing
    );
    config.ssh.target = "fixture-only".into();
    config.max_size = 1;
    assert!(matches!(
        send_request(&config, request).await.unwrap().kind,
        ResponseKind::Error {
            code: ErrorCode::PayloadTooLarge,
            ..
        }
    ));
}

#[test]
fn retained_stderr_memory_is_bounded_and_keeps_both_the_start_and_end() {
    let mut capture = StderrCapture::default();
    capture.observe(b"start");
    for _ in 0..100 {
        capture.observe(&[b'x'; 4096]);
    }
    capture.observe(b"end");
    assert_eq!(capture.head.len(), STDERR_PART_BYTES);
    assert_eq!(capture.tail.len(), STDERR_PART_BYTES);
    assert!(capture.head.starts_with(b"start"));
    assert!(capture.tail.ends_with(b"end"));
}
