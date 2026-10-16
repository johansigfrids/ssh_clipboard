//! Explicit item-aware CLI. Metadata on stdout unless an explicit fetch exports bytes.
use super::{ClientConfigArgs, build_client_config, settings};
use crate::client::session::{Action, SessionPolicy};
use crate::client::session_transport::{self as transport, Operation, Outcome};
use crate::session_protocol::{ContentType, Origin, Representation};
use clap::{Args, Subcommand, ValueEnum};
use eyre::{Result, ensure, eyre};
use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;

#[derive(Args)]
pub struct ShelfArgs {
    #[arg(long, global = true)]
    profile: Option<String>,
    #[arg(long, global = true)]
    settings_file: Option<PathBuf>,
    #[arg(long, global = true)]
    target: Option<String>,
    #[arg(long, global = true)]
    port: Option<u16>,
    #[arg(long, global = true)]
    identity_file: Option<PathBuf>,
    #[arg(long, global = true)]
    ssh_option: Vec<String>,
    #[arg(long, global = true)]
    ssh_bin: Option<PathBuf>,
    #[arg(long, global = true)]
    timeout_ms: Option<u64>,
    /// Encoded payload budget for the new endpoint; independent of the V2 frame setting.
    #[arg(long, global=true, default_value_t=64*1024*1024)]
    max_item_bytes: u32,
    #[arg(long, global=true, default_value_t=256*1024*1024)]
    max_decoded_bytes: u32,
    #[command(subcommand)]
    command: ShelfCommand,
}
#[derive(Clone, Copy, ValueEnum)]
enum InputFormat {
    Text,
    Png,
}
#[derive(Subcommand)]
enum ShelfCommand {
    /// Print a consistent metadata-only history snapshot as JSON.
    List,
    /// Print metadata-only health as JSON.
    Health,
    /// Share captured file/stdin bytes; no OS clipboard is read.
    Share {
        #[arg(long, conflicts_with = "stdin", required_unless_present = "stdin")]
        file: Option<PathBuf>,
        #[arg(long)]
        stdin: bool,
        #[arg(long, value_enum, default_value = "text")]
        format: InputFormat,
        /// Direct destinations require an explicit identity; saved profiles use their device UUID.
        #[arg(long)]
        device_id: Option<Uuid>,
        #[arg(long)]
        device_name: Option<String>,
    },
    /// Export a selected item. Existing output files are never overwritten.
    Fetch {
        item_id: Uuid,
        #[arg(long, conflicts_with = "output", required_unless_present = "output")]
        stdout: bool,
        #[arg(long)]
        output: Option<PathBuf>,
        /// Optionally bind the fetch to a prior list's server epoch.
        #[arg(long)]
        epoch: Option<Uuid>,
    },
    /// Delete an item from the epoch returned by List.
    Delete {
        item_id: Uuid,
        #[arg(long)]
        epoch: Uuid,
    },
    /// Clear the shelf only if it still has the epoch returned by List.
    Clear {
        #[arg(long)]
        epoch: Uuid,
    },
}

pub async fn run(args: ShelfArgs) -> Result<()> {
    let direct = args.target.is_some() && args.profile.is_none();
    let config = build_client_config(ClientConfigArgs {
        profile: args.profile,
        settings_file: args.settings_file.clone(),
        target: args.target,
        port: args.port,
        identity_file: args.identity_file,
        ssh_option: args.ssh_option,
        ssh_bin: args.ssh_bin,
        timeout_ms: args.timeout_ms,
        ..Default::default()
    })?;
    let policy = SessionPolicy {
        max_item_bytes: args.max_item_bytes,
        max_decoded_bytes: args.max_decoded_bytes,
        operation_timeout: std::time::Duration::from_millis(config.timeout_ms),
    };
    policy.validate()?;
    let mut payload = Vec::new();
    let mut export = None;
    let operation = match args.command {
        ShelfCommand::List => Operation::Snapshot,
        ShelfCommand::Health => Operation::Action(Action::Health),
        ShelfCommand::Delete { item_id, epoch } => Operation::AtEpoch {
            epoch,
            action: Action::Delete { item_id },
        },
        ShelfCommand::Clear { epoch } => Operation::AtEpoch {
            epoch,
            action: Action::Clear,
        },
        ShelfCommand::Fetch {
            item_id,
            stdout,
            output,
            epoch,
        } => {
            ensure!(
                stdout || output.is_some(),
                "Choose --stdout or --output for fetched bytes."
            );
            if let Some(path) = &output {
                ensure!(!path.try_exists()?, "Output file already exists.");
            }
            export = Some(output);
            match epoch {
                Some(epoch) => Operation::AtEpoch {
                    epoch,
                    action: Action::Fetch { item_id },
                },
                None => Operation::Action(Action::Fetch { item_id }),
            }
        }
        ShelfCommand::Share {
            file,
            stdin,
            format,
            device_id,
            device_name,
        } => {
            let device = if direct || device_id.is_some() {
                None
            } else {
                settings::store_at(args.settings_file.as_deref())?
                    .load()?
                    .map(|config| config.device)
            };
            let origin = Origin {
                device_id: device_id.or_else(|| device.as_ref().and_then(|device| device.id.as_ref()?.parse().ok())).ok_or_else(|| eyre!("Sharing needs a device UUID. Save shared settings first, or pass --device-id for a direct destination."))?,
                name: device_name.or_else(|| device.map(|device| device.name)).unwrap_or_else(|| "This computer".into()),
            };
            // Validate metadata before reading potentially blocking input or launching SSH.
            let operation_id = Uuid::new_v4();
            let mut representation = Representation {
                id: 0,
                content_type: match format {
                    InputFormat::Text => ContentType::Text,
                    InputFormat::Png => ContentType::Png,
                },
                encoded_bytes: 0,
                dimensions: None,
                decoded_bytes: None,
            };
            crate::session_protocol::Request::Share {
                request_id: 1.into(),
                epoch: Uuid::from_u128(1),
                operation_id,
                origin: origin.clone(),
                representation: representation.clone(),
                created_at_ms: None,
            }
            .validate(0)?;
            payload = read_input(file, stdin, policy.max_item_bytes as usize).await?;
            representation.encoded_bytes = payload.len() as u32;
            if matches!(format, InputFormat::Text) {
                std::str::from_utf8(&payload)
                    .map_err(|_| eyre!("Input is not valid UTF-8 text."))?;
            } else {
                let maximum = policy.max_decoded_bytes as usize;
                let (data, width, height, decoded) = tokio::task::spawn_blocking(move || {
                    let (width, height, decoded) = crate::shelf::validate_png(&payload, maximum)?;
                    Ok::<_, crate::shelf::ShelfError>((payload, width, height, decoded))
                })
                .await??;
                payload = data;
                representation.dimensions =
                    Some(crate::session_protocol::Dimensions { width, height });
                representation.decoded_bytes = Some(decoded as u32);
            }
            // A failed invocation is never automatically retried with a fresh clipboard sample.
            eprintln!("Share operation ID: {operation_id}");
            Operation::Action(Action::Share {
                operation_id,
                origin,
                representation,
                created_at_ms: None,
            })
        }
    };
    let result = transport::run(&config.ssh, policy, operation, &payload).await?;
    match result {
        Outcome::Snapshot(snapshot) => println!(
            "{}",
            serde_json::to_string_pretty(
                &serde_json::json!({"epoch":snapshot.epoch,"sequence":snapshot.sequence,"items":snapshot.items})
            )?
        ),
        Outcome::Reply(reply) => {
            if let Some(path) = export {
                match path {
                    Some(path) => {
                        // Complete and validate the fetch before creating a destination. Stage
                        // beside it and publish without clobbering a competing/existing file.
                        tokio::task::spawn_blocking(move || -> Result<()> {
                            use std::io::Write;
                            let parent = path.parent().filter(|parent| !parent.as_os_str().is_empty()).unwrap_or_else(|| std::path::Path::new("."));
                            let mut staged = tempfile::NamedTempFile::new_in(parent)?;
                            staged.write_all(&reply.payload)?;
                            staged.as_file().sync_all()?;
                            staged.persist_noclobber(path).map_err(|_| eyre!("Could not publish fetched file; the destination may already exist."))?;
                            Ok(())
                        }).await??;
                    }
                    None => {
                        let mut output = tokio::io::stdout();
                        output.write_all(&reply.payload).await?;
                        output.flush().await?;
                    }
                }
            } else {
                println!("{}", serde_json::to_string_pretty(&reply.response)?);
            }
        }
    }
    Ok(())
}

async fn read_input(file: Option<PathBuf>, stdin: bool, maximum: usize) -> Result<Vec<u8>> {
    let data = if let Some(path) = file {
        tokio::task::spawn_blocking(move || -> Result<Vec<u8>> {
            use std::io::Read;
            let mut bytes = Vec::new();
            std::fs::File::open(path)?
                .take(maximum as u64 + 1)
                .read_to_end(&mut bytes)?;
            Ok(bytes)
        })
        .await??
    } else {
        ensure!(stdin, "Choose --file or --stdin.");
        let mut bytes = Vec::new();
        tokio::io::stdin()
            .take(maximum as u64 + 1)
            .read_to_end(&mut bytes)
            .await?;
        bytes
    };
    ensure!(data.len() <= maximum, "Input exceeds --max-item-bytes.");
    Ok(data)
}
