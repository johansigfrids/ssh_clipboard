//! Keep ordinary CLI builds usable without exposing retired agent setup.
#[cfg(all(feature = "system-clipboard", not(feature = "agent")))]
#[test]
fn default_cli_keeps_transfers_and_settings_without_legacy_agent_commands() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_ssh_clipboard"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).unwrap();
    let commands: Vec<&str> = help
        .lines()
        .filter(|line| line.starts_with("  "))
        .filter_map(|line| line.split_whitespace().next())
        .collect();
    for command in ["settings", "shelf", "push", "pull", "peek", "doctor"] {
        assert!(commands.contains(&command), "missing {command}: {help}");
    }
    for command in [
        "agent",
        "config",
        "autostart",
        "setup-agent",
        "install-client",
        "uninstall-client",
    ] {
        assert!(
            !commands.contains(&command),
            "retired {command} leaked into the default CLI: {help}"
        );
    }
}
