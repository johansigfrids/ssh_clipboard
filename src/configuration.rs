//! Shared, versioned configuration storage. Never silently repair a bad file.
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

pub mod migration;

const SCHEMA_VERSION: u32 = 2;
const MAX_CONFIG_BYTES: u64 = 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Configuration {
    pub schema_version: u32,
    pub revision: u64,
    pub default_profile: String,
    pub profiles: Vec<Profile>,
    pub preferences: Preferences,
    pub device: Device,
    pub legacy_import: Option<LegacyImport>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub target: String,
    pub port: Option<u16>,
    pub identity_file: String,
    pub transport: TransportSettings,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preferences {
    pub open_shortcut: String,
    pub shortcut_enabled: bool,
    pub show_preview: bool,
    pub close_after_copy: bool,
    pub start_at_login: bool,
    pub direct_share_shortcut: Option<String>,
    pub direct_copy_shortcut: Option<String>,
    pub direct_shortcuts_enabled: bool,
    pub check_updates_automatically: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Device {
    pub id: Option<String>,
    pub name: String,
}

impl Default for Device {
    fn default() -> Self {
        Self {
            id: None,
            name: "This computer".into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyImport {
    pub source: PathBuf,
    pub backup: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransportSettings {
    pub ssh_options: Vec<String>,
    pub ssh_bin: Option<PathBuf>,
    pub max_size: usize,
    pub timeout_ms: u64,
    pub resync_frames: bool,
    pub resync_max_bytes: usize,
}

impl Default for TransportSettings {
    fn default() -> Self {
        Self {
            ssh_options: Vec::new(),
            ssh_bin: None,
            max_size: crate::protocol::DEFAULT_MAX_SIZE,
            timeout_ms: 7000,
            resync_frames: true,
            resync_max_bytes: 8192,
        }
    }
}

impl Profile {
    pub fn client_config(&self) -> crate::client::transport::ClientConfig {
        crate::client::transport::ClientConfig {
            ssh: crate::client::ssh::SshConfig {
                target: self.target.clone(),
                port: self.port,
                identity_file: (!self.identity_file.trim().is_empty())
                    .then(|| self.identity_file.trim().into()),
                user: None,
                host: None,
                ssh_options: self.transport.ssh_options.clone(),
                ssh_bin: self.transport.ssh_bin.clone(),
            },
            max_size: self.transport.max_size,
            timeout_ms: self.transport.timeout_ms,
            resync_frames: self.transport.resync_frames,
            resync_max_bytes: self.transport.resync_max_bytes,
        }
    }
}

impl Default for Configuration {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            revision: 0,
            default_profile: "default".into(),
            profiles: vec![Profile {
                id: "default".into(),
                name: "Default".into(),
                target: String::new(),
                port: None,
                identity_file: String::new(),
                transport: TransportSettings::default(),
            }],
            preferences: Preferences {
                open_shortcut: "CommandOrControl+Shift+V".into(),
                shortcut_enabled: true,
                show_preview: true,
                close_after_copy: false,
                start_at_login: false,
                direct_share_shortcut: None,
                direct_copy_shortcut: None,
                direct_shortcuts_enabled: false,
                check_updates_automatically: false,
            },
            device: Device::default(),
            legacy_import: None,
        }
    }
}

impl Configuration {
    pub fn selected_profile(&self) -> Result<&Profile, ConfigError> {
        self.profiles
            .iter()
            .find(|p| p.id == self.default_profile)
            .ok_or(ConfigError::Invalid)
    }

    pub fn selected_profile_mut(&mut self) -> Result<&mut Profile, ConfigError> {
        self.profiles
            .iter_mut()
            .find(|p| p.id == self.default_profile)
            .ok_or(ConfigError::Invalid)
    }

    fn validate(&self) -> Result<(), ConfigError> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(ConfigError::UnsupportedVersion(self.schema_version));
        }
        self.selected_profile()?;
        let mut ids = std::collections::HashSet::new();
        for profile in &self.profiles {
            if profile.id.is_empty() || !ids.insert(&profile.id) || profile.name.trim().is_empty() {
                return Err(ConfigError::Invalid);
            }
            validate_connection(&profile.target, profile.port, &profile.identity_file)
                .map_err(|_| ConfigError::Invalid)?;
            let transport = &profile.transport;
            if transport.max_size == 0
                || transport.timeout_ms == 0
                || transport.resync_max_bytes == 0
                || transport
                    .ssh_options
                    .iter()
                    .any(|s| s.chars().any(char::is_control))
            {
                return Err(ConfigError::Invalid);
            }
        }
        if self.preferences.open_shortcut.is_empty() {
            return Err(ConfigError::Invalid);
        }
        if self.device.name.trim().is_empty()
            || self
                .device
                .id
                .as_deref()
                .is_some_and(|id| uuid::Uuid::parse_str(id).is_err())
        {
            return Err(ConfigError::Invalid);
        }
        Ok(())
    }
}

/// An empty target is permitted during setup, but not for a transfer.
pub fn validate_connection(target: &str, port: Option<u16>, identity: &str) -> Result<(), String> {
    if target.starts_with('-') || target.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(
            "The SSH target must be one alias or user@host, without options or spaces.".into(),
        );
    }
    if port == Some(0) {
        return Err("SSH port must be between 1 and 65535.".into());
    }
    if identity.chars().any(char::is_control) {
        return Err("The identity file path contains a control character.".into());
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Could not access the settings file. Check its location and permissions.")]
    Io(#[from] std::io::Error),
    #[error("Settings are invalid. Preserve the file and repair it before saving.")]
    Invalid,
    #[error("Settings use unsupported schema version {0}. Use the matching application version.")]
    UnsupportedVersion(u32),
    #[error("Settings changed in another process. Reload saved settings before editing again.")]
    Conflict,
    #[error("Another process is saving settings. Try again after it finishes.")]
    Busy,
    #[error(
        "A migration backup already exists with different contents. Preserve or move that backup before retrying."
    )]
    BackupConflict,
    #[error(
        "Legacy agent settings are invalid or unsupported. Repair the original file; it has not been replaced."
    )]
    LegacyInvalid,
    #[error("Could not migrate legacy settings from {path}: {error}")]
    Migration {
        path: PathBuf,
        #[source]
        error: Box<ConfigError>,
    },
}

#[derive(Clone)]
pub struct ConfigStore {
    path: PathBuf,
}

impl ConfigStore {
    pub fn user() -> Result<Self, ConfigError> {
        let directory = dirs::config_dir().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "user configuration directory unavailable",
            )
        })?;
        Ok(Self::new(
            directory.join("ssh_clipboard").join("settings-v1.json"),
        ))
    }

    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Read only: starting an unconfigured application does not create a file.
    pub fn load(&self) -> Result<Option<Configuration>, ConfigError> {
        read_bounded(&self.path)?
            .map(|bytes| decode_config(&bytes))
            .transpose()
    }

    /// Only default startup/target-less CLI discovery imports the legacy agent.
    pub fn initialize(&self) -> Result<Option<Configuration>, ConfigError> {
        if let Some(config) = self.load()? {
            return Ok(Some(config));
        }
        let legacy_path = confy::get_configuration_file_path("ssh_clipboard", Some("agent"))
            .map_err(|_| ConfigError::LegacyInvalid)?;
        self.initialize_from_legacy(&legacy_path)
    }

    pub fn initialize_from_legacy(
        &self,
        legacy_path: &Path,
    ) -> Result<Option<Configuration>, ConfigError> {
        let legacy_path = if legacy_path.is_absolute() {
            legacy_path.to_path_buf()
        } else {
            std::env::current_dir()?.join(legacy_path)
        };
        self.import_legacy(&legacy_path)
            .map_err(|error| ConfigError::Migration {
                path: legacy_path,
                error: Box::new(error),
            })
    }

    fn import_legacy(&self, legacy_path: &Path) -> Result<Option<Configuration>, ConfigError> {
        if let Some(config) = self.load()? {
            return Ok(Some(config));
        }
        let Some(bytes) = read_bounded(legacy_path)? else {
            return Ok(None);
        };
        let mut next = migration::convert_agent(&bytes)?;
        let backup = legacy_path.with_extension("pre-shared.bak");
        next.legacy_import = Some(LegacyImport {
            source: legacy_path.into(),
            backup: backup.clone(),
        });
        let pending = self.prepare(None, next)?;
        create_backup(&backup, &bytes)?;
        if read_bounded(legacy_path)?.as_deref() != Some(bytes.as_slice()) {
            return Err(ConfigError::Conflict);
        }
        // Keep the writer lock held while the backup and provenance are completed.
        Ok(Some(pending.commit()?))
    }

    /// Keep the separate lock inode/handle stable across atomic replacement.
    /// The caller may activate OS settings after preparation, before committing.
    pub fn prepare(
        &self,
        expected: Option<&Configuration>,
        mut next: Configuration,
    ) -> Result<PreparedSave, ConfigError> {
        next.validate()?;
        let parent = self
            .path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .ok_or(ConfigError::Invalid)?;
        let mut directory = fs::DirBuilder::new();
        directory.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            directory.mode(0o700);
        }
        directory.create(parent)?;
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let lock = options.open(self.path.with_extension("json.lock"))?;
        lock.try_lock().map_err(|error| match error {
            std::fs::TryLockError::WouldBlock => ConfigError::Busy,
            std::fs::TryLockError::Error(error) => ConfigError::Io(error),
        })?;
        let original = read_bounded(&self.path)?;
        let current = original.as_deref().map(decode_config).transpose()?;
        if current.as_ref() != expected {
            return Err(ConfigError::Conflict);
        }
        next.revision = expected
            .map_or(0, |c| c.revision)
            .checked_add(1)
            .ok_or(ConfigError::Invalid)?;
        if next.device.id.is_none() {
            next.device.id = Some(uuid::Uuid::new_v4().to_string());
        }
        if let Some(bytes) = &original {
            let version = serde_json::from_slice::<serde_json::Value>(bytes)
                .map_err(|_| ConfigError::Invalid)?;
            if version["schema_version"] == 1 {
                create_backup(&self.path.with_extension("schema-1.bak"), bytes)?;
            }
        }
        let temporary = NamedTempFile::new_in(parent)?;
        let mut pending = PreparedSave {
            path: self.path.clone(),
            temporary,
            config: next,
            _lock: lock,
        };
        pending.rewrite()?;
        Ok(pending)
    }
}

fn read_bounded(path: &Path) -> Result<Option<Vec<u8>>, ConfigError> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let mut bytes = Vec::new();
    file.take(MAX_CONFIG_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_CONFIG_BYTES {
        return Err(ConfigError::Invalid);
    }
    Ok(Some(bytes))
}

fn decode_config(bytes: &[u8]) -> Result<Configuration, ConfigError> {
    // Check the schema before decoding fields, so future files get a useful error.
    let mut value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|_| ConfigError::Invalid)?;
    let version = value
        .get("schema_version")
        .and_then(serde_json::Value::as_u64)
        .and_then(|v| u32::try_from(v).ok())
        .ok_or(ConfigError::Invalid)?;
    if version != 1 && version != SCHEMA_VERSION {
        return Err(ConfigError::UnsupportedVersion(version));
    }
    if version == 1 {
        migration::upgrade_schema_one(&mut value)?;
    }
    let config: Configuration = serde_json::from_value(value).map_err(|_| ConfigError::Invalid)?;
    config.validate()?;
    Ok(config)
}

fn create_backup(path: &Path, bytes: &[u8]) -> Result<(), ConfigError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if !metadata.file_type().is_file() || read_bounded(path)?.as_deref() != Some(bytes) {
                return Err(ConfigError::BackupConflict);
            }
            return Ok(());
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let mut temporary = NamedTempFile::new_in(path.parent().ok_or(ConfigError::Invalid)?)?;
    temporary.write_all(bytes)?;
    temporary.as_file().sync_all()?;
    temporary
        .persist_noclobber(path)
        .map_err(|error| ConfigError::Io(error.error))?;
    Ok(())
}

pub struct PreparedSave {
    path: PathBuf,
    temporary: NamedTempFile,
    config: Configuration,
    _lock: File,
}

impl PreparedSave {
    fn rewrite(&mut self) -> Result<(), ConfigError> {
        use std::io::{Seek, SeekFrom};
        let bytes = serde_json::to_vec_pretty(&self.config).map_err(|_| ConfigError::Invalid)?;
        if bytes.len() as u64 >= MAX_CONFIG_BYTES {
            return Err(ConfigError::Invalid);
        }
        self.temporary.as_file_mut().set_len(0)?;
        self.temporary.seek(SeekFrom::Start(0))?;
        self.temporary.write_all(&bytes)?;
        self.temporary.write_all(b"\n")?;
        self.temporary.as_file().sync_all()?;
        Ok(())
    }
    pub fn commit(self) -> Result<Configuration, ConfigError> {
        self.temporary
            .persist(&self.path)
            .map_err(|error| ConfigError::Io(error.error))?;
        Ok(self.config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn store() -> (tempfile::TempDir, ConfigStore) {
        let directory = tempfile::tempdir().unwrap();
        let store = ConfigStore::new(directory.path().join("config/settings-v1.json"));
        (directory, store)
    }

    #[test]
    fn save_survives_reopening_without_writing_on_initial_load() {
        let (_directory, store) = store();
        assert!(store.load().unwrap().is_none());
        assert!(!store.path().parent().unwrap().exists());
        let mut config = Configuration::default();
        config.profiles[0].target = "test-hub".into();
        config.preferences.show_preview = false;
        let saved = store.prepare(None, config).unwrap().commit().unwrap();
        assert_eq!(saved.revision, 1);
        assert_eq!(
            ConfigStore::new(store.path.clone()).load().unwrap(),
            Some(saved)
        );
    }

    #[test]
    fn stale_writer_cannot_overwrite_another_edit() {
        let (_directory, store) = store();
        let original = store
            .prepare(None, Configuration::default())
            .unwrap()
            .commit()
            .unwrap();
        let mut edited = original.clone();
        edited.profiles[0].target = "other-hub".into();
        let saved = store
            .prepare(Some(&original), edited)
            .unwrap()
            .commit()
            .unwrap();
        assert!(matches!(
            store.prepare(Some(&original), original.clone()),
            Err(ConfigError::Conflict)
        ));
        assert_eq!(store.load().unwrap(), Some(saved));
    }

    #[test]
    fn lock_and_abandoned_save_preserve_the_previous_file() {
        let (_directory, store) = store();
        let original = store
            .prepare(None, Configuration::default())
            .unwrap()
            .commit()
            .unwrap();
        let bytes = fs::read(store.path()).unwrap();
        let pending = store.prepare(Some(&original), original.clone()).unwrap();
        assert!(matches!(
            store.prepare(Some(&original), original.clone()),
            Err(ConfigError::Busy)
        ));
        assert_eq!(fs::read(store.path()).unwrap(), bytes);
        drop(pending);
        assert_eq!(store.load().unwrap(), Some(original.clone()));
        assert!(store.prepare(Some(&original), original.clone()).is_ok());
    }

    #[test]
    fn corrupt_future_and_unknown_fields_are_never_replaced() {
        let (_directory, store) = store();
        fs::create_dir_all(store.path().parent().unwrap()).unwrap();
        for bytes in [
            b"broken".as_slice(),
            br#"{"schema_version":42}"#,
            br#"{"schema_version":1,"unrecognized":true}"#,
        ] {
            fs::write(store.path(), bytes).unwrap();
            assert!(store.load().is_err());
            assert!(store.prepare(None, Configuration::default()).is_err());
            assert_eq!(fs::read(store.path()).unwrap(), bytes);
        }
    }

    #[test]
    fn profile_identity_and_selection_are_validated() {
        let (_directory, store) = store();
        let mut config = Configuration::default();
        config.profiles.push(config.profiles[0].clone());
        assert!(store.prepare(None, config).is_err());
        let config = Configuration {
            default_profile: "missing".into(),
            ..Default::default()
        };
        assert!(store.prepare(None, config).is_err());
    }

    #[test]
    fn oversized_files_and_saves_are_bounded_and_preserved() {
        let (_directory, store) = store();
        let original = store
            .prepare(None, Configuration::default())
            .unwrap()
            .commit()
            .unwrap();
        let mut oversized = original.clone();
        oversized.profiles[0].name = "x".repeat(MAX_CONFIG_BYTES as usize);
        assert!(matches!(
            store.prepare(Some(&original), oversized),
            Err(ConfigError::Invalid)
        ));
        assert_eq!(store.load().unwrap(), Some(original));
        let bytes = vec![b' '; MAX_CONFIG_BYTES as usize + 1];
        fs::write(store.path(), &bytes).unwrap();
        assert!(matches!(store.load(), Err(ConfigError::Invalid)));
        assert_eq!(
            fs::metadata(store.path()).unwrap().len(),
            MAX_CONFIG_BYTES + 1
        );
    }

    #[test]
    fn failed_replacement_keeps_the_original_bytes_and_releases_the_lock() {
        let (_directory, store) = store();
        let original = store
            .prepare(None, Configuration::default())
            .unwrap()
            .commit()
            .unwrap();
        let bytes = fs::read(store.path()).unwrap();
        let pending = store.prepare(Some(&original), original.clone()).unwrap();
        // A directory at the destination deterministically rejects replacement on all platforms.
        fs::remove_file(store.path()).unwrap();
        fs::create_dir(store.path()).unwrap();
        assert!(pending.commit().is_err());
        fs::remove_dir(store.path()).unwrap();
        fs::write(store.path(), &bytes).unwrap();
        assert_eq!(store.load().unwrap(), Some(original.clone()));
        assert!(store.prepare(Some(&original), original.clone()).is_ok());
    }
}
