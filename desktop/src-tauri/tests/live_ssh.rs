//! Opt-in network probe. Writes only to an explicitly supplied disposable socket.
use ssh_clipboard::client::ssh::SshConfig;
use ssh_clipboard::client::transport::{ClientConfig, make_request, send_request};
use ssh_clipboard::framing::{
    decode_message, encode_message, read_frame_payload_resync, write_frame_payload,
};
use ssh_clipboard::protocol::{
    CONTENT_TYPE_PNG, CONTENT_TYPE_TEXT, ClipboardValue, DEFAULT_MAX_SIZE, RESPONSE_OVERHEAD,
    RequestKind, Response, ResponseKind,
};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

async fn exchange(target: &str, socket: &str, kind: RequestKind) -> ResponseKind {
    let request = make_request(kind);
    let mut child = Command::new("ssh")
        .args([
            "-T",
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=10",
            target,
            "ssh_clipboard",
            "proxy",
            "--socket-path",
            socket,
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(15), async {
        let mut stdin = child.stdin.take().unwrap();
        write_frame_payload(&mut stdin, &encode_message(&request).unwrap())
            .await
            .unwrap();
        stdin.shutdown().await.unwrap();
        drop(stdin);
        let mut stdout = child.stdout.take().unwrap();
        let frame =
            read_frame_payload_resync(&mut stdout, DEFAULT_MAX_SIZE + RESPONSE_OVERHEAD, 8192)
                .await
                .unwrap();
        let response: Response = decode_message(&frame.payload).unwrap();
        assert_eq!(response.request_id, request.request_id);
        let output = child.wait_with_output().await.unwrap();
        assert!(
            output.status.success() || matches!(response.kind, ResponseKind::Error { .. }),
            "SSH proxy exited unsuccessfully"
        );
        response.kind
    })
    .await
    .expect("live SSH probe timed out")
}

async fn fetched(target: &str, socket: &str) -> ClipboardValue {
    match exchange(target, socket, RequestKind::Get).await {
        ResponseKind::Value { value } => value,
        _ => panic!("expected an isolated clipboard value"),
    }
}

#[test]
#[ignore = "requires SSH_CLIPBOARD_TEST_TARGET and a fresh disposable SSH_CLIPBOARD_TEST_SOCKET"]
fn live_legacy_server_round_trip() {
    let target = std::env::var("SSH_CLIPBOARD_TEST_TARGET").expect("explicit SSH target required");
    let socket =
        std::env::var("SSH_CLIPBOARD_TEST_SOCKET").expect("explicit disposable socket required");
    assert!(!target.starts_with('-') && !target.chars().any(char::is_whitespace));
    let suffix = socket
        .strip_prefix("/tmp/ssh-clipboard-prototype.")
        .expect("only disposable prototype paths are allowed");
    let token = suffix
        .strip_suffix("/daemon.sock")
        .expect("unexpected socket name");
    assert!(!token.is_empty() && token.chars().all(|c| c.is_ascii_alphanumeric()));
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            // Exercise the actual desktop transport against the installed service without fetching contents.
            let config = ClientConfig {
                ssh: SshConfig {
                    target: target.clone(),
                    port: None,
                    user: None,
                    host: None,
                    identity_file: None,
                    ssh_options: vec!["BatchMode=yes".into(), "ConnectTimeout=10".into()],
                    ssh_bin: None,
                },
                max_size: DEFAULT_MAX_SIZE,
                timeout_ms: 7000,
                resync_frames: true,
                resync_max_bytes: 8192,
            };
            let request = make_request(RequestKind::PeekMeta);
            let id = request.request_id;
            let reply = send_request(&config, request)
                .await
                .expect("installed service metadata probe failed");
            assert_eq!(reply.request_id, id);
            assert!(matches!(
                reply.kind,
                ResponseKind::Meta { .. } | ResponseKind::Empty
            ));
            assert!(
                matches!(
                    exchange(&target, &socket, RequestKind::Get).await,
                    ResponseKind::Empty
                ),
                "test socket must start empty"
            );

            let original = ClipboardValue {
                content_type: CONTENT_TYPE_TEXT.into(),
                data: "\n  Prototype: åäö 🙂\r\nKeep trailing spaces  \n"
                    .as_bytes()
                    .to_vec(),
                created_at: 123,
            };
            assert!(matches!(
                exchange(
                    &target,
                    &socket,
                    RequestKind::Set {
                        value: original.clone()
                    }
                )
                .await,
                ResponseKind::Ok
            ));
            let snapshot = fetched(&target, &socket).await;
            assert_eq!(snapshot.data, original.data);
            assert_eq!(snapshot.content_type, original.content_type);
            assert_eq!(snapshot.created_at, original.created_at);
            match exchange(&target, &socket, RequestKind::PeekMeta).await {
                ResponseKind::Meta {
                    size, created_at, ..
                } => {
                    assert_eq!(size, original.data.len() as u64);
                    assert_eq!(created_at, 123);
                }
                _ => panic!("expected isolated metadata"),
            }

            let image = ClipboardValue {
                content_type: CONTENT_TYPE_PNG.into(),
                data: ssh_clipboard::client::image::encode_png(arboard::ImageData {
                    width: 2,
                    height: 1,
                    bytes: vec![255, 0, 0, 255, 0, 255, 0, 128].into(),
                })
                .unwrap(),
                created_at: 456,
            };
            assert!(matches!(
                exchange(
                    &target,
                    &socket,
                    RequestKind::Set {
                        value: image.clone()
                    }
                )
                .await,
                ResponseKind::Ok
            ));
            let refreshed = fetched(&target, &socket).await;
            assert_eq!(refreshed.data, image.data);
            assert_eq!(refreshed.content_type, CONTENT_TYPE_PNG);
            assert_eq!(refreshed.created_at, 456);
            assert_eq!(
                ssh_clipboard::client::image::decode_png(&refreshed.data, 64 * 1024 * 1024)
                    .unwrap()
                    .bytes
                    .as_ref(),
                &[255, 0, 0, 255, 0, 255, 0, 128]
            );
            assert_eq!(
                snapshot.data, original.data,
                "retained snapshot must survive server replacement"
            );
            assert!(matches!(
                exchange(
                    &target,
                    &socket,
                    RequestKind::Set {
                        value: ClipboardValue {
                            content_type: CONTENT_TYPE_TEXT.into(),
                            data: vec![255],
                            created_at: 789
                        }
                    }
                )
                .await,
                ResponseKind::Error {
                    code: ssh_clipboard::protocol::ErrorCode::InvalidUtf8,
                    ..
                }
            ));
            assert_eq!(
                fetched(&target, &socket).await.data,
                image.data,
                "rejected value must preserve existing item"
            );
            assert!(matches!(
                exchange(&target, &format!("{socket}.missing"), RequestKind::PeekMeta).await,
                ResponseKind::Error {
                    code: ssh_clipboard::protocol::ErrorCode::DaemonNotRunning,
                    ..
                }
            ));
        });
}
