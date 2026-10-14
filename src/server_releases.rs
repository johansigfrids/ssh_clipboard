//! User-owned, immutable server releases and recoverable executable selection.
//! No CLI or live systemd integration yet. Callers must supply signature policy.
use crate::file_lock::ExclusiveLock;
use eyre::{Result, WrapErr, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt, symlink};
use std::path::{Path, PathBuf};

const MAX_BINARY: usize = 256 * 1024 * 1024;
const MAX_RECORD: usize = 32 * 1024;

pub trait Verifier {
    /// Authenticate bytes AND canonical version/platform/channel before executable selection.
    fn verify(&self, version: &str, signature: &str, bytes: &[u8]) -> Result<()>;
}
pub trait Service {
    fn restart(&mut self) -> Result<()>;
    /// Must test protocol health, not merely process/service state.
    fn health(&mut self) -> Result<()>;
    fn stop(&mut self) -> Result<()>;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    version: String,
    signature: String,
    digest: String,
    bytes: usize,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Transition {
    previous: Option<String>,
    next: String,
}
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Activated,
    RestoredPrevious,
    RemovedFailedInitial,
}

pub struct Store {
    root: PathBuf,
    _lock: ExclusiveLock,
}
impl Store {
    /// The parent must already be user-owned and not writable by another user.
    pub fn open(root: &Path) -> Result<Self> {
        if !root.is_absolute() {
            bail!("server installation path must be absolute");
        }
        let parent = root
            .parent()
            .ok_or_else(|| eyre::eyre!("missing installation parent"))?;
        owned_directory(parent, false)?;
        directory(root)?;
        let root = root.canonicalize()?;
        directory(&root.join("releases"))?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(root.join("maintenance.lock"))?;
        let lock = ExclusiveLock::acquire(lock)
            .wrap_err("another server maintenance operation is running")?;
        Ok(Self { root, _lock: lock })
    }
    pub fn stage(
        &self,
        version: &str,
        signature: &str,
        bytes: &[u8],
        verifier: &impl Verifier,
    ) -> Result<()> {
        validate_version(version)?;
        if signature.len() > 16384
            || signature.is_empty()
            || bytes.is_empty()
            || bytes.len() > MAX_BINARY
        {
            bail!("server package exceeds its bounds");
        }
        verifier.verify(version, signature, bytes)?;
        let receipt = Receipt {
            version: version.into(),
            signature: signature.into(),
            digest: hash(bytes),
            bytes: bytes.len(),
        };
        let destination = self.root.join("releases").join(version);
        if fs::symlink_metadata(&destination).is_ok() {
            let existing = self.read_release(version, verifier)?;
            if existing != bytes {
                bail!("an immutable release already has different bytes");
            }
            return Ok(());
        }
        let staged = tempfile::Builder::new()
            .prefix(".stage-")
            .tempdir_in(self.root.join("releases"))?;
        fs::set_permissions(staged.path(), fs::Permissions::from_mode(0o700))?;
        write_new(&staged.path().join("ssh_clipboard"), bytes, 0o700)?;
        write_new(
            &staged.path().join("receipt.json"),
            &serde_json::to_vec(&receipt)?,
            0o600,
        )?;
        sync(staged.path())?;
        fs::rename(staged.path(), &destination)?;
        sync(&self.root.join("releases"))?;
        Ok(())
    }
    pub fn active(&self) -> Result<Option<String>> {
        match fs::symlink_metadata(self.root.join("current")) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
            Ok(meta) => {
                if !meta.is_symlink() || meta.uid() != unsafe { libc::geteuid() } {
                    bail!("current is not an owned release link");
                }
                let target = fs::read_link(self.root.join("current"))?;
                let text = target
                    .to_str()
                    .ok_or_else(|| eyre::eyre!("invalid current link"))?;
                let version = text
                    .strip_prefix("releases/")
                    .ok_or_else(|| eyre::eyre!("current points outside the managed releases"))?;
                validate_version(version)?;
                Ok(Some(version.into()))
            }
        }
    }
    pub fn needs_recovery(&self) -> Result<bool> {
        match fs::symlink_metadata(self.root.join("transition.json")) {
            Ok(meta) if meta.is_file() && !meta.is_symlink() => Ok(true),
            Ok(_) => bail!("unexpected transition path; repair is required"),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error.into()),
        }
    }
    /// Restarts clear in-memory clipboard items; callers must disclose this before invoking.
    pub fn activate(
        &self,
        version: &str,
        verifier: &impl Verifier,
        service: &mut impl Service,
    ) -> Result<Outcome> {
        if self.needs_recovery()? {
            bail!("recover the unfinished server transition before updating");
        }
        self.read_release(version, verifier)?;
        let previous = self.active()?;
        if previous.as_deref() == Some(version) {
            bail!("this server release is already selected");
        }
        if let Some(previous) = &previous {
            if version_numbers(version)? <= version_numbers(previous)? {
                bail!("normal server activation requires a newer stable version");
            }
            self.read_release(previous, verifier)?;
        }
        let transition = Transition {
            previous,
            next: version.into(),
        };
        self.record("transition.json", &transition)?;
        // After this journal is durable, any interruption is recovered conservatively.
        self.select(Some(version))?;
        if service.restart().and_then(|()| service.health()).is_err() {
            return self.restore(&transition, verifier, service);
        }
        self.finish(Outcome::Activated)
    }
    /// No new activation is allowed until a durable unfinished transition is resolved.
    pub fn recover(&self, verifier: &impl Verifier, service: &mut impl Service) -> Result<Outcome> {
        let transition: Transition =
            serde_json::from_slice(&read_owned(&self.root.join("transition.json"), MAX_RECORD)?)?;
        validate_version(&transition.next)?;
        let current = self.active()?;
        if current.as_ref() != Some(&transition.next) && current != transition.previous {
            bail!("current release does not match the recovery journal; manual repair is required");
        }
        self.restore(&transition, verifier, service)
    }
    fn restore(
        &self,
        transition: &Transition,
        verifier: &impl Verifier,
        service: &mut impl Service,
    ) -> Result<Outcome> {
        match transition.previous.as_deref() {
            Some(previous) => {
                self.read_release(previous, verifier).wrap_err(
                    "previous executable cannot be verified; transition retained for repair",
                )?;
                self.select(Some(previous))?;
                service.restart().and_then(|()|service.health()).wrap_err("previous release selected but service health failed; transition retained for repair")?;
                self.finish(Outcome::RestoredPrevious)
            }
            None => {
                service
                    .stop()
                    .wrap_err("failed initial service could not be stopped; transition retained")?;
                self.select(None)?;
                self.finish(Outcome::RemovedFailedInitial)
            }
        }
    }
    fn finish(&self, outcome: Outcome) -> Result<Outcome> {
        self.record("last-outcome.json", &outcome)?;
        fs::remove_file(self.root.join("transition.json"))?;
        sync(&self.root)?;
        Ok(outcome)
    }
    fn read_release(&self, version: &str, verifier: &impl Verifier) -> Result<Vec<u8>> {
        validate_version(version)?;
        let directory = self.root.join("releases").join(version);
        owned_directory(&directory, true)?;
        let receipt: Receipt =
            serde_json::from_slice(&read_owned(&directory.join("receipt.json"), MAX_RECORD)?)?;
        let bytes = read_owned(&directory.join("ssh_clipboard"), MAX_BINARY)?;
        if receipt.version != version
            || receipt.bytes != bytes.len()
            || receipt.digest != hash(&bytes)
        {
            bail!("stored release no longer matches its receipt");
        }
        verifier.verify(version, &receipt.signature, &bytes)?;
        Ok(bytes)
    }
    fn select(&self, version: Option<&str>) -> Result<()> {
        if let Some(version) = version {
            validate_version(version)?;
            let next = self.root.join(format!(".current-{}", uuid::Uuid::new_v4()));
            symlink(Path::new("releases").join(version), &next)?;
            if let Err(error) = fs::rename(&next, self.root.join("current")) {
                let _ = fs::remove_file(next);
                return Err(error.into());
            }
        } else if self.active()?.is_some() {
            fs::remove_file(self.root.join("current"))?;
        }
        sync(&self.root)
    }
    fn record(&self, name: &str, value: &impl Serialize) -> Result<()> {
        let bytes = serde_json::to_vec(value)?;
        if bytes.len() > MAX_RECORD {
            bail!("server record exceeds its limit");
        }
        let mut temporary = tempfile::NamedTempFile::new_in(&self.root)?;
        temporary.write_all(&bytes)?;
        temporary.as_file().sync_all()?;
        temporary.persist(self.root.join(name))?;
        sync(&self.root)
    }
}

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn sync(directory: &Path) -> Result<()> {
    File::open(directory)?.sync_all()?;
    Ok(())
}
fn directory(path: &Path) -> Result<()> {
    match fs::DirBuilder::new().mode(0o700).create(path) {
        Ok(()) => sync(
            path.parent()
                .ok_or_else(|| eyre::eyre!("missing directory parent"))?,
        )?,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error.into()),
    }
    owned_directory(path, true)
}
fn owned_directory(path: &Path, private: bool) -> Result<()> {
    let meta = fs::symlink_metadata(path)?;
    let forbidden = if private { 0o077 } else { 0o022 };
    if !meta.is_dir()
        || meta.is_symlink()
        || meta.uid() != unsafe { libc::geteuid() }
        || meta.mode() & forbidden != 0
    {
        bail!(
            "{} must be an owned directory with safe permissions",
            path.display()
        );
    }
    Ok(())
}
fn read_owned(path: &Path, limit: usize) -> Result<Vec<u8>> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)?;
    let meta = file.metadata()?;
    if !meta.is_file()
        || meta.uid() != unsafe { libc::geteuid() }
        || meta.mode() & 0o022 != 0
        || meta.len() > limit as u64
    {
        bail!("unsafe or oversized server release file");
    }
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        bail!("server release file exceeded its bound");
    }
    Ok(bytes)
}
fn write_new(path: &Path, bytes: &[u8], mode: u32) -> Result<()> {
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(mode)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}
fn validate_version(value: &str) -> Result<()> {
    let parts: Vec<_> = value.split('.').collect();
    if value.len() > 64
        || parts.len() != 3
        || parts.iter().any(|part| {
            part.is_empty()
                || (part.len() > 1 && part.starts_with('0'))
                || !part.bytes().all(|byte| byte.is_ascii_digit())
                || part.parse::<u64>().is_err()
        })
    {
        bail!("server version must be a canonical stable version");
    }
    Ok(())
}

fn version_numbers(value: &str) -> Result<[u64; 3]> {
    validate_version(value)?;
    let numbers = value
        .split('.')
        .map(str::parse)
        .collect::<std::result::Result<Vec<u64>, _>>()?;
    Ok([numbers[0], numbers[1], numbers[2]])
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Verified;
    impl Verifier for Verified {
        fn verify(&self, version: &str, signature: &str, bytes: &[u8]) -> Result<()> {
            if signature != format!("synthetic-{version}") || bytes != version.as_bytes() {
                bail!("synthetic signature rejected");
            }
            Ok(())
        }
    }
    #[derive(Default)]
    struct FakeService {
        failures: usize,
        restart_failures: usize,
        fail_always: bool,
        calls: Vec<&'static str>,
    }
    impl Service for FakeService {
        fn restart(&mut self) -> Result<()> {
            self.calls.push("restart");
            if self.restart_failures > 0 {
                self.restart_failures -= 1;
                bail!("synthetic restart failure");
            }
            Ok(())
        }
        fn health(&mut self) -> Result<()> {
            self.calls.push("health");
            if self.fail_always || self.failures > 0 {
                self.failures = self.failures.saturating_sub(1);
                bail!("synthetic protocol health failure");
            }
            Ok(())
        }
        fn stop(&mut self) -> Result<()> {
            self.calls.push("stop");
            Ok(())
        }
    }
    fn fixture() -> (tempfile::TempDir, Store) {
        let root = tempfile::tempdir().unwrap();
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let store = Store::open(&root.path().join("managed")).unwrap();
        (root, store)
    }
    fn stage(store: &Store, version: &str) {
        store
            .stage(
                version,
                &format!("synthetic-{version}"),
                version.as_bytes(),
                &Verified,
            )
            .unwrap();
    }
    #[test]
    fn releases_survive_download_removal_and_immutable_versions_never_overwrite() {
        let (root, store) = fixture();
        let download = root.path().join("extracted");
        fs::write(&download, b"1.0.0").unwrap();
        store
            .stage(
                "1.0.0",
                "synthetic-1.0.0",
                &fs::read(&download).unwrap(),
                &Verified,
            )
            .unwrap();
        fs::remove_file(download).unwrap();
        assert_eq!(store.read_release("1.0.0", &Verified).unwrap(), b"1.0.0");
        assert!(
            store
                .stage("1.0.0", "wrong", b"changed", &Verified)
                .is_err()
        );
        assert_eq!(store.read_release("1.0.0", &Verified).unwrap(), b"1.0.0");
        assert!(Store::open(&store.root).is_err());
        struct BothPackagesAccepted;
        impl Verifier for BothPackagesAccepted {
            fn verify(&self, _: &str, _: &str, _: &[u8]) -> Result<()> {
                Ok(())
            }
        }
        // Even two admitted artifacts cannot reuse a version with different bytes.
        assert!(
            store
                .stage(
                    "1.0.0",
                    "second-admitted-artifact",
                    b"changed",
                    &BothPackagesAccepted
                )
                .is_err()
        );
        assert_eq!(store.read_release("1.0.0", &Verified).unwrap(), b"1.0.0");
    }
    #[test]
    fn failed_package_admission_never_changes_the_selected_release_or_calls_service() {
        let (_root, store) = fixture();
        stage(&store, "1.0.0");
        store.select(Some("1.0.0")).unwrap();
        assert!(store.stage("2.0.0", "wrong", b"2.0.0", &Verified).is_err());
        assert_eq!(store.active().unwrap().as_deref(), Some("1.0.0"));
        assert!(!store.root.join("releases/2.0.0").exists());
    }
    #[test]
    fn healthy_updates_keep_the_previous_binary_and_clear_the_durable_journal() {
        let (_root, store) = fixture();
        stage(&store, "1.0.0");
        stage(&store, "2.0.0");
        store.select(Some("1.0.0")).unwrap();
        let mut service = FakeService::default();
        assert_eq!(
            store.activate("2.0.0", &Verified, &mut service).unwrap(),
            Outcome::Activated
        );
        assert_eq!(store.active().unwrap().as_deref(), Some("2.0.0"));
        assert_eq!(store.read_release("1.0.0", &Verified).unwrap(), b"1.0.0");
        assert!(!store.needs_recovery().unwrap());
        assert_eq!(service.calls, ["restart", "health"]);
        assert!(store.activate("1.0.0", &Verified, &mut service).is_err());
        assert_eq!(service.calls, ["restart", "health"]);
    }
    #[test]
    fn new_protocol_failure_restores_and_checks_the_previous_executable() {
        let (_root, store) = fixture();
        stage(&store, "1.0.0");
        stage(&store, "2.0.0");
        store.select(Some("1.0.0")).unwrap();
        let mut service = FakeService {
            failures: 1,
            ..Default::default()
        };
        assert_eq!(
            store.activate("2.0.0", &Verified, &mut service).unwrap(),
            Outcome::RestoredPrevious
        );
        assert_eq!(store.active().unwrap().as_deref(), Some("1.0.0"));
        assert_eq!(service.calls, ["restart", "health", "restart", "health"]);
    }
    #[test]
    fn failed_start_and_failed_outcome_write_leave_a_recoverable_previous_release() {
        let (_root, store) = fixture();
        stage(&store, "1.0.0");
        stage(&store, "2.0.0");
        store.select(Some("1.0.0")).unwrap();
        let mut service = FakeService {
            restart_failures: 1,
            ..Default::default()
        };
        assert_eq!(
            store.activate("2.0.0", &Verified, &mut service).unwrap(),
            Outcome::RestoredPrevious
        );
        assert_eq!(service.calls, ["restart", "restart", "health"]);
        // Fault the atomic outcome write after the new service is healthy.
        fs::create_dir(store.root.join("last-outcome.json-fault")).unwrap();
        fs::remove_file(store.root.join("last-outcome.json")).unwrap();
        fs::rename(
            store.root.join("last-outcome.json-fault"),
            store.root.join("last-outcome.json"),
        )
        .unwrap();
        assert!(store.activate("2.0.0", &Verified, &mut service).is_err());
        assert!(store.needs_recovery().unwrap());
        assert_eq!(store.active().unwrap().as_deref(), Some("2.0.0"));
        fs::remove_dir(store.root.join("last-outcome.json")).unwrap();
        assert_eq!(
            store.recover(&Verified, &mut service).unwrap(),
            Outcome::RestoredPrevious
        );
        assert_eq!(store.active().unwrap().as_deref(), Some("1.0.0"));
    }
    #[test]
    fn interruption_after_selection_is_recovered_before_another_update() {
        let (root, store) = fixture();
        stage(&store, "1.0.0");
        stage(&store, "2.0.0");
        store.select(Some("1.0.0")).unwrap();
        store
            .record(
                "transition.json",
                &Transition {
                    previous: Some("1.0.0".into()),
                    next: "2.0.0".into(),
                },
            )
            .unwrap();
        store.select(Some("2.0.0")).unwrap();
        drop(store);
        let store = Store::open(&root.path().join("managed")).unwrap();
        let mut service = FakeService::default();
        assert!(store.activate("2.0.0", &Verified, &mut service).is_err());
        assert!(service.calls.is_empty());
        assert_eq!(
            store.recover(&Verified, &mut service).unwrap(),
            Outcome::RestoredPrevious
        );
        assert!(!store.needs_recovery().unwrap());
    }
    #[test]
    fn rollback_failure_keeps_the_journal_and_first_install_failure_removes_selection() {
        let (_root, store) = fixture();
        stage(&store, "1.0.0");
        stage(&store, "2.0.0");
        store.select(Some("1.0.0")).unwrap();
        let mut service = FakeService {
            fail_always: true,
            ..Default::default()
        };
        assert!(store.activate("2.0.0", &Verified, &mut service).is_err());
        assert!(store.needs_recovery().unwrap());
        assert_eq!(store.active().unwrap().as_deref(), Some("1.0.0"));
        let (_fresh_root, fresh) = fixture();
        stage(&fresh, "1.0.0");
        let mut service = FakeService {
            failures: 1,
            ..Default::default()
        };
        assert_eq!(
            fresh.activate("1.0.0", &Verified, &mut service).unwrap(),
            Outcome::RemovedFailedInitial
        );
        assert_eq!(fresh.active().unwrap(), None);
        assert_eq!(service.calls, ["restart", "health", "stop"]);
    }
    #[test]
    fn unsafe_links_versions_permissions_and_changed_staged_bytes_are_rejected() {
        let (_root, store) = fixture();
        stage(&store, "1.0.0");
        for version in ["../outside", "01.0.0", "1.0.0/../../outside", "1.0.0-beta"] {
            assert!(store.stage(version, "synthetic", b"x", &Verified).is_err());
        }
        symlink("../../outside", store.root.join("current")).unwrap();
        assert!(store.active().is_err());
        fs::remove_file(store.root.join("current")).unwrap();
        fs::write(store.root.join("releases/1.0.0/ssh_clipboard"), b"tampered").unwrap();
        let mut service = FakeService::default();
        assert!(store.activate("1.0.0", &Verified, &mut service).is_err());
        assert!(service.calls.is_empty());
        fs::set_permissions(&store.root, fs::Permissions::from_mode(0o777)).unwrap();
        assert!(Store::open(&store.root).is_err());
    }
}
