//! Read-only server package verification. Install/update actions are not exposed yet.
use crate::server_package::{MAX_PACKAGE_BYTES, Policy, native_platform};
use crate::server_releases::Verifier;
use clap::{Args, Subcommand};
use eyre::{Result, bail};
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Args)]
pub struct MaintenanceArgs {
    #[command(subcommand)]
    command: Commands,
}
#[derive(Subcommand)]
enum Commands {
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

pub fn run(args: MaintenanceArgs) -> Result<()> {
    match args.command {
        Commands::Verify {
            version,
            public_key,
            signature,
            package,
        } => {
            let public_key = String::from_utf8(read(&public_key, 4096)?)?;
            let signature = String::from_utf8(read(&signature, 16384)?)?;
            let bytes = read(&package, MAX_PACKAGE_BYTES)?;
            let platform = native_platform()?;
            Policy::new(&public_key, platform)?.verify(&version, signature.trim(), &bytes)?;
            println!(
                "Verified server package {version} for {platform}. Nothing was executed or installed."
            );
            Ok(())
        }
    }
}

fn read(path: &Path, limit: usize) -> Result<Vec<u8>> {
    let mut file = std::fs::File::open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() > limit as u64 {
        bail!("package input exceeds its limit or is not a regular file");
    }
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 16384];
    loop {
        let count = file.read(&mut chunk)?;
        if count == 0 {
            break;
        }
        if count > limit.saturating_sub(bytes.len()) {
            bail!("package input grew beyond its limit");
        }
        bytes.try_reserve_exact(count)?;
        bytes.extend_from_slice(&chunk[..count]);
    }
    Ok(bytes)
}
