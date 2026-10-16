use std::process::Command;

#[test]
fn doctor_reports_missing_ssh_without_exposing_raw_paths_or_creating_settings() {
    let directory = tempfile::tempdir().unwrap();
    let settings = directory.path().join("settings.json");
    let output = Command::new(env!("CARGO_BIN_EXE_ssh_clipboard"))
        .args([
            "doctor",
            "--target",
            "fixture-no-network",
            "--ssh-bin",
            "PRIVATE_SENTINEL-nonexistent-ssh",
        ])
        .arg("--settings-file")
        .arg(&settings)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let report = String::from_utf8(output.stdout).unwrap();
    assert!(report.contains("[fail] SSH executable"));
    assert!(report.contains("Install OpenSSH"));
    assert!(report.contains("Not verified by this check."));
    assert!(!report.contains("PRIVATE_SENTINEL"));
    assert!(
        !String::from_utf8(output.stderr)
            .unwrap()
            .contains("PRIVATE_SENTINEL")
    );
    assert!(!settings.exists());
    assert!(!settings.with_extension("json.lock").exists());
}
