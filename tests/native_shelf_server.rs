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

// Even CLOEXEC descriptors exist between fork and exec. Another test's freshly
// written shim can stay text-busy through that inherited writable descriptor.
// Serialize disposable process probes, retaining concurrent clients within each.
static PROCESS_PROBES: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(1);

fn async_binary() -> AsyncCommand {
    let mut command = AsyncCommand::new(env!("CARGO_BIN_EXE_ssh_clipboard"));
    command.env("TOKIO_WORKER_THREADS", "2");
    command
}

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
    let mut command = Command::new(env!("CARGO_BIN_EXE_ssh_clipboard"));
    command.env("TOKIO_WORKER_THREADS", "2");
    command.arg("shelf-daemon");
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

#[tokio::test]
async fn item_aware_cli_lists_exports_and_guards_mutations_through_the_real_proxy() {
    let _serial = PROCESS_PROBES.acquire().await.unwrap();
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("runtime/shelf.sock");
    let _server = daemon(&socket).await;
    let shim = dir.path().join("synthetic-ssh");
    let quote = |text: &str| format!("'{}'", text.replace('\'', "'\\''"));
    let script = format!(
        "#!/bin/sh\nexport TOKIO_WORKER_THREADS=2\nexec {} session-proxy --socket-path {}\n",
        quote(env!("CARGO_BIN_EXE_ssh_clipboard")),
        quote(socket.to_str().unwrap())
    );
    std::fs::write(&shim, script).unwrap();
    std::fs::set_permissions(&shim, std::fs::Permissions::from_mode(0o700)).unwrap();
    let invoke = |args: Vec<String>| {
        let mut command = async_binary();
        command
            .args(["shelf", "--target", "synthetic-only", "--ssh-bin"])
            .arg(&shim)
            .env("RUST_LOG", "trace")
            .args(args)
            .stdin(Stdio::null())
            .kill_on_drop(true);
        async move {
            tokio::time::timeout(Duration::from_secs(5), command.output())
                .await
                .unwrap()
                .unwrap()
        }
    };
    let text = "  synthetic α\n \n";
    let source = dir.path().join("input.txt");
    std::fs::write(&source, text).unwrap();
    let shared = invoke(vec![
        "share".into(),
        "--file".into(),
        source.to_str().unwrap().into(),
        "--device-id".into(),
        Uuid::from_u128(10).to_string(),
    ])
    .await;
    assert!(
        shared.status.success(),
        "{}",
        String::from_utf8_lossy(&shared.stderr)
    );
    let listed = invoke(vec!["list".into()]).await;
    assert!(
        listed.status.success(),
        "{}",
        String::from_utf8_lossy(&listed.stderr)
    );
    let snapshot: serde_json::Value = serde_json::from_slice(&listed.stdout).unwrap();
    let epoch = snapshot["epoch"].as_str().unwrap().to_owned();
    let item = snapshot["items"][0]["id"].as_str().unwrap().to_owned();
    assert_eq!(snapshot["items"][0]["label"], "synthetic α");
    assert!(snapshot["items"][0].get("data").is_none());
    assert!(snapshot["items"][0].get("payload").is_none());
    let fetched = invoke(vec!["fetch".into(), item.clone(), "--stdout".into()]).await;
    assert!(
        fetched.status.success(),
        "{}",
        String::from_utf8_lossy(&fetched.stderr)
    );
    assert_eq!(fetched.stdout, text.as_bytes());
    let not_files = dir.path().join("not-files");
    let rejected = invoke(vec![
        "fetch-files".into(),
        item.clone(),
        "--output-dir".into(),
        not_files.to_str().unwrap().into(),
    ])
    .await;
    assert!(!rejected.status.success());
    assert!(!not_files.exists());
    let destination = dir.path().join("export.txt");
    std::fs::write(&destination, b"preserved").unwrap();
    let rejected = invoke(vec![
        "fetch".into(),
        item.clone(),
        "--output".into(),
        destination.to_str().unwrap().into(),
    ])
    .await;
    assert!(!rejected.status.success());
    assert_eq!(std::fs::read(&destination).unwrap(), b"preserved");
    let wrong = invoke(vec![
        "clear".into(),
        "--epoch".into(),
        Uuid::from_u128(123).to_string(),
    ])
    .await;
    assert!(!wrong.status.success());
    let fetched = invoke(vec![
        "fetch".into(),
        item.clone(),
        "--stdout".into(),
        "--epoch".into(),
        epoch.clone(),
    ])
    .await;
    assert_eq!(fetched.stdout, text.as_bytes());
    let fresh = dir.path().join("fresh.txt");
    let exported = invoke(vec![
        "fetch".into(),
        item.clone(),
        "--output".into(),
        fresh.to_str().unwrap().into(),
    ])
    .await;
    assert!(
        exported.status.success(),
        "{}",
        String::from_utf8_lossy(&exported.stderr)
    );
    assert!(exported.stdout.is_empty());
    assert_eq!(std::fs::read(&fresh).unwrap(), text.as_bytes());
    let deleted = invoke(vec![
        "delete".into(),
        item.clone(),
        "--epoch".into(),
        epoch.clone(),
    ])
    .await;
    assert!(
        deleted.status.success(),
        "{}",
        String::from_utf8_lossy(&deleted.stderr)
    );
    let missing = invoke(vec![
        "fetch".into(),
        item,
        "--output".into(),
        dir.path().join("missing.txt").to_str().unwrap().into(),
    ])
    .await;
    assert!(!missing.status.success());
    assert!(!dir.path().join("missing.txt").exists());
    let cleared = invoke(vec!["clear".into(), "--epoch".into(), epoch]).await;
    assert!(cleared.status.success());
    let folder = dir.path().join("bundle");
    std::fs::create_dir(&folder).unwrap();
    std::fs::create_dir(folder.join("empty")).unwrap();
    std::fs::write(folder.join(".hidden"), [0, 255, 1]).unwrap();
    std::fs::write(folder.join("CON.txt"), b"reserved").unwrap();
    std::fs::write(folder.join("a?b"), b"one").unwrap();
    std::fs::write(folder.join("a:b"), b"two").unwrap();
    let shared = invoke(vec![
        "share-files".into(),
        folder.to_str().unwrap().into(),
        folder.join(".hidden").to_str().unwrap().into(),
        "--device-id".into(),
        Uuid::from_u128(10).to_string(),
    ])
    .await;
    assert!(
        shared.status.success(),
        "{}",
        String::from_utf8_lossy(&shared.stderr)
    );
    let response: serde_json::Value = serde_json::from_slice(&shared.stdout).unwrap();
    let item = response["item"]["id"].as_str().unwrap().to_owned();
    let epoch = response["epoch"].as_str().unwrap().to_owned();
    let raw = invoke(vec!["fetch".into(), item.clone(), "--stdout".into()]).await;
    assert!(raw.status.success());
    assert!(ssh_clipboard::files::validate(&raw.stdout).is_ok());
    let destination = dir.path().join("received");
    let args = vec![
        "fetch-files".into(),
        item.clone(),
        "--output-dir".into(),
        destination.to_str().unwrap().into(),
        "--epoch".into(),
        epoch.clone(),
    ];
    let fetched = invoke(args.clone()).await;
    assert!(
        fetched.status.success(),
        "{}",
        String::from_utf8_lossy(&fetched.stderr)
    );
    let extracted: serde_json::Value = serde_json::from_slice(&fetched.stdout).unwrap();
    assert_eq!(extracted["paths"].as_array().unwrap().len(), 1);
    assert!(extracted["renames"].as_array().unwrap().len() >= 2);
    assert_eq!(
        std::fs::read(destination.join("bundle/.hidden")).unwrap(),
        [0, 255, 1]
    );
    assert_eq!(
        std::fs::read(destination.join("bundle/_CON.txt")).unwrap(),
        b"reserved"
    );
    assert!(destination.join("bundle/empty").is_dir());
    let repeated = invoke(args).await;
    assert!(!repeated.status.success());
    let wrong_epoch_output = dir.path().join("wrong-epoch");
    let wrong = invoke(vec![
        "fetch-files".into(),
        item,
        "--output-dir".into(),
        wrong_epoch_output.to_str().unwrap().into(),
        "--epoch".into(),
        Uuid::new_v4().to_string(),
    ])
    .await;
    assert!(!wrong.status.success());
    assert!(!wrong_epoch_output.exists());
}

#[tokio::test]
async fn reusable_connection_keeps_one_proxy_and_never_replays_into_a_restarted_shelf() {
    let _serial = PROCESS_PROBES.acquire().await.unwrap();
    let _ = tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter("ssh_clipboard::client::connection=debug")
        .try_init();
    use ssh_clipboard::client::connection::Connection;
    use ssh_clipboard::client::session::{Action, SessionPolicy};
    use ssh_clipboard::client::ssh::SshConfig;
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("runtime/shelf.sock");
    let mut server = daemon(&socket).await;
    let shim = dir.path().join("reusable-ssh");
    let count = dir.path().join("launches");
    let quote = |value: &str| format!("'{}'", value.replace('\'', "'\\''"));
    let script = format!(
        "#!/bin/sh\nexport TOKIO_WORKER_THREADS=2\nprintf x >> {}\nexec {} session-proxy --socket-path {}\n",
        quote(count.to_str().unwrap()),
        quote(env!("CARGO_BIN_EXE_ssh_clipboard")),
        quote(socket.to_str().unwrap())
    );
    std::fs::write(&shim, script).unwrap();
    std::fs::set_permissions(&shim, std::fs::Permissions::from_mode(0o700)).unwrap();
    let config = SshConfig {
        target: "synthetic-only".into(),
        port: None,

        identity_file: None,
        ssh_options: vec![],
        ssh_bin: Some(shim),
    };
    let policy = SessionPolicy {
        max_item_bytes: 1024,
        max_decoded_bytes: 1024,
        operation_timeout: Duration::from_secs(2),
    };
    let mut connection = Connection::connect(config.clone(), policy).await.unwrap();
    let epoch = connection.epoch().unwrap();
    let data = "  reusable α\n ".as_bytes();
    let share = Action::Share {
        operation_id: Uuid::from_u128(900),
        origin: Origin {
            device_id: Uuid::from_u128(901),
            name: "Synthetic".into(),
        },
        representation: Representation {
            id: 0,
            content_type: ContentType::Text,
            encoded_bytes: data.len() as u32,
            dimensions: None,
            decoded_bytes: None,
        },
        created_at_ms: None,
    };
    connection.execute(share, data).await.unwrap();
    let snapshot = connection.snapshot().await.unwrap();
    let status = connection
        .execute(
            Action::OperationStatus {
                device_id: Uuid::from_u128(901),
                operation_id: Uuid::from_u128(900),
            },
            &[],
        )
        .await
        .unwrap();
    assert!(
        matches!(status.response,Response::Operation {item:Some(item),available:true,..} if item.id==snapshot.items[0].id)
    );
    assert!(connection.supports("watch"));
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
    assert!(changed.payload.is_empty());
    assert!(
        matches!(changed.response,Response::Changed {sequence,..} if sequence==snapshot.sequence)
    );
    let heartbeat = connection
        .execute(
            Action::Watch {
                after_sequence: snapshot.sequence,
                wait_ms: 5,
            },
            &[],
        )
        .await
        .unwrap();
    assert!(
        matches!(heartbeat.response,Response::Changed {sequence,..} if sequence==snapshot.sequence)
    );
    assert_eq!(snapshot.items.len(), 1);
    let item_id = snapshot.items[0].id;
    assert_eq!(
        connection
            .execute(Action::Fetch { item_id }, &[])
            .await
            .unwrap()
            .payload,
        data
    );
    connection.execute(Action::Health, &[]).await.unwrap();
    assert_eq!(std::fs::read(&count).unwrap(), b"x");
    server.stop();
    assert!(connection.execute(Action::Clear, &[]).await.is_err());
    assert!(!connection.is_ready());
    connection.close().await;
    assert_eq!(std::fs::read(&count).unwrap(), b"x");
    let _restarted = daemon(&socket).await;
    let mut next = Connection::connect(config, policy).await.unwrap();
    assert_ne!(next.epoch(), Some(epoch));
    assert!(next.snapshot().await.unwrap().items.is_empty());
    assert_eq!(std::fs::read(&count).unwrap(), b"xx");
    next.close().await;
}

fn proxy(path: &Path) -> tokio::process::Child {
    async_binary()
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
        versions: vec![2],
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
    let _serial = PROCESS_PROBES.acquire().await.unwrap();
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
    let duplicate = async_binary()
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
    let _serial = PROCESS_PROBES.acquire().await.unwrap();
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
    let _serial = PROCESS_PROBES.acquire().await.unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("not-created/shelf.sock");
    let status = async_binary()
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
