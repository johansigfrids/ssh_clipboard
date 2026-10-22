//! Per-user managed installation. Existing installations are never adopted.
use crate::server_package::{MAX_PACKAGE_BYTES, Policy, artifact_name, native_platform};
use crate::server_releases::{Outcome, Service, Store, Verifier, version_numbers};
use eyre::{Result, WrapErr, bail, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, symlink};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::process::Command;
mod process;
#[cfg(test)]
mod tests;

const UNIT: &str = "ssh_clipboard-shelf.service";
const CONFIG: &str = "installation.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DaemonSettings {
    pub socket_path: PathBuf,
    pub max_items: usize,
    pub retention_secs: u64,
    pub max_item_bytes: usize,
    pub max_stored_bytes: usize,
    pub max_resident_bytes: usize,
    pub max_decoded_bytes: usize,
    pub max_connections: usize,
    pub io_timeout_ms: u64,
    pub idle_timeout_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Configuration {
    schema: u32,
    uid: u32,
    public_key: String,
    launcher: PathBuf,
    unit: PathBuf,
    daemon: DaemonSettings,
}

#[derive(Serialize)]
pub struct Status {
    root: PathBuf,
    selected_version: Option<String>,
    recovery_required: bool,
    launcher: PathBuf,
    unit: PathBuf,
    public_key_id: String,
}

pub struct Manager {
    store: Store,
    config: Configuration,
    runtime: tokio::runtime::Runtime,
}

pub fn default_root() -> Result<PathBuf> {
    Ok(dirs::data_local_dir()
        .ok_or_else(|| eyre::eyre!("user data location is unavailable"))?
        .join("ssh_clipboard/server"))
}

impl Manager {
    /// Call only on a blocking thread; the private runtime bounds child lifecycles.
    pub fn open(root: &Path) -> Result<Self> {
        ensure!(
            root.is_dir(),
            "No managed installation exists at this path."
        );
        let config = read_configuration(&root.join(CONFIG))?;
        let store = Store::open(root)?;
        validate(&config)?;
        Ok(Self {
            store,
            config,
            runtime: runtime()?,
        })
    }

    /// Verify the chosen package before creating an installation, unit, or launcher.
    pub fn install(
        root: &Path,
        public_key: String,
        daemon: DaemonSettings,
        version: &str,
        package: Option<&Path>,
        signature: Option<&Path>,
    ) -> Result<Outcome> {
        if root.join(CONFIG).exists() {
            read_configuration(&root.join(CONFIG))?;
        }
        let home = dirs::home_dir().ok_or_else(|| eyre::eyre!("home directory is unavailable"))?;
        let config = Configuration {
            schema: 2,
            uid: unsafe { libc::geteuid() },
            public_key: public_key.trim().into(),
            launcher: home.join(".local/bin/ssh_clipboard"),
            unit: dirs::config_dir()
                .ok_or_else(|| eyre::eyre!("user configuration directory is unavailable"))?
                .join("systemd/user")
                .join(UNIT),
            daemon,
        };
        validate(&config)?;
        let policy = Policy::new(&config.public_key, native_platform()?)?;
        let runtime = runtime()?;
        let (bytes, signature) = load_package(&runtime, version, package, signature)?;
        policy.verify(version, &signature, &bytes)?;
        ensure!(root.is_absolute(), "Installation root must be absolute.");
        create_user_directory(
            root.parent()
                .ok_or_else(|| eyre::eyre!("missing installation parent"))?,
        )?;
        let store = Store::open(root)?;
        let config_path = store.root().join(CONFIG);
        match fs::symlink_metadata(&config_path) {
            Ok(_) => {
                let previous = read_configuration(&config_path)?;
                ensure!(
                    previous == config,
                    "This installation already has different pinned trust or daemon options. Use update; migration needs separate review."
                );
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                write_new(&config_path, &serde_json::to_vec(&config)?)?
            }
            Err(error) => return Err(error.into()),
        }
        let manager = Self {
            store,
            config,
            runtime,
        };
        ensure!(
            !manager.store.needs_recovery()?,
            "Recover the unfinished transition first."
        );
        ensure!(
            manager.store.active()?.is_none(),
            "This installation already has an active version. Use update."
        );
        manager.store.stage(version, &signature, &bytes, &policy)?;
        manager.check_binary(version)?;
        manager.prepare_files()?;
        manager.systemctl(&["daemon-reload"])?;
        // Initial enable belongs inside the journaled transition. A crash or partial
        // enable failure must be recoverable through the same stop/disable path.
        let mut service = Driver(&manager, true);
        let result = manager.store.activate(version, &policy, &mut service);
        if result.is_err() && manager.store.active()?.is_none() {
            let _ = service.stop(); // A pre-switch failure must not leave an enabled broken launcher.
        }
        result
    }

    pub fn update(
        &self,
        version: &str,
        package: Option<&Path>,
        signature: Option<&Path>,
    ) -> Result<Outcome> {
        self.check_files()?;
        ensure!(
            !self.store.needs_recovery()?,
            "Recover the unfinished transition first."
        );
        let previous = self
            .store
            .active()?
            .ok_or_else(|| eyre::eyre!("No active version. Finish installation first."))?;
        ensure!(
            version_numbers(version)? > version_numbers(&previous)?,
            "Updates require a newer stable version."
        );
        let policy = self.policy()?;
        let (bytes, signature) = load_package(&self.runtime, version, package, signature)?;
        // All network/crypto/disk/preflight work precedes current selection or service mutation.
        self.store.stage(version, &signature, &bytes, &policy)?;
        self.check_binary(version)?;
        self.store
            .activate(version, &policy, &mut Driver(self, false))
    }

    pub fn recover(&self) -> Result<Outcome> {
        self.check_files()?;
        self.store
            .recover(&self.policy()?, &mut Driver(self, false))
    }

    pub fn status(&self) -> Result<Status> {
        Ok(Status {
            root: self.store.root().into(),
            selected_version: self.store.active()?,
            recovery_required: self.store.needs_recovery()?,
            launcher: self.config.launcher.clone(),
            unit: self.config.unit.clone(),
            public_key_id: format!("{:x}", Sha256::digest(self.config.public_key.as_bytes()))[..16]
                .into(),
        })
    }

    fn policy(&self) -> Result<Policy> {
        Policy::new(&self.config.public_key, native_platform()?)
    }

    fn check_binary(&self, version: &str) -> Result<()> {
        self.store.read_release(version, &self.policy()?)?;
        let path = self
            .store
            .root()
            .join("releases")
            .join(version)
            .join("ssh_clipboard");
        let bytes = self.runtime.block_on(process::output(
            Command::new(path).arg("--version"),
            1024,
            Duration::from_secs(5),
        ))?;
        ensure!(
            std::str::from_utf8(&bytes)?.trim() == format!("ssh_clipboard {version}"),
            "Signed executable reports a different version; service was left unchanged."
        );
        Ok(())
    }

    fn prepare_files(&self) -> Result<()> {
        create_user_directory(
            self.config
                .launcher
                .parent()
                .ok_or_else(|| eyre::eyre!("invalid launcher path"))?,
        )?;
        create_user_directory(
            self.config
                .unit
                .parent()
                .ok_or_else(|| eyre::eyre!("invalid unit path"))?,
        )?;
        let target = self.store.root().join("current/ssh_clipboard");
        let create_launcher = match fs::symlink_metadata(&self.config.launcher) {
            Ok(meta) => {
                ensure!(
                    meta.is_symlink()
                        && meta.uid() == self.config.uid
                        && fs::read_link(&self.config.launcher)? == target,
                    "An existing user launcher was not replaced. Review migration first."
                );
                false
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => true,
            Err(error) => return Err(error.into()),
        };
        let unit = render_unit(&self.config)?;
        let create_unit = match fs::symlink_metadata(&self.config.unit) {
            Ok(_) => {
                ensure!(
                    owned_file(&self.config.unit, 32768)? == unit.as_bytes(),
                    "An existing unit was not replaced. Review migration first."
                );
                false
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => true,
            Err(error) => return Err(error.into()),
        };
        // Detect both launcher/unit conflicts before creating either managed file.
        if create_launcher {
            symlink(target, &self.config.launcher)?;
            sync_parent(&self.config.launcher)?;
        }
        if create_unit {
            write_new(&self.config.unit, unit.as_bytes())?;
        }
        Ok(())
    }
    fn check_files(&self) -> Result<()> {
        let metadata = fs::symlink_metadata(&self.config.launcher)?;
        ensure!(
            metadata.is_symlink()
                && metadata.uid() == self.config.uid
                && fs::read_link(&self.config.launcher)?
                    == self.store.root().join("current/ssh_clipboard"),
            "Managed launcher changed; review it before maintenance."
        );
        ensure!(
            owned_file(&self.config.unit, 32768)? == render_unit(&self.config)?.as_bytes(),
            "Managed unit changed; review it before maintenance."
        );
        Ok(())
    }
    fn systemctl(&self, args: &[&str]) -> Result<()> {
        let mut command = Command::new("systemctl");
        command
            .args(["--user", "--no-ask-password", "--no-pager"])
            .args(args);
        self.runtime
            .block_on(process::output(&mut command, 8192, Duration::from_secs(20)))?;
        Ok(())
    }
    fn health(&self) -> Result<()> {
        let version = self
            .store
            .active()?
            .ok_or_else(|| eyre::eyre!("no selected server version"))?;
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        loop {
            let result = self.runtime.block_on(async {
                let mut command = Command::new(&self.config.launcher);
                command
                    .arg("session-proxy")
                    .arg("--socket-path")
                    .arg(&self.config.daemon.socket_path)
                    .args(["--io-timeout-ms", "1000", "--idle-timeout-ms", "1000"])
                    .stdin(std::process::Stdio::piped())
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .kill_on_drop(true);
                let child = command.spawn()?;
                crate::client::session_transport::run_child_version(
                    child,
                    crate::client::session::SessionPolicy {
                        operation_timeout: Duration::from_secs(1),
                        ..Default::default()
                    },
                    crate::client::session_transport::Operation::Action(
                        crate::client::session::Action::Health,
                    ),
                    &[],
                    Some(&version),
                )
                .await?;
                Ok::<_, eyre::Report>(())
            });
            if result.is_ok() {
                return Ok(());
            }
            if std::time::Instant::now() >= deadline {
                bail!("Selected server version did not pass proxy protocol health.");
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

struct Driver<'a>(&'a Manager, bool);
impl Service for Driver<'_> {
    fn restart(&mut self) -> Result<()> {
        if self.1 {
            self.0.systemctl(&["enable", UNIT])?;
        }
        // A crashing candidate can exhaust systemd's start limit. Restoring the
        // executable alone then cannot start its predecessor (or recover later).
        // Query first: reset-failed rejects an inactive unit that was unloaded.
        if self.0.systemctl(&["--quiet", "is-failed", UNIT]).is_ok() {
            self.0.systemctl(&["reset-failed", UNIT])?;
        }
        self.0.systemctl(&["restart", UNIT])
    }
    fn health(&mut self) -> Result<()> {
        self.0.health()
    }
    fn stop(&mut self) -> Result<()> {
        self.0.systemctl(&["stop", UNIT])?;
        self.0.systemctl(&["disable", UNIT])
    }
}

fn runtime() -> Result<tokio::runtime::Runtime> {
    Ok(tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?)
}

fn validate(config: &Configuration) -> Result<()> {
    ensure!(
        config.schema == 2 && config.uid == unsafe { libc::geteuid() },
        "Installation schema or target user does not match."
    );
    Policy::new(&config.public_key, native_platform()?)?;
    for path in [&config.launcher, &config.unit, &config.daemon.socket_path] {
        ensure!(path.is_absolute(), "Managed paths must be absolute.");
        quote(
            path.to_str()
                .ok_or_else(|| eyre::eyre!("Managed paths must be UTF-8"))?,
        )?;
    }
    ensure!(
        config.unit.file_name().is_some_and(|name| name == UNIT),
        "Unexpected managed service name."
    );
    let d = &config.daemon;
    crate::shelf_service::ShelfService::new(
        crate::shelf::ShelfLimits {
            max_items: d.max_items,
            retention: Duration::from_secs(d.retention_secs),
            max_item_bytes: d.max_item_bytes,
            max_stored_bytes: d.max_stored_bytes,
            max_resident_bytes: d.max_resident_bytes,
            max_decoded_bytes: d.max_decoded_bytes,
            ..Default::default()
        },
        crate::shelf_service::ServiceLimits {
            max_connections: d.max_connections,
            operation_timeout: Duration::from_millis(d.io_timeout_ms),
            idle_timeout: Duration::from_millis(d.idle_timeout_ms),
            ..Default::default()
        },
    )?;
    Ok(())
}

fn daemon_argv(d: &DaemonSettings) -> Vec<String> {
    let mut args = vec![
        "shelf-daemon".into(),
        "--socket-path".into(),
        d.socket_path.to_string_lossy().into_owned(),
    ];
    for (flag, value) in [
        ("--max-items", d.max_items.to_string()),
        ("--retention-secs", d.retention_secs.to_string()),
        ("--max-item-bytes", d.max_item_bytes.to_string()),
        ("--max-stored-bytes", d.max_stored_bytes.to_string()),
        ("--max-resident-bytes", d.max_resident_bytes.to_string()),
        ("--max-decoded-bytes", d.max_decoded_bytes.to_string()),
        ("--max-connections", d.max_connections.to_string()),
        ("--io-timeout-ms", d.io_timeout_ms.to_string()),
        ("--idle-timeout-ms", d.idle_timeout_ms.to_string()),
    ] {
        args.extend([flag.into(), value]);
    }
    args
}

fn quote(value: &str) -> Result<String> {
    ensure!(
        value.len() <= 4096 && !value.chars().any(char::is_control),
        "Managed argument contains controls or is too long."
    );
    Ok(format!(
        "\"{}\"",
        value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('%', "%%")
    ))
}
fn render_unit(config: &Configuration) -> Result<String> {
    let mut args = vec![quote(
        config
            .launcher
            .to_str()
            .ok_or_else(|| eyre::eyre!("Invalid launcher"))?,
    )?];
    args.extend(
        daemon_argv(&config.daemon)
            .iter()
            .map(|arg| quote(arg))
            .collect::<Result<Vec<_>>>()?,
    );
    // ':' disables environment substitution; '%%' above disables specifier expansion.
    Ok(format!(
        "# Managed by ssh_clipboard server-maintenance\n[Unit]\nDescription=SSH Clipboard shelf\n[Service]\nType=exec\nExecStart=:{}\nRestart=on-failure\nRestartSec=1\n[Install]\nWantedBy=default.target\n",
        args.join(" ")
    ))
}

fn create_user_directory(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(meta) => ensure!(
            meta.is_dir()
                && !meta.is_symlink()
                && meta.uid() == unsafe { libc::geteuid() }
                && meta.mode() & 0o022 == 0,
            "Directory must be owned by the target user and not writable by others: {}",
            path.display()
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            create_user_directory(
                path.parent()
                    .ok_or_else(|| eyre::eyre!("Missing user directory parent"))?,
            )?;
            fs::DirBuilder::new().mode(0o700).create(path)?;
            sync_parent(path)?;
        }
        Err(error) => return Err(error.into()),
    }
    Ok(())
}
pub(crate) fn owned_file(path: &Path, limit: usize) -> Result<Vec<u8>> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
        .open(path)?;
    let meta = file.metadata()?;
    ensure!(
        meta.is_file() && meta.uid() == unsafe { libc::geteuid() } && meta.mode() & 0o022 == 0,
        "Unsafe owned maintenance file."
    );
    read_file(file, limit)
}
pub(crate) fn input_file(path: &Path, limit: usize) -> Result<Vec<u8>> {
    read_file(
        OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_CLOEXEC | libc::O_NONBLOCK)
            .open(path)?,
        limit,
    )
}
fn read_file(mut file: File, limit: usize) -> Result<Vec<u8>> {
    let meta = file.metadata()?;
    ensure!(
        meta.is_file() && meta.len() <= limit as u64,
        "Maintenance input is not a bounded regular file."
    );
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 16384];
    loop {
        let count = file.read(&mut chunk)?;
        if count == 0 {
            return Ok(bytes);
        }
        ensure!(
            count <= limit.saturating_sub(bytes.len()),
            "Maintenance input grew beyond its limit."
        );
        bytes.try_reserve_exact(count)?;
        bytes.extend_from_slice(&chunk[..count]);
    }
}
fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    sync_parent(path)
}
fn sync_parent(path: &Path) -> Result<()> {
    File::open(path.parent().ok_or_else(|| eyre::eyre!("missing parent"))?)?.sync_all()?;
    Ok(())
}

fn load_package(
    runtime: &tokio::runtime::Runtime,
    version: &str,
    package: Option<&Path>,
    signature: Option<&Path>,
) -> Result<(Vec<u8>, String)> {
    let name = artifact_name(version, native_platform()?)?;
    match (package,signature) {
        (Some(package),Some(signature))=>Ok((input_file(package,MAX_PACKAGE_BYTES)?,String::from_utf8(input_file(signature,16384)?)?.trim().into())),
        (None,None)=>runtime.block_on(async {
            let url=format!("https://github.com/johansigfrids/ssh_clipboard/releases/download/server-v{version}/{name}");
            let signature=String::from_utf8(download(&format!("{url}.sig"),16384,20).await?)?;
            Ok((download(&url,MAX_PACKAGE_BYTES,300).await?,signature.trim().into()))
        }),
        _=>Err(eyre::eyre!("Provide both --package and --signature, or neither to download the requested version.")),
    }
}
async fn download(url: &str, limit: usize, seconds: u64) -> Result<Vec<u8>> {
    let mut command = Command::new("curl");
    command
        .args([
            "--disable",
            "--fail",
            "--silent",
            "--show-error",
            "--globoff",
            "--proto",
            "=https",
            "--proto-redir",
            "=https",
            "--location",
            "--max-redirs",
            "5",
            "--connect-timeout",
            "10",
            "--max-time",
        ])
        .arg(seconds.to_string())
        .arg("--max-filesize")
        .arg(limit.to_string())
        .arg("--url")
        .arg(url);
    process::output(&mut command, limit, Duration::from_secs(seconds))
        .await
        .wrap_err("Could not download the requested signed server package")
}

fn read_configuration(path: &Path) -> Result<Configuration> {
    let bytes = owned_file(path, 32768)?;
    let value: serde_json::Value = serde_json::from_slice(&bytes)?;
    ensure!(
        value.get("schema").and_then(|value| value.as_u64()) == Some(2),
        "Unsupported installation schema. Preserve the old installation and perform a fresh 0.5 setup."
    );
    Ok(serde_json::from_value(value)?)
}
