//! The only executable surface is the headless session CLI.
use std::process::Command;
#[test]
fn cli_exposes_sessions_and_rejects_retired_commands() {
    let binary = env!("CARGO_BIN_EXE_ssh_clipboard");
    let output = Command::new(binary).arg("--help").output().unwrap();
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).unwrap();
    for command in ["settings", "shelf"] {
        assert!(
            help.lines()
                .any(|line| line.split_whitespace().next() == Some(command))
        );
    }
    for command in [
        "push",
        "pull",
        "peek",
        "doctor",
        "agent",
        "config",
        "autostart",
        "setup-agent",
        "install-client",
        "uninstall-client",
        "daemon",
        "proxy",
        "install-daemon",
        "uninstall-daemon",
    ] {
        let output = Command::new(binary).arg(command).output().unwrap();
        assert_eq!(output.status.code(), Some(2), "{command}");
        assert!(String::from_utf8_lossy(&output.stderr).contains("unrecognized subcommand"));
    }
}

#[test]
fn removed_options_and_agent_migration_are_parser_errors() {
    for args in [
        vec!["settings", "migrate-agent"],
        vec!["shelf", "--strict-frames", "health"],
        vec!["settings", "set", "--resync-frames", "true"],
        vec!["settings", "set", "--max-size", "1024"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_ssh_clipboard"))
            .args(args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
    }
    #[cfg(target_os = "linux")]
    {
        let output = Command::new(env!("CARGO_BIN_EXE_ssh_clipboard"))
            .args([
                "shelf-daemon",
                "--legacy-socket-path",
                "/unused/legacy.sock",
                "--help",
            ])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
    }
}
