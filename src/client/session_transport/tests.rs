use super::*;
use crate::session_protocol::{self as wire, Response};
use crate::shelf::ShelfLimits;
use std::process::Stdio;
use tokio::process::Command;
use uuid::Uuid;

fn frame(response: &Response) -> Vec<u8> {
    let data = serde_json::to_vec(response).unwrap();
    let mut bytes = b"SCBS\x01\0\0\0".to_vec();
    bytes.extend_from_slice(&(data.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend(data);
    bytes
}
fn welcome() -> Vec<u8> {
    frame(
        &wire::negotiate(
            &wire::Request::Hello {
                request_id: 1.into(),
                versions: vec![1],
                max_item_bytes: 1024,
                max_decoded_bytes: 1024,
            },
            Uuid::from_u128(1),
            0,
            &ShelfLimits::default(),
        )
        .unwrap(),
    )
}
#[cfg(windows)]
fn fixture(reply: &[u8], hold: bool) -> Child {
    use base64::{Engine, engine::general_purpose::STANDARD};
    let script = format!(
        r#"
$inputPipe = [Console]::OpenStandardInput()
$outputPipe = [Console]::OpenStandardOutput()
$errorPipe = [Console]::OpenStandardError()
function Read-Bytes([int]$length) {{
  $bytes = New-Object byte[] $length
  $offset = 0
  while ($offset -lt $length) {{ $count = $inputPipe.Read($bytes, $offset, $length - $offset); if ($count -eq 0) {{ exit 8 }}; $offset += $count }}
  return ,$bytes
}}
function Read-Frame {{
  $header = Read-Bytes 16
  $null = Read-Bytes ([BitConverter]::ToUInt32($header, 8) + [BitConverter]::ToUInt32($header, 12))
}}
Read-Frame
$bytes = [Convert]::FromBase64String('{}')
$outputPipe.Write($bytes, 0, $bytes.Length); $outputPipe.Flush()
Read-Frame
$noise = New-Object byte[] 4096
for ($i = 0; $i -lt 80; $i++) {{ $errorPipe.Write($noise, 0, $noise.Length) }}
$noise = [Text.Encoding]::UTF8.GetBytes('PRIVATE-SENTINEL')
$errorPipe.Write($noise, 0, $noise.Length)
$bytes = [Convert]::FromBase64String('{}')
$outputPipe.Write($bytes, 0, $bytes.Length); $outputPipe.Flush()
{}
exit 9
"#,
        STANDARD.encode(welcome()),
        STANDARD.encode(reply),
        if hold { "Start-Sleep -Seconds 20" } else { "" }
    );
    let encoded = STANDARD.encode(
        script
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>(),
    );
    Command::new("powershell.exe")
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-EncodedCommand",
            &encoded,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap()
}
#[cfg(unix)]
fn fixture(reply: &[u8], hold: bool) -> Child {
    let octal = |bytes: &[u8]| {
        bytes
            .iter()
            .map(|byte| format!("\\{byte:03o}"))
            .collect::<String>()
    };
    // These control requests carry no payload. Read the known Hello and either
    // Health or Clear length; Share timeout fixtures need only drain some bytes.
    let hello = wire::Request::Hello {
        request_id: 1.into(),
        versions: vec![1],
        max_item_bytes: 1024,
        max_decoded_bytes: 1024,
    };
    let hello_len = 16 + serde_json::to_vec(&hello).unwrap().len();
    let script = format!(
        "dd bs=1 count={hello_len} of=/dev/null 2>/dev/null; printf '{}'; dd bs=1 count=16 of=/dev/null 2>/dev/null; dd if=/dev/zero bs=4096 count=80 >&2 2>/dev/null; printf 'PRIVATE-SENTINEL' >&2; printf '{}'; {}; exit 9",
        octal(&welcome()),
        octal(reply),
        if hold { "exec sleep 20" } else { ":" }
    );
    Command::new("/bin/sh")
        .args(["-c", &script])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap()
}
fn policy() -> SessionPolicy {
    SessionPolicy {
        max_item_bytes: 1024,
        max_decoded_bytes: 1024,
        operation_timeout: Duration::from_secs(3),
    }
}
static PROCESSES: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(2);

#[tokio::test]
async fn stderr_flood_and_nonzero_exit_do_not_erase_a_validated_reply() {
    let _slot = PROCESSES.acquire().await.unwrap();
    let reply = frame(&Response::Health {
        request_id: 2.into(),
        epoch: Uuid::from_u128(1),
        sequence: 0.into(),
        items: 0,
        stored_bytes: 0,
    });
    let outcome = run_child(
        fixture(&reply, false),
        policy(),
        Operation::Action(Action::Health),
        &[],
    )
    .await
    .unwrap();
    assert!(matches!(
        outcome,
        Outcome::Reply(Reply {
            response: Response::Health { items: 0, .. },
            ..
        })
    ));
}
#[tokio::test]
async fn correlated_remote_errors_outrank_exit_status_and_never_expose_stderr() {
    let _slot = PROCESSES.acquire().await.unwrap();
    let reply = frame(&Response::Error {
        request_id: 2.into(),
        code: wire::ErrorCode::Busy,
        epoch: Some(Uuid::from_u128(1)),
    });
    let error = match run_child(
        fixture(&reply, false),
        policy(),
        Operation::Action(Action::Clear),
        &[],
    )
    .await
    {
        Err(error) => error,
        Ok(_) => panic!("unexpected success"),
    };
    assert!(matches!(
        error,
        RunError::Session(SessionError::Remote(wire::ErrorCode::Busy))
    ));
    assert!(!error.to_string().contains("PRIVATE-SENTINEL"));
}
#[tokio::test]
async fn unacknowledged_mutations_timeout_and_reap_without_replaying() {
    let _slot = PROCESSES.acquire().await.unwrap();
    let start = tokio::time::Instant::now();
    let result = run_child(
        fixture(&[], true),
        policy(),
        Operation::Action(Action::Clear),
        &[],
    )
    .await;
    assert!(matches!(
        result,
        Err(RunError::Uncertain {source:SessionError::Timeout, epoch, ..}) if epoch == Uuid::from_u128(1)
    ));
    assert!(start.elapsed() < Duration::from_secs(6));
}

#[tokio::test]
async fn acknowledged_mutations_remain_confirmed_when_process_cleanup_hits_the_deadline() {
    let _slot = PROCESSES.acquire().await.unwrap();
    let reply = frame(&Response::Cleared {
        request_id: 2.into(),
        epoch: Uuid::from_u128(1),
        sequence: 1.into(),
    });
    let result = run_child(
        fixture(&reply, true),
        policy(),
        Operation::Action(Action::Clear),
        &[],
    )
    .await
    .unwrap();
    assert!(matches!(
        result,
        Outcome::Reply(Reply {
            response: Response::Cleared { .. },
            ..
        })
    ));
}
#[tokio::test]
async fn explicit_session_selection_rejects_missing_or_option_targets_before_launch() {
    for target in ["", "-F", "two hosts"] {
        let config = SshConfig {
            target: target.into(),
            port: None,
            user: None,
            host: None,
            identity_file: None,
            ssh_options: vec![],
            ssh_bin: Some("does-not-exist".into()),
        };
        assert!(matches!(
            run(&config, policy(), Operation::Action(Action::Health), &[]).await,
            Err(RunError::Transport(TransportFailure {
                kind: FailureKind::TargetMissing,
                ..
            }))
        ));
    }
}
