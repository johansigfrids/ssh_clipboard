use super::{ConfigError, Configuration, Device, TransportSettings};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyAgent {
    config_version: u32,
    target: String,
    port: Option<u16>,
    identity_file: Option<String>,
    ssh_options: Vec<String>,
    max_size: usize,
    timeout_ms: u64,
    #[serde(default = "resync_default")]
    resync_frames: bool,
    #[serde(default = "resync_budget_default")]
    resync_max_bytes: usize,
    hotkeys: LegacyHotkeys,
    autostart_enabled: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyHotkeys {
    push: String,
    pull: String,
}
fn resync_default() -> bool {
    true
}
fn resync_budget_default() -> usize {
    8192
}

pub(super) fn convert_agent(bytes: &[u8]) -> Result<Configuration, ConfigError> {
    let text = std::str::from_utf8(bytes).map_err(|_| ConfigError::LegacyInvalid)?;
    let legacy: LegacyAgent = toml::from_str(text).map_err(|_| ConfigError::LegacyInvalid)?;
    if legacy.config_version != 1
        || legacy.hotkeys.push.trim().is_empty()
        || legacy.hotkeys.pull.trim().is_empty()
    {
        return Err(ConfigError::LegacyInvalid);
    }
    let mut config = Configuration::default();
    let profile = &mut config.profiles[0];
    profile.target = legacy.target.trim().into();
    profile.port = legacy.port;
    profile.identity_file = legacy.identity_file.unwrap_or_default();
    profile.transport = TransportSettings {
        ssh_options: legacy.ssh_options,
        ssh_bin: None,
        max_size: legacy.max_size,
        timeout_ms: legacy.timeout_ms,
        resync_frames: legacy.resync_frames,
        resync_max_bytes: legacy.resync_max_bytes,
    };
    config.preferences.start_at_login = legacy.autostart_enabled;
    config.preferences.direct_share_shortcut = Some(legacy.hotkeys.push);
    config.preferences.direct_copy_shortcut = Some(legacy.hotkeys.pull);
    // Preserve direct bindings, but never register them alongside the old agent automatically.
    config.preferences.direct_shortcuts_enabled = false;
    config.validate().map_err(|_| ConfigError::LegacyInvalid)?;
    Ok(config)
}

pub(super) fn upgrade_schema_one(value: &mut serde_json::Value) -> Result<(), ConfigError> {
    let object = value.as_object_mut().ok_or(ConfigError::Invalid)?;
    for key in ["device", "legacy_import"] {
        if object.contains_key(key) {
            return Err(ConfigError::Invalid);
        }
    }
    object.insert("schema_version".into(), 2.into());
    object.insert(
        "device".into(),
        serde_json::to_value(Device::default()).map_err(|_| ConfigError::Invalid)?,
    );
    object.insert("legacy_import".into(), serde_json::Value::Null);
    for profile in object
        .get_mut("profiles")
        .and_then(|v| v.as_array_mut())
        .ok_or(ConfigError::Invalid)?
    {
        let profile = profile.as_object_mut().ok_or(ConfigError::Invalid)?;
        if profile.contains_key("transport") || profile.contains_key("endpoint") {
            return Err(ConfigError::Invalid);
        }
        profile.insert(
            "transport".into(),
            serde_json::to_value(TransportSettings::default()).map_err(|_| ConfigError::Invalid)?,
        );
    }
    let preferences = object
        .get_mut("preferences")
        .and_then(|v| v.as_object_mut())
        .ok_or(ConfigError::Invalid)?;
    for (key, value) in [
        ("start_at_login", false.into()),
        ("direct_share_shortcut", serde_json::Value::Null),
        ("direct_copy_shortcut", serde_json::Value::Null),
        ("direct_shortcuts_enabled", false.into()),
        ("check_updates_automatically", false.into()),
    ] {
        if preferences.insert(key.into(), value).is_some() {
            return Err(ConfigError::Invalid);
        }
    }
    Ok(())
}
