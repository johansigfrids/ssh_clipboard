#![cfg(target_os = "linux")]
use base64::{Engine, engine::general_purpose::STANDARD};
use std::process::Command;

#[test]
#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
fn standalone_package_verification_is_read_only_and_fails_closed() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/server-signing.json")).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let key = directory.path().join("trusted public key.pub");
    let signature = directory.path().join("package.sig");
    let package = directory.path().join("package");
    let name = if cfg!(target_arch = "aarch64") {
        "arm"
    } else {
        "release"
    };
    let entry = &fixture["artifacts"][name];
    let bytes = STANDARD.decode(entry["bytes"].as_str().unwrap()).unwrap();
    std::fs::write(&key, fixture["key_a"].as_str().unwrap()).unwrap();
    std::fs::write(&signature, entry["signature"].as_str().unwrap()).unwrap();
    std::fs::write(&package, &bytes).unwrap();
    let invoke = |version: &str| {
        Command::new(env!("CARGO_BIN_EXE_ssh_clipboard"))
            .args([
                "server-maintenance",
                "verify",
                "--version",
                version,
                "--public-key",
            ])
            .arg(&key)
            .arg("--signature")
            .arg(&signature)
            .arg("--package")
            .arg(&package)
            .output()
            .unwrap()
    };
    let valid = invoke("2.0.0");
    assert!(
        valid.status.success(),
        "{}",
        String::from_utf8_lossy(&valid.stderr)
    );
    let output = String::from_utf8(valid.stdout).unwrap();
    assert!(output.contains("Nothing was executed or installed."));
    assert!(!output.contains("SYNTHETIC NONEXECUTABLE FIXTURE"));
    assert!(!invoke("9.0.0").status.success());
    std::fs::write(&key, fixture["key_b"].as_str().unwrap()).unwrap();
    assert!(!invoke("2.0.0").status.success());
    std::fs::write(&key, fixture["key_a"].as_str().unwrap()).unwrap();
    let mut altered = bytes.clone();
    altered[64] ^= 1;
    std::fs::write(&package, &altered).unwrap();
    assert!(!invoke("2.0.0").status.success());
    assert_eq!(std::fs::read(&package).unwrap(), altered);
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 3);
}
