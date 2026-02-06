use auto_launch::AutoLaunchBuilder;
#[cfg(target_os = "macos")]
use auto_launch::MacOSLaunchMode;
use eyre::{Result, WrapErr, eyre};
use std::path::PathBuf;

fn expected_agent_binary_path() -> Result<PathBuf> {
    let exe = std::env::current_exe().wrap_err("current_exe failed")?;
    let agent_name = agent_binary_name();
    Ok(exe.with_file_name(agent_name))
}

fn agent_binary_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "ssh_clipboard_agent.exe"
    } else {
        "ssh_clipboard_agent"
    }
}

pub fn agent_binary_path() -> Result<PathBuf> {
    let agent_path = expected_agent_binary_path()?;
    if !agent_path.exists() {
        return Err(eyre!("agent binary not found at {}", agent_path.display()));
    }
    Ok(agent_path)
}

fn build_autolaunch(require_existing_binary: bool) -> Result<auto_launch::AutoLaunch> {
    let agent_path = if require_existing_binary {
        agent_binary_path()?
    } else {
        expected_agent_binary_path()?
    };
    let exe_str = agent_path
        .to_str()
        .ok_or_else(|| eyre!("agent binary path is not valid utf-8"))?
        .to_string();

    let mut builder = AutoLaunchBuilder::new();
    builder.set_app_name("ssh_clipboard");
    builder.set_app_path(&exe_str);
    builder.set_args(&["--autostart"]);

    #[cfg(target_os = "macos")]
    builder.set_macos_launch_mode(MacOSLaunchMode::LaunchAgent);

    builder.build().map_err(|err| eyre!(err))
}

pub fn enable() -> Result<()> {
    let auto = build_autolaunch(true)?;
    auto.enable().map_err(|err| eyre!(err))?;
    Ok(())
}

pub fn disable() -> Result<()> {
    let auto = build_autolaunch(false)?;
    auto.disable().map_err(|err| eyre!(err))?;
    Ok(())
}

pub fn is_enabled() -> Result<bool> {
    let auto = build_autolaunch(false)?;
    auto.is_enabled().map_err(|err| eyre!(err))
}

pub fn refresh() -> Result<()> {
    let auto = build_autolaunch(true)?;
    let _ = auto.disable();
    auto.enable().map_err(|err| eyre!(err))?;
    Ok(())
}
