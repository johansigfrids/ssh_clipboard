//! Desktop-owned text/PNG payload and native adapter boundary.
use eyre::Result;
use ssh_clipboard::{
    client::image::{self, ImageData},
    session_protocol::ContentType,
};
pub const CONTENT_TYPE_TEXT: ContentType = ContentType::Text;
pub const CONTENT_TYPE_PNG: ContentType = ContentType::Png;
#[derive(Clone, Debug)]
pub struct ClipboardPayload {
    pub content_type: ContentType,
    pub data: Vec<u8>,
    pub created_at: i64,
}
pub trait ClipboardAccess {
    /// None means that no native file-list representation is available.
    fn read_files(&mut self) -> Result<Option<Vec<std::path::PathBuf>>> {
        Ok(None)
    }
    fn write_files(&mut self, _: &[std::path::PathBuf]) -> Result<()> {
        eyre::bail!("Native file clipboard is unavailable.")
    }
    fn read_text(&mut self) -> Result<String>;
    fn read_image(&mut self) -> Result<ImageData<'static>>;
    fn write_text(&mut self, text: &str) -> Result<()>;
    fn write_image(&mut self, image: ImageData<'static>) -> Result<()>;
}
pub fn build_text_value(text: String, maximum: usize) -> Result<ClipboardPayload, String> {
    if text.len() > maximum {
        return Err("Text exceeds the item limit.".into());
    }
    Ok(ClipboardPayload {
        content_type: ContentType::Text,
        data: text.into_bytes(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64,
    })
}
pub fn apply_payload(
    value: ClipboardPayload,
    max_decoded: usize,
    clipboard: &mut impl ClipboardAccess,
) -> Result<(), String> {
    match value.content_type {
        ContentType::Files => {
            Err("File bundles require private staging before publication.".into())
        }
        ContentType::Text => clipboard
            .write_text(
                std::str::from_utf8(&value.data).map_err(|_| "The item is not valid UTF-8.")?,
            )
            .map_err(|error| error.to_string()),
        ContentType::Png => clipboard
            .write_image(
                image::decode_png(&value.data, max_decoded).map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Writes(usize);
    impl ClipboardAccess for Writes {
        fn read_text(&mut self) -> Result<String> {
            unreachable!()
        }
        fn read_image(&mut self) -> Result<ImageData<'static>> {
            unreachable!()
        }
        fn write_text(&mut self, _: &str) -> Result<()> {
            self.0 += 1;
            Ok(())
        }
        fn write_image(&mut self, _: ImageData<'static>) -> Result<()> {
            self.0 += 1;
            Ok(())
        }
    }

    #[test]
    fn malformed_or_over_budget_payloads_leave_native_clipboard_untouched() {
        let png = image::encode_png(ImageData {
            width: 1,
            height: 1,
            bytes: vec![1, 2, 3, 255].into(),
        })
        .unwrap();
        let mut clipboard = Writes(0);
        for (content_type, data, budget) in [
            (ContentType::Text, vec![255], 1024),
            (ContentType::Png, vec![1, 2, 3], 1024),
            (ContentType::Png, png, 3),
        ] {
            assert!(
                apply_payload(
                    ClipboardPayload {
                        content_type,
                        data,
                        created_at: 0
                    },
                    budget,
                    &mut clipboard
                )
                .is_err()
            );
            assert_eq!(clipboard.0, 0);
        }
    }
}
