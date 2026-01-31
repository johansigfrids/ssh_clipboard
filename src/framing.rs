use crate::protocol::{MAGIC, VERSION};
use bincode::config;
use bincode::serde::{decode_from_slice, encode_to_vec};
use eyre::Result;
use serde::{Serialize, de::DeserializeOwned};
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

#[derive(Debug, Error)]
pub enum FramingError {
    #[error("invalid magic")]
    InvalidMagic,
    #[error("magic not found within scan limit")]
    MagicNotFound,
    #[error("unsupported version {0}")]
    UnsupportedVersion(u16),
    #[error("payload too large: {0} bytes")]
    PayloadTooLarge(u32),
}

pub async fn read_frame_payload<R: AsyncRead + Unpin>(
    reader: &mut R,
    max_size: usize,
) -> Result<Vec<u8>> {
    Ok(read_frame_payload_inner(reader, max_size, false, 0)
        .await?
        .payload)
}

pub struct FrameReadResult {
    pub payload: Vec<u8>,
    pub discarded_bytes: usize,
}

pub async fn read_frame_payload_resync<R: AsyncRead + Unpin>(
    reader: &mut R,
    max_size: usize,
    max_scan_bytes: usize,
) -> Result<FrameReadResult> {
    read_frame_payload_inner(reader, max_size, true, max_scan_bytes).await
}

async fn read_frame_payload_inner<R: AsyncRead + Unpin>(
    reader: &mut R,
    max_size: usize,
    resync: bool,
    max_scan_bytes: usize,
) -> Result<FrameReadResult> {
    let discarded = read_magic(reader, resync, max_scan_bytes).await?;

    let mut version_bytes = [0u8; 2];
    reader.read_exact(&mut version_bytes).await?;
    let version = u16::from_le_bytes(version_bytes);
    if version != VERSION {
        return Err(FramingError::UnsupportedVersion(version).into());
    }

    let mut len_bytes = [0u8; 4];
    reader.read_exact(&mut len_bytes).await?;
    let len = u32::from_le_bytes(len_bytes);
    if len as usize > max_size {
        return Err(FramingError::PayloadTooLarge(len).into());
    }

    let mut payload = vec![0u8; len as usize];
    reader.read_exact(&mut payload).await?;
    Ok(FrameReadResult {
        payload,
        discarded_bytes: discarded,
    })
}

async fn read_magic<R: AsyncRead + Unpin>(
    reader: &mut R,
    resync: bool,
    max_scan_bytes: usize,
) -> Result<usize> {
    let mut window = [0u8; 4];
    reader.read_exact(&mut window).await?;
    if window == MAGIC {
        return Ok(0);
    }
    if !resync {
        return Err(FramingError::InvalidMagic.into());
    }

    let mut total_read = 4usize;
    loop {
        let mut byte = [0u8; 1];
        reader.read_exact(&mut byte).await?;
        total_read += 1;
        window[0] = window[1];
        window[1] = window[2];
        window[2] = window[3];
        window[3] = byte[0];

        if window == MAGIC {
            return Ok(total_read.saturating_sub(4));
        }
        if total_read.saturating_sub(4) > max_scan_bytes {
            return Err(FramingError::MagicNotFound.into());
        }
    }
}

pub async fn write_frame_payload<W: AsyncWrite + Unpin>(
    writer: &mut W,
    payload: &[u8],
) -> Result<()> {
    writer.write_all(&MAGIC).await?;
    writer.write_all(&VERSION.to_le_bytes()).await?;
    writer
        .write_all(&(payload.len() as u32).to_le_bytes())
        .await?;
    writer.write_all(payload).await?;
    writer.flush().await?;
    Ok(())
}

pub fn encode_message<T: Serialize>(message: &T) -> Result<Vec<u8>> {
    let config = config::standard();
    Ok(encode_to_vec(message, config)?)
}

pub fn decode_message<T: DeserializeOwned>(payload: &[u8]) -> Result<T> {
    let config = config::standard();
    let (value, _) = decode_from_slice(payload, config)?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{Request, RequestKind, Response};
    use proptest::prelude::*;
    use tokio::io::AsyncWriteExt;
    use tokio::io::duplex;

    #[tokio::test]
    async fn round_trip_frame() {
        let request = Request {
            request_id: 1,
            kind: RequestKind::Get,
        };
        let payload = encode_message(&request).unwrap();
        let (mut a, mut b) = duplex(1024);

        write_frame_payload(&mut a, &payload).await.unwrap();
        let received = read_frame_payload(&mut b, 1024).await.unwrap();
        let decoded: Request = decode_message(&received).unwrap();

        assert!(matches!(decoded.kind, RequestKind::Get));
    }

    #[tokio::test]
    async fn rejects_oversized_payload() {
        let response = Response {
            request_id: 1,
            kind: crate::protocol::ResponseKind::Ok,
        };
        let payload = encode_message(&response).unwrap();
        let (mut a, mut b) = duplex(1024);

        write_frame_payload(&mut a, &payload).await.unwrap();
        let result = read_frame_payload(&mut b, 0).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn rejects_invalid_magic() {
        let (mut writer, mut reader) = duplex(64);
        writer.write_all(b"BAD!").await.unwrap();
        writer.write_all(&VERSION.to_le_bytes()).await.unwrap();
        writer.write_all(&0u32.to_le_bytes()).await.unwrap();
        writer.flush().await.unwrap();

        let err = read_frame_payload(&mut reader, 16).await.unwrap_err();
        assert!(matches!(
            err.downcast_ref::<FramingError>(),
            Some(FramingError::InvalidMagic)
        ));
    }

    #[tokio::test]
    async fn rejects_unsupported_version() {
        let (mut writer, mut reader) = duplex(64);
        writer.write_all(&MAGIC).await.unwrap();
        writer
            .write_all(&(VERSION + 1).to_le_bytes())
            .await
            .unwrap();
        writer.write_all(&0u32.to_le_bytes()).await.unwrap();
        writer.flush().await.unwrap();

        let err = read_frame_payload(&mut reader, 16).await.unwrap_err();
        assert!(matches!(
            err.downcast_ref::<FramingError>(),
            Some(FramingError::UnsupportedVersion(_))
        ));
    }

    #[tokio::test]
    async fn resync_skips_garbage_prefix() {
        let request = Request {
            request_id: 9,
            kind: RequestKind::Get,
        };
        let payload = encode_message(&request).unwrap();
        let (mut writer, mut reader) = duplex(2048);

        writer.write_all(b"garbage!").await.unwrap();
        write_frame_payload(&mut writer, &payload).await.unwrap();

        let result = read_frame_payload_resync(&mut reader, 1024, 64)
            .await
            .unwrap();
        let decoded: Request = decode_message(&result.payload).unwrap();
        assert!(matches!(decoded.kind, RequestKind::Get));
        assert!(result.discarded_bytes >= 8);
    }

    #[tokio::test]
    async fn resync_fails_when_strict() {
        let request = Request {
            request_id: 9,
            kind: RequestKind::Get,
        };
        let payload = encode_message(&request).unwrap();
        let (mut writer, mut reader) = duplex(2048);

        writer.write_all(b"noise").await.unwrap();
        write_frame_payload(&mut writer, &payload).await.unwrap();

        let err = read_frame_payload(&mut reader, 1024).await.unwrap_err();
        assert!(matches!(
            err.downcast_ref::<FramingError>(),
            Some(FramingError::InvalidMagic)
        ));
    }

    proptest! {
        #[test]
        fn frame_round_trip_random(payload in proptest::collection::vec(any::<u8>(), 0..512)) {
            let runtime = tokio::runtime::Runtime::new().unwrap();
            runtime.block_on(async {
                let (mut writer, mut reader) = duplex(4096);
                write_frame_payload(&mut writer, &payload).await.unwrap();
                let received = read_frame_payload(&mut reader, 4096).await.unwrap();
                prop_assert_eq!(received, payload);
                Ok(())
            })?;
        }

        #[test]
        fn frame_rejects_payload_over_max(payload in proptest::collection::vec(any::<u8>(), 1..512)) {
            let runtime = tokio::runtime::Runtime::new().unwrap();
            runtime.block_on(async {
                let max_size = payload.len() - 1;
                let (mut writer, mut reader) = duplex(4096);
                write_frame_payload(&mut writer, &payload).await.unwrap();
                let err = read_frame_payload(&mut reader, max_size).await.unwrap_err();
                prop_assert!(matches!(
                    err.downcast_ref::<FramingError>(),
                    Some(FramingError::PayloadTooLarge(_))
                ));
                Ok(())
            })?;
        }
    }
}
