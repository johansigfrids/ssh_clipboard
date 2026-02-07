use serde::{Deserialize, Serialize};
use wincode::{SchemaRead, SchemaWrite};

pub const MAGIC: [u8; 4] = *b"SCB1";
pub const VERSION: u16 = 2;
pub const CONTENT_TYPE_TEXT: &str = "text/plain; charset=utf-8";
pub const CONTENT_TYPE_PNG: &str = "image/png";
pub const DEFAULT_MAX_SIZE: usize = 10 * 1024 * 1024;
pub const RESPONSE_OVERHEAD: usize = 1024;

#[derive(Debug, Clone, Serialize, Deserialize, SchemaWrite, SchemaRead)]
pub struct ClipboardValue {
    pub content_type: String,
    pub data: Vec<u8>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, SchemaWrite, SchemaRead)]
pub struct Request {
    pub request_id: u64,
    pub kind: RequestKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, SchemaWrite, SchemaRead)]
pub enum RequestKind {
    Set { value: ClipboardValue },
    Get,
    PeekMeta,
}

#[derive(Debug, Clone, Serialize, Deserialize, SchemaWrite, SchemaRead)]
pub struct Response {
    pub request_id: u64,
    pub kind: ResponseKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, SchemaWrite, SchemaRead)]
pub enum ResponseKind {
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

#[derive(Debug, Clone, Serialize, Deserialize, SchemaWrite, SchemaRead)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    InvalidRequest,
    PayloadTooLarge,
    InvalidUtf8,
    Internal,
    DaemonNotRunning,
    VersionMismatch,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framing::codec_config;
    use wincode::config;

    const REQUEST_V2_SET_FIXTURE: &[u8] = &[
        42, 0, 25, 116, 101, 120, 116, 47, 112, 108, 97, 105, 110, 59, 32, 99, 104, 97, 114, 115,
        101, 116, 61, 117, 116, 102, 45, 56, 5, 104, 101, 108, 108, 111, 246,
    ];

    const RESPONSE_V2_ERROR_FIXTURE: &[u8] = &[7, 4, 1, 7, 116, 111, 111, 32, 98, 105, 103];

    #[test]
    fn request_round_trip_codec() {
        let request = Request {
            request_id: 42,
            kind: RequestKind::Set {
                value: ClipboardValue {
                    content_type: CONTENT_TYPE_TEXT.to_string(),
                    data: b"hello".to_vec(),
                    created_at: 123,
                },
            },
        };
        let payload = config::serialize(&request, codec_config()).unwrap();
        let decoded = config::deserialize::<Request, _>(&payload, codec_config()).unwrap();
        assert_eq!(decoded.request_id, 42);
        match decoded.kind {
            RequestKind::Set { value } => {
                assert_eq!(value.content_type, CONTENT_TYPE_TEXT);
                assert_eq!(value.data, b"hello");
                assert_eq!(value.created_at, 123);
            }
            other => panic!("unexpected request kind: {other:?}"),
        }
    }

    #[test]
    fn response_round_trip_codec() {
        let response = Response {
            request_id: 7,
            kind: ResponseKind::Meta {
                content_type: CONTENT_TYPE_PNG.to_string(),
                size: 999,
                created_at: 456,
            },
        };
        let payload = config::serialize(&response, codec_config()).unwrap();
        let decoded = config::deserialize::<Response, _>(&payload, codec_config()).unwrap();
        assert_eq!(decoded.request_id, 7);
        match decoded.kind {
            ResponseKind::Meta {
                content_type,
                size,
                created_at,
            } => {
                assert_eq!(content_type, CONTENT_TYPE_PNG);
                assert_eq!(size, 999);
                assert_eq!(created_at, 456);
            }
            other => panic!("unexpected response kind: {other:?}"),
        }
    }

    #[test]
    fn request_wire_fixture_is_stable() {
        let request = Request {
            request_id: 42,
            kind: RequestKind::Set {
                value: ClipboardValue {
                    content_type: CONTENT_TYPE_TEXT.to_string(),
                    data: b"hello".to_vec(),
                    created_at: 123,
                },
            },
        };
        let payload = config::serialize(&request, codec_config()).unwrap();
        assert_eq!(payload, REQUEST_V2_SET_FIXTURE);
        let decoded =
            config::deserialize::<Request, _>(REQUEST_V2_SET_FIXTURE, codec_config()).unwrap();
        assert_eq!(decoded.request_id, 42);
    }

    #[test]
    fn response_wire_fixture_is_stable() {
        let response = Response {
            request_id: 7,
            kind: ResponseKind::Error {
                code: ErrorCode::PayloadTooLarge,
                message: "too big".to_string(),
            },
        };
        let payload = config::serialize(&response, codec_config()).unwrap();
        assert_eq!(payload, RESPONSE_V2_ERROR_FIXTURE);
        let decoded =
            config::deserialize::<Response, _>(RESPONSE_V2_ERROR_FIXTURE, codec_config()).unwrap();
        assert_eq!(decoded.request_id, 7);
    }

    #[test]
    fn error_code_is_snake_case_in_json() {
        let encoded = serde_json::to_string(&ErrorCode::DaemonNotRunning).unwrap();
        assert_eq!(encoded, "\"daemon_not_running\"");
        let encoded = serde_json::to_string(&ErrorCode::VersionMismatch).unwrap();
        assert_eq!(encoded, "\"version_mismatch\"");
    }

    #[test]
    fn codec_rejects_truncated_payload() {
        let request = Request {
            request_id: 1,
            kind: RequestKind::Get,
        };
        let mut payload = config::serialize(&request, codec_config()).unwrap();
        payload.pop();
        let decoded = config::deserialize::<Request, _>(&payload, codec_config());
        assert!(decoded.is_err());
    }
}
