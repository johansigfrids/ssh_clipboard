//! CLI preflight probes; no SSH, clipboard, or saved user configuration.
use std::process::Command;

#[test]
fn explicit_export_and_epoch_arguments_are_required() {
    for args in [
        vec!["shelf", "fetch", "00000000-0000-0000-0000-000000000001"],
        vec!["shelf", "clear"],
        vec!["shelf", "delete", "00000000-0000-0000-0000-000000000001"],
        vec!["shelf", "share-files"],
        vec![
            "shelf",
            "fetch-files",
            "00000000-0000-0000-0000-000000000001",
        ],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_ssh_clipboard"))
            .args(args)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains("required"));
    }
}

#[test]
fn invalid_share_metadata_and_input_fail_before_ssh_without_loading_a_direct_destinations_settings()
{
    let dir = tempfile::tempdir().unwrap();
    let settings = dir.path().join("corrupt-settings.json");
    std::fs::write(&settings, "not json").unwrap();
    let input = dir.path().join("invalid.bin");
    std::fs::write(&input, [255]).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_ssh_clipboard"))
        .args([
            "shelf",
            "--target",
            "fixture-only",
            "--ssh-bin",
            "does-not-exist",
            "--settings-file",
        ])
        .arg(&settings)
        .args([
            "share",
            "--device-id",
            "00000000-0000-0000-0000-000000000001",
            "--file",
        ])
        .arg(&input)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("valid UTF-8"));
    assert_eq!(std::fs::read_to_string(&settings).unwrap(), "not json");
    assert!(output.stdout.is_empty());
    let invalid_identity = Command::new(env!("CARGO_BIN_EXE_ssh_clipboard"))
        .args([
            "shelf",
            "--target",
            "fixture-only",
            "--ssh-bin",
            "does-not-exist",
            "share",
            "--device-id",
            "00000000-0000-0000-0000-000000000000",
            "--file",
        ])
        .arg(dir.path().join("never-read.txt"))
        .output()
        .unwrap();
    assert!(!invalid_identity.status.success());
    assert!(String::from_utf8_lossy(&invalid_identity.stderr).contains("invalid session metadata"));
    assert!(!String::from_utf8_lossy(&invalid_identity.stderr).contains("never-read.txt"));
}
