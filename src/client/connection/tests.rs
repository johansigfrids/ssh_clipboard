use super::*;
use crate::session_protocol::{self as wire, Request, Response};
use crate::shelf::ShelfLimits;
use std::process::Stdio;
use tokio::process::Command;

#[tokio::test]
#[ignore = "requires an explicitly started disposable SSH_CLIPBOARD_TEST_TARGET/BINARY/SOCKET fixture"]
async fn live_shelf_over_ssh_reuses_one_child_for_protected_share_watch_and_reconciliation() {
    let target = std::env::var("SSH_CLIPBOARD_TEST_TARGET").expect("explicit test target required");
    let binary =
        std::env::var("SSH_CLIPBOARD_TEST_BINARY").expect("temporary test binary required");
    let socket =
        std::env::var("SSH_CLIPBOARD_TEST_SOCKET").expect("temporary test socket required");
    assert!(
        !target.is_empty()
            && !target.starts_with('-')
            && !target.chars().any(|c| c.is_whitespace() || c.is_control())
    );
    for path in [&binary, &socket] {
        assert!(
            path.starts_with("/tmp/ssh-clipboard-integration-")
                && path
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"/._-".contains(&byte)),
            "only disposable shell-safe fixture paths are accepted"
        );
    }
    let child = Command::new("ssh")
        .args([
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=8",
            &target,
            &format!("{binary} session-proxy --socket-path {socket}"),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let pid = child.id();
    let mut config = config();
    config.target = target;
    let mut policy = policy();
    policy.operation_timeout = Duration::from_secs(10);
    let mut connection = Connection::from_child(child, config, policy).await.unwrap();
    assert!(
        connection.snapshot().await.unwrap().items.is_empty(),
        "fixture must start empty"
    );
    let operation_id = Uuid::new_v4();
    let device_id = Uuid::new_v4();
    let status = connection
        .execute(
            Action::OperationStatus {
                device_id,
                operation_id,
            },
            &[],
        )
        .await
        .unwrap();
    let Response::Operation { epoch_time_ms, .. } = status.response else {
        panic!("no operation clock");
    };
    let deadline =
        (epoch_time_ms.get() + u64::from(connection.limits().unwrap().retry_retention_ms)).into();
    let data = "  synthetic SSH α\n  ".as_bytes();
    let share = || Action::RetryShare {
        share: Box::new(Action::Share {
            operation_id,
            origin: wire::Origin {
                device_id,
                name: "Disposable SSH test".into(),
            },
            representation: wire::Representation {
                id: 0,
                content_type: wire::ContentType::Text,
                encoded_bytes: data.len() as u32,
                dimensions: None,
                decoded_bytes: None,
            },
            created_at_ms: Some(123.into()),
        }),
        not_after_ms: deadline,
    };
    connection.execute(share(), data).await.unwrap();
    let snapshot = connection.snapshot().await.unwrap();
    assert_eq!(snapshot.items.len(), 1);
    let item_id = snapshot.items[0].id;
    let retry = connection.execute(share(), data).await.unwrap();
    assert!(matches!(retry.response,Response::Shared {item,duplicate:true,..} if item.id==item_id));
    let fetched = connection
        .execute(Action::Fetch { item_id }, &[])
        .await
        .unwrap();
    assert_eq!(fetched.payload, data);
    let changed = connection
        .execute(
            Action::Watch {
                after_sequence: 0.into(),
                wait_ms: 1000,
            },
            &[],
        )
        .await
        .unwrap();
    assert!(matches!(changed.response,Response::Changed {sequence,..} if sequence.get()>=1));
    connection
        .execute(Action::Delete { item_id }, &[])
        .await
        .unwrap();
    let status = connection
        .execute(
            Action::OperationStatus {
                device_id,
                operation_id,
            },
            &[],
        )
        .await
        .unwrap();
    assert!(
        matches!(status.response,Response::Operation {item:Some(item),available:false,..} if item.id==item_id)
    );
    assert!(connection.snapshot().await.unwrap().items.is_empty());
    assert_eq!(connection.child.as_ref().unwrap().id(), pid);
    connection.close().await;
}

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
    let _slot = crate::client::SUBPROCESS_PROBES.acquire().await.unwrap();
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
    let _slot = crate::client::SUBPROCESS_PROBES.acquire().await.unwrap();
    // This budget also covers starting Windows PowerShell and draining its stderr
    // flood. A short startup budget can fail before the mutation under test.
    let mut connection = Connection::from_child(fixture(true), config(), policy())
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
