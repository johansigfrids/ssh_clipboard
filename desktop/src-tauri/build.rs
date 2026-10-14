fn main() {
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
