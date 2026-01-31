#[cfg(target_os = "linux")]
mod linux {
    use clap::{Parser, Subcommand};
    use eyre::{Result, WrapErr};
    use ssh_clipboard::daemon::default_socket_path;
    use ssh_clipboard::protocol::DEFAULT_MAX_SIZE;
    use ssh_clipboard::{daemon, proxy};
    use std::path::PathBuf;
    use tracing_subscriber::EnvFilter;

    #[derive(Parser)]
    #[command(name = "ssh_clipboard", version, about = "SSH clipboard daemon/proxy")]
    struct Cli {
        #[command(subcommand)]
        command: Commands,
    }

    #[derive(Subcommand)]
    enum Commands {
        Daemon {
            #[arg(long)]
            socket_path: Option<PathBuf>,
            #[arg(long, default_value_t = DEFAULT_MAX_SIZE)]
            max_size: usize,
        },
        Proxy {
            #[arg(long)]
            socket_path: Option<PathBuf>,
            #[arg(long, default_value_t = DEFAULT_MAX_SIZE)]
            max_size: usize,
        },
    }

    #[tokio::main]
    async fn main() -> Result<()> {
        tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::from_default_env())
            .init();

        let cli = Cli::parse();
        match cli.command {
            Commands::Daemon {
                socket_path,
                max_size,
            } => {
                let socket_path = socket_path.unwrap_or(default_socket_path()?);
                daemon::run_daemon(socket_path, max_size)
                    .await
                    .wrap_err("daemon failed")?;
            }
            Commands::Proxy {
                socket_path,
                max_size,
            } => {
                let socket_path = socket_path.unwrap_or(default_socket_path()?);
                let exit_code = proxy::run_proxy(socket_path, max_size)
                    .await
                    .wrap_err("proxy failed")?;
                std::process::exit(exit_code);
            }
        }
        Ok(())
    }
}

#[cfg(target_os = "linux")]
fn main() -> eyre::Result<()> {
    linux::main()
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("ssh_clipboard Phase 1 is Linux-only (daemon/proxy)");
    std::process::exit(1);
}
