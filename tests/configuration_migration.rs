use ssh_clipboard::configuration::{ConfigStore, Configuration};

const LEGACY: &str = include_str!("fixtures/legacy-agent.toml");

const SCHEMA_ONE: &str = r#"{
  "schema_version": 1,
  "revision": 5,
  "default_profile": "default",
  "profiles": [{"id":"default","name":"Existing","target":"existing-hub","port":2222,"identity_file":"existing-key"}],
  "preferences": {"open_shortcut":"CommandOrControl+Shift+V","shortcut_enabled":true,"show_preview":false,"close_after_copy":true}
}"#;

#[test]
fn legacy_import_is_backed_up_losslessly_idempotent_and_preserves_connection_policy() {
    let directory = tempfile::tempdir().unwrap();
    let legacy = directory.path().join("agent.toml");
    let store = ConfigStore::new(directory.path().join("settings.json"));
    std::fs::write(&legacy, LEGACY).unwrap();
    let imported = store.initialize_from_legacy(&legacy).unwrap().unwrap();
    let profile = imported.selected_profile().unwrap();
    assert_eq!(imported.schema_version, 2);
    assert_eq!(profile.target, "legacy-hub");
    assert_eq!(profile.port, Some(2222));
    assert_eq!(profile.identity_file, "keys/path with spaces");
    assert_eq!(
        profile.transport.ssh_options,
        ["BatchMode=no", "ServerAliveInterval=20"]
    );
    assert_eq!(profile.transport.max_size, 123456);
    assert_eq!(profile.transport.timeout_ms, 9000);
    assert!(!profile.transport.resync_frames);
    assert_eq!(profile.transport.resync_max_bytes, 2048);
    assert!(imported.preferences.start_at_login);
    assert_eq!(
        imported.preferences.direct_share_shortcut.as_deref(),
        Some("CmdOrCtrl+Alt+KeyC")
    );
    assert_eq!(
        imported.preferences.direct_copy_shortcut.as_deref(),
        Some("CmdOrCtrl+Alt+KeyV")
    );
    assert!(!imported.preferences.direct_shortcuts_enabled);
    assert!(uuid::Uuid::parse_str(imported.device.id.as_deref().unwrap()).is_ok());
    let receipt = imported.legacy_import.as_ref().unwrap();
    assert_eq!(receipt.source, legacy);
    assert_eq!(std::fs::read(&receipt.backup).unwrap(), LEGACY.as_bytes());
    assert_eq!(std::fs::read(&legacy).unwrap(), LEGACY.as_bytes());
    std::fs::write(&legacy, "invalid now").unwrap();
    assert_eq!(
        store.initialize_from_legacy(&legacy).unwrap(),
        Some(imported.clone())
    );
    assert_eq!(store.load().unwrap(), Some(imported));
}

#[test]
fn schema_one_reads_without_writes_and_first_save_keeps_an_exact_backup() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("settings-v1.json");
    let store = ConfigStore::new(path.clone());
    std::fs::write(&path, SCHEMA_ONE).unwrap();
    let upgraded = store.load().unwrap().unwrap();
    assert_eq!(upgraded.schema_version, 2);
    assert_eq!(upgraded.revision, 5);
    assert_eq!(upgraded.selected_profile().unwrap().target, "existing-hub");
    assert!(!upgraded.preferences.show_preview);
    assert!(upgraded.preferences.close_after_copy);
    assert_eq!(std::fs::read(&path).unwrap(), SCHEMA_ONE.as_bytes());
    assert!(!path.with_extension("schema-1.bak").exists());
    let saved = store
        .prepare(Some(&upgraded), upgraded.clone())
        .unwrap()
        .commit()
        .unwrap();
    assert_eq!(saved.revision, 6);
    assert!(saved.device.id.is_some());
    assert_eq!(
        std::fs::read(path.with_extension("schema-1.bak")).unwrap(),
        SCHEMA_ONE.as_bytes()
    );
    assert_eq!(store.load().unwrap(), Some(saved.clone()));
    let saved_again = store
        .prepare(Some(&saved), saved.clone())
        .unwrap()
        .commit()
        .unwrap();
    assert_eq!(saved_again.device.id, saved.device.id);
    assert_eq!(
        std::fs::read(path.with_extension("schema-1.bak")).unwrap(),
        SCHEMA_ONE.as_bytes()
    );
}

#[test]
fn invalid_legacy_or_backup_conflicts_do_not_replace_originals_or_create_shared_settings() {
    let directory = tempfile::tempdir().unwrap();
    let legacy = directory.path().join("agent.toml");
    let store = ConfigStore::new(directory.path().join("settings.json"));
    for bytes in [
        "invalid",
        &LEGACY.replace("config_version = 1", "config_version = 42"),
        &LEGACY.replace("timeout_ms = 9000", "timeout_ms = 0"),
    ] {
        std::fs::write(&legacy, bytes).unwrap();
        assert!(store.initialize_from_legacy(&legacy).is_err());
        assert_eq!(std::fs::read(&legacy).unwrap(), bytes.as_bytes());
        assert!(!store.path().exists());
        assert!(!legacy.with_extension("pre-shared.bak").exists());
    }
    std::fs::write(&legacy, LEGACY).unwrap();
    let backup = legacy.with_extension("pre-shared.bak");
    std::fs::write(&backup, "another backup").unwrap();
    assert!(store.initialize_from_legacy(&legacy).is_err());
    assert_eq!(std::fs::read(&legacy).unwrap(), LEGACY.as_bytes());
    assert_eq!(std::fs::read(&backup).unwrap(), b"another backup");
    assert!(!store.path().exists());
    std::fs::write(&backup, LEGACY).unwrap();
    assert!(store.initialize_from_legacy(&legacy).unwrap().is_some());
}

#[test]
fn corrupt_or_existing_shared_settings_take_precedence_over_legacy_import() {
    let directory = tempfile::tempdir().unwrap();
    let legacy = directory.path().join("agent.toml");
    let store = ConfigStore::new(directory.path().join("settings.json"));
    std::fs::write(&legacy, LEGACY).unwrap();
    std::fs::write(store.path(), "corrupt shared settings").unwrap();
    assert!(store.initialize_from_legacy(&legacy).is_err());
    assert_eq!(
        std::fs::read(store.path()).unwrap(),
        b"corrupt shared settings"
    );
    assert!(!legacy.with_extension("pre-shared.bak").exists());
    std::fs::remove_file(store.path()).unwrap();
    let saved = store
        .prepare(None, Configuration::default())
        .unwrap()
        .commit()
        .unwrap();
    assert_eq!(store.initialize_from_legacy(&legacy).unwrap(), Some(saved));
    assert!(!legacy.with_extension("pre-shared.bak").exists());
}

#[test]
fn schema_two_missing_required_fields_and_ambiguous_schema_one_are_preserved() {
    let directory = tempfile::tempdir().unwrap();
    let store = ConfigStore::new(directory.path().join("settings.json"));
    let mut invalid = serde_json::to_value(Configuration::default()).unwrap();
    invalid["profiles"][0]
        .as_object_mut()
        .unwrap()
        .remove("transport");
    std::fs::write(store.path(), invalid.to_string()).unwrap();
    assert!(store.load().is_err());
    let mut ambiguous: serde_json::Value = serde_json::from_str(SCHEMA_ONE).unwrap();
    ambiguous["profiles"][0]["transport"] = serde_json::json!({"custom":true});
    let bytes = ambiguous.to_string();
    std::fs::write(store.path(), &bytes).unwrap();
    assert!(store.prepare(None, Configuration::default()).is_err());
    assert_eq!(std::fs::read(store.path()).unwrap(), bytes.as_bytes());
    let mut ambiguous_endpoint: serde_json::Value = serde_json::from_str(SCHEMA_ONE).unwrap();
    ambiguous_endpoint["profiles"][0]["endpoint"] =
        serde_json::json!({"protocol":"shelf","max_item_bytes":1024,"max_decoded_bytes":1024});
    let bytes = ambiguous_endpoint.to_string();
    std::fs::write(store.path(), &bytes).unwrap();
    assert!(store.load().is_err());
    assert_eq!(std::fs::read(store.path()).unwrap(), bytes.as_bytes());
}

#[test]
fn executable_migration_and_advanced_settings_edits_preserve_imported_preferences() {
    let directory = tempfile::tempdir().unwrap();
    let legacy = directory.path().join("agent.toml");
    let path = directory.path().join("settings.json");
    std::fs::write(&legacy, LEGACY).unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_ssh_clipboard"))
        .arg("settings")
        .arg("--settings-file")
        .arg(&path)
        .arg("migrate-agent")
        .arg("--legacy-file")
        .arg(&legacy)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let imported: Configuration = serde_json::from_slice(&output.stdout).unwrap();
    assert!(imported.legacy_import.is_some());
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_ssh_clipboard"))
        .arg("settings")
        .arg("--settings-file")
        .arg(&path)
        .args([
            "set",
            "--clear-ssh-options",
            "--ssh-option",
            "BatchMode=yes",
            "--timeout-ms",
            "7000",
            "--max-size",
            "10485760",
            "--resync-frames",
            "true",
            "--resync-max-bytes",
            "8192",
            "--device-name",
            "Test computer",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let edited: Configuration = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(edited.device.id, imported.device.id);
    assert_eq!(edited.device.name, "Test computer");
    assert_eq!(edited.preferences, imported.preferences);
    assert_eq!(edited.legacy_import, imported.legacy_import);
    assert_eq!(edited.profiles[0].transport.ssh_options, ["BatchMode=yes"]);
    assert_eq!(edited.profiles[0].transport.timeout_ms, 7000);
    assert_eq!(edited.profiles[0].transport.max_size, 10485760);
    assert!(edited.profiles[0].transport.resync_frames);
    assert_eq!(edited.profiles[0].transport.resync_max_bytes, 8192);
}

#[cfg(feature = "agent")]
#[test]
fn migration_accepts_the_existing_agent_serializer_without_loading_user_settings() {
    let directory = tempfile::tempdir().unwrap();
    let legacy_path = directory.path().join("agent.toml");
    let mut legacy = ssh_clipboard::agent::default_agent_config();
    legacy.target = "serialized-hub".into();
    legacy.identity_file = Some("keys/path with spaces".into());
    legacy.autostart_enabled = true;
    let bytes = toml::to_string(&legacy).unwrap();
    std::fs::write(&legacy_path, &bytes).unwrap();
    let imported = ConfigStore::new(directory.path().join("settings.json"))
        .initialize_from_legacy(&legacy_path)
        .unwrap()
        .unwrap();
    assert_eq!(imported.profiles[0].target, legacy.target);
    assert_eq!(imported.profiles[0].identity_file, "keys/path with spaces");
    assert!(imported.preferences.start_at_login);
    assert_eq!(std::fs::read(&legacy_path).unwrap(), bytes.as_bytes());
}
