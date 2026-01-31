use crate::client::clipboard;
use crate::client::image;
use crate::protocol::{
    CONTENT_TYPE_PNG, CONTENT_TYPE_TEXT, ClipboardValue, Response, ResponseKind,
};
use eyre::{Result, eyre};
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct ClipboardBuildError {
    pub code: i32,
    pub message: String,
}

pub trait ClipboardAccess {
    fn read_text(&mut self) -> Result<String>;
    fn read_image(&mut self) -> Result<arboard::ImageData<'static>>;
    fn write_text(&mut self, text: &str) -> Result<()>;
    fn write_image(&mut self, image: arboard::ImageData<'static>) -> Result<()>;
}

struct SystemClipboard;

impl ClipboardAccess for SystemClipboard {
    fn read_text(&mut self) -> Result<String> {
        clipboard::read_text()
    }

    fn read_image(&mut self) -> Result<arboard::ImageData<'static>> {
        clipboard::read_image()
    }

    fn write_text(&mut self, text: &str) -> Result<()> {
        clipboard::write_text(text)
    }

    fn write_image(&mut self, image: arboard::ImageData<'static>) -> Result<()> {
        clipboard::write_image(image)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PullApplyErrorKind {
    InvalidUtf8,
    InvalidPayload,
    UnsupportedContentType,
    NoValue,
    Server,
    Clipboard,
    Unexpected,
}

#[derive(Debug, Clone, Error)]
#[error("{message}")]
pub struct PullApplyError {
    pub kind: PullApplyErrorKind,
    pub message: String,
}

pub fn build_clipboard_value_from_clipboard(
    max_size: usize,
) -> Result<ClipboardValue, ClipboardBuildError> {
    let mut clipboard = SystemClipboard;
    build_clipboard_value_with_clipboard(&mut clipboard, max_size)
}

pub fn build_clipboard_value_with_clipboard(
    clipboard: &mut impl ClipboardAccess,
    max_size: usize,
) -> Result<ClipboardValue, ClipboardBuildError> {
    match clipboard.read_text() {
        Ok(text) => build_text_value(text, max_size),
        Err(text_err) => match clipboard.read_image() {
            Ok(img) => {
                let png = image::encode_png(img).map_err(|err| ClipboardBuildError {
                    code: 2,
                    message: err.to_string(),
                })?;
                if png.len() > max_size {
                    return Err(ClipboardBuildError {
                        code: 3,
                        message: "payload too large".to_string(),
                    });
                }
                Ok(ClipboardValue {
                    content_type: CONTENT_TYPE_PNG.to_string(),
                    data: png,
                    created_at: now_epoch_millis(),
                })
            }
            Err(image_err) => Err(ClipboardBuildError {
                code: 6,
                message: format!("clipboard read failed (text: {text_err}; image: {image_err})"),
            }),
        },
    }
}

pub fn apply_pull_response_to_clipboard(
    response: Response,
    max_decoded_bytes: usize,
) -> Result<()> {
    let mut clipboard = SystemClipboard;
    apply_pull_response_with_clipboard(response, max_decoded_bytes, &mut clipboard)
        .map_err(|err| eyre!(err.message))
}

pub fn apply_pull_response_with_system_clipboard(
    response: Response,
    max_decoded_bytes: usize,
) -> Result<(), PullApplyError> {
    let mut clipboard = SystemClipboard;
    apply_pull_response_with_clipboard(response, max_decoded_bytes, &mut clipboard)
}

pub fn apply_pull_response_with_clipboard(
    response: Response,
    max_decoded_bytes: usize,
    clipboard: &mut impl ClipboardAccess,
) -> Result<(), PullApplyError> {
    match response.kind {
        ResponseKind::Value { value } => {
            if value.content_type == CONTENT_TYPE_TEXT {
                let text = String::from_utf8(value.data).map_err(|_| PullApplyError {
                    kind: PullApplyErrorKind::InvalidUtf8,
                    message: "response was not valid UTF-8".to_string(),
                })?;
                clipboard
                    .write_text(&text)
                    .map_err(|err| PullApplyError {
                        kind: PullApplyErrorKind::Clipboard,
                        message: err.to_string(),
                    })?;
                return Ok(());
            }

            if value.content_type == CONTENT_TYPE_PNG {
                let img = image::decode_png(&value.data, max_decoded_bytes).map_err(|err| {
                    PullApplyError {
                        kind: PullApplyErrorKind::InvalidPayload,
                        message: err.to_string(),
                    }
                })?;
                clipboard
                    .write_image(img)
                    .map_err(|err| PullApplyError {
                        kind: PullApplyErrorKind::Clipboard,
                        message: err.to_string(),
                    })?;
                return Ok(());
            }

            Err(PullApplyError {
                kind: PullApplyErrorKind::UnsupportedContentType,
                message: format!("unsupported content type: {}", value.content_type),
            })
        }
        ResponseKind::Empty => Err(PullApplyError {
            kind: PullApplyErrorKind::NoValue,
            message: "no clipboard value set".to_string(),
        }),
        ResponseKind::Error { message, .. } => Err(PullApplyError {
            kind: PullApplyErrorKind::Server,
            message,
        }),
        other => Err(PullApplyError {
            kind: PullApplyErrorKind::Unexpected,
            message: format!("unexpected response: {other:?}"),
        }),
    }
}

pub fn build_text_value(text: String, max_size: usize) -> Result<ClipboardValue, ClipboardBuildError> {
    let bytes = text.into_bytes();
    if bytes.len() > max_size {
        return Err(ClipboardBuildError {
            code: 3,
            message: "payload too large".to_string(),
        });
    }
    Ok(ClipboardValue {
        content_type: CONTENT_TYPE_TEXT.to_string(),
        data: bytes,
        created_at: now_epoch_millis(),
    })
}

fn now_epoch_millis() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use arboard::ImageData;

    #[derive(Default)]
    struct MockClipboard {
        text: Option<String>,
        image: Option<ImageData<'static>>,
        wrote_text: Option<String>,
        wrote_image: bool,
    }

    impl ClipboardAccess for MockClipboard {
        fn read_text(&mut self) -> Result<String> {
            self.text
                .take()
                .ok_or_else(|| eyre!("no text available"))
        }

        fn read_image(&mut self) -> Result<ImageData<'static>> {
            self.image
                .take()
                .ok_or_else(|| eyre!("no image available"))
        }

        fn write_text(&mut self, text: &str) -> Result<()> {
            self.wrote_text = Some(text.to_string());
            Ok(())
        }

        fn write_image(&mut self, _image: ImageData<'static>) -> Result<()> {
            self.wrote_image = true;
            Ok(())
        }
    }

    #[test]
    fn build_text_value_rejects_oversize() {
        let err = build_text_value("hello".to_string(), 3).unwrap_err();
        assert_eq!(err.code, 3);
    }

    #[test]
    fn apply_pull_response_rejects_unknown_content_type() {
        let response = Response {
            request_id: 1,
            kind: ResponseKind::Value {
                value: ClipboardValue {
                    content_type: "application/octet-stream".to_string(),
                    data: vec![1, 2, 3],
                    created_at: 0,
                },
            },
        };
        let mut clipboard = MockClipboard::default();
        let err = apply_pull_response_with_clipboard(response, 1024, &mut clipboard).unwrap_err();
        assert_eq!(err.kind, PullApplyErrorKind::UnsupportedContentType);
    }

    #[test]
    fn apply_pull_response_empty_is_error() {
        let response = Response {
            request_id: 1,
            kind: ResponseKind::Empty,
        };
        let mut clipboard = MockClipboard::default();
        let err = apply_pull_response_with_clipboard(response, 1024, &mut clipboard).unwrap_err();
        assert_eq!(err.kind, PullApplyErrorKind::NoValue);
    }

    #[test]
    fn apply_pull_response_error_message_passthrough() {
        let response = Response {
            request_id: 1,
            kind: ResponseKind::Error {
                code: crate::protocol::ErrorCode::Internal,
                message: "boom".to_string(),
            },
        };
        let mut clipboard = MockClipboard::default();
        let err = apply_pull_response_with_clipboard(response, 1024, &mut clipboard).unwrap_err();
        assert_eq!(err.kind, PullApplyErrorKind::Server);
        assert_eq!(err.message, "boom");
    }

    #[test]
    fn build_clipboard_value_prefers_text() {
        let mut clipboard = MockClipboard {
            text: Some("hi".to_string()),
            image: Some(ImageData {
                width: 1,
                height: 1,
                bytes: vec![255, 0, 0, 255].into(),
            }),
            wrote_text: None,
            wrote_image: false,
        };
        let value = build_clipboard_value_with_clipboard(&mut clipboard, 1024).unwrap();
        assert_eq!(value.content_type, CONTENT_TYPE_TEXT);
        assert_eq!(value.data, b"hi");
    }

    #[test]
    fn build_clipboard_value_falls_back_to_image() {
        let mut clipboard = MockClipboard {
            text: None,
            image: Some(ImageData {
                width: 1,
                height: 1,
                bytes: vec![0, 0, 0, 255].into(),
            }),
            wrote_text: None,
            wrote_image: false,
        };
        let value = build_clipboard_value_with_clipboard(&mut clipboard, 1024).unwrap();
        assert_eq!(value.content_type, CONTENT_TYPE_PNG);
        assert!(!value.data.is_empty());
    }

    #[test]
    fn apply_pull_response_writes_text() {
        let response = Response {
            request_id: 1,
            kind: ResponseKind::Value {
                value: ClipboardValue {
                    content_type: CONTENT_TYPE_TEXT.to_string(),
                    data: b"hello".to_vec(),
                    created_at: 0,
                },
            },
        };
        let mut clipboard = MockClipboard::default();
        apply_pull_response_with_clipboard(response, 1024, &mut clipboard).unwrap();
        assert_eq!(clipboard.wrote_text.as_deref(), Some("hello"));
    }

    #[test]
    fn apply_pull_response_writes_png() {
        let png = image::encode_png(ImageData {
            width: 1,
            height: 1,
            bytes: vec![255, 255, 255, 255].into(),
        })
        .unwrap();
        let response = Response {
            request_id: 1,
            kind: ResponseKind::Value {
                value: ClipboardValue {
                    content_type: CONTENT_TYPE_PNG.to_string(),
                    data: png,
                    created_at: 0,
                },
            },
        };
        let mut clipboard = MockClipboard::default();
        apply_pull_response_with_clipboard(response, 1024 * 1024, &mut clipboard).unwrap();
        assert!(clipboard.wrote_image);
    }
}
