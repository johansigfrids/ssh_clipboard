use crate::cli::{InstallClientArgs, UninstallClientArgs};
use eyre::{Result, WrapErr, eyre};
use std::env;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[cfg(target_os = "windows")]
use winreg::RegKey;
#[cfg(target_os = "windows")]
use winreg::RegValue;
#[cfg(target_os = "windows")]
use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, RegType};

#[cfg(not(target_os = "windows"))]
const PATH_MARKER_BEGIN: &str = "# >>> ssh_clipboard install-client >>>";
#[cfg(not(target_os = "windows"))]
const PATH_MARKER_END: &str = "# <<< ssh_clipboard install-client <<<";

#[derive(Clone, Copy, PartialEq, Eq)]
enum CheckStatus {
    Ok,
    Warn,
    Fail,
}

impl CheckStatus {
    fn label(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Warn => "warn",
            Self::Fail => "fail",
        }
    }
}

struct CheckOutcome {
    name: &'static str,
    status: CheckStatus,
    detail: String,
    hint: Option<String>,
}

impl CheckOutcome {
    fn ok(name: &'static str, detail: impl Into<String>) -> Self {
        Self {
            name,
            status: CheckStatus::Ok,
            detail: detail.into(),
            hint: None,
        }
    }

    fn warn(name: &'static str, detail: impl Into<String>, hint: impl Into<String>) -> Self {
        Self {
            name,
            status: CheckStatus::Warn,
            detail: detail.into(),
            hint: Some(hint.into()),
        }
    }

    fn fail(name: &'static str, detail: impl Into<String>, hint: Option<String>) -> Self {
        Self {
            name,
            status: CheckStatus::Fail,
            detail: detail.into(),
            hint,
        }
    }
}

pub fn run_install(args: InstallClientArgs) -> Result<()> {
    let mut checks = Vec::new();
    let result = run_install_inner(&args, &mut checks);
    if let Err(ref err) = result {
        checks.push(CheckOutcome::fail("install-client", err.to_string(), None));
    }
    print_report("install-client", &checks);
    if result.is_err() {
        return crate::cli::exit::exit_with_code(2, "install-client failed");
    }
    Ok(())
}

pub fn run_uninstall(args: UninstallClientArgs) -> Result<()> {
    let mut checks = Vec::new();
    let result = run_uninstall_inner(&args, &mut checks);
    if let Err(ref err) = result {
        checks.push(CheckOutcome::fail(
            "uninstall-client",
            err.to_string(),
            None,
        ));
    }
    print_report("uninstall-client", &checks);
    if result.is_err() {
        return crate::cli::exit::exit_with_code(2, "uninstall-client failed");
    }
    Ok(())
}

fn run_install_inner(args: &InstallClientArgs, checks: &mut Vec<CheckOutcome>) -> Result<()> {
    if args.target.trim().is_empty() {
        return Err(eyre!("--target must not be empty"));
    }

    let install_dir = resolve_install_dir(args.install_dir.clone())?;
    let current_exe = env::current_exe().wrap_err("failed to resolve current executable")?;
    let source_agent = current_exe.with_file_name(agent_binary_name());
    if !source_agent.exists() {
        return Err(eyre!(
            "agent binary not found at {}",
            source_agent.display()
        ));
    }

    let installed_cli = install_dir.join(cli_binary_name());
    let installed_agent = install_dir.join(agent_binary_name());

    install_binary(
        &current_exe,
        &installed_cli,
        args.force,
        args.dry_run,
        checks,
        "cli binary",
    )?;
    install_binary(
        &source_agent,
        &installed_agent,
        args.force,
        args.dry_run,
        checks,
        "agent binary",
    )?;

    if args.no_path_update {
        checks.push(CheckOutcome::ok(
            "path update",
            "skipped (--no-path-update)",
        ));
    } else {
        update_path_for_install(&install_dir, args.dry_run, checks)?;
    }

    let setup_args = build_setup_agent_args(args);
    run_cli_command(
        &installed_cli,
        &setup_args,
        args.dry_run,
        checks,
        "setup-agent",
        true,
    )?;

    let status_result = run_cli_command(
        &installed_cli,
        &["autostart".into(), "status".into()],
        args.dry_run,
        checks,
        "autostart status",
        true,
    )?;
    if !args.dry_run {
        let stdout = status_result
            .as_ref()
            .map(|r| r.stdout.trim().to_string())
            .unwrap_or_default();
        if stdout != "enabled" {
            return Err(eyre!(
                "autostart status check failed (expected `enabled`, got `{}`)",
                if stdout.is_empty() {
                    "<empty>"
                } else {
                    &stdout
                }
            ));
        }
    }

    let doctor_args = build_doctor_args(args);
    match run_cli_command(
        &installed_cli,
        &doctor_args,
        args.dry_run,
        checks,
        "doctor verify",
        false,
    ) {
        Ok(Some(_)) | Ok(None) => {}
        Err(err) => checks.push(CheckOutcome::warn(
            "doctor verify",
            err.to_string(),
            "local install succeeded; fix remote setup and rerun `ssh_clipboard doctor`",
        )),
    }

    if args.no_start_now {
        checks.push(CheckOutcome::ok("agent start", "skipped (--no-start-now)"));
    } else {
        start_agent_now(&installed_agent, args.dry_run, checks);
    }

    checks.push(CheckOutcome::ok(
        "result",
        format!("installed client binaries to {}", install_dir.display()),
    ));
    Ok(())
}

fn run_uninstall_inner(args: &UninstallClientArgs, checks: &mut Vec<CheckOutcome>) -> Result<()> {
    let install_dir = resolve_install_dir(args.install_dir.clone())?;
    let installed_cli = install_dir.join(cli_binary_name());
    let installed_agent = install_dir.join(agent_binary_name());

    disable_autostart_best_effort(&installed_cli, args.dry_run, checks);
    #[cfg(target_os = "windows")]
    stop_running_agent_best_effort(&installed_agent, args.dry_run, checks);

    if args.no_path_cleanup {
        checks.push(CheckOutcome::ok(
            "path cleanup",
            "skipped (--no-path-cleanup)",
        ));
    } else {
        cleanup_path_for_uninstall(&install_dir, args.dry_run, args.force, checks)?;
    }

    remove_file_with_policy(
        &installed_agent,
        args.dry_run,
        args.force,
        checks,
        "agent binary",
    )?;

    remove_cli_binary_with_policy(
        &installed_cli,
        args.dry_run,
        args.force,
        checks,
        "cli binary",
    )?;

    remove_dir_if_empty(&install_dir, args.dry_run, args.force, checks)?;

    checks.push(CheckOutcome::ok(
        "result",
        format!("uninstall finished for {}", install_dir.display()),
    ));
    Ok(())
}

fn cli_binary_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "ssh_clipboard.exe"
    } else {
        "ssh_clipboard"
    }
}

fn agent_binary_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "ssh_clipboard_agent.exe"
    } else {
        "ssh_clipboard_agent"
    }
}

fn install_binary(
    source: &Path,
    destination: &Path,
    force: bool,
    dry_run: bool,
    checks: &mut Vec<CheckOutcome>,
    label: &'static str,
) -> Result<()> {
    if !source.exists() {
        return Err(eyre!("{label} source does not exist: {}", source.display()));
    }

    let should_copy = should_copy_file(source, destination, force)?;
    if !should_copy {
        checks.push(CheckOutcome::ok(
            label,
            format!(
                "already using {} at {}",
                source.file_name().unwrap_or_default().to_string_lossy(),
                destination.display()
            ),
        ));
        return Ok(());
    }

    if dry_run {
        checks.push(CheckOutcome::ok(
            label,
            format!(
                "dry-run: would copy {} -> {}",
                source.display(),
                destination.display()
            ),
        ));
        return Ok(());
    }

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).wrap_err("failed to create install directory")?;
    }
    fs::copy(source, destination).wrap_err_with(|| {
        format!(
            "failed to copy {} to {}",
            source.display(),
            destination.display()
        )
    })?;
    ensure_executable(destination)?;

    checks.push(CheckOutcome::ok(
        label,
        format!("installed at {}", destination.display()),
    ));
    Ok(())
}

fn should_copy_file(source: &Path, destination: &Path, force: bool) -> Result<bool> {
    if paths_equivalent(source, destination)? {
        return Ok(false);
    }
    if destination.exists() && !force {
        return Err(eyre!(
            "{} already exists; use --force to overwrite",
            destination.display()
        ));
    }
    Ok(true)
}

fn ensure_executable(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        let metadata = fs::metadata(path).wrap_err("failed to read installed file metadata")?;
        let mut permissions = metadata.permissions();
        let mode = permissions.mode();
        if mode & 0o111 == 0 {
            permissions.set_mode(mode | 0o755);
            fs::set_permissions(path, permissions)
                .wrap_err("failed to set executable permissions")?;
        }
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

fn build_setup_agent_args(args: &InstallClientArgs) -> Vec<String> {
    let mut out = vec![
        "setup-agent".to_string(),
        "--target".to_string(),
        args.target.clone(),
    ];
    if let Some(port) = args.port {
        out.push("--port".to_string());
        out.push(port.to_string());
    }
    if let Some(identity_file) = &args.identity_file {
        out.push("--identity-file".to_string());
        out.push(identity_file.display().to_string());
    }
    for opt in &args.ssh_option {
        out.push("--ssh-option".to_string());
        out.push(opt.clone());
    }
    if args.clear_ssh_options {
        out.push("--clear-ssh-options".to_string());
    }
    if let Some(max_size) = args.max_size {
        out.push("--max-size".to_string());
        out.push(max_size.to_string());
    }
    if let Some(timeout_ms) = args.timeout_ms {
        out.push("--timeout-ms".to_string());
        out.push(timeout_ms.to_string());
    }
    if let Some(resync_frames) = args.resync_frames {
        out.push("--resync-frames".to_string());
        out.push(resync_frames.to_string());
    }
    if let Some(resync_max_bytes) = args.resync_max_bytes {
        out.push("--resync-max-bytes".to_string());
        out.push(resync_max_bytes.to_string());
    }
    out
}

fn build_doctor_args(args: &InstallClientArgs) -> Vec<String> {
    let mut out = vec![
        "doctor".to_string(),
        "--target".to_string(),
        args.target.clone(),
    ];
    if let Some(port) = args.port {
        out.push("--port".to_string());
        out.push(port.to_string());
    }
    if let Some(identity_file) = &args.identity_file {
        out.push("--identity-file".to_string());
        out.push(identity_file.display().to_string());
    }
    for opt in &args.ssh_option {
        out.push("--ssh-option".to_string());
        out.push(opt.clone());
    }
    if let Some(timeout_ms) = args.timeout_ms {
        out.push("--timeout-ms".to_string());
        out.push(timeout_ms.to_string());
    }
    out
}

struct CommandResult {
    stdout: String,
}

fn run_cli_command(
    cli_path: &Path,
    args: &[String],
    dry_run: bool,
    checks: &mut Vec<CheckOutcome>,
    name: &'static str,
    fail_on_error: bool,
) -> Result<Option<CommandResult>> {
    if dry_run {
        checks.push(CheckOutcome::ok(
            name,
            format!(
                "dry-run: would run `{}` {}",
                cli_path.display(),
                args.join(" ")
            ),
        ));
        return Ok(None);
    }

    let output = Command::new(cli_path)
        .args(args)
        .output()
        .wrap_err_with(|| format!("failed to run {}", cli_path.display()))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if output.status.success() {
        checks.push(CheckOutcome::ok(
            name,
            format!("command succeeded: {}", args.join(" ")),
        ));
        return Ok(Some(CommandResult { stdout }));
    }

    let message = format!(
        "command failed ({:?}): {}",
        output.status.code(),
        summarize_command_output(&stdout, &stderr)
    );
    if fail_on_error {
        return Err(eyre!("{name}: {message}"));
    }
    Err(eyre!("{message}"))
}

fn summarize_command_output(stdout: &str, stderr: &str) -> String {
    let stdout = stdout.trim();
    let stderr = stderr.trim();
    if !stderr.is_empty() {
        stderr.to_string()
    } else if !stdout.is_empty() {
        stdout.to_string()
    } else {
        "no output".to_string()
    }
}

fn start_agent_now(agent_path: &Path, dry_run: bool, checks: &mut Vec<CheckOutcome>) {
    if dry_run {
        checks.push(CheckOutcome::ok(
            "agent start",
            format!("dry-run: would launch {}", agent_path.display()),
        ));
        return;
    }

    let result = Command::new(agent_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    match result {
        Ok(_) => checks.push(CheckOutcome::ok(
            "agent start",
            format!("launched {}", agent_path.display()),
        )),
        Err(err) => checks.push(CheckOutcome::warn(
            "agent start",
            format!("failed to launch {}: {err}", agent_path.display()),
            "run `ssh_clipboard agent` manually",
        )),
    }
}

fn disable_autostart_best_effort(cli_path: &Path, dry_run: bool, checks: &mut Vec<CheckOutcome>) {
    if dry_run {
        checks.push(CheckOutcome::ok(
            "autostart disable",
            "dry-run: would disable autostart",
        ));
        return;
    }

    let result = if cli_path.exists() {
        Command::new(cli_path)
            .args(["autostart", "disable"])
            .output()
            .ok()
            .map(|output| {
                if output.status.success() {
                    Ok(())
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                    Err(eyre!("cli autostart disable failed: {stderr}"))
                }
            })
            .unwrap_or_else(|| Err(eyre!("failed to spawn installed cli")))
    } else {
        crate::agent::autostart::disable()
    };

    match result {
        Ok(()) => checks.push(CheckOutcome::ok("autostart disable", "disabled")),
        Err(err) => checks.push(CheckOutcome::warn(
            "autostart disable",
            err.to_string(),
            "you can rerun `ssh_clipboard autostart disable` manually",
        )),
    }
}

#[cfg(target_os = "windows")]
fn stop_running_agent_best_effort(
    installed_agent: &Path,
    dry_run: bool,
    checks: &mut Vec<CheckOutcome>,
) {
    if dry_run {
        checks.push(CheckOutcome::ok(
            "agent stop",
            format!(
                "dry-run: would stop running {} processes",
                agent_binary_name()
            ),
        ));
        return;
    }

    if !installed_agent.exists() {
        checks.push(CheckOutcome::ok(
            "agent stop",
            "agent binary not present; skipped",
        ));
        return;
    }

    let output = Command::new("taskkill")
        .args(["/IM", agent_binary_name(), "/T", "/F"])
        .output();
    match output {
        Ok(output) if output.status.success() => checks.push(CheckOutcome::ok(
            "agent stop",
            format!("stopped running {}", agent_binary_name()),
        )),
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let report = summarize_command_output(&stdout, &stderr);
            let no_process =
                output.status.code() == Some(128) || taskkill_report_indicates_no_process(&report);
            if no_process {
                checks.push(CheckOutcome::ok(
                    "agent stop",
                    format!("no running {} process found", agent_binary_name()),
                ));
            } else {
                checks.push(CheckOutcome::warn(
                    "agent stop",
                    format!("failed to stop {}: {}", agent_binary_name(), report),
                    "continuing uninstall; close the agent manually if file removal fails",
                ));
            }
        }
        Err(err) => checks.push(CheckOutcome::warn(
            "agent stop",
            format!("failed to run taskkill: {err}"),
            "continuing uninstall; close the agent manually if file removal fails",
        )),
    }
}

#[cfg(target_os = "windows")]
fn taskkill_report_indicates_no_process(report: &str) -> bool {
    let text = report.to_ascii_lowercase();
    text.contains("not found") || text.contains("no running instance")
}

fn remove_file_with_policy(
    path: &Path,
    dry_run: bool,
    force: bool,
    checks: &mut Vec<CheckOutcome>,
    label: &'static str,
) -> Result<()> {
    if !path.exists() {
        checks.push(CheckOutcome::ok(
            label,
            format!("not present: {}", path.display()),
        ));
        return Ok(());
    }
    if dry_run {
        checks.push(CheckOutcome::ok(
            label,
            format!("dry-run: would remove {}", path.display()),
        ));
        return Ok(());
    }
    match fs::remove_file(path) {
        Ok(()) => {
            checks.push(CheckOutcome::ok(
                label,
                format!("removed {}", path.display()),
            ));
            Ok(())
        }
        #[cfg(target_os = "windows")]
        Err(err) if is_windows_file_in_use(&err) => match schedule_windows_deferred_delete(path) {
            Ok(()) => {
                checks.push(CheckOutcome::warn(
                    label,
                    format!(
                        "file in use; scheduled deferred removal for {}",
                        path.display()
                    ),
                    "close remaining processes if the file still exists",
                ));
                Ok(())
            }
            Err(schedule_err) if force => {
                checks.push(CheckOutcome::warn(
                    label,
                    format!(
                        "failed to remove {} ({err}) and deferred removal failed ({schedule_err})",
                        path.display()
                    ),
                    "continuing because --force is set",
                ));
                Ok(())
            }
            Err(schedule_err) => Err(eyre!(
                "failed to remove {} ({err}); deferred removal failed: {schedule_err}",
                path.display()
            )),
        },
        Err(err) if force => {
            checks.push(CheckOutcome::warn(
                label,
                format!("failed to remove {}: {err}", path.display()),
                "continuing because --force is set",
            ));
            Ok(())
        }
        Err(err) => Err(eyre!("failed to remove {}: {err}", path.display())),
    }
}

#[cfg(target_os = "windows")]
fn is_windows_file_in_use(err: &std::io::Error) -> bool {
    matches!(err.raw_os_error(), Some(5) | Some(32))
}

fn remove_cli_binary_with_policy(
    path: &Path,
    dry_run: bool,
    force: bool,
    checks: &mut Vec<CheckOutcome>,
    label: &'static str,
) -> Result<()> {
    if !path.exists() {
        checks.push(CheckOutcome::ok(
            label,
            format!("not present: {}", path.display()),
        ));
        return Ok(());
    }
    if dry_run {
        checks.push(CheckOutcome::ok(
            label,
            format!("dry-run: would remove {}", path.display()),
        ));
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        let current = env::current_exe().ok();
        if current
            .as_ref()
            .map(|current| paths_equivalent(current, path).unwrap_or(false))
            .unwrap_or(false)
        {
            match schedule_windows_deferred_delete(path) {
                Ok(()) => {
                    checks.push(CheckOutcome::warn(
                        label,
                        format!("scheduled deferred removal for {}", path.display()),
                        "close remaining processes if the file still exists",
                    ));
                    return Ok(());
                }
                Err(err) if force => {
                    checks.push(CheckOutcome::warn(
                        label,
                        format!("failed to schedule deferred delete: {err}"),
                        "continuing because --force is set",
                    ));
                    return Ok(());
                }
                Err(err) => return Err(err),
            }
        }
    }

    remove_file_with_policy(path, dry_run, force, checks, label)
}

#[cfg(target_os = "windows")]
fn schedule_windows_deferred_delete(path: &Path) -> Result<()> {
    let target = path.display().to_string().replace('"', "\"\"");
    let script = format!("ping 127.0.0.1 -n 2 > nul && del /F /Q \"{target}\"");
    Command::new("cmd")
        .arg("/C")
        .arg(script)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .wrap_err("failed to schedule deferred self-delete")?;
    Ok(())
}

fn remove_dir_if_empty(
    path: &Path,
    dry_run: bool,
    force: bool,
    checks: &mut Vec<CheckOutcome>,
) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let mut iter = fs::read_dir(path).wrap_err("failed to read install directory")?;
    let has_entries = iter.next().transpose()?.is_some();
    if has_entries {
        checks.push(CheckOutcome::warn(
            "install dir",
            format!("directory not empty, kept {}", path.display()),
            "remove leftover files manually if desired",
        ));
        return Ok(());
    }
    if dry_run {
        checks.push(CheckOutcome::ok(
            "install dir",
            format!("dry-run: would remove empty directory {}", path.display()),
        ));
        return Ok(());
    }
    match fs::remove_dir(path) {
        Ok(()) => {
            checks.push(CheckOutcome::ok(
                "install dir",
                format!("removed empty directory {}", path.display()),
            ));
            Ok(())
        }
        Err(err) if force => {
            checks.push(CheckOutcome::warn(
                "install dir",
                format!("failed to remove {}: {err}", path.display()),
                "continuing because --force is set",
            ));
            Ok(())
        }
        Err(err) => Err(eyre!("failed to remove {}: {err}", path.display())),
    }
}

fn update_path_for_install(
    install_dir: &Path,
    dry_run: bool,
    checks: &mut Vec<CheckOutcome>,
) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        let detail = upsert_windows_user_path(install_dir, dry_run)?;
        checks.push(CheckOutcome::ok("path update", detail));
    }
    #[cfg(not(target_os = "windows"))]
    {
        let detail = upsert_unix_shell_path_block(install_dir, dry_run)?;
        checks.push(CheckOutcome::ok("path update", detail));
    }
    Ok(())
}

fn cleanup_path_for_uninstall(
    install_dir: &Path,
    dry_run: bool,
    force: bool,
    checks: &mut Vec<CheckOutcome>,
) -> Result<()> {
    #[cfg(not(target_os = "windows"))]
    let _ = install_dir;

    #[cfg(target_os = "windows")]
    {
        match remove_windows_user_path_entry(install_dir, dry_run) {
            Ok(detail) => checks.push(CheckOutcome::ok("path cleanup", detail)),
            Err(err) if force => checks.push(CheckOutcome::warn(
                "path cleanup",
                err.to_string(),
                "continuing because --force is set",
            )),
            Err(err) => return Err(err),
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        match remove_unix_shell_path_block(dry_run) {
            Ok(detail) => checks.push(CheckOutcome::ok("path cleanup", detail)),
            Err(err) if force => checks.push(CheckOutcome::warn(
                "path cleanup",
                err.to_string(),
                "continuing because --force is set",
            )),
            Err(err) => return Err(err),
        }
    }
    Ok(())
}

fn resolve_install_dir(override_dir: Option<PathBuf>) -> Result<PathBuf> {
    let raw = if let Some(path) = override_dir {
        path
    } else {
        default_install_dir()?
    };
    if raw.is_absolute() {
        return Ok(raw);
    }
    let cwd = env::current_dir().wrap_err("failed to resolve current directory")?;
    Ok(cwd.join(raw))
}

fn default_install_dir() -> Result<PathBuf> {
    #[cfg(target_os = "windows")]
    let is_windows = true;
    #[cfg(not(target_os = "windows"))]
    let is_windows = false;

    default_install_dir_for(
        is_windows,
        home_dir().as_deref(),
        env::var_os("LOCALAPPDATA").map(PathBuf::from).as_deref(),
    )
}

fn default_install_dir_for(
    is_windows: bool,
    home: Option<&Path>,
    local_app_data: Option<&Path>,
) -> Result<PathBuf> {
    if is_windows {
        if let Some(local_app_data) = local_app_data {
            return Ok(local_app_data.join("ssh_clipboard").join("bin"));
        }
        let home = home.ok_or_else(|| eyre!("cannot resolve home directory"))?;
        Ok(home
            .join("AppData")
            .join("Local")
            .join("ssh_clipboard")
            .join("bin"))
    } else {
        let home = home.ok_or_else(|| eyre!("cannot resolve home directory"))?;
        Ok(home.join(".local").join("bin"))
    }
}

fn home_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        env::var_os("USERPROFILE").map(PathBuf::from).or_else(|| {
            let drive = env::var_os("HOMEDRIVE")?;
            let path = env::var_os("HOMEPATH")?;
            Some(PathBuf::from(format!(
                "{}{}",
                drive.to_string_lossy(),
                path.to_string_lossy()
            )))
        })
    }
    #[cfg(not(target_os = "windows"))]
    {
        env::var_os("HOME").map(PathBuf::from)
    }
}

fn paths_equivalent(a: &Path, b: &Path) -> Result<bool> {
    if a == b {
        return Ok(true);
    }
    let a_canon = fs::canonicalize(a).unwrap_or_else(|_| a.to_path_buf());
    let b_canon = fs::canonicalize(b).unwrap_or_else(|_| b.to_path_buf());
    Ok(a_canon == b_canon)
}

fn print_report(title: &str, checks: &[CheckOutcome]) {
    println!("ssh_clipboard {title}");
    for check in checks {
        println!(
            "[{}] {}: {}",
            check.status.label(),
            check.name,
            check.detail
        );
        if let Some(hint) = &check.hint {
            println!("      hint: {hint}");
        }
    }

    let ok = checks
        .iter()
        .filter(|check| check.status == CheckStatus::Ok)
        .count();
    let warn = checks
        .iter()
        .filter(|check| check.status == CheckStatus::Warn)
        .count();
    let fail = checks
        .iter()
        .filter(|check| check.status == CheckStatus::Fail)
        .count();
    println!("summary: {ok} ok, {warn} warning(s), {fail} failure(s)");
}

#[cfg(target_os = "windows")]
fn upsert_windows_user_path(install_dir: &Path, dry_run: bool) -> Result<String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let env_key = hkcu
        .open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
        .wrap_err("failed to open HKCU\\Environment")?;
    let existing: String = env_key.get_value("Path").unwrap_or_default();
    let path_value_type = existing_windows_path_value_type(&env_key);
    let install = install_dir.display().to_string();
    let (updated, changed) = add_path_entry(&existing, &install, ';', true);
    if !changed {
        return Ok(format!(
            "already present in user PATH: {}",
            install_dir.display()
        ));
    }
    if dry_run {
        return Ok(format!(
            "dry-run: would add {} to user PATH",
            install_dir.display()
        ));
    }
    set_windows_path_value(&env_key, &updated, path_value_type)?;
    Ok(format!("added {} to user PATH", install_dir.display()))
}

#[cfg(target_os = "windows")]
fn remove_windows_user_path_entry(install_dir: &Path, dry_run: bool) -> Result<String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let env_key = hkcu
        .open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
        .wrap_err("failed to open HKCU\\Environment")?;
    let existing: String = env_key.get_value("Path").unwrap_or_default();
    let path_value_type = existing_windows_path_value_type(&env_key);
    let install = install_dir.display().to_string();
    let (updated, changed) = remove_path_entry(&existing, &install, ';', true);
    if !changed {
        return Ok(format!(
            "user PATH did not contain {}",
            install_dir.display()
        ));
    }
    if dry_run {
        return Ok(format!(
            "dry-run: would remove {} from user PATH",
            install_dir.display()
        ));
    }
    set_windows_path_value(&env_key, &updated, path_value_type)?;
    Ok(format!("removed {} from user PATH", install_dir.display()))
}

#[cfg(target_os = "windows")]
fn existing_windows_path_value_type(env_key: &RegKey) -> RegType {
    match env_key.get_raw_value("Path") {
        Ok(value) if value.vtype == RegType::REG_EXPAND_SZ => RegType::REG_EXPAND_SZ,
        Ok(_) => RegType::REG_SZ,
        Err(_) => RegType::REG_EXPAND_SZ,
    }
}

#[cfg(target_os = "windows")]
fn set_windows_path_value(env_key: &RegKey, value: &str, existing_type: RegType) -> Result<()> {
    if existing_type == RegType::REG_EXPAND_SZ {
        let reg_value = utf16_string_reg_value(value, RegType::REG_EXPAND_SZ);
        env_key
            .set_raw_value("Path", &reg_value)
            .wrap_err("failed to update HKCU\\Environment Path")?;
    } else {
        env_key
            .set_value("Path", &value)
            .wrap_err("failed to update HKCU\\Environment Path")?;
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn utf16_string_reg_value(value: &str, value_type: RegType) -> RegValue {
    let mut bytes = Vec::with_capacity((value.encode_utf16().count() + 1) * 2);
    for unit in value.encode_utf16().chain(std::iter::once(0)) {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    RegValue {
        bytes,
        vtype: value_type,
    }
}

#[cfg(not(target_os = "windows"))]
fn upsert_unix_shell_path_block(install_dir: &Path, dry_run: bool) -> Result<String> {
    if env_path_contains(install_dir) {
        return Ok(format!(
            "already present in current PATH: {}",
            install_dir.display()
        ));
    }
    let home = home_dir().ok_or_else(|| eyre!("cannot resolve home directory"))?;
    let profile = preferred_profile_file(&home);
    let existing = if profile.exists() {
        fs::read_to_string(&profile).wrap_err("failed to read shell profile")?
    } else {
        String::new()
    };
    let block = render_path_block(install_dir);
    let (updated, changed) = upsert_managed_block(&existing, &block)?;
    if !changed {
        return Ok(format!(
            "PATH block already present in {}",
            profile.display()
        ));
    }
    if dry_run {
        return Ok(format!(
            "dry-run: would update {} with PATH block",
            profile.display()
        ));
    }
    fs::write(&profile, updated).wrap_err("failed to write shell profile")?;
    Ok(format!(
        "updated {} (restart shell to refresh PATH)",
        profile.display()
    ))
}

#[cfg(not(target_os = "windows"))]
fn remove_unix_shell_path_block(dry_run: bool) -> Result<String> {
    let home = home_dir().ok_or_else(|| eyre!("cannot resolve home directory"))?;
    let candidates = candidate_profile_files(&home);
    let mut removed_from = Vec::new();
    for profile in candidates {
        if !profile.exists() {
            continue;
        }
        let existing = fs::read_to_string(&profile).wrap_err("failed to read shell profile")?;
        let (updated, changed) = remove_managed_block(&existing)?;
        if !changed {
            continue;
        }
        if !dry_run {
            fs::write(&profile, updated).wrap_err("failed to write shell profile")?;
        }
        removed_from.push(profile);
    }

    if removed_from.is_empty() {
        return Ok("no managed PATH block found".to_string());
    }
    let files = removed_from
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    if dry_run {
        Ok(format!("dry-run: would remove PATH block from {files}"))
    } else {
        Ok(format!("removed PATH block from {files}"))
    }
}

#[cfg(any(target_os = "windows", test))]
fn add_path_entry(
    path_list: &str,
    entry: &str,
    sep: char,
    case_insensitive: bool,
) -> (String, bool) {
    let mut entries = split_path_list(path_list, sep);
    let normalized_entry = normalize_path_entry(entry, case_insensitive);
    let already = entries
        .iter()
        .any(|candidate| normalize_path_entry(candidate, case_insensitive) == normalized_entry);
    if !already {
        entries.push(entry.to_string());
    }
    let joined = entries.join(&sep.to_string());
    let changed = joined != path_list;
    (joined, changed)
}

#[cfg(any(target_os = "windows", test))]
fn remove_path_entry(
    path_list: &str,
    entry: &str,
    sep: char,
    case_insensitive: bool,
) -> (String, bool) {
    let normalized_entry = normalize_path_entry(entry, case_insensitive);
    let old_entries = split_path_list(path_list, sep);
    let new_entries = old_entries
        .iter()
        .filter(|candidate| normalize_path_entry(candidate, case_insensitive) != normalized_entry)
        .cloned()
        .collect::<Vec<_>>();
    let joined = new_entries.join(&sep.to_string());
    let changed = joined != path_list;
    (joined, changed)
}

#[cfg(any(target_os = "windows", test))]
fn split_path_list(path_list: &str, sep: char) -> Vec<String> {
    path_list
        .split(sep)
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn normalize_path_entry(path: &str, case_insensitive: bool) -> String {
    let mut out = path.trim().replace('\\', "/");
    while out.ends_with('/') && out.len() > 1 {
        out.pop();
    }
    if case_insensitive {
        out.make_ascii_lowercase();
    }
    out
}

#[cfg(not(target_os = "windows"))]
fn env_path_contains(path: &Path) -> bool {
    let Some(path_var) = env::var_os("PATH") else {
        return false;
    };
    let target = normalize_path_entry(&path.display().to_string(), false);
    env::split_paths(&path_var)
        .any(|entry| normalize_path_entry(&entry.display().to_string(), false) == target)
}

#[cfg(not(target_os = "windows"))]
fn preferred_profile_file(home: &Path) -> PathBuf {
    let shell = env::var("SHELL").unwrap_or_default();
    if shell.contains("zsh") {
        home.join(".zprofile")
    } else {
        home.join(".profile")
    }
}

#[cfg(not(target_os = "windows"))]
fn candidate_profile_files(home: &Path) -> Vec<PathBuf> {
    let mut files = vec![
        preferred_profile_file(home),
        home.join(".profile"),
        home.join(".zprofile"),
    ];
    files.sort();
    files.dedup();
    files
}

#[cfg(not(target_os = "windows"))]
fn render_path_block(install_dir: &Path) -> String {
    let escaped = install_dir.display().to_string().replace('"', "\\\"");
    format!("{PATH_MARKER_BEGIN}\nexport PATH=\"{escaped}:$PATH\"\n{PATH_MARKER_END}\n")
}

#[cfg(not(target_os = "windows"))]
fn upsert_managed_block(contents: &str, block: &str) -> Result<(String, bool)> {
    let begin = contents.find(PATH_MARKER_BEGIN);
    let end = contents.find(PATH_MARKER_END);
    match (begin, end) {
        (Some(begin_idx), Some(end_idx)) if end_idx >= begin_idx => {
            let end_idx = end_idx + PATH_MARKER_END.len();
            let mut updated = String::new();
            updated.push_str(&contents[..begin_idx]);
            if !updated.ends_with('\n') && !updated.is_empty() {
                updated.push('\n');
            }
            updated.push_str(block);
            let tail = contents[end_idx..].trim_start_matches('\n');
            if !tail.is_empty() {
                updated.push_str(tail);
                if !updated.ends_with('\n') {
                    updated.push('\n');
                }
            }
            let changed = updated != contents;
            Ok((updated, changed))
        }
        (Some(_), None) | (None, Some(_)) => Err(eyre!("managed PATH block markers are malformed")),
        (None, None) => {
            let mut updated = contents.to_string();
            if !updated.is_empty() && !updated.ends_with('\n') {
                updated.push('\n');
            }
            updated.push_str(block);
            let changed = updated != contents;
            Ok((updated, changed))
        }
        _ => Err(eyre!("unexpected marker state")),
    }
}

#[cfg(not(target_os = "windows"))]
fn remove_managed_block(contents: &str) -> Result<(String, bool)> {
    let begin = contents.find(PATH_MARKER_BEGIN);
    let end = contents.find(PATH_MARKER_END);
    match (begin, end) {
        (Some(begin_idx), Some(end_idx)) if end_idx >= begin_idx => {
            let end_idx = end_idx + PATH_MARKER_END.len();
            let mut updated = String::new();
            updated.push_str(&contents[..begin_idx]);
            let tail = contents[end_idx..].trim_start_matches('\n');
            if !tail.is_empty() {
                if !updated.ends_with('\n') && !updated.is_empty() {
                    updated.push('\n');
                }
                updated.push_str(tail);
            }
            if !updated.is_empty() && !updated.ends_with('\n') {
                updated.push('\n');
            }
            let changed = updated != contents;
            Ok((updated, changed))
        }
        (Some(_), None) | (None, Some(_)) => Err(eyre!("managed PATH block markers are malformed")),
        (None, None) => Ok((contents.to_string(), false)),
        _ => Err(eyre!("unexpected marker state")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_install_dir_windows_prefers_localappdata() {
        let dir = default_install_dir_for(
            true,
            Some(Path::new("C:/Users/test")),
            Some(Path::new("C:/Users/test/AppData/Local")),
        )
        .unwrap();
        assert_eq!(
            dir,
            PathBuf::from("C:/Users/test/AppData/Local/ssh_clipboard/bin")
        );
    }

    #[test]
    fn default_install_dir_unix_uses_home_local_bin() {
        let dir = default_install_dir_for(false, Some(Path::new("/home/test")), None).unwrap();
        assert_eq!(dir, PathBuf::from("/home/test/.local/bin"));
    }

    #[test]
    fn add_remove_path_entry_is_idempotent() {
        let (added, changed) = add_path_entry("/usr/bin:/bin", "/home/u/.local/bin", ':', false);
        assert!(changed);
        let (added_again, changed_again) =
            add_path_entry(&added, "/home/u/.local/bin/", ':', false);
        assert!(!changed_again);
        assert_eq!(added, added_again);

        let (removed, changed_removed) =
            remove_path_entry(&added_again, "/home/u/.local/bin", ':', false);
        assert!(changed_removed);
        assert_eq!(removed, "/usr/bin:/bin");
    }

    #[test]
    fn add_path_entry_normalizes_empty_segments() {
        let (updated, _) = add_path_entry("A;;B;", "C", ';', true);
        assert_eq!(updated, "A;B;C");
    }

    #[test]
    fn should_copy_file_skips_when_same_path() {
        let source = Path::new("C:/x/ssh_clipboard.exe");
        let dest = Path::new("C:/x/ssh_clipboard.exe");
        let should_copy = should_copy_file(source, dest, false).unwrap();
        assert!(!should_copy);
    }

    #[test]
    fn should_copy_file_errors_when_existing_without_force() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("src");
        let dest = temp.path().join("dst");
        fs::write(&source, b"a").unwrap();
        fs::write(&dest, b"b").unwrap();
        let err = should_copy_file(&source, &dest, false).unwrap_err();
        assert!(err.to_string().contains("already exists"));
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn upsert_and_remove_managed_block_are_idempotent() {
        let block = render_path_block(Path::new("/home/u/.local/bin"));
        let (once, changed_once) = upsert_managed_block("", &block).unwrap();
        assert!(changed_once);
        let (twice, changed_twice) = upsert_managed_block(&once, &block).unwrap();
        assert!(!changed_twice);
        assert_eq!(once, twice);

        let (removed, changed_removed) = remove_managed_block(&twice).unwrap();
        assert!(changed_removed);
        assert_eq!(removed, "");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn detects_windows_file_in_use_errors() {
        assert!(is_windows_file_in_use(&std::io::Error::from_raw_os_error(
            5
        )));
        assert!(is_windows_file_in_use(&std::io::Error::from_raw_os_error(
            32
        )));
        assert!(!is_windows_file_in_use(&std::io::Error::from_raw_os_error(
            2
        )));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn taskkill_not_found_report_is_detected() {
        assert!(taskkill_report_indicates_no_process(
            "ERROR: The process \"ssh_clipboard_agent.exe\" not found."
        ));
        assert!(taskkill_report_indicates_no_process(
            "No running instance of the task."
        ));
        assert!(!taskkill_report_indicates_no_process("Access is denied."));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn utf16_registry_value_roundtrips() {
        let input = r"%USERPROFILE%\AppData\Local\ssh_clipboard\bin";
        let reg_value = utf16_string_reg_value(input, RegType::REG_EXPAND_SZ);
        assert_eq!(reg_value.vtype, RegType::REG_EXPAND_SZ);
        let words = reg_value
            .bytes
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect::<Vec<_>>();
        assert_eq!(words.last().copied(), Some(0));
        let decoded = String::from_utf16(&words[..words.len() - 1]).unwrap();
        assert_eq!(decoded, input);
    }
}
