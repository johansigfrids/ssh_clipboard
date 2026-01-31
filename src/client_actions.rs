use crate::client::clipboard;
use crate::client::image;
use crate::protocol::{
    CONTENT_TYPE_PNG, CONTENT_TYPE_TEXT, ClipboardValue, Response, ResponseKind,
};
use eyre::{Result, eyre};

pub struct ClipboardBuildError {
    pub code: i32,
    pub message: String,
}

pub async fn build_clipboard_value(
    stdin: bool,
    max_size: usize,
) -> Result<ClipboardValue, ClipboardBuildError> {
    if stdin {
        return Err(ClipboardBuildError {
            code: 2,
            message: "stdin mode not supported in agent".to_string(),
        });
    }

    match clipboard::read_text() {
        Ok(text) => build_text_value(text, max_size),
        Err(text_err) => match clipboard::read_image() {
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
    match response.kind {
        ResponseKind::Value { value } => {
            if value.content_type == CONTENT_TYPE_TEXT {
                let text = String::from_utf8(value.data)
                    .map_err(|_| eyre!("response was not valid UTF-8"))?;
                clipboard::write_text(&text)?;
                return Ok(());
            }

            if value.content_type == CONTENT_TYPE_PNG {
                let img = image::decode_png(&value.data, max_decoded_bytes)?;
                clipboard::write_image(img)?;
                return Ok(());
            }

            Err(eyre!("unsupported content type: {}", value.content_type))
        }
        ResponseKind::Empty => Err(eyre!("no clipboard value set")),
        ResponseKind::Error { message, .. } => Err(eyre!(message)),
        other => Err(eyre!("unexpected response: {other:?}")),
    }
}

fn build_text_value(text: String, max_size: usize) -> Result<ClipboardValue, ClipboardBuildError> {
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
