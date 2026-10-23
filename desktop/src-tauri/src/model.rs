use crate::clipboard_payload::{CONTENT_TYPE_PNG, CONTENT_TYPE_TEXT, ClipboardPayload};
use crate::connection::TransportDraft;
use base64::{Engine, engine::general_purpose::STANDARD};
use image::ImageEncoder;
use serde::{Deserialize, Serialize};
use ssh_clipboard::client::config::ClientConfig;

#[cfg(test)]
pub(crate) const MAX_DECODED_BYTES: usize = 256 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSettings {
    pub profile_id: Option<String>,
    pub profile_name: String,
    pub target: String,
    pub port: Option<u16>,
    pub identity_file: String,
    pub transport: TransportDraft,
    #[serde(default)]
    pub limits: ssh_clipboard::configuration::SessionLimits,
    pub open_shortcut: String,
    pub shortcut_enabled: bool,
    pub direct_shortcuts_enabled: bool,
    pub direct_share_shortcut: Option<String>,
    pub direct_copy_shortcut: Option<String>,
    pub show_preview: bool,
    pub close_after_copy: bool,
    pub notifications: ssh_clipboard::configuration::NotificationPreference,
}

impl Default for SessionSettings {
    fn default() -> Self {
        Self {
            profile_id: Some("default".into()),
            profile_name: "Default".into(),
            target: String::new(),
            port: None,
            identity_file: String::new(),
            transport: TransportDraft::default(),
            limits: Default::default(),
            open_shortcut: "CommandOrControl+Alt+Shift+S".into(),
            shortcut_enabled: !is_wayland(),
            direct_shortcuts_enabled: false,
            direct_share_shortcut: None,
            direct_copy_shortcut: None,
            show_preview: true,
            close_after_copy: false,
            notifications: Default::default(),
        }
    }
}

impl SessionSettings {
    pub fn from_configuration(
        config: &ssh_clipboard::configuration::Configuration,
    ) -> Result<Self, String> {
        let profile = config.selected_profile().map_err(|e| e.to_string())?;
        Ok(Self {
            profile_id: Some(profile.id.clone()),
            profile_name: profile.name.clone(),
            target: profile.target.clone(),
            port: profile.port,
            identity_file: profile.identity_file.clone(),
            transport: TransportDraft::from(&profile.transport),
            limits: profile.limits.clone(),
            open_shortcut: config.preferences.open_shortcut.clone(),
            shortcut_enabled: config.preferences.shortcut_enabled,
            direct_shortcuts_enabled: config.preferences.direct_shortcuts_enabled,
            direct_share_shortcut: config.preferences.direct_share_shortcut.clone(),
            direct_copy_shortcut: config.preferences.direct_copy_shortcut.clone(),
            show_preview: config.preferences.show_preview,
            close_after_copy: config.preferences.close_after_copy,
            notifications: config.preferences.notifications,
        })
    }

    pub fn update_configuration(
        &self,
        config: &mut ssh_clipboard::configuration::Configuration,
    ) -> Result<(), String> {
        self.validate()?;
        let transport = self.transport.to_shared()?;
        let id = if let Some(id) = &self.profile_id {
            if !config.profiles.iter().any(|p| &p.id == id) {
                return Err("This connection no longer exists. Discard your draft to reload saved settings.".into());
            }
            id.clone()
        } else {
            if self.target.trim().is_empty() {
                return Err("Enter an SSH alias or user@host for the new connection.".into());
            }
            let profile =
                ssh_clipboard::configuration::Profile::new(self.profile_name.trim().into());
            let id = profile.id.clone();
            config.profiles.push(profile);
            id
        };
        config.default_profile = id;
        let profile = config.selected_profile_mut().map_err(|e| e.to_string())?;
        profile.name = self.profile_name.trim().into();
        profile.target = self.target.clone();
        profile.port = self.port;
        profile.identity_file = self.identity_file.clone();
        profile.transport = transport;
        profile.limits = self.limits.clone();
        config.preferences.open_shortcut = self.open_shortcut.clone();
        config.preferences.shortcut_enabled = self.shortcut_enabled;
        config.preferences.direct_shortcuts_enabled = self.direct_shortcuts_enabled;
        config.preferences.direct_share_shortcut = self.direct_share_shortcut.clone();
        config.preferences.direct_copy_shortcut = self.direct_copy_shortcut.clone();
        config.preferences.show_preview = self.show_preview;
        config.preferences.close_after_copy = self.close_after_copy;
        config.preferences.notifications = self.notifications;
        Ok(())
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.profile_name.trim().is_empty() || self.profile_name.chars().any(char::is_control) {
            return Err("Enter a connection name without control characters.".into());
        }
        let transport = self.transport.to_shared()?;
        self.limits.session_policy(transport.timeout_ms)?;
        ssh_clipboard::configuration::validate_connection(
            self.target.trim(),
            self.port,
            &self.identity_file,
        )
    }

    pub fn validate_connection(&self) -> Result<(), String> {
        self.validate()?;
        if self.target.trim().is_empty() {
            return Err("Enter an SSH alias or user@host.".into());
        }
        Ok(())
    }

    pub fn client_config(&self) -> Result<ClientConfig, String> {
        ssh_clipboard::configuration::Profile {
            id: self.profile_id.clone().unwrap_or_default(),
            name: self.profile_name.clone(),
            target: self.target.trim().into(),
            port: self.port,
            identity_file: self.identity_file.clone(),
            transport: self.transport.to_shared()?,
            limits: self.limits.clone(),
        }
        .client_config()
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionProfile {
    pub id: String,
    pub name: String,
    pub target: String,
    pub port: Option<u16>,
    pub identity_file: String,
    pub transport: TransportDraft,
    pub limits: ssh_clipboard::configuration::SessionLimits,
}

impl From<&ssh_clipboard::configuration::Profile> for ConnectionProfile {
    fn from(profile: &ssh_clipboard::configuration::Profile) -> Self {
        Self {
            id: profile.id.clone(),
            name: profile.name.clone(),
            target: profile.target.clone(),
            port: profile.port,
            identity_file: profile.identity_file.clone(),
            transport: TransportDraft::from(&profile.transport),
            limits: profile.limits.clone(),
        }
    }
}

pub fn is_wayland() -> bool {
    cfg!(target_os = "linux")
        && (std::env::var_os("WAYLAND_DISPLAY").is_some()
            || std::env::var("XDG_SESSION_TYPE").is_ok_and(|v| v.eq_ignore_ascii_case("wayland")))
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemPreview {
    pub kind: &'static str,
    pub label: String,
    pub text: Option<String>,
    pub image_url: Option<String>,
    pub encoded_bytes: usize,
    pub created_at: i64,
    /// Local preparation metadata, never part of preview IPC (including hidden previews).
    #[serde(skip)]
    pub dimensions: Option<ssh_clipboard::session_protocol::Dimensions>,
}

impl ItemPreview {
    #[cfg(test)]
    pub fn from_value(value: &ClipboardPayload, show_preview: bool) -> Result<Self, String> {
        Self::from_value_with_limit(value, show_preview, MAX_DECODED_BYTES)
    }

    pub fn from_value_with_limit(
        value: &ClipboardPayload,
        show_preview: bool,
        max_decoded: usize,
    ) -> Result<Self, String> {
        let mut preview = Self {
            kind: "text",
            label: "Text".into(),
            text: None,
            image_url: None,
            encoded_bytes: value.data.len(),
            created_at: value.created_at,
            dimensions: None,
        };
        match value.content_type {
            ssh_clipboard::session_protocol::ContentType::Files => {
                let bundle = ssh_clipboard::files::validate(&value.data)
                    .map_err(|_| "The remote file bundle is invalid.".to_string())?;
                preview.kind = "files";
                preview.label = bundle.label();
                if show_preview {
                    preview.text = Some(bundle.preview());
                }
            }
            CONTENT_TYPE_TEXT => {
                let text = std::str::from_utf8(&value.data)
                    .map_err(|_| "The remote text is not valid UTF-8.".to_string())?;
                if show_preview {
                    let line = text
                        .lines()
                        .find(|line| !line.trim().is_empty())
                        .unwrap_or("");
                    preview.label = if line.is_empty() {
                        "Empty text".into()
                    } else {
                        truncate(line.trim(), 60)
                    };
                    preview.text = Some(truncate(text, 2000));
                }
            }
            CONTENT_TYPE_PNG => {
                let decoded = ssh_clipboard::client::image::decode_png(&value.data, max_decoded)
                    .map_err(|_| {
                        format!(
                            "The remote PNG is invalid or exceeds the {} MiB decoded-image limit.",
                            max_decoded / 1048576
                        )
                    })?;
                preview.kind = "image";
                preview.dimensions = Some(ssh_clipboard::session_protocol::Dimensions {
                    width: decoded.width as u32,
                    height: decoded.height as u32,
                });
                preview.label = format!("Image ({} x {})", decoded.width, decoded.height);
                if show_preview {
                    let rgba = image::RgbaImage::from_raw(
                        decoded.width as u32,
                        decoded.height as u32,
                        decoded.bytes.into_owned(),
                    )
                    .ok_or_else(|| "Invalid image dimensions.".to_string())?;
                    let thumbnail = image::DynamicImage::ImageRgba8(rgba)
                        .thumbnail(320, 180)
                        .into_rgba8();
                    let mut png = Vec::new();
                    image::codecs::png::PngEncoder::new(&mut png)
                        .write_image(
                            thumbnail.as_raw(),
                            thumbnail.width(),
                            thumbnail.height(),
                            image::ExtendedColorType::Rgba8,
                        )
                        .map_err(|_| "Could not prepare the image preview.".to_string())?;
                    preview.image_url =
                        Some(format!("data:image/png;base64,{}", STANDARD.encode(png)));
                }
            }
        }
        Ok(preview)
    }
}

fn truncate(text: &str, limit: usize) -> String {
    let mut characters = text.chars();
    let prefix: String = characters.by_ref().take(limit).collect();
    if characters.next().is_some() {
        format!("{prefix}...")
    } else {
        prefix
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_or_last_profile_cannot_be_removed_or_recreated_by_a_stale_edit() {
        let mut config = ssh_clipboard::configuration::Configuration::default();
        let original = config.clone();
        assert!(config.remove_profile("missing").is_err());
        assert!(config.remove_profile("default").is_err());
        let stale = SessionSettings {
            profile_id: Some("missing".into()),
            target: "stale-hub".into(),
            ..Default::default()
        };
        assert!(stale.update_configuration(&mut config).is_err());
        assert_eq!(config, original);

        let empty = SessionSettings {
            profile_id: None,
            ..Default::default()
        };
        assert!(empty.update_configuration(&mut config).is_err());
        assert_eq!(config, original);
    }

    #[test]
    fn settings_round_trip_preserves_profile_identity_and_other_profiles() {
        let mut config = ssh_clipboard::configuration::Configuration::default();
        config.preferences.start_at_login = true;
        config.preferences.direct_share_shortcut = Some("Ctrl+Alt+KeyC".into());
        config.preferences.direct_copy_shortcut = Some("Ctrl+Alt+KeyV".into());
        config.profiles[0].transport.timeout_ms = 9000;
        let mut other = config.profiles[0].clone();
        other.id = "other".into();
        other.name = "Other".into();
        other.target = "another-hub".into();
        config.profiles.push(other.clone());
        let settings = SessionSettings {
            target: "selected-hub".into(),
            identity_file: "path with spaces/key".into(),
            port: Some(2222),
            show_preview: false,
            close_after_copy: true,
            direct_shortcuts_enabled: true,
            notifications: ssh_clipboard::configuration::NotificationPreference::Off,
            ..SessionSettings::from_configuration(&config).unwrap()
        };
        settings.update_configuration(&mut config).unwrap();
        let restored = SessionSettings::from_configuration(&config).unwrap();
        assert_eq!(restored.target, settings.target);
        assert_eq!(restored.port, Some(2222));
        assert_eq!(restored.identity_file, settings.identity_file);
        assert!(!restored.show_preview);
        assert!(restored.close_after_copy);
        assert!(restored.direct_shortcuts_enabled);
        assert_eq!(
            restored.notifications,
            ssh_clipboard::configuration::NotificationPreference::Off
        );
        assert_eq!(
            restored.direct_copy_shortcut.as_deref(),
            Some("Ctrl+Alt+KeyV")
        );
        assert_eq!(config.profiles[0].id, "default");
        assert_eq!(config.default_profile, "default");
        assert_eq!(config.profiles[1], other);
        assert!(config.preferences.start_at_login);
        assert_eq!(
            config.preferences.direct_share_shortcut.as_deref(),
            Some("Ctrl+Alt+KeyC")
        );
        assert_eq!(config.profiles[0].transport.timeout_ms, 9000);
    }

    #[test]
    fn labels_use_content_without_modifying_the_payload() {
        let value = ClipboardPayload {
            content_type: CONTENT_TYPE_TEXT,
            data: "\n  Notes\nKeep spacing  ".as_bytes().to_vec(),
            created_at: 7,
        };
        let preview = ItemPreview::from_value(&value, true).unwrap();
        assert_eq!(preview.label, "Notes");
        assert_eq!(preview.text.as_deref(), Some("\n  Notes\nKeep spacing  "));
        assert_eq!(value.data, b"\n  Notes\nKeep spacing  ");
    }

    #[test]
    fn hidden_previews_do_not_send_text_or_labels_to_the_frontend() {
        let value = ClipboardPayload {
            content_type: CONTENT_TYPE_TEXT,
            data: b"private contents".to_vec(),
            created_at: 0,
        };
        let preview = ItemPreview::from_value(&value, false).unwrap();
        assert_eq!(preview.label, "Text");
        assert!(preview.text.is_none());
    }

    #[test]
    fn labels_truncate_unicode_on_character_boundaries() {
        assert_eq!(
            truncate(&"\u{1f642}".repeat(61), 60),
            format!("{}...", "\u{1f642}".repeat(60))
        );
    }

    #[test]
    fn target_cannot_be_an_ssh_option_or_multiple_arguments() {
        for target in ["", "-oProxyCommand=anything", "host extra", "host\nextra"] {
            let settings = SessionSettings {
                target: target.into(),
                ..Default::default()
            };
            assert!(settings.validate_connection().is_err());
        }
        let settings = SessionSettings {
            target: "clipboard-hub".into(),
            ..Default::default()
        };
        assert!(settings.validate().is_ok());
    }

    #[test]
    fn unconfigured_app_can_change_preferences_without_a_connection() {
        let settings = SessionSettings {
            show_preview: false,
            ..Default::default()
        };
        assert!(settings.validate().is_ok());
        assert!(settings.validate_connection().is_err());
    }

    #[test]
    fn image_preview_is_bounded_and_can_be_hidden_without_losing_payload() {
        let data =
            ssh_clipboard::client::image::encode_png(ssh_clipboard::client::image::ImageData {
                width: 1,
                height: 1,
                bytes: vec![255, 0, 0, 255].into(),
            })
            .unwrap();
        let value = ClipboardPayload {
            content_type: CONTENT_TYPE_PNG,
            data: data.clone(),
            created_at: 0,
        };
        let preview = ItemPreview::from_value(&value, true).unwrap();
        assert_eq!(preview.label, "Image (1 x 1)");
        assert!(
            preview
                .image_url
                .unwrap()
                .starts_with("data:image/png;base64,")
        );
        assert!(
            ItemPreview::from_value(&value, false)
                .unwrap()
                .image_url
                .is_none()
        );
        assert_eq!(value.data, data);
    }
}
