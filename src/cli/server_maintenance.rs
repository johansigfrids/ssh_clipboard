//! Explicit, per-user signed server maintenance. Run blocking orchestration off the CLI runtime.
use super::ShelfDaemonArgs;
use crate::server_management::{Manager, default_root};
use crate::server_package::{MAX_PACKAGE_BYTES, Policy, native_platform};
use crate::server_releases::{Outcome, Verifier};
use clap::{Args, Subcommand};
use eyre::{Result, bail};
use std::path::PathBuf;

#[derive(Args)]
pub struct MaintenanceArgs {
    #[command(subcommand)]
    command: Commands,
}
#[derive(Subcommand)]
enum Commands {
    /// Create a separate managed user service. Does not adopt a legacy/global service.
    Install {
        #[command(flatten)]
        location: Location,
        /// Independently trusted signing key. Updates retain this pinned key.
        #[arg(long)]
        public_key: PathBuf,
        #[command(flatten)]
        release: Release,
        #[command(flatten)]
        daemon: ShelfDaemonArgs,
        /// Service restarts discard all in-memory shared clipboard items.
        #[arg(long, required = true)]
        acknowledge_clipboard_loss: bool,
    },
    /// Verify, stage, restart and check protocol health; restore the previous release on failure.
    Update {
        #[command(flatten)]
        location: Location,
        #[command(flatten)]
        release: Release,
        #[arg(long, required = true)]
        acknowledge_clipboard_loss: bool,
    },
    /// Resolve an interrupted transition by restoring the previously verified release.
    Recover {
        #[command(flatten)]
        location: Location,
        #[arg(long, required = true)]
        acknowledge_clipboard_loss: bool,
    },
    /// Show saved installation/selection metadata; does not assert that a service is healthy.
    Status {
        #[command(flatten)]
        location: Location,
    },
    /// Verify bytes and signed server identity without executing or installing them.
    Verify {
        #[arg(long)]
        version: String,
        /// Independently trusted public key, never a key supplied only by the package itself.
        #[arg(long)]
        public_key: PathBuf,
        #[arg(long)]
        signature: PathBuf,
        #[arg(long)]
        package: PathBuf,
    },
}

#[derive(Args)]
struct Location {
    /// Absolute managed data directory (default: user data/ssh_clipboard/server).
    #[arg(long)]
    root: Option<PathBuf>,
}
impl Location {
    fn path(self) -> Result<PathBuf> {
        self.root.map_or_else(default_root, Ok)
    }
}
#[derive(Args)]
struct Release {
    #[arg(long)]
    version: String,
    /// Local signed binary; omit both input flags to download the exact GitHub version.
    #[arg(long, requires = "signature")]
    package: Option<PathBuf>,
    #[arg(long, requires = "package")]
    signature: Option<PathBuf>,
}

pub async fn run(args: MaintenanceArgs) -> Result<()> {
    tokio::task::spawn_blocking(move || run_sync(args)).await?
}

fn run_sync(args: MaintenanceArgs) -> Result<()> {
    match args.command {
        Commands::Install {
            location,
            public_key,
            release,
            daemon,
            acknowledge_clipboard_loss,
        } => {
            acknowledge(acknowledge_clipboard_loss)?;
            let key = String::from_utf8(crate::server_management::input_file(&public_key, 4096)?)?;
            report(
                Manager::install(
                    &location.path()?,
                    key,
                    daemon.try_into()?,
                    &release.version,
                    release.package.as_deref(),
                    release.signature.as_deref(),
                )?,
                false,
            )
        }
        Commands::Update {
            location,
            release,
            acknowledge_clipboard_loss,
        } => {
            acknowledge(acknowledge_clipboard_loss)?;
            report(
                Manager::open(&location.path()?)?.update(
                    &release.version,
                    release.package.as_deref(),
                    release.signature.as_deref(),
                )?,
                false,
            )
        }
        Commands::Recover {
            location,
            acknowledge_clipboard_loss,
        } => {
            acknowledge(acknowledge_clipboard_loss)?;
            report(Manager::open(&location.path()?)?.recover()?, true)
        }
        Commands::Status { location } => {
            println!(
                "{}",
                serde_json::to_string_pretty(&Manager::open(&location.path()?)?.status()?)?
            );
            Ok(())
        }
        Commands::Verify {
            version,
            public_key,
            signature,
            package,
        } => {
            let public_key =
                String::from_utf8(crate::server_management::input_file(&public_key, 4096)?)?;
            let signature =
                String::from_utf8(crate::server_management::input_file(&signature, 16384)?)?;
            let bytes = crate::server_management::input_file(&package, MAX_PACKAGE_BYTES)?;
            let platform = native_platform()?;
            Policy::new(&public_key, platform)?.verify(&version, signature.trim(), &bytes)?;
            println!(
                "Verified server package {version} for {platform}. Nothing was executed or installed."
            );
            Ok(())
        }
    }
}

fn acknowledge(value: bool) -> Result<()> {
    if !value {
        bail!("Pass --acknowledge-clipboard-loss: restarts discard the in-memory shelf.");
    }
    eprintln!(
        "Maintenance may restart the service and discard in-memory clipboard items, including during rollback."
    );
    Ok(())
}

fn report(outcome: Outcome, recovery: bool) -> Result<()> {
    match outcome {
        Outcome::Activated => println!("Selected server release passed proxy protocol health."),
        Outcome::RestoredPrevious if recovery => {
            println!("Recovered the previous release; proxy protocol health passed.")
        }
        Outcome::RemovedFailedInitial if recovery => println!(
            "Recovered the failed initial installation; service disabled and selection removed."
        ),
        Outcome::RestoredPrevious => {
            bail!(
                "Requested release failed; previous executable restored and protocol health passed. In-memory items were not restored."
            );
        }
        Outcome::RemovedFailedInitial => {
            bail!(
                "Initial release failed; service disabled and selection removed. Inspect the user service locally before retrying."
            );
        }
    }
    Ok(())
}

impl TryFrom<super::ShelfDaemonArgs> for crate::server_management::DaemonSettings {
    type Error = eyre::Report;

    fn try_from(args: super::ShelfDaemonArgs) -> Result<Self> {
        Ok(Self {
            socket_path: match args.socket_path {
                Some(path) => path,
                None => crate::shelf_daemon::default_socket_path()?,
            },
            max_items: args.max_items,
            retention_secs: args.retention_secs,
            max_item_bytes: args.max_item_bytes,
            max_stored_bytes: args.max_stored_bytes,
            max_resident_bytes: args.max_resident_bytes,
            max_decoded_bytes: args.max_decoded_bytes,
            max_connections: args.max_connections,
            io_timeout_ms: args.io_timeout_ms,
            idle_timeout_ms: args.idle_timeout_ms,
        })
    }
}
