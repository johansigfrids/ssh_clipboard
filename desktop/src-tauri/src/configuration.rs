use ssh_clipboard::configuration::{ConfigError, ConfigStore, Configuration};

#[cfg(any(all(target_os = "macos", native_update_test), test))]
fn isolated_store(shared: &ConfigStore) -> Result<ConfigStore, ConfigError> {
    let root = shared
        .path()
        .parent()
        .and_then(std::path::Path::parent)
        .ok_or(ConfigError::Invalid)?;
    Ok(ConfigStore::new(
        root.join("ssh_clipboard-update-test")
            .join("settings-v1.json"),
    ))
}

pub fn store() -> Result<ConfigStore, ConfigError> {
    let shared = ConfigStore::user()?;
    #[cfg(all(target_os = "macos", native_update_test))]
    {
        isolated_store(&shared)
    }
    #[cfg(not(all(target_os = "macos", native_update_test)))]
    {
        Ok(shared)
    }
}

pub fn initialize(store: &ConfigStore) -> Result<Option<Configuration>, ConfigError> {
    // Native Mac rehearsals must neither discover nor back up real legacy files.
    #[cfg(all(target_os = "macos", native_update_test))]
    {
        store.load()
    }
    #[cfg(not(all(target_os = "macos", native_update_test)))]
    {
        store.initialize()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rehearsal_settings_writes_preserve_the_normal_store() {
        let directory = tempfile::tempdir().unwrap();
        let normal = ConfigStore::new(directory.path().join("ssh_clipboard/settings-v1.json"));
        let production = normal
            .prepare(None, Configuration::default())
            .unwrap()
            .commit()
            .unwrap();
        let original = std::fs::read(normal.path()).unwrap();
        let rehearsal = isolated_store(&normal).unwrap();
        assert!(rehearsal.load().unwrap().is_none());
        let mut config = production;
        config.device.name = "Disposable Mac update test".into();
        config.preferences.start_at_login = false;
        rehearsal.prepare(None, config).unwrap().commit().unwrap();
        assert_eq!(std::fs::read(normal.path()).unwrap(), original);
        assert_eq!(
            rehearsal.load().unwrap().unwrap().device.name,
            "Disposable Mac update test"
        );
    }
}
