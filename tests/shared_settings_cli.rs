use ssh_clipboard::configuration::{ConfigStore, Configuration};
use std::path::Path;
use std::process::{Command, Output};

fn cli(path: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ssh_clipboard"))
        .arg("settings")
        .arg("--settings-file")
        .arg(path)
        .args(args)
        .output()
        .unwrap()
}

fn document(output: Output) -> Configuration {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn cli_settings_restore_the_same_profile_and_preferences_as_the_desktop_store() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("settings.json");
    let defaults = document(cli(&path, &["show"]));
    assert_eq!(defaults.revision, 0);
    assert!(!path.exists());
    assert!(!path.with_extension("json.lock").exists());
    let saved = document(cli(
        &path,
        &[
            "set",
            "--target",
            "test-hub",
            "--port",
            "2222",
            "--identity-file",
            "path with spaces/key",
            "--show-preview",
            "false",
            "--close-after-copy",
            "true",
        ],
    ));
    assert_eq!(saved.revision, 1);
    assert_eq!(saved.selected_profile().unwrap().target, "test-hub");
    assert_eq!(saved.selected_profile().unwrap().port, Some(2222));
    assert_eq!(
        saved.selected_profile().unwrap().identity_file,
        "path with spaces/key"
    );
    assert!(!saved.preferences.show_preview);
    assert!(saved.preferences.close_after_copy);
    assert_eq!(
        ConfigStore::new(path.clone()).load().unwrap(),
        Some(saved.clone())
    );
    assert_eq!(document(cli(&path, &["show"])), saved);
    document(cli(
        &path,
        &[
            "add-profile",
            "--id",
            "second",
            "--name",
            "Second",
            "--target",
            "second-hub",
        ],
    ));
    let selected = document(cli(&path, &["select-profile", "second"]));
    assert_eq!(selected.default_profile, "second");
    assert_eq!(selected.selected_profile().unwrap().target, "second-hub");
    assert_eq!(selected.profiles[0], saved.profiles[0]);
    assert_eq!(selected.preferences, saved.preferences);
    let rejected = cli(&path, &["select-profile", "missing"]);
    assert!(!rejected.status.success());
    assert_eq!(document(cli(&path, &["show"])), selected);
}

#[test]
fn removing_profiles_preserves_other_connections_and_rejects_the_last_or_missing_one() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("settings.json");
    assert!(!cli(&path, &["remove-profile", "default"]).status.success());
    assert!(!path.exists());
    let original = document(cli(
        &path,
        &[
            "set",
            "--target",
            "first-hub",
            "--timeout-ms",
            "9000",
            "--show-preview",
            "false",
        ],
    ));
    for id in ["second", "third"] {
        document(cli(
            &path,
            &[
                "add-profile",
                "--id",
                id,
                "--name",
                id,
                "--target",
                "another-hub",
            ],
        ));
    }
    let unselected = document(cli(&path, &["remove-profile", "third"]));
    assert_eq!(unselected.default_profile, "default");
    assert_eq!(unselected.profiles[0], original.profiles[0]);
    document(cli(&path, &["select-profile", "second"]));
    let removed = document(cli(&path, &["remove-profile", "second"]));
    assert_eq!(removed.default_profile, "default");
    assert_eq!(removed.profiles, original.profiles);
    assert_eq!(removed.preferences, original.preferences);
    assert_eq!(removed.device, original.device);
    let before = std::fs::read(&path).unwrap();
    for id in ["default", "missing"] {
        assert!(!cli(&path, &["remove-profile", id]).status.success());
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }
    assert_eq!(document(cli(&path, &["show"])), removed);
}

#[test]
fn failed_cli_edits_preserve_corrupt_files_and_existing_profiles() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("settings.json");
    let saved = document(cli(&path, &["set", "--target", "test-hub"]));
    assert!(
        !cli(&path, &["set", "--target=-oUnexpectedOption"])
            .status
            .success()
    );
    assert!(!cli(&path, &["set", "--port", "0"]).status.success());
    assert!(
        !cli(
            &path,
            &[
                "add-profile",
                "--id",
                "default",
                "--name",
                "Duplicate",
                "--target",
                "other"
            ]
        )
        .status
        .success()
    );
    assert_eq!(document(cli(&path, &["show"])), saved);
    std::fs::write(&path, b"corrupt settings").unwrap();
    assert!(!cli(&path, &["show"]).status.success());
    assert!(
        !cli(&path, &["set", "--target", "replacement"])
            .status
            .success()
    );
    assert_eq!(std::fs::read(&path).unwrap(), b"corrupt settings");
}

#[test]
fn missing_profile_fails_before_ssh_or_clipboard_access() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("settings.json");
    for command in ["health", "list"] {
        let output = Command::new(env!("CARGO_BIN_EXE_ssh_clipboard"))
            .arg("shelf")
            .arg(command)
            .arg("--settings-file")
            .arg(&path)
            .arg("--profile")
            .arg("missing")
            .arg("--ssh-bin")
            .arg("nonexistent-test-ssh")
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("unknown profile ID"));
        assert!(!path.exists());
    }
}

#[test]
fn relative_settings_paths_work_without_rewriting_other_files() {
    let directory = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_ssh_clipboard"))
        .current_dir(directory.path())
        .args([
            "settings",
            "--settings-file",
            "local.json",
            "set",
            "--target",
            "relative-hub",
        ])
        .output()
        .unwrap();
    assert_eq!(
        document(output).selected_profile().unwrap().target,
        "relative-hub"
    );
    assert!(directory.path().join("local.json").is_file());
}

#[test]
fn old_and_future_schemas_are_rejected_without_rewriting_or_importing() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("settings.json");
    let old = directory.path().join("settings-v1.json");
    let agent = directory.path().join("agent.toml");
    std::fs::write(&old, b"old shared settings").unwrap();
    std::fs::write(&agent, b"old agent settings").unwrap();
    let fresh = document(cli(&path, &["show"]));
    assert_eq!(fresh.schema_version, 3);
    assert!(!path.exists());
    for schema in [1, 2, 4] {
        let bytes = format!("{{\"schema_version\":{schema}}}");
        std::fs::write(&path, &bytes).unwrap();
        for command in [vec!["show"], vec!["set", "--target", "replacement"]] {
            let output = cli(&path, &command);
            assert!(!output.status.success());
            assert!(String::from_utf8_lossy(&output.stderr).contains("unsupported schema"));
            assert_eq!(std::fs::read_to_string(&path).unwrap(), bytes);
        }
    }
    assert_eq!(std::fs::read(&old).unwrap(), b"old shared settings");
    assert_eq!(std::fs::read(&agent).unwrap(), b"old agent settings");
    assert_eq!(
        ConfigStore::user().unwrap().path().file_name().unwrap(),
        "settings.json"
    );
}
