#[path = "build-support/native_update_test.rs"]
mod native_test;

fn main() {
    println!("cargo:rustc-check-cfg=cfg(native_update_test)");
    const NATIVE_TEST_ENV: &str = "SSH_CLIPBOARD_NATIVE_TEST_ORIGIN";
    println!("cargo:rerun-if-env-changed={NATIVE_TEST_ENV}");
    println!("cargo:rerun-if-changed=build-support/native_update_test.rs");
    if let Ok(origin) = std::env::var(NATIVE_TEST_ENV) {
        let overlay: serde_json::Value = serde_json::from_str(
            &std::env::var("TAURI_CONFIG").expect("native test requires an explicit Tauri overlay"),
        )
        .expect("invalid native test overlay");
        native_test::validate(
            &origin,
            overlay["identifier"].as_str().unwrap_or(""),
            overlay["productName"].as_str().unwrap_or(""),
            std::env::var_os("CARGO_FEATURE_INSTALLED_DESKTOP").is_some(),
        )
        .expect("native updater test must never enter a production app");
        // Exercise the same strict mapping code used by the compiled fixture.
        native_test::resolve(&origin, "https://github.com/johansigfrids/ssh_clipboard/releases/latest/download/desktop-stable-0123456789abcdef.json").unwrap();
        println!("cargo:rustc-cfg=native_update_test");
        println!("cargo:rustc-env={NATIVE_TEST_ENV}={origin}");
    }
    // Mock-runtime integration tests can link TaskDialogIndirect through Tauri.
    // Scope to integration tests: app binaries already have a manifest resource.
    if std::env::var("TARGET").is_ok_and(|target| target.ends_with("windows-msvc")) {
        println!("cargo:rustc-link-arg-tests=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg-tests=/MANIFESTDEPENDENCY:type='win32' name='Microsoft.Windows.Common-Controls' version='6.0.0.0' processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'"
        );
    }
    for name in [
        "SSH_CLIPBOARD_UPDATE_PUBLIC_KEY",
        "SSH_CLIPBOARD_UPDATE_FEED",
    ] {
        println!("cargo:rerun-if-env-changed={name}");
    }
    if std::env::var_os("CARGO_FEATURE_INSTALLED_DESKTOP").is_some()
        && std::env::var("PROFILE").as_deref() == Ok("release")
    {
        let key = std::env::var("SSH_CLIPBOARD_UPDATE_PUBLIC_KEY")
            .expect("installed release builds require an explicit public update key");
        assert!(
            !key.trim().is_empty() && key.len() <= 4096,
            "invalid public update key"
        );
        let feed = std::env::var("SSH_CLIPBOARD_UPDATE_FEED")
            .expect("installed release builds require an explicit update feed");
        let fingerprint=feed.strip_prefix("https://github.com/johansigfrids/ssh_clipboard/releases/latest/download/desktop-stable-").and_then(|tail|tail.strip_suffix(".json")).expect("invalid desktop update feed");
        assert!(
            fingerprint.len() == 16 && fingerprint.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "invalid update feed identity"
        );
    }
    tauri_build::build();
}
