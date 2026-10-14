use super::*;
use crate::session_protocol::{self as wire, Request, Response};
use crate::shelf::ShelfLimits;
use std::process::Stdio;
use tokio::process::Command;

fn policy() -> SessionPolicy {
    SessionPolicy {
        max_item_bytes: 1024,
        max_decoded_bytes: 1024,
        operation_timeout: Duration::from_secs(10),
    }
}
fn config() -> SshConfig {
    SshConfig {
        target: "synthetic-only".into(),
        port: None,
        user: None,
        host: None,
        identity_file: None,
        ssh_options: vec![],
        ssh_bin: None,
    }
}
fn frame(response: &Response) -> Vec<u8> {
    let data = serde_json::to_vec(response).unwrap();
    let mut bytes = b"SCBS\x01\0\0\0".to_vec();
    bytes.extend_from_slice(&(data.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend(data);
    bytes
}
fn hello() -> Request {
    Request::Hello {
        request_id: 1.into(),
        versions: vec![1],
        max_item_bytes: 1024,
        max_decoded_bytes: 1024,
    }
}
fn responses() -> Vec<Vec<u8>> {
    let mut responses = vec![frame(
        &wire::negotiate(&hello(), Uuid::from_u128(1), 0, &ShelfLimits::default()).unwrap(),
    )];
    responses.extend((2..=4).map(|id| {
        frame(&Response::Health {
            request_id: id.into(),
            epoch: Uuid::from_u128(1),
            sequence: 0.into(),
            items: 0,
            stored_bytes: 0,
        })
    }));
    responses
}
#[cfg(windows)]
fn fixture(stall: bool) -> Child {
    use base64::{Engine, engine::general_purpose::STANDARD};
    let mut frames = responses();
    if stall {
        frames.truncate(1);
    }
    let exchanges=frames.iter().map(|bytes|format!("Read-Frame; $bytes=[Convert]::FromBase64String('{}'); $outPipe.Write($bytes,0,$bytes.Length); $outPipe.Flush()",STANDARD.encode(bytes))).collect::<Vec<_>>().join("\n");
    let script = format!(
        r#"
$inPipe=[Console]::OpenStandardInput(); $outPipe=[Console]::OpenStandardOutput(); $errPipe=[Console]::OpenStandardError()
function Read-Bytes([int]$length) {{ $bytes=New-Object byte[] $length; $offset=0; while($offset -lt $length) {{ $count=$inPipe.Read($bytes,$offset,$length-$offset); if($count -eq 0) {{exit 8}}; $offset+=$count }}; return ,$bytes }}
function Read-Frame {{ $header=Read-Bytes 16; $null=Read-Bytes ([BitConverter]::ToUInt32($header,8)+[BitConverter]::ToUInt32($header,12)) }}
$noise=New-Object byte[] 4096; for($i=0;$i -lt 32;$i++) {{$errPipe.Write($noise,0,$noise.Length)}}
{exchanges}
{}
$tail=New-Object byte[] 4096; while($inPipe.Read($tail,0,$tail.Length) -gt 0) {{}}
exit 0
"#,
        if stall { "Read-Frame" } else { "" }
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
fn fixture(stall: bool) -> Child {
    let mut script = "dd if=/dev/zero bs=4096 count=32 >&2 2>/dev/null; ".to_owned();
    let mut frames = responses();
    if stall {
        frames.truncate(1);
    }
    for (index, response) in frames.iter().enumerate() {
        let request = if index == 0 {
            hello()
        } else {
            Request::Health {
                request_id: (index as u64 + 1).into(),
                epoch: Uuid::from_u128(1),
            }
        };
        let length = 16 + serde_json::to_vec(&request).unwrap().len();
        let octal = response
            .iter()
            .map(|byte| format!("\\{byte:03o}"))
            .collect::<String>();
        script.push_str(&format!(
            "dd bs=1 count={length} of=/dev/null 2>/dev/null; printf '{octal}'; "
        ));
    }
    if stall {
        let request = Request::Clear {
            request_id: 2.into(),
            epoch: Uuid::from_u128(1),
        };
        let length = 16 + serde_json::to_vec(&request).unwrap().len();
        script.push_str(&format!(
            "dd bs=1 count={length} of=/dev/null 2>/dev/null; "
        ));
    }
    script.push_str("cat >/dev/null; exit 0");
    Command::new("/bin/sh")
        .args(["-c", &script])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap()
}

#[tokio::test]
async fn one_child_and_bounded_reader_survive_multiple_metadata_operations_until_explicit_close() {
    let child = fixture(false);
    let pid = child.id();
    let mut connection = Connection::from_child(child, config(), policy())
        .await
        .unwrap();
    assert!(connection.matches(&config(), policy()));
    assert!(connection.is_ready());
    assert_eq!(connection.epoch(), Some(Uuid::from_u128(1)));
    for _ in 0..3 {
        let reply = connection.execute(Action::Health, &[]).await.unwrap();
        assert!(matches!(reply.response, Response::Health { .. }));
        assert_eq!(connection.child.as_ref().unwrap().id(), pid);
    }
    assert!(connection.is_ready());
    assert!(
        connection
            .diagnostics
            .lock()
            .unwrap()
            .classification_bytes()
            .len()
            <= 16385
    );
    connection.close().await;
}

#[tokio::test]
async fn an_unacknowledged_mutation_keeps_its_epoch_and_reaps_the_failed_connection() {
    let policy = SessionPolicy {
        operation_timeout: Duration::from_secs(3),
        ..policy()
    };
    let mut connection = Connection::from_child(fixture(true), config(), policy)
        .await
        .unwrap();
    assert!(
        matches!(connection.execute(Action::Clear,&[]).await, Err(RunError::Uncertain {source:SessionError::Timeout,epoch,..}) if epoch==Uuid::from_u128(1))
    );
    assert!(!connection.is_ready());
    assert!(
        connection
            .child
            .as_mut()
            .unwrap()
            .try_wait()
            .unwrap()
            .is_some()
    );
    assert!(connection.diagnostic_reader.is_none());
}

#[tokio::test]
async fn dropping_a_reader_taken_for_cleanup_still_aborts_its_task() {
    struct Signal(Option<tokio::sync::oneshot::Sender<()>>);
    impl Drop for Signal {
        fn drop(&mut self) {
            if let Some(signal) = self.0.take() {
                let _ = signal.send(());
            }
        }
    }
    let (finished, done) = tokio::sync::oneshot::channel();
    let signal = Signal(Some(finished));
    let task = tokio::spawn(async move {
        let _signal = signal;
        std::future::pending::<()>().await;
    });
    let reader = DiagnosticReader(task);
    drop(reader);
    timeout(Duration::from_secs(1), done)
        .await
        .unwrap()
        .unwrap();
}
