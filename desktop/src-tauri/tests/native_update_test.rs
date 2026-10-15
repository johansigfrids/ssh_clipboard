#[path = "../build-support/native_update_test.rs"]
mod fixture;

#[test]
fn native_fixture_refuses_production_identity_and_non_loopback_or_ambiguous_origins() {
    let origin = "http://127.0.0.1:41234/0123456789abcdef0123456789abcdef";
    assert!(fixture::validate(origin, fixture::IDENTIFIER, fixture::PRODUCT, true).is_ok());
    for (identifier, product, installed) in [
        (
            "io.github.johansigfrids.sshclipboard",
            fixture::PRODUCT,
            true,
        ),
        (fixture::IDENTIFIER, "SSH Clipboard", true),
        (fixture::IDENTIFIER, fixture::PRODUCT, false),
    ] {
        assert!(fixture::validate(origin, identifier, product, installed).is_err());
    }
    for location in [
        "http://localhost:41234/0123456789abcdef0123456789abcdef",
        "http://0.0.0.0:41234/0123456789abcdef0123456789abcdef",
        "http://127.0.0.1:80/0123456789abcdef0123456789abcdef",
        "http://127.0.0.1:041234/0123456789abcdef0123456789abcdef",
        "http://127.0.0.1:41234/0123456789abcdef0123456789abcdef/",
        "http://user@127.0.0.1:41234/0123456789abcdef0123456789abcdef",
    ] {
        assert!(fixture::validate(location, fixture::IDENTIFIER, fixture::PRODUCT, true).is_err());
    }
}

#[test]
fn fixture_mapping_preserves_canonical_signed_selectors_and_rejects_other_sources() {
    let origin = "http://127.0.0.1:41234/0123456789abcdef0123456789abcdef";
    for path in [
        "latest/download/desktop-stable-0123456789abcdef.json",
        "download/desktop-v0.3.3/SSHClipboard-stable-windows-x86_64-0.3.3-setup.exe",
    ] {
        let source = format!("https://github.com/johansigfrids/ssh_clipboard/releases/{path}");
        assert_eq!(
            fixture::resolve(origin, &source).unwrap(),
            format!("{origin}/{path}")
        );
        for invalid in [
            source.replace("github.com", "evil.example"),
            format!("{source}?x=y"),
            format!("{source}#fragment"),
            source.replace("/releases/", "/releases/../"),
        ] {
            assert!(fixture::resolve(origin, &invalid).is_err());
        }
    }
}
