#![cfg(feature = "installed-desktop")]
use ssh_clipboard_desktop_prototype::{update_handoff::Bridge, update_policy::Candidate};
#[tokio::test]
async fn framework_accepts_the_bounded_verified_handoff_without_remote_fetch_or_install() {
    use base64::Engine;
    use tauri_plugin_updater::UpdaterExt;
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/update-signing.json")).unwrap();
    let artifact = &fixture["artifacts"]["release"];
    let key = fixture["key_a"].as_str().unwrap();
    let candidate = Candidate {
        version: artifact["version"].as_str().unwrap().into(),
        platform: "windows-x86_64".into(),
        url: format!(
            "{}/desktop-v{}/{}",
            ssh_clipboard_desktop_prototype::update_policy::RELEASE_ROOT,
            artifact["version"].as_str().unwrap(),
            artifact["file"].as_str().unwrap()
        ),
        signature: artifact["signature"].as_str().unwrap().into(),
    };
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(artifact["bytes"].as_str().unwrap())
        .unwrap();
    ssh_clipboard_desktop_prototype::update_policy::Policy::new(key, "1.0.0", &candidate.platform)
        .unwrap()
        .verify(&candidate, &bytes)
        .unwrap();
    let mut context = tauri::test::mock_context(tauri::test::noop_assets());
    context.package_info_mut().version = "1.0.0".parse().unwrap();
    context.config_mut().plugins.0.insert("updater".into(),serde_json::json!({"pubkey":key,"endpoints":[],"dangerousInsecureTransportProtocol":true,"requireSignedVersion":true,"allowDowngrades":false}));
    let app = tauri::test::mock_builder()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .build(context)
        .unwrap();
    let bridge = Bridge::new(&candidate).await.unwrap();
    let installer = app
        .updater_builder()
        .target(&candidate.platform)
        .endpoints(vec![bridge.url.clone()])
        .unwrap()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(4))
        .configure_client(|client| client.redirect(reqwest::redirect::Policy::none()))
        .build()
        .unwrap()
        .check()
        .await
        .unwrap()
        .unwrap();
    assert_eq!(installer.version, candidate.version);
    assert_eq!(installer.download_url.as_str(), candidate.url);
    assert_eq!(installer.signature, candidate.signature);
    // No Update::download or Update::install: synthetic payloads are never executable.
}
