//! Frozen V2 shapes, borrowed decoding, and budgeted response serialization.
use super::*;
use crate::framing::{self as frame, codec_config};
use crate::protocol::{self as v2, ErrorCode as LegacyCode};
use wincode::{SchemaRead, SchemaWrite};

#[derive(SchemaRead, SchemaWrite)]
struct Value<'a> {
    content_type: &'a str,
    data: &'a [u8],
    created_at: i64,
}
#[derive(SchemaRead, SchemaWrite)]
struct LegacyRequest<'a> {
    request_id: u64,
    kind: LegacyRequestKind<'a>,
}
#[derive(SchemaRead, SchemaWrite)]
enum LegacyRequestKind<'a> {
    Set { value: Value<'a> },
    Get,
    PeekMeta,
}
#[derive(SchemaWrite)]
struct LegacyResponse<'a> {
    request_id: u64,
    kind: LegacyResponseKind<'a>,
}
#[derive(SchemaWrite)]
enum LegacyResponseKind<'a> {
    Ok,
    Value {
        value: Value<'a>,
    },
    Meta {
        content_type: &'a str,
        size: u64,
        created_at: i64,
    },
    Empty,
    Error {
        code: LegacyCode,
        message: &'a str,
    },
}

impl ShelfService {
    /// One legacy request, sharing admission/storage with negotiated sessions.
    pub async fn serve_legacy(
        &self,
        mut stream: impl AsyncRead + AsyncWrite + Unpin,
        max_frame: usize,
    ) -> Result<(), WireError> {
        if max_frame == 0 || max_frame > wire::MAX_PAYLOAD_BYTES + v2::RESPONSE_OVERHEAD {
            return Err(ShelfError::InvalidLimits.into());
        }
        let _slot = self.slots.try_acquire().map_err(|_| ShelfError::Busy)?;
        timeout(
            self.limits.operation_timeout,
            self.legacy_inner(&mut stream, max_frame),
        )
        .await
        .map_err(|_| timed_out())?
    }

    async fn legacy_inner(
        &self,
        stream: &mut (impl AsyncRead + AsyncWrite + Unpin),
        max_frame: usize,
    ) -> Result<(), WireError> {
        let length = match frame::read_frame_length(stream, max_frame).await {
            Ok(length) => length,
            Err(error) => {
                if error
                    .downcast_ref::<std::io::Error>()
                    .is_some_and(|error| error.kind() == std::io::ErrorKind::UnexpectedEof)
                {
                    return Ok(());
                }
                let code = match error.downcast_ref::<frame::FramingError>() {
                    Some(frame::FramingError::PayloadTooLarge(_)) => LegacyCode::PayloadTooLarge,
                    Some(frame::FramingError::UnsupportedVersion(_)) => LegacyCode::VersionMismatch,
                    _ => LegacyCode::InvalidRequest,
                };
                return self
                    .legacy_error(stream, 0, code, "Invalid legacy frame.")
                    .await;
            }
        };
        let legacy_budget = self.budget.restricted(usize::MAX, 64 * 1024 * 1024)?;
        let mut input = match legacy_budget.allocate_framed(length, v2::RESPONSE_OVERHEAD) {
            Ok(input) => input,
            Err(failure) => {
                return self
                    .legacy_error(
                        stream,
                        0,
                        legacy_code(&failure),
                        "Shelf resource limit reached.",
                    )
                    .await;
            }
        };
        tokio::io::AsyncReadExt::read_exact(stream, input.bytes_mut()).await?;
        enum Action {
            Set {
                kind: crate::shelf::ContentType,
                created: i64,
                range: std::ops::Range<usize>,
            },
            Get,
            Meta,
        }
        let (request_id, action) = {
            let request: LegacyRequest<'_> =
                match wincode::config::deserialize(input.bytes(), codec_config()) {
                    Ok(request) => request,
                    Err(_) => {
                        return self
                            .legacy_error(
                                stream,
                                0,
                                LegacyCode::InvalidRequest,
                                "Invalid legacy request.",
                            )
                            .await;
                    }
                };
            if wincode::config::serialized_size(&request, codec_config())
                .map_err(|_| WireError::InvalidMetadata)?
                != length as u64
            {
                return self
                    .legacy_error(
                        stream,
                        request.request_id,
                        LegacyCode::InvalidRequest,
                        "Trailing legacy request bytes.",
                    )
                    .await;
            }
            let action = match request.kind {
                LegacyRequestKind::Set { value } => {
                    let kind = match value.content_type {
                        v2::CONTENT_TYPE_TEXT => crate::shelf::ContentType::Text,
                        v2::CONTENT_TYPE_PNG => crate::shelf::ContentType::Png,
                        _ => {
                            return self
                                .legacy_error(
                                    stream,
                                    request.request_id,
                                    LegacyCode::InvalidRequest,
                                    "Unsupported clipboard format.",
                                )
                                .await;
                        }
                    };
                    let start = if value.data.is_empty() {
                        0
                    } else {
                        (value.data.as_ptr() as usize)
                            .checked_sub(input.bytes().as_ptr() as usize)
                            .ok_or(WireError::InvalidMetadata)?
                    };
                    let end = start
                        .checked_add(value.data.len())
                        .ok_or(WireError::InvalidMetadata)?;
                    Action::Set {
                        kind,
                        created: value.created_at,
                        range: start..end,
                    }
                }
                LegacyRequestKind::Get => Action::Get,
                LegacyRequestKind::PeekMeta => Action::Meta,
            };
            (request.request_id, action)
        };
        match action {
            Action::Set {
                kind,
                created,
                range,
            } => {
                let prepared = tokio::task::spawn_blocking(move || {
                    input
                        .retain_payload(range)?
                        .validate(kind, None, Some(created), None)
                })
                .await
                .map_err(|_| WireError::Rejected(ErrorCode::Internal))?;
                let prepared = match prepared {
                    Ok(prepared) => prepared,
                    Err(failure) => {
                        return self
                            .legacy_error(
                                stream,
                                request_id,
                                legacy_code(&failure),
                                "Invalid or oversized clipboard item.",
                            )
                            .await;
                    }
                };
                let result =
                    self.with_shelf(|shelf, now| Ok(shelf.commit(shelf.epoch(), prepared, now)))?;
                if let Err(failure) = result {
                    return self
                        .legacy_error(
                            stream,
                            request_id,
                            legacy_code(&failure),
                            "Shelf could not commit this item.",
                        )
                        .await;
                }
                self.legacy_control(
                    stream,
                    LegacyResponse {
                        request_id,
                        kind: LegacyResponseKind::Ok,
                    },
                )
                .await
            }
            Action::Get | Action::Meta => {
                drop(input);
                let Some(item) = self.with_shelf(|shelf, now| Ok(shelf.latest(now)?))? else {
                    return self
                        .legacy_control(
                            stream,
                            LegacyResponse {
                                request_id,
                                kind: LegacyResponseKind::Empty,
                            },
                        )
                        .await;
                };
                let mime = match item.metadata.content_type {
                    crate::shelf::ContentType::Text => v2::CONTENT_TYPE_TEXT,
                    crate::shelf::ContentType::Png => v2::CONTENT_TYPE_PNG,
                };
                let created_at = item
                    .metadata
                    .created_at_ms
                    .unwrap_or(item.metadata.received_at_ms);
                if matches!(action, Action::Meta) {
                    return self
                        .legacy_control(
                            stream,
                            LegacyResponse {
                                request_id,
                                kind: LegacyResponseKind::Meta {
                                    content_type: mime,
                                    size: item.metadata.encoded_bytes as u64,
                                    created_at,
                                },
                            },
                        )
                        .await;
                }
                if item
                    .metadata
                    .decoded_bytes
                    .is_some_and(|bytes| bytes > 64 * 1024 * 1024)
                {
                    return self
                        .legacy_error(
                            stream,
                            request_id,
                            LegacyCode::PayloadTooLarge,
                            "Item exceeds the legacy decoded-image limit.",
                        )
                        .await;
                }
                let response = LegacyResponse {
                    request_id,
                    kind: LegacyResponseKind::Value {
                        value: Value {
                            content_type: mime,
                            data: item.bytes(),
                            created_at,
                        },
                    },
                };
                let size = usize::try_from(
                    wincode::config::serialized_size(&response, codec_config())
                        .map_err(|_| WireError::InvalidMetadata)?,
                )
                .map_err(|_| WireError::InvalidMetadata)?;
                if size > max_frame + v2::RESPONSE_OVERHEAD {
                    return self
                        .legacy_error(
                            stream,
                            request_id,
                            LegacyCode::PayloadTooLarge,
                            "Item exceeds the legacy transfer limit.",
                        )
                        .await;
                }
                let mut encoded = match self.budget.allocate_framed(size, v2::RESPONSE_OVERHEAD) {
                    Ok(buffer) => buffer,
                    Err(failure) => {
                        return self
                            .legacy_error(
                                stream,
                                request_id,
                                legacy_code(&failure),
                                "Shelf resource limit reached.",
                            )
                            .await;
                    }
                };
                wincode::config::serialize_into(encoded.bytes_mut(), &response, codec_config())
                    .map_err(|_| WireError::InvalidMetadata)?;
                frame::write_frame_payload(stream, encoded.bytes())
                    .await
                    .map_err(|_| WireError::InvalidState)
            }
        }
    }

    async fn legacy_error(
        &self,
        stream: &mut (impl AsyncWrite + Unpin),
        request_id: u64,
        code: LegacyCode,
        message: &'static str,
    ) -> Result<(), WireError> {
        self.legacy_control(
            stream,
            LegacyResponse {
                request_id,
                kind: LegacyResponseKind::Error { code, message },
            },
        )
        .await
    }
    async fn legacy_control(
        &self,
        stream: &mut (impl AsyncWrite + Unpin),
        response: LegacyResponse<'_>,
    ) -> Result<(), WireError> {
        if wincode::config::serialized_size(&response, codec_config())
            .map_err(|_| WireError::InvalidMetadata)?
            > v2::RESPONSE_OVERHEAD as u64
        {
            return Err(WireError::InvalidMetadata);
        }
        let payload = frame::encode_message(&response).map_err(|_| WireError::InvalidMetadata)?;
        frame::write_frame_payload(stream, &payload)
            .await
            .map_err(|_| WireError::InvalidState)
    }
}

fn legacy_code(error: &ShelfError) -> LegacyCode {
    match error {
        ShelfError::InvalidText => LegacyCode::InvalidUtf8,
        ShelfError::ItemTooLarge | ShelfError::InvalidImage => LegacyCode::PayloadTooLarge,
        ShelfError::InvalidOrigin | ShelfError::InvalidOperation | ShelfError::ForeignUpload => {
            LegacyCode::InvalidRequest
        }
        _ => LegacyCode::Internal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::io::AsyncWriteExt;

    #[tokio::test]
    async fn malformed_and_incomplete_legacy_frames_release_memory_without_committing() {
        let service = Arc::new(
            ShelfService::new(
                ShelfLimits::default(),
                ServiceLimits {
                    operation_timeout: Duration::from_millis(100),
                    ..Default::default()
                },
            )
            .unwrap(),
        );
        for payload in [vec![42, 255], {
            let mut bytes = frame::encode_message(&v2::Request {
                request_id: 42,
                kind: v2::RequestKind::Get,
            })
            .unwrap();
            bytes.push(99);
            bytes
        }] {
            let (mut client, server) = tokio::io::duplex(4096);
            let shared = service.clone();
            let task = tokio::spawn(async move { shared.serve_legacy(server, 4096).await });
            frame::write_frame_payload(&mut client, &payload)
                .await
                .unwrap();
            let reply: v2::Response =
                frame::decode_message(&frame::read_frame_payload(&mut client, 4096).await.unwrap())
                    .unwrap();
            assert!(matches!(
                reply.kind,
                v2::ResponseKind::Error {
                    code: LegacyCode::InvalidRequest,
                    ..
                }
            ));
            task.await.unwrap().unwrap();
            assert_eq!(service.resident_bytes(), 0);
        }
        let (mut client, server) = tokio::io::duplex(4096);
        let shared = service.clone();
        let task = tokio::spawn(async move { shared.serve_legacy(server, 4096).await });
        client.write_all(&v2::MAGIC).await.unwrap();
        client.write_all(&v2::VERSION.to_le_bytes()).await.unwrap();
        client.write_all(&2048u32.to_le_bytes()).await.unwrap();
        assert!(
            timeout(Duration::from_secs(2), task)
                .await
                .unwrap()
                .unwrap()
                .is_err()
        );
        assert_eq!(service.resident_bytes(), 0);
        assert!(
            service
                .with_shelf(|shelf, now| Ok(shelf.latest(now)?))
                .unwrap()
                .is_none()
        );
    }
    #[test]
    fn borrowed_legacy_shapes_preserve_v2_bytes_and_point_inside_the_original_frame() {
        let request = v2::Request {
            request_id: u64::MAX,
            kind: v2::RequestKind::Set {
                value: v2::ClipboardValue {
                    content_type: v2::CONTENT_TYPE_TEXT.into(),
                    data: b" utf8 \n".to_vec(),
                    created_at: i64::MIN,
                },
            },
        };
        let bytes = frame::encode_message(&request).unwrap();
        let borrowed: LegacyRequest<'_> =
            wincode::config::deserialize(&bytes, codec_config()).unwrap();
        assert_eq!(frame::encode_message(&borrowed).unwrap(), bytes);
        let LegacyRequestKind::Set { value } = borrowed.kind else {
            panic!("not set")
        };
        assert_eq!(value.data, b" utf8 \n");
        assert!(
            value.data.as_ptr() >= bytes.as_ptr()
                && value.data.as_ptr() < bytes.as_ptr().wrapping_add(bytes.len())
        );
        let response = LegacyResponse {
            request_id: 7,
            kind: LegacyResponseKind::Value { value },
        };
        let decoded: v2::Response =
            frame::decode_message(&frame::encode_message(&response).unwrap()).unwrap();
        assert!(
            matches!(decoded.kind, v2::ResponseKind::Value {value} if value.data == b" utf8 \n" && value.created_at == i64::MIN)
        );
    }
}
