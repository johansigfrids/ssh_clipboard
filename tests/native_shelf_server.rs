#![cfg(target_os = "linux")]
//! Real child-process/UNIX-socket probes. Synthetic data, no SSH, clipboard, services or user settings.
use ssh_clipboard::session_protocol::framing::{self, FrameLimits};
use ssh_clipboard::session_protocol::{
    ContentType, ErrorCode, Origin, Representation, Request, Response,
};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::process::Command as AsyncCommand;
use uuid::Uuid;

struct Daemon(Child);
impl Daemon {
    fn stop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
impl Drop for Daemon {
    fn drop(&mut self) {
        self.stop();
    }
}

async fn daemon(path: &Path) -> Daemon {
    daemon_mode(path, "shelf-daemon").await
}

async fn daemon_mode(path: &Path, mode: &str) -> Daemon {
    let child = Command::new(env!("CARGO_BIN_EXE_ssh_clipboard"))
        .arg(mode)
        .arg("--socket-path")
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut daemon = Daemon(child);
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            assert!(
                daemon.0.try_wait().unwrap().is_none(),
                "disposable daemon exited during startup"
            );
            if UnixStream::connect(path).await.is_ok() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    daemon
}

fn proxy(path: &Path) -> tokio::process::Child {
    AsyncCommand::new(env!("CARGO_BIN_EXE_ssh_clipboard"))
        .arg("session-proxy")
        .arg("--socket-path")
        .arg(path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .unwrap()
}

async fn receive(
    output: &mut tokio::process::ChildStdout,
    limits: FrameLimits,
) -> (Response, Vec<u8>) {
    tokio::time::timeout(Duration::from_secs(3), async {
        let header = framing::read_header(output, limits).await.unwrap();
        let response: Response = framing::read_metadata(output, &header).await.unwrap();
        response.validate(header.payload_bytes()).unwrap();
        let mut data = vec![0; header.payload_bytes()];
        output.read_exact(&mut data).await.unwrap();
        (response, data)
    })
    .await
    .unwrap()
}

async fn hello(
    input: &mut tokio::process::ChildStdin,
    output: &mut tokio::process::ChildStdout,
) -> Response {
    let request = Request::Hello {
        request_id: 1.into(),
        versions: vec![1],
        max_item_bytes: 1024,
        max_decoded_bytes: 1024,
    };
    framing::write_message(input, &request, &[], FrameLimits::bootstrap())
        .await
        .unwrap();
    receive(output, FrameLimits::bootstrap()).await.0
}

#[tokio::test]
async fn native_proxy_preserves_bytes_and_exits_on_daemon_eof_with_stdin_held_open() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("runtime with spaces/shelf.sock");
    let mut server = daemon(&path).await;
    let mut child = proxy(&path);
    // Taking stdin prevents Child::wait from closing it and masking a runtime-shutdown bug.
    let mut input = child.stdin.take().unwrap();
    let mut output = child.stdout.take().unwrap();
    let Response::Welcome { epoch, limits, .. } = hello(&mut input, &mut output).await else {
        panic!("not welcomed")
    };
    let limits = limits.frame_limits().unwrap();
    let bytes = " synthetic α\n  ".as_bytes();
    let request = Request::Share {
        request_id: 2.into(),
        epoch,
        operation_id: Uuid::from_u128(1),
        origin: Origin {
            device_id: Uuid::from_u128(2),
            name: "Synthetic".into(),
        },
        representation: Representation {
            id: 0,
            content_type: ContentType::Text,
            encoded_bytes: bytes.len() as u32,
            dimensions: None,
            decoded_bytes: None,
        },
        created_at_ms: Some(123.into()),
    };
    framing::write_message(&mut input, &request, bytes, limits)
        .await
        .unwrap();
    let Response::Shared { item, .. } = receive(&mut output, limits).await.0 else {
        panic!("not shared")
    };
    let duplicate = AsyncCommand::new(env!("CARGO_BIN_EXE_ssh_clipboard"))
        .arg("shelf-daemon")
        .arg("--socket-path")
        .arg(&path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .unwrap();
    assert!(!duplicate.success());
    let request = Request::Fetch {
        request_id: 3.into(),
        epoch,
        item_id: item.id,
        representation_id: 0,
    };
    framing::write_message(&mut input, &request, &[], limits)
        .await
        .unwrap();
    assert_eq!(receive(&mut output, limits).await.1, bytes);
    server.stop();
    let status = tokio::time::timeout(Duration::from_secs(3), child.wait())
        .await
        .expect("proxy must exit even while its stdin remains open")
        .unwrap();
    assert!(status.success());
    drop(input);
    // A killed process leaves a stale socket, but ownership recovery starts a fresh epoch.
    let _restarted = daemon(&path).await;
    let mut next = proxy(&path);
    let mut input = next.stdin.take().unwrap();
    let mut output = next.stdout.take().unwrap();
    let Response::Welcome {
        epoch: next_epoch, ..
    } = hello(&mut input, &mut output).await
    else {
        panic!("not welcomed")
    };
    assert_ne!(epoch, next_epoch);
    input.shutdown().await.unwrap();
    drop(input); // Closing the owned pipe, not shutdown alone, delivers stdin EOF.
    assert!(
        tokio::time::timeout(Duration::from_secs(3), next.wait())
            .await
            .unwrap()
            .unwrap()
            .success()
    );
}

#[tokio::test]
async fn missing_daemon_returns_a_correlated_frame_and_exits_without_stdin_eof() {
    let dir = tempfile::tempdir().unwrap();
    let mut child = proxy(&dir.path().join("missing.sock"));
    let mut input = child.stdin.take().unwrap();
    let mut output = child.stdout.take().unwrap();
    assert!(
        matches!(hello(&mut input, &mut output).await, Response::Error {request_id, code: ErrorCode::DaemonUnavailable, ..} if request_id.get() == 1)
    );
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(3), child.wait())
            .await
            .unwrap()
            .unwrap()
            .code(),
        Some(4)
    );
    drop(input);
}

#[tokio::test]
async fn invalid_shelf_policy_fails_before_creating_the_socket_directory() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("not-created/shelf.sock");
    let status = AsyncCommand::new(env!("CARGO_BIN_EXE_ssh_clipboard"))
        .arg("shelf-daemon")
        .arg("--socket-path")
        .arg(&path)
        .arg("--max-items")
        .arg("0")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .unwrap();
    assert!(!status.success());
    assert!(!path.parent().unwrap().exists());
}

#[tokio::test]
async fn legacy_daemon_duplicate_startup_preserves_its_existing_clipboard() {
    use ssh_clipboard::{framing as legacy_frame, protocol as legacy};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("runtime/daemon.sock");
    let _primary = daemon_mode(&path, "daemon").await;
    let request = legacy::Request {
        request_id: 1,
        kind: legacy::RequestKind::Set {
            value: legacy::ClipboardValue {
                content_type: legacy::CONTENT_TYPE_TEXT.into(),
                data: b"synthetic legacy item".to_vec(),
                created_at: 123,
            },
        },
    };
    let mut stream = UnixStream::connect(&path).await.unwrap();
    legacy_frame::write_frame_payload(
        &mut stream,
        &legacy_frame::encode_message(&request).unwrap(),
    )
    .await
    .unwrap();
    let reply: legacy::Response = legacy_frame::decode_message(
        &legacy_frame::read_frame_payload(&mut stream, 2048)
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(matches!(reply.kind, legacy::ResponseKind::Ok));
    drop(stream);
    let duplicate = AsyncCommand::new(env!("CARGO_BIN_EXE_ssh_clipboard"))
        .arg("daemon")
        .arg("--socket-path")
        .arg(&path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .unwrap();
    assert!(!duplicate.success());
    let request = legacy::Request {
        request_id: 2,
        kind: legacy::RequestKind::Get,
    };
    let mut stream = UnixStream::connect(&path).await.unwrap();
    legacy_frame::write_frame_payload(
        &mut stream,
        &legacy_frame::encode_message(&request).unwrap(),
    )
    .await
    .unwrap();
    let reply: legacy::Response = legacy_frame::decode_message(
        &legacy_frame::read_frame_payload(&mut stream, 2048)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(reply.request_id, 2);
    let legacy::ResponseKind::Value { value } = reply.kind else {
        panic!("legacy value was detached")
    };
    assert_eq!(value.data, b"synthetic legacy item");
    assert_eq!(value.created_at, 123);
}
