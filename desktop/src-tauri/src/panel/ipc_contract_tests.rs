//! Backend-produced frontend fixtures and real command argument decoding, without native I/O.
use super::*;
use serde_json::{Value, json};

fn contract() -> Value {
    use ssh_clipboard::session_protocol::{ContentType, Item, Origin, Representation};
    let mut session = super::tests::session();
    session.settings.shortcut_enabled = true;
    let mut initial = session.view().unwrap();
    // The same wire shape is used on all hosts; host labels are not this fixture's contract.
    initial.platform = "windows";
    initial.shortcut_available = true;
    let epoch = Uuid::from_u128(1);
    let item_id = Uuid::from_u128(2);
    let representation = Representation {
        id: 0,
        content_type: ContentType::Text,
        encoded_bytes: 17,
        dimensions: None,
        decoded_bytes: None,
    };
    let origin = Origin {
        device_id: Uuid::from_u128(3),
        name: "Contract laptop".into(),
    };
    session.settings.limits = ssh_clipboard::configuration::SessionLimits {
        max_item_bytes: 10485760,
        max_decoded_bytes: 67108864,
    };
    session.saved_config.as_mut().unwrap().revision = u64::MAX;
    session.saved_config.as_mut().unwrap().profiles[0].limits = session.settings.limits.clone();
    session.history.epoch = Some(epoch);
    session.history.selected = Some(item_id);
    session.history.items = vec![Item {
        id: item_id,
        sequence: 1.into(),
        label: "Contract history item".into(),
        origin: Some(origin.clone()),
        created_at_ms: None,
        received_at_ms: 1792324800000i64.into(),
        expires_at_ms: 1792328400000i64.into(),
        representations: vec![representation.clone()],
    }];
    session.history.preview = initial.item.clone();
    session.pending_share = Some(PendingShare::new(
        epoch,
        Uuid::from_u128(4),
        origin,
        representation,
        crate::clipboard_payload::build_text_value("retained snapshot".into(), 1024).unwrap(),
        60000,
        Some(100.into()),
    ));
    session.history_status = Some(monitor::Status {
        phase: monitor::Phase::NeedsAction,
        detail: "Reconnect the saved connection.".into(),
        retry_in_ms: None,
    });
    session.operation_notice = Some(OperationNotice {
        id: "7".into(),
        message: "Synthetic retained result".into(),
        failed: true,
    });
    session.connection_report = Some(ConnectionReport {
        healthy: false,
        summary: "Synthetic connection report",
        hint: Some("Reconnect explicitly"),
        checks: vec![ssh_clipboard::client::diagnostics::DiagnosticCheck {
            stage: ssh_clipboard::client::diagnostics::CheckStage::Daemon,
            label: "Clipboard daemon",
            status: ssh_clipboard::client::diagnostics::CheckStatus::Failed,
            detail: "Synthetic unavailable daemon",
        }],
    });
    let mut shelf = session.view().unwrap();
    shelf.platform = "windows";
    shelf.shortcut_available = true;
    shelf.startup_status.enabled = Some(false);
    let updates = crate::updates::View {
        available: true,
        current_version: "1.0.0".into(),
        offered_version: Some("2.0.0".into()),
        notes: "- Synthetic verified update".into(),
        phase: "ready",
        detail: "Ready to install after confirmation".into(),
        busy: false,
        downloaded_bytes: 123,
        total_bytes: Some(123),
    };
    shelf.update_status = updates.clone();
    let operations = Controller::default();
    let _active = operations.begin("Synthetic transfer").unwrap();
    let bytes = ssh_clipboard::files::encode(
        &[
            ssh_clipboard::files::Entry {
                parent: None,
                name: "PRIVATE-FILE.txt".into(),
                kind: ssh_clipboard::files::Kind::File,
                length: 3,
            },
            ssh_clipboard::files::Entry {
                parent: None,
                name: "empty-folder".into(),
                kind: ssh_clipboard::files::Kind::Directory,
                length: 0,
            },
        ],
        b"abc",
        1024,
    )
    .unwrap();
    session.pending_share = None;
    session.history.items[0].representations[0].content_type = ContentType::Files;
    session.history.items[0].representations[0].encoded_bytes = bytes.len() as u32;
    session.history.items[0].label = "1 file, 1 folder".into();
    session.history.preview = Some(
        ItemPreview::from_value(
            &ClipboardPayload {
                content_type: ContentType::Files,
                data: bytes,
                created_at: 0,
            },
            true,
        )
        .unwrap(),
    );
    let mut files = session.view().unwrap();
    files.platform = "windows";
    files.shortcut_available = true;
    session.settings.show_preview = false;
    let mut hidden_files = session.view().unwrap();
    hidden_files.platform = "windows";
    hidden_files.shortcut_available = true;
    assert!(
        !serde_json::to_string(&hidden_files)
            .unwrap()
            .contains("PRIVATE-FILE")
    );
    json!({ "initial": initial, "shelf": shelf, "files":files,"hiddenFiles":hidden_files,"operation": operations.view().unwrap(), "updates": updates })
}

#[test]
fn ipc_contract_fixture_matches_backend_serialization() {
    let fixture = contract();
    let generated = format!(
        "// Generated by the Rust ipc_contract test. Do not edit by hand.\nimport type {{ PanelView, OperationView, UpdateView }} from \"../../src/api\";\n\nexport const ipc = {} satisfies {{ initial: PanelView; shelf: PanelView; files: PanelView; hiddenFiles: PanelView; operation: OperationView; updates: UpdateView }};\n",
        serde_json::to_string_pretty(&fixture).unwrap()
    );
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/ipc.ts");
    if std::env::var_os("SSH_CLIPBOARD_WRITE_IPC_FIXTURE").is_some() {
        std::fs::write(&path, &generated).unwrap();
    }
    let checked_in = std::fs::read_to_string(path).unwrap().replace("\r\n", "\n");
    assert_eq!(
        checked_in, generated,
        "Review wire changes and regenerate the shared IPC fixture explicitly"
    );
}

#[test]
fn ipc_contract_requests_decode_and_stale_actions_never_access_the_clipboard() {
    use std::sync::atomic::AtomicUsize;
    struct ForbiddenClipboard;
    impl crate::clipboard_payload::ClipboardAccess for ForbiddenClipboard {
        fn read_text(&mut self) -> eyre::Result<String> {
            unreachable!("IPC fixture must not read the clipboard")
        }
        fn read_image(&mut self) -> eyre::Result<ssh_clipboard::client::image::ImageData<'static>> {
            unreachable!("IPC fixture must not read the clipboard")
        }
        fn write_text(&mut self, _: &str) -> eyre::Result<()> {
            unreachable!("IPC fixture must not write the clipboard")
        }
        fn write_image(
            &mut self,
            _: ssh_clipboard::client::image::ImageData<'static>,
        ) -> eyre::Result<()> {
            unreachable!("IPC fixture must not write the clipboard")
        }
    }
    // Starting the worker is lazy. Any attempt to initialize a native adapter is observable.
    let accesses = Arc::new(AtomicUsize::new(0));
    let observed = accesses.clone();
    let clipboard = ClipboardWorker::start_with(move || {
        observed.fetch_add(1, Ordering::SeqCst);
        Err::<ForbiddenClipboard, _>(eyre::eyre!("No native adapter in IPC tests"))
    })
    .unwrap();
    let session = super::tests::session();
    let updates = session.updates.clone();
    let desktop = Desktop {
        session: Arc::new(Mutex::new(session)),
        operations: Controller::default(),
        open_shortcut: AtomicU64::new(0),
        pressed_shortcuts: StdMutex::new(HashSet::new()),
        recording_shortcut: AtomicBool::new(false),
        clipboard,
        config_store: None,
        monitor_request: watch::channel(monitor::Request::default()).0,
        monitor_tasks: StdMutex::new(vec![]),
        updates,
    };
    let app = tauri::test::mock_builder()
        .manage(desktop)
        .invoke_handler(tauri::generate_handler![
            super::get_panel,
            super::get_operation,
            super::select_item,
            super::share_clipboard,
            super::cancel_operation
        ])
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .unwrap();
    let webview = tauri::WebviewWindowBuilder::new(&app, "ipc-contract", Default::default())
        .build()
        .unwrap();
    let invoke = |name: &str, body: Value| {
        tauri::test::get_ipc_response(
            &webview,
            tauri::webview::InvokeRequest {
                cmd: name.into(),
                callback: tauri::ipc::CallbackFn(0),
                error: tauri::ipc::CallbackFn(1),
                url: if cfg!(windows) {
                    "http://tauri.localhost"
                } else {
                    "tauri://localhost"
                }
                .parse()
                .unwrap(),
                body: body.into(),
                headers: Default::default(),
                invoke_key: tauri::test::INVOKE_KEY.into(),
            },
        )
        .map(|response| response.deserialize::<Value>().unwrap())
    };
    let actual = invoke("get_panel", json!({})).unwrap();
    assert_eq!(actual["settings"], contract()["initial"]["settings"]);
    assert_eq!(
        invoke("get_operation", json!({})).unwrap(),
        json!({"generation":"0", "active":null})
    );
    let requests: Value =
        serde_json::from_str(include_str!("../../../tests/fixtures/ipc-requests.json")).unwrap();
    for name in ["select_item", "share_clipboard"] {
        let error = invoke(name, requests[name].clone()).unwrap_err();
        assert!(
            error.as_str().unwrap().contains("settings changed"),
            "{error}"
        );
    }
    let mut invalid = requests["select_item"].clone();
    invalid["itemId"] = json!(42);
    assert!(
        invoke("select_item", invalid)
            .unwrap_err()
            .as_str()
            .unwrap()
            .contains("itemId")
    );
    assert!(invoke("cancel_operation", requests["cancel_operation"].clone()).is_err());
    assert_eq!(accesses.load(Ordering::SeqCst), 0);
    app.state::<Desktop>().clipboard.stop();
}
