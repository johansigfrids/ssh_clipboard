use super::*;
use clap::Parser;

fn configuration(parent: &Path) -> Configuration {
    #[derive(Parser)]
    struct Daemon {
        #[command(flatten)]
        args: ShelfDaemonArgs,
    }
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../../tests/fixtures/server-signing.json")).unwrap();
    let mut daemon =
        Daemon::parse_from(["test", "--max-items", "7", "--retention-secs", "42"]).args;
    daemon.socket_path = Some(parent.join("shelf %$ socket.sock"));
    daemon.legacy_socket_path = Some(parent.join("legacy.sock"));
    Configuration {
        schema: 1,
        uid: unsafe { libc::geteuid() },
        public_key: fixture["key_a"].as_str().unwrap().into(),
        launcher: parent.join("bin $NAME %n/ssh_clipboard"),
        unit: parent.join(UNIT),
        daemon,
    }
}

#[test]
fn generated_service_preserves_literal_paths_and_custom_daemon_options() {
    let directory = tempfile::tempdir().unwrap();
    let config = configuration(directory.path());
    validate(&config).unwrap();
    let unit = render_unit(&config).unwrap();
    assert!(unit.contains("ExecStart=:\""));
    assert!(unit.contains("bin $NAME %%n/ssh_clipboard"));
    assert!(unit.contains("shelf %%$ socket.sock"));
    assert!(unit.contains("\"--max-items\" \"7\""));
    assert!(unit.contains("\"--retention-secs\" \"42\""));
    assert!(unit.contains("\"--legacy-socket-path\""));
    assert_eq!(quote("a\\b\"%$c").unwrap(), "\"a\\\\b\\\"%%$c\"");
    assert!(quote("injected\nExecStart=/bad").is_err());
    let mut invalid = config.clone();
    invalid.daemon.max_connections = 0;
    assert!(validate(&invalid).is_err());
    invalid = config;
    invalid.daemon.legacy_socket_path = invalid.daemon.socket_path.clone();
    assert!(validate(&invalid).is_err());
}

#[test]
fn a_conflicting_unit_is_not_replaced_or_given_a_dangling_launcher() {
    let directory = tempfile::tempdir().unwrap();
    let config = configuration(directory.path());
    fs::write(&config.unit, b"existing legacy unit").unwrap();
    let manager = Manager {
        store: Store::open(&directory.path().join("managed")).unwrap(),
        config,
        runtime: runtime().unwrap(),
    };
    assert!(manager.prepare_files().is_err());
    assert!(fs::symlink_metadata(&manager.config.launcher).is_err());
    assert_eq!(
        fs::read(&manager.config.unit).unwrap(),
        b"existing legacy unit"
    );
}

#[test]
fn missing_installation_and_unsafe_inputs_fail_without_initializing_or_blocking() {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let directory = tempfile::tempdir().unwrap();
    let missing = directory.path().join("missing");
    assert!(Manager::open(&missing).is_err());
    assert!(!missing.exists());
    let fifo = directory.path().join("fifo");
    let name = CString::new(fifo.as_os_str().as_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
    assert!(input_file(&fifo, 1024).is_err());
    assert!(owned_file(&fifo, 1024).is_err());
    let original = directory.path().join("original");
    fs::write(&original, b"unchanged").unwrap();
    let link = directory.path().join("link");
    symlink(&original, &link).unwrap();
    assert!(owned_file(&link, 1024).is_err());
    assert!(write_new(&original, b"replacement").is_err());
    assert_eq!(fs::read(original).unwrap(), b"unchanged");
}
