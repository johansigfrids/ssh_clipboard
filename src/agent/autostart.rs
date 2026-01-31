use auto_launch::AutoLaunchBuilder;
use eyre::{Result, WrapErr, eyre};

fn build_autolaunch() -> Result<auto_launch::AutoLaunch> {
    let exe = std::env::current_exe().wrap_err("current_exe failed")?;
    let exe_str = exe
        .to_str()
        .ok_or_else(|| eyre!("executable path is not valid utf-8"))?
        .to_string();

    let mut builder = AutoLaunchBuilder::new();
    builder.set_app_name("ssh_clipboard");
    builder.set_app_path(&exe_str);
    builder.set_args(&["agent"]);

    #[cfg(target_os = "macos")]
    builder.set_use_launch_agent(true);

    builder.build().map_err(|err| eyre!(err))
}

pub fn enable() -> Result<()> {
    let auto = build_autolaunch()?;
    auto.enable().map_err(|err| eyre!(err))?;
    Ok(())
}

pub fn disable() -> Result<()> {
    let auto = build_autolaunch()?;
    auto.disable().map_err(|err| eyre!(err))?;
    Ok(())
}

pub fn is_enabled() -> Result<bool> {
    let auto = build_autolaunch()?;
    auto.is_enabled().map_err(|err| eyre!(err))
}

pub fn refresh() -> Result<()> {
    let auto = build_autolaunch()?;
    let _ = auto.disable();
    auto.enable().map_err(|err| eyre!(err))?;
    Ok(())
}
