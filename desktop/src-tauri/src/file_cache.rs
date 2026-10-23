//! Private, bounded staging survives Quit. Never infer clipboard ownership by polling.
use eyre::{Result, ensure};
use serde::{Deserialize, Serialize};
use ssh_clipboard::files::{self, Materialized};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const QUOTA: u64 = 1024 * 1024 * 1024;
const RETENTION: u64 = 24 * 60 * 60;
const MARKER: &[u8] = b"ssh-clipboard-received-files-v1\n";

pub struct Cache {
    root: PathBuf,
    _lock: File,
    quota: u64,
}
impl Drop for Cache {
    fn drop(&mut self) {
        let _ = self._lock.unlock();
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    created: u64,
    bytes: u64,
    pinned: bool,
}
pub struct Staged {
    pub id: Uuid,
    pub files: Materialized,
}

fn now() -> Result<u64> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs())
}
fn plain_directory(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    ensure!(
        metadata.is_dir() && !metadata.file_type().is_symlink(),
        "File cache contains a linked directory."
    );
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        ensure!(
            metadata.file_attributes() & 0x400 == 0,
            "File cache contains a reparse point."
        );
    }
    Ok(())
}

impl Cache {
    pub fn open(root: PathBuf) -> Result<Self> {
        if !root.try_exists()? {
            let builder = fs::DirBuilder::new();
            #[cfg(unix)]
            let mut builder = builder;
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            builder.create(&root)?;
            let mut marker = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(root.join("owner"))?;
            marker.write_all(MARKER)?;
            marker.sync_all()?;
        }
        plain_directory(&root)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let metadata = fs::metadata(&root)?;
            ensure!(
                metadata.permissions().mode() & 0o077 == 0,
                "File cache must be private to the current user."
            );
        }
        ensure!(
            fs::read(root.join("owner"))? == MARKER,
            "Unrecognized file cache directory; left untouched."
        );
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(root.join("lock"))?;
        lock.try_lock()?;
        let cache = Self {
            root: root.canonicalize()?,
            _lock: lock,
            quota: QUOTA,
        };
        cache.clean(now()?)?;
        Ok(cache)
    }
    fn path(&self, id: Uuid) -> PathBuf {
        self.root.join(id.to_string())
    }
    fn records(&self) -> Result<Vec<(Uuid, Record)>> {
        let mut records = Vec::new();
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let Ok(id) = Uuid::parse_str(&name) else {
                continue;
            };
            if name != id.to_string() {
                continue;
            }
            plain_directory(&entry.path())?;
            let record_path = entry.path().join("record.json");
            let metadata = fs::symlink_metadata(&record_path)?;
            ensure!(
                metadata.is_file() && !metadata.file_type().is_symlink() && metadata.len() <= 4096,
                "Invalid cache record; left untouched."
            );
            records.push((id, serde_json::from_slice(&fs::read(record_path)?)?));
        }
        Ok(records)
    }
    fn write_record(&self, id: Uuid, record: &Record) -> Result<()> {
        let directory = self.path(id);
        let mut staged = tempfile::NamedTempFile::new_in(&directory)?;
        staged.write_all(&serde_json::to_vec(record)?)?;
        staged.as_file().sync_all()?;
        staged.persist(directory.join("record.json"))?;
        Ok(())
    }
    pub fn clean(&self, clock: u64) -> Result<()> {
        for (id, record) in self.records()? {
            if !record.pinned && clock.saturating_sub(record.created) >= RETENTION {
                self.remove(id)?;
            }
        }
        // Only our reserved staging prefix is eligible for abandoned-build cleanup.
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            if entry
                .file_name()
                .to_str()
                .is_some_and(|s| s.starts_with(".building-"))
            {
                plain_directory(&entry.path())?;
                fs::remove_dir_all(entry.path())?;
            }
        }
        Ok(())
    }
    pub fn stage(&self, bytes: &[u8], cancel: &AtomicBool) -> Result<Staged> {
        let bundle = files::validate(bytes)?;
        self.clean(now()?)?;
        let records = self.records()?;
        ensure!(
            records.len() < 128,
            "Received-file cache has 128 retained copies; wait for older copies to expire."
        );
        let used = records.iter().try_fold(0u64, |sum, (_, r)| {
            sum.checked_add(r.bytes)
                .ok_or_else(|| eyre::eyre!("Cache accounting overflow."))
        })?;
        ensure!(
            used.saturating_add(bundle.data.len() as u64) <= self.quota,
            "Received-file cache is full. Existing copies are retained until they expire."
        );
        let id = Uuid::new_v4();
        let staged = tempfile::Builder::new()
            .prefix(".building-")
            .tempdir_in(&self.root)?;
        let data = staged.path().join("data");
        fs::create_dir(&data)?;
        let mut result = files::materialize(bytes, &data, cancel)?;
        let record = Record {
            created: now()?,
            bytes: bundle.data.len() as u64,
            pinned: true,
        };
        let mut record_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(staged.path().join("record.json"))?;
        record_file.write_all(&serde_json::to_vec(&record)?)?;
        record_file.sync_all()?;
        drop(record_file);
        files::publish_directory(staged.path(), &self.path(id))?;
        for path in &mut result.paths {
            *path = self.path(id).join("data").join(&*path);
        }
        Ok(Staged { id, files: result })
    }
    pub fn replaced(&self, active: Option<Uuid>) -> Result<()> {
        for (id, mut record) in self.records()? {
            if record.pinned && Some(id) != active {
                record.pinned = false;
                record.created = now()?;
                self.write_record(id, &record)?;
            }
        }
        Ok(())
    }
    pub fn remove(&self, id: Uuid) -> Result<()> {
        let path = self.path(id);
        // UUIDs are one component; the canonical cache root is fixed at construction.
        ensure!(
            path.parent() == Some(self.root.as_path()),
            "Invalid cache cleanup target."
        );
        plain_directory(&path)?;
        fs::remove_dir_all(path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn payload(dir: &Path) -> Vec<u8> {
        let source = dir.join("original");
        fs::write(&source, b"contents").unwrap();
        files::capture(
            &[source.canonicalize().unwrap()],
            4096,
            &AtomicBool::new(false),
        )
        .unwrap()
    }
    #[test]
    fn pinned_data_survives_reopen_and_expiry_until_successful_replacement() {
        let dir = tempfile::tempdir().unwrap();
        let bytes = payload(dir.path());
        let root = dir.path().join("cache");
        let cache = Cache::open(root.clone()).unwrap();
        let staged = cache.stage(&bytes, &AtomicBool::new(false)).unwrap();
        let path = staged.files.paths[0].clone();
        cache.clean(now().unwrap() + RETENTION + 1).unwrap();
        assert!(path.exists());
        drop(cache);
        let cache = Cache::open(root).unwrap();
        assert!(path.exists());
        cache.replaced(None).unwrap();
        cache.clean(now().unwrap() + RETENTION + 1).unwrap();
        assert!(!path.exists());
    }
    #[test]
    fn quota_cancel_and_cleanup_preserve_other_paths() {
        let dir = tempfile::tempdir().unwrap();
        let bytes = payload(dir.path());
        let mut cache = Cache::open(dir.path().join("cache")).unwrap();
        cache.quota = 8;
        let first = cache.stage(&bytes, &AtomicBool::new(false)).unwrap();
        assert!(cache.stage(&bytes, &AtomicBool::new(false)).is_err());
        assert!(first.files.paths[0].exists());
        fs::create_dir(cache.root.join("unrelated")).unwrap();
        fs::write(cache.root.join("unrelated/keep"), b"keep").unwrap();
        cache.replaced(None).unwrap();
        cache.clean(now().unwrap() + RETENTION + 1).unwrap();
        assert!(cache.root.join("unrelated/keep").exists());
        assert!(cache.stage(&bytes, &AtomicBool::new(true)).is_err());
        let empty = cache.records().unwrap();
        assert!(empty.is_empty());
    }
    #[test]
    fn startup_removes_abandoned_builds_and_refuses_unowned_directories() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("cache");
        let cache = Cache::open(root.clone()).unwrap();
        let abandoned = root.join(".building-abandoned");
        fs::create_dir(&abandoned).unwrap();
        fs::write(abandoned.join("partial"), b"partial").unwrap();
        fs::create_dir(root.join("unrelated")).unwrap();
        drop(cache);
        let _cache = Cache::open(root.clone()).unwrap();
        assert!(!abandoned.exists());
        assert!(root.join("unrelated").is_dir());
        let foreign = dir.path().join("foreign");
        fs::create_dir(&foreign).unwrap();
        fs::write(foreign.join("keep"), b"keep").unwrap();
        assert!(Cache::open(foreign.clone()).is_err());
        assert!(foreign.join("keep").is_file());
    }
}
