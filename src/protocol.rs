use serde::{Deserialize, Serialize};

pub const MAGIC: [u8; 4] = *b"SCB1";
pub const VERSION: u16 = 1;
pub const CONTENT_TYPE_TEXT: &str = "text/plain; charset=utf-8";
pub const DEFAULT_MAX_SIZE: usize = 10 * 1024 * 1024;
pub const RESPONSE_OVERHEAD: usize = 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardValue {
    pub content_type: String,
    pub data: Vec<u8>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Request {
    Set { value: ClipboardValue },
    Get,
    PeekMeta,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Response {
    Ok,
    Value {
        value: ClipboardValue,
    },
    Meta {
        content_type: String,
        size: u64,
        created_at: i64,
    },
    Empty,
    Error {
        code: ErrorCode,
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    InvalidRequest,
    PayloadTooLarge,
    InvalidUtf8,
    Internal,
    DaemonNotRunning,
}
