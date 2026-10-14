#[cfg(feature = "custom-protocol")]
#[test]
fn standalone_context_contains_the_frontend_without_a_dev_server() {
    // This uses the same generated context as panel::run, without opening a GUI.
    let context: tauri::Context<tauri::Wry> = tauri::generate_context!();
    let main = context
        .config()
        .app
        .windows
        .iter()
        .find(|window| window.label == "main")
        .expect("main window configuration");
    assert!(
        !main.visible,
        "background startup must never create a visible panel"
    );
    assert!(!main.focus, "hidden startup must not request native focus");
    const { assert!(!cfg!(dev), "standalone builds must not use the dev URL") };
    let index = context
        .assets()
        .get(&"index.html".into())
        .expect("standalone build must embed index.html");
    let index = std::str::from_utf8(&index).expect("frontend entry must be UTF-8");
    assert!(index.contains("SSH Clipboard"));
    // The entry page is not sufficient: its generated script/style must exist too.
    for segment in index.split("\"") {
        if segment.starts_with("/assets/") {
            assert!(
                context.assets().get(&segment.into()).is_some(),
                "missing embedded frontend asset: {segment}"
            );
        }
    }
}
