use crate::cli::{InstallDaemonArgs, UninstallDaemonArgs};
use crate::protocol::DEFAULT_MAX_SIZE;
use eyre::{Result, WrapErr, eyre};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

pub async fn run(args: InstallDaemonArgs) -> Result<()> {
    let exe = std::env::current_exe().wrap_err("failed to resolve current executable")?;
    let exe_dir = exe
        .parent()
        .ok_or_else(|| eyre!("failed to resolve executable directory"))?
        .to_path_buf();

    ensure_executable(&exe)?;

    let unit_source = exe_dir.join("ssh_clipboard.service");
    let unit_link = user_unit_link_path()?;
    let bin_link = PathBuf::from("/usr/local/bin/ssh_clipboard");

    let max_size = if args.max_size == 0 {
        DEFAULT_MAX_SIZE
    } else {
        args.max_size
    };

    let unit_contents = render_unit_file(
        &bin_link,
        args.socket_path.as_deref(),
        max_size,
        args.io_timeout_ms,
    );

    if args.dry_run {
        print_dry_run(&exe, &bin_link, &unit_source, &unit_link, &unit_contents)?;
        return Ok(());
    }

    install_symlink(&exe, &bin_link, args.no_sudo, args.force)?;
    write_unit_file(&unit_source, &unit_contents, args.force)?;
    link_unit_file(&unit_source, &unit_link, args.force)?;
    reload_and_start_service()?;
    verify_service_active()?;
    print_success(&bin_link, &unit_source, &unit_link)?;
    Ok(())
}

fn ensure_executable(path: &Path) -> Result<()> {
    if !path.is_absolute() {
        return Err(eyre!("current executable path is not absolute"));
    }
    let meta = std::fs::metadata(path).wrap_err("failed to read executable metadata")?;
    if !meta.is_file() {
        return Err(eyre!("current executable is not a file"));
    }
    let mode = meta.permissions().mode();
    if mode & 0o111 == 0 {
        return Err(eyre!(
            "current executable is not marked executable; run chmod +x {}",
            path.display()
        ));
    }
    Ok(())
}

fn render_unit_file(
    bin_path: &Path,
    socket_path: Option<&Path>,
    max_size: usize,
    io_timeout_ms: u64,
) -> String {
    let mut exec = format!(
        "{} daemon --io-timeout-ms {} --max-size {}",
        bin_path.display(),
        io_timeout_ms,
        max_size
    );
    if let Some(path) = socket_path {
        let quoted = systemd_quote_arg(&path.to_string_lossy());
        exec.push_str(&format!(" --socket-path {quoted}"));
    }

    format!(
        "[Unit]\n\
Description=SSH Clipboard Daemon\n\
After=network.target\n\
\n\
[Service]\n\
ExecStart={exec}\n\
Restart=on-failure\n\
RestartSec=1\n\
\n\
[Install]\n\
WantedBy=default.target\n"
    )
}

fn systemd_quote_arg(value: &str) -> String {
    if !value
        .chars()
        .any(|c| c.is_whitespace() || c == '"' || c == '\\')
    {
        return value.to_string();
    }
    let mut out = String::from("\"");
    for ch in value.chars() {
        match ch {
            '"' | '\\' => {
                out.push('\\');
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
    out.push('"');
    out
}

fn user_unit_link_path() -> Result<PathBuf> {
    let home = std::env::var("HOME").wrap_err("HOME is not set")?;
    Ok(Path::new(&home)
        .join(".config")
        .join("systemd")
        .join("user")
        .join("ssh_clipboard.service"))
}

fn print_dry_run(
    exe: &Path,
    bin_link: &Path,
    unit_source: &Path,
    unit_link: &Path,
    unit_contents: &str,
) -> Result<()> {
    println!("dry-run: would link {bin_link} -> {}", exe.display());
    println!(
        "dry-run: would write unit file to {}",
        unit_source.display()
    );
    println!(
        "dry-run: would link unit {} -> {}",
        unit_link.display(),
        unit_source.display()
    );
    println!("dry-run: would run `systemctl --user daemon-reload`");
    println!("dry-run: would run `systemctl --user enable --now ssh_clipboard.service`");
    println!();
    println!("unit file contents:\n{unit_contents}");
    Ok(())
}

fn install_symlink(exe: &Path, bin_link: &Path, no_sudo: bool, force: bool) -> Result<()> {
    if bin_link.exists() {
        let meta = std::fs::symlink_metadata(bin_link)?;
        if !meta.file_type().is_symlink() && !force {
            return Err(eyre!(
                "{} exists and is not a symlink; use --force to overwrite",
                bin_link.display()
            ));
        }
    }

    if is_root() {
        if bin_link.exists() {
            let _ = std::fs::remove_file(bin_link);
        }
        std::os::unix::fs::symlink(exe, bin_link)
            .wrap_err("failed to create /usr/local/bin symlink")?;
        return Ok(());
    }

    if no_sudo {
        return Err(eyre!(
            "root permissions required to create {}; run: sudo ln -sf {} {}",
            bin_link.display(),
            exe.display(),
            bin_link.display()
        ));
    }

    run_sudo(&["-v"])?;
    run_sudo(&[
        "ln",
        "-sf",
        &exe.display().to_string(),
        &bin_link.display().to_string(),
    ])?;
    Ok(())
}

fn write_unit_file(path: &Path, contents: &str, force: bool) -> Result<()> {
    if path.exists() && !force {
        return Err(eyre!(
            "{} already exists; use --force to overwrite",
            path.display()
        ));
    }
    std::fs::write(path, contents).wrap_err("failed to write unit file")?;
    Ok(())
}

fn link_unit_file(source: &Path, link: &Path, force: bool) -> Result<()> {
    if let Some(parent) = link.parent() {
        std::fs::create_dir_all(parent).wrap_err("failed to create systemd user dir")?;
    }
    if link.exists() {
        let meta = std::fs::symlink_metadata(link)?;
        if !meta.file_type().is_symlink() && !force {
            return Err(eyre!(
                "{} exists and is not a symlink; use --force to overwrite",
                link.display()
            ));
        }
        let _ = std::fs::remove_file(link);
    }
    std::os::unix::fs::symlink(source, link).wrap_err("failed to create unit symlink")?;
    Ok(())
}

fn reload_and_start_service() -> Result<()> {
    run_systemctl_user(&["daemon-reload"])?;
    run_systemctl_user(&["enable", "--now", "ssh_clipboard.service"])?;
    Ok(())
}

fn verify_service_active() -> Result<()> {
    let output = run_systemctl_user_allow_failure(&["is-active", "ssh_clipboard.service"])
        .wrap_err("failed to run systemctl is-active")?;
    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(eyre!(
            "service did not start ({}{}); try: systemctl --user status ssh_clipboard.service",
            stdout.trim(),
            if stderr.trim().is_empty() {
                ""
            } else {
                " (see stderr)"
            }
        ));
    }
    Ok(())
}

fn print_success(bin_link: &Path, unit_source: &Path, unit_link: &Path) -> Result<()> {
    println!("installed:");
    println!("- binary link: {}", bin_link.display());
    println!("- unit source: {}", unit_source.display());
    println!("- unit link: {}", unit_link.display());
    println!();
    println!("status:");
    println!("  systemctl --user status ssh_clipboard.service");
    println!("  journalctl --user -u ssh_clipboard.service -f");
    println!();
    println!("test over SSH:");
    println!("  ssh -T user@server ssh_clipboard proxy");
    println!();
    println!("note:");
    println!("  do not move or delete this folder; rerun install-daemon if you do.");
    Ok(())
}

fn run_command(cmd: &str, args: &[&str]) -> Result<()> {
    let output = Command::new(cmd)
        .args(args)
        .output()
        .wrap_err("failed to spawn command")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(eyre!("{cmd} failed: {stderr}"));
    }
    Ok(())
}

fn run_sudo(args: &[&str]) -> Result<()> {
    run_command("sudo", args)
}

fn run_systemctl_user(args: &[&str]) -> Result<Output> {
    run_systemctl_user_inner(args, false)
}

fn run_systemctl_user_allow_failure(args: &[&str]) -> Result<Output> {
    run_systemctl_user_inner(args, true)
}

fn run_systemctl_user_inner(args: &[&str], allow_failure: bool) -> Result<Output> {
    let output = Command::new("systemctl")
        .args(std::iter::once("--user").chain(args.iter().copied()))
        .output()
        .wrap_err("failed to spawn systemctl")?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        if stderr.contains("Failed to connect to bus") || stderr.contains("No medium found") {
            return Err(eyre!(
                "systemctl --user failed (no user bus). Try: loginctl enable-linger $USER, then re-run install-daemon"
            ));
        }
        if !allow_failure {
            return Err(eyre!("systemctl --user failed: {stderr}"));
        }
    }
    Ok(output)
}

fn is_root() -> bool {
    unsafe { libc::geteuid() == 0 }
}

pub async fn run_uninstall(args: UninstallDaemonArgs) -> Result<()> {
    let exe = std::env::current_exe().wrap_err("failed to resolve current executable")?;
    let exe_dir = exe
        .parent()
        .ok_or_else(|| eyre!("failed to resolve executable directory"))?
        .to_path_buf();

    ensure_executable(&exe)?;

    let unit_source = exe_dir.join("ssh_clipboard.service");
    let unit_link = user_unit_link_path()?;
    let bin_link = PathBuf::from("/usr/local/bin/ssh_clipboard");

    if args.dry_run {
        println!("dry-run: would run `systemctl --user disable --now ssh_clipboard.service`");
        println!("dry-run: would remove unit link {}", unit_link.display());
        println!(
            "dry-run: would remove unit source {}",
            unit_source.display()
        );
        println!("dry-run: would remove binary link {}", bin_link.display());
        return Ok(());
    }

    disable_service_if_present()?;
    remove_unit_link(&unit_link)?;
    remove_unit_source(&unit_source)?;
    remove_bin_link_if_matches(&exe, &bin_link, args.no_sudo)?;
    print_uninstall_success(&bin_link, &unit_source, &unit_link)?;
    Ok(())
}

fn disable_service_if_present() -> Result<()> {
    let output = Command::new("systemctl")
        .args(["--user", "disable", "--now", "ssh_clipboard.service"])
        .output()
        .wrap_err("failed to run systemctl disable")?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.contains("not loaded") || stderr.contains("not found") {
        return Ok(());
    }
    if stderr.contains("Failed to connect to bus") || stderr.contains("No medium found") {
        return Err(eyre!(
            "systemctl --user failed (no user bus). Try: loginctl enable-linger $USER, then re-run uninstall-daemon"
        ));
    }
    Err(eyre!("systemctl --user failed: {stderr}"))
}

fn remove_unit_link(link: &Path) -> Result<()> {
    if !link.exists() {
        return Ok(());
    }
    let meta = std::fs::symlink_metadata(link)?;
    if !meta.file_type().is_symlink() {
        return Err(eyre!(
            "{} exists and is not a symlink; remove manually if desired",
            link.display()
        ));
    }
    std::fs::remove_file(link).wrap_err("failed to remove unit link")?;
    Ok(())
}

fn remove_unit_source(path: &Path) -> Result<()> {
    if path.exists() {
        std::fs::remove_file(path).wrap_err("failed to remove unit source")?;
    }
    Ok(())
}

fn remove_bin_link_if_matches(exe: &Path, link: &Path, no_sudo: bool) -> Result<()> {
    if !link.exists() {
        return Ok(());
    }
    let meta = std::fs::symlink_metadata(link)?;
    if !meta.file_type().is_symlink() {
        return Err(eyre!(
            "{} exists and is not a symlink; remove manually if desired",
            link.display()
        ));
    }
    let target = std::fs::read_link(link).wrap_err("failed to read symlink target")?;
    if target != exe {
        return Err(eyre!(
            "{} points to {}; refusing to remove (not this install)",
            link.display(),
            target.display()
        ));
    }

    if is_root() {
        std::fs::remove_file(link).wrap_err("failed to remove binary link")?;
        return Ok(());
    }

    if no_sudo {
        return Err(eyre!(
            "root permissions required to remove {}; run: sudo rm -f {}",
            link.display(),
            link.display()
        ));
    }

    run_sudo(&["rm", "-f", &link.display().to_string()])?;
    Ok(())
}

fn print_uninstall_success(bin_link: &Path, unit_source: &Path, unit_link: &Path) -> Result<()> {
    println!("removed:");
    println!("- unit link: {}", unit_link.display());
    println!("- unit source: {}", unit_source.display());
    println!("- binary link: {}", bin_link.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_render_contains_execstart() {
        let contents = render_unit_file(Path::new("/usr/local/bin/ssh_clipboard"), None, 10, 7000);
        assert!(contents.contains("ExecStart=/usr/local/bin/ssh_clipboard daemon"));
        assert!(contents.contains("--max-size 10"));
        assert!(contents.contains("--io-timeout-ms 7000"));
    }

    #[test]
    fn unit_render_quotes_socket_path() {
        let contents = render_unit_file(
            Path::new("/usr/local/bin/ssh_clipboard"),
            Some(Path::new("/run/user/1000/ssh clipboard.sock")),
            10,
            7000,
        );
        assert!(contents.contains("--socket-path \"/run/user/1000/ssh clipboard.sock\""));
    }
}
