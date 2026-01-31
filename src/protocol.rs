use serde::{Deserialize, Serialize};

pub const MAGIC: [u8; 4] = *b"SCB1";
pub const VERSION: u16 = 2;
pub const CONTENT_TYPE_TEXT: &str = "text/plain; charset=utf-8";
pub const CONTENT_TYPE_PNG: &str = "image/png";
pub const DEFAULT_MAX_SIZE: usize = 10 * 1024 * 1024;
pub const RESPONSE_OVERHEAD: usize = 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardValue {
    pub content_type: String,
    pub data: Vec<u8>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub request_id: u64,
    pub kind: RequestKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RequestKind {
    Set { value: ClipboardValue },
    Get,
    PeekMeta,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub request_id: u64,
    pub kind: ResponseKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    InvalidRequest,
    PayloadTooLarge,
    InvalidUtf8,
    Internal,
    DaemonNotRunning,
}

#[cfg(test)]
mod tests {
    use super::*;
    use bincode::config;
    use bincode::serde::{decode_from_slice, encode_to_vec};

    #[test]
    fn request_round_trip_bincode() {
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
        let payload = encode_to_vec(&request, config::standard()).unwrap();
        let (decoded, _) = decode_from_slice::<Request, _>(&payload, config::standard()).unwrap();
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
    fn response_round_trip_bincode() {
        let response = Response {
            request_id: 7,
            kind: ResponseKind::Meta {
                content_type: CONTENT_TYPE_PNG.to_string(),
                size: 999,
                created_at: 456,
            },
        };
        let payload = encode_to_vec(&response, config::standard()).unwrap();
        let (decoded, _) = decode_from_slice::<Response, _>(&payload, config::standard()).unwrap();
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
    fn error_code_is_snake_case_in_json() {
        let encoded = serde_json::to_string(&ErrorCode::DaemonNotRunning).unwrap();
        assert_eq!(encoded, "\"daemon_not_running\"");
    }

    #[test]
    fn bincode_rejects_truncated_payload() {
        let request = Request {
            request_id: 1,
            kind: RequestKind::Get,
        };
        let mut payload = encode_to_vec(&request, config::standard()).unwrap();
        payload.pop();
        let decoded = decode_from_slice::<Request, _>(&payload, config::standard());
        assert!(decoded.is_err());
    }
}
