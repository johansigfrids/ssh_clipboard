//! Local import review only: no OS registration, clipboard or SSH access.
use serde::Serialize;
use ssh_clipboard::configuration::{ConfigStore, Configuration};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    pub device_name: String,
    pub backup_path: String,
    pub start_at_login: bool,
}

pub fn view(config: &Configuration) -> Option<View> {
    let import = config.legacy_import.as_ref()?;
    (!import.reviewed).then(|| View {
        device_name: config.device.name.clone(),
        backup_path: import.backup.to_string_lossy().into_owned(),
        start_at_login: config.preferences.start_at_login,
    })
}

pub fn acknowledge(store: &ConfigStore, expected: &Configuration) -> Result<Configuration, String> {
    let mut next = expected.clone();
    let Some(import) = next
        .legacy_import
        .as_mut()
        .filter(|import| !import.reviewed)
    else {
        return Ok(next);
    };
    import.reviewed = true;
    store
        .prepare(Some(expected), next)
        .and_then(|pending| pending.commit())
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn imported() -> (tempfile::TempDir, ConfigStore, Configuration) {
        let directory = tempfile::tempdir().unwrap();
        let legacy = directory.path().join("agent.toml");
        std::fs::write(
            &legacy,
            include_bytes!("../../../tests/fixtures/legacy-agent.toml"),
        )
        .unwrap();
        let store = ConfigStore::new(directory.path().join("settings.json"));
        let config = store.initialize_from_legacy(&legacy).unwrap().unwrap();
        (directory, store, config)
    }

    #[test]
    fn acknowledgement_survives_reload_without_changing_setup_or_recovery_files() {
        let (_directory, store, config) = imported();
        let receipt = config.legacy_import.as_ref().unwrap();
        let source = std::fs::read(&receipt.source).unwrap();
        let backup = std::fs::read(&receipt.backup).unwrap();
        assert!(view(&config).is_some());
        let saved = acknowledge(&store, &config).unwrap();
        let mut expected = config.clone();
        expected.revision += 1;
        expected.legacy_import.as_mut().unwrap().reviewed = true;
        assert_eq!(saved, expected);
        let reloaded = store.load().unwrap().unwrap();
        assert_eq!(reloaded, saved);
        assert!(view(&reloaded).is_none());
        let bytes = std::fs::read(store.path()).unwrap();
        assert_eq!(acknowledge(&store, &reloaded).unwrap(), reloaded);
        assert_eq!(std::fs::read(store.path()).unwrap(), bytes);
        assert_eq!(std::fs::read(&receipt.source).unwrap(), source);
        assert_eq!(std::fs::read(&receipt.backup).unwrap(), backup);
    }

    #[test]
    fn a_stale_acknowledgement_cannot_overwrite_an_external_settings_change() {
        let (_directory, store, config) = imported();
        let mut external = config.clone();
        external.profiles[0].target = "changed-elsewhere".into();
        let external = store
            .prepare(Some(&config), external)
            .unwrap()
            .commit()
            .unwrap();
        assert!(acknowledge(&store, &config).is_err());
        assert_eq!(store.load().unwrap(), Some(external.clone()));
        assert!(view(&external).is_some());
    }

    #[test]
    fn a_failed_save_keeps_the_import_unreviewed() {
        let (_directory, store, config) = imported();
        // An occupied destination must not turn a failed write into an in-memory dismissal.
        let bytes = std::fs::read(store.path()).unwrap();
        std::fs::remove_file(store.path()).unwrap();
        std::fs::create_dir(store.path()).unwrap();
        assert!(acknowledge(&store, &config).is_err());
        assert!(view(&config).is_some());
        std::fs::remove_dir(store.path()).unwrap();
        std::fs::write(store.path(), bytes).unwrap();
        assert!(view(&store.load().unwrap().unwrap()).is_some());
    }
}
