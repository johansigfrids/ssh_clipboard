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
    daemon_with_legacy(path, mode, None).await
}

async fn daemon_with_legacy(path: &Path, mode: &str, legacy: Option<&Path>) -> Daemon {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ssh_clipboard"));
    command.arg(mode);
    if let Some(legacy) = legacy {
        command.arg("--legacy-socket-path").arg(legacy);
    }
    let child = command
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
            if UnixStream::connect(path).await.is_ok()
                && (legacy.is_none() || UnixStream::connect(legacy.unwrap()).await.is_ok())
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    daemon
}

async fn old_proxy(
    path: &Path,
    kind: ssh_clipboard::protocol::RequestKind,
) -> ssh_clipboard::protocol::Response {
    use ssh_clipboard::{framing as frame, protocol as v2};
    let mut child = AsyncCommand::new(env!("CARGO_BIN_EXE_ssh_clipboard"))
        .arg("proxy")
        .arg("--socket-path")
        .arg(path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    frame::write_frame_payload(
        &mut input,
        &frame::encode_message(&v2::Request {
            request_id: 42,
            kind,
        })
        .unwrap(),
    )
    .await
    .unwrap();
    let response = frame::decode_message(
        &tokio::time::timeout(
            Duration::from_secs(3),
            frame::read_frame_payload(child.stdout.as_mut().unwrap(), 2048),
        )
        .await
        .unwrap()
        .unwrap(),
    )
    .unwrap();
    assert!(
        tokio::time::timeout(Duration::from_secs(3), child.wait())
            .await
            .unwrap()
            .unwrap()
            .success()
    );
    drop(input);
    response
}

#[tokio::test]
async fn both_real_proxy_modes_share_the_same_shelf() {
    use ssh_clipboard::protocol::{self as v2, RequestKind as Old, ResponseKind as Reply};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("runtime/shelf.sock");
    let legacy = path.with_file_name("daemon.sock");
    let _server = daemon_with_legacy(&path, "shelf-daemon", Some(&legacy)).await;
    let data = " legacy α\n ".as_bytes();
    assert!(matches!(
        old_proxy(
            &legacy,
            Old::Set {
                value: v2::ClipboardValue {
                    content_type: v2::CONTENT_TYPE_TEXT.into(),
                    data: data.to_vec(),
                    created_at: -123
                }
            }
        )
        .await
        .kind,
        Reply::Ok
    ));
    let mut child = proxy(&path);
    let mut input = child.stdin.take().unwrap();
    let mut output = child.stdout.take().unwrap();
    let Response::Welcome { epoch, limits, .. } = hello(&mut input, &mut output).await else {
        panic!("not welcomed")
    };
    let limits = limits.frame_limits().unwrap();
    framing::write_message(
        &mut input,
        &Request::List {
            request_id: 2.into(),
            epoch,
            offset: 0,
            limit: 16,
            expected_sequence: None,
        },
        &[],
        limits,
    )
    .await
    .unwrap();
    let Response::Listed { items, .. } = receive(&mut output, limits).await.0 else {
        panic!("not listed")
    };
    assert_eq!(items.len(), 1);
    assert!(items[0].origin.is_none());
    framing::write_message(
        &mut input,
        &Request::Fetch {
            request_id: 3.into(),
            epoch,
            item_id: items[0].id,
            representation_id: 0,
        },
        &[],
        limits,
    )
    .await
    .unwrap();
    assert_eq!(receive(&mut output, limits).await.1, data);
    // A real PNG stays byte-for-byte identical through new share -> old proxy Get.
    let mut png = std::io::Cursor::new(Vec::new());
    image::DynamicImage::new_rgba8(2, 3)
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
    let png = png.into_inner();
    let share = Request::Share {
        request_id: 4.into(),
        epoch,
        operation_id: Uuid::from_u128(30),
        origin: Origin {
            device_id: Uuid::from_u128(31),
            name: "Synthetic".into(),
        },
        representation: Representation {
            id: 0,
            content_type: ContentType::Png,
            encoded_bytes: png.len() as u32,
            dimensions: None,
            decoded_bytes: None,
        },
        created_at_ms: Some(456.into()),
    };
    framing::write_message(&mut input, &share, &png, limits)
        .await
        .unwrap();
    assert!(matches!(
        receive(&mut output, limits).await.0,
        Response::Shared { .. }
    ));
    assert!(
        matches!(old_proxy(&legacy, Old::Get).await.kind, Reply::Value {value} if value.data == png && value.created_at == 456 && value.content_type == v2::CONTENT_TYPE_PNG)
    );
    framing::write_message(
        &mut input,
        &Request::Clear {
            request_id: 5.into(),
            epoch,
        },
        &[],
        limits,
    )
    .await
    .unwrap();
    assert!(matches!(
        receive(&mut output, limits).await.0,
        Response::Cleared { .. }
    ));
    assert!(matches!(
        old_proxy(&legacy, Old::Get).await.kind,
        Reply::Empty
    ));
    drop(input);
    assert!(
        tokio::time::timeout(Duration::from_secs(3), child.wait())
            .await
            .unwrap()
            .unwrap()
            .success()
    );
}

#[tokio::test]
async fn occupied_legacy_endpoint_refuses_cutover_and_releases_the_new_socket() {
    let dir = tempfile::tempdir().unwrap();
    let legacy = dir.path().join("runtime/daemon.sock");
    let shelf = legacy.with_file_name("shelf.sock");
    let _old = daemon_mode(&legacy, "daemon").await;
    let status = AsyncCommand::new(env!("CARGO_BIN_EXE_ssh_clipboard"))
        .arg("shelf-daemon")
        .arg("--socket-path")
        .arg(&shelf)
        .arg("--legacy-socket-path")
        .arg(&legacy)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .unwrap();
    assert!(!status.success());
    assert!(!shelf.exists());
    assert!(matches!(
        old_proxy(&legacy, ssh_clipboard::protocol::RequestKind::Get)
            .await
            .kind,
        ssh_clipboard::protocol::ResponseKind::Empty
    ));
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
