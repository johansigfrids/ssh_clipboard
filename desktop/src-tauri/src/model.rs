use base64::{Engine, engine::general_purpose::STANDARD};
use image::ImageEncoder;
use serde::{Deserialize, Serialize};
use ssh_clipboard::client::{ssh::SshConfig, transport::ClientConfig};
use ssh_clipboard::protocol::{
    CONTENT_TYPE_PNG, CONTENT_TYPE_TEXT, ClipboardValue, DEFAULT_MAX_SIZE, Request, RequestKind,
};

pub const MAX_DECODED_BYTES: usize = 64 * 1024 * 1024;

pub fn validate_transfer_size(request: &Request) -> Result<(), String> {
    if let RequestKind::Set { value } = &request.kind {
        let bytes = ssh_clipboard::framing::encode_message(request)
            .map_err(|_| "Could not prepare this item for sharing.".to_string())?
            .len();
        if bytes > DEFAULT_MAX_SIZE {
            let item = if value.content_type == CONTENT_TYPE_PNG {
                "image"
            } else {
                "text"
            };
            return Err(format!(
                "This {item} needs {:.2} MiB to share, above the 10 MiB transfer limit including metadata. Copy a smaller item and try again.",
                bytes as f64 / 1048576.0
            ));
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSettings {
    pub target: String,
    pub port: Option<u16>,
    pub identity_file: String,
    pub open_shortcut: String,
    pub shortcut_enabled: bool,
    pub show_preview: bool,
    pub close_after_copy: bool,
}

impl Default for SessionSettings {
    fn default() -> Self {
        Self {
            target: String::new(),
            port: None,
            identity_file: String::new(),
            open_shortcut: "CommandOrControl+Shift+V".into(),
            shortcut_enabled: !is_wayland(),
            show_preview: true,
            close_after_copy: false,
        }
    }
}

impl SessionSettings {
    pub fn validate(&self) -> Result<(), String> {
        let target = self.target.trim();
        if target.starts_with('-') || target.chars().any(|c| c.is_whitespace() || c.is_control()) {
            return Err(
                "The SSH target must be one alias or user@host, without options or spaces.".into(),
            );
        }
        if self.port == Some(0) {
            return Err("SSH port must be between 1 and 65535.".into());
        }
        if self.identity_file.chars().any(char::is_control) {
            return Err("The identity file path contains a control character.".into());
        }
        Ok(())
    }

    pub fn validate_connection(&self) -> Result<(), String> {
        self.validate()?;
        if self.target.trim().is_empty() {
            return Err("Enter an SSH alias or user@host.".into());
        }
        Ok(())
    }

    pub fn client_config(&self) -> ClientConfig {
        ClientConfig {
            ssh: SshConfig {
                target: self.target.trim().into(),
                port: self.port,
                identity_file: (!self.identity_file.trim().is_empty())
                    .then(|| self.identity_file.trim().into()),
                user: None,
                host: None,
                // No password/host-trust prompts may block a hidden GUI child.
                ssh_options: vec!["BatchMode=yes".into()],
                ssh_bin: None,
            },
            max_size: DEFAULT_MAX_SIZE,
            timeout_ms: 7000,
            resync_frames: true,
            resync_max_bytes: 8192,
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
}

impl ItemPreview {
    pub fn from_value(value: &ClipboardValue, show_preview: bool) -> Result<Self, String> {
        let mut preview = Self {
            kind: "text",
            label: "Text".into(),
            text: None,
            image_url: None,
            encoded_bytes: value.data.len(),
            created_at: value.created_at,
        };
        match value.content_type.as_str() {
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
                let decoded =
                    ssh_clipboard::client::image::decode_png(&value.data, MAX_DECODED_BYTES)
                        .map_err(|_| {
                            "The remote PNG is invalid or exceeds the 64 MiB decoded-image limit."
                                .to_string()
                        })?;
                preview.kind = "image";
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
            _ => return Err("The remote clipboard format is not supported.".into()),
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
    fn share_budget_includes_metadata_at_the_actual_wire_boundary() {
        let mut request = Request {
            request_id: u64::MAX,
            kind: RequestKind::Set {
                value: ClipboardValue {
                    content_type: CONTENT_TYPE_PNG.into(),
                    data: vec![0; DEFAULT_MAX_SIZE],
                    created_at: i64::MAX,
                },
            },
        };
        assert!(
            validate_transfer_size(&request)
                .unwrap_err()
                .contains("including metadata")
        );
        let overhead = ssh_clipboard::framing::encode_message(&request)
            .unwrap()
            .len()
            - DEFAULT_MAX_SIZE;
        if let RequestKind::Set { value } = &mut request.kind {
            value.data.truncate(DEFAULT_MAX_SIZE - overhead);
        }
        assert_eq!(
            ssh_clipboard::framing::encode_message(&request)
                .unwrap()
                .len(),
            DEFAULT_MAX_SIZE
        );
        assert!(validate_transfer_size(&request).is_ok());
        if let RequestKind::Set { value } = &mut request.kind {
            value.data.push(0);
        }
        assert!(validate_transfer_size(&request).is_err());
    }

    #[test]
    fn labels_use_content_without_modifying_the_payload() {
        let value = ClipboardValue {
            content_type: CONTENT_TYPE_TEXT.into(),
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
        let value = ClipboardValue {
            content_type: CONTENT_TYPE_TEXT.into(),
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
    fn prototype_codec_preserves_the_legacy_version_two_wire_contract() {
        use ssh_clipboard::framing::{decode_message, encode_message};
        use ssh_clipboard::protocol::{ErrorCode, Request, RequestKind, Response, ResponseKind};
        // Match the locked fixtures in src/protocol.rs using this project's lockfile.
        let set_fixture: &[u8] = &[
            42, 0, 25, 116, 101, 120, 116, 47, 112, 108, 97, 105, 110, 59, 32, 99, 104, 97, 114,
            115, 101, 116, 61, 117, 116, 102, 45, 56, 5, 104, 101, 108, 108, 111, 246,
        ];
        let error_fixture: &[u8] = &[7, 4, 1, 7, 116, 111, 111, 32, 98, 105, 103];
        let request = Request {
            request_id: 42,
            kind: RequestKind::Set {
                value: ClipboardValue {
                    content_type: CONTENT_TYPE_TEXT.into(),
                    data: b"hello".to_vec(),
                    created_at: 123,
                },
            },
        };
        assert_eq!(encode_message(&request).unwrap(), set_fixture);
        assert_eq!(
            decode_message::<Request>(set_fixture).unwrap().request_id,
            42
        );
        let response = Response {
            request_id: 7,
            kind: ResponseKind::Error {
                code: ErrorCode::PayloadTooLarge,
                message: "too big".into(),
            },
        };
        assert_eq!(encode_message(&response).unwrap(), error_fixture);
        assert_eq!(
            decode_message::<Response>(error_fixture)
                .unwrap()
                .request_id,
            7
        );
    }

    #[test]
    fn image_preview_is_bounded_and_can_be_hidden_without_losing_payload() {
        let data = ssh_clipboard::client::image::encode_png(arboard::ImageData {
            width: 1,
            height: 1,
            bytes: vec![255, 0, 0, 255].into(),
        })
        .unwrap();
        let value = ClipboardValue {
            content_type: CONTENT_TYPE_PNG.into(),
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
