#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use clap::Parser;
use eyre::Result;

#[derive(Parser)]
#[command(name = "ssh_clipboard_agent", version, about = "SSH clipboard agent (tray + hotkeys)")]
struct AgentCli {
    #[arg(long)]
    no_tray: bool,
    #[arg(long)]
    no_hotkeys: bool,
    #[arg(long, hide = true)]
    autostart: bool,
}

fn main() -> Result<()> {
    let args = AgentCli::parse();
    ssh_clipboard::cli::init_tracing_for_agent()?;
    ssh_clipboard::agent::run::run_agent(args.no_tray, args.no_hotkeys, args.autostart)
}
