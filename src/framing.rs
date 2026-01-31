use crate::protocol::{MAGIC, VERSION};
use bincode::config;
use bincode::serde::{decode_from_slice, encode_to_vec};
use eyre::Result;
use serde::{de::DeserializeOwned, Serialize};
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

#[derive(Debug, Error)]
pub enum FramingError {
    #[error("invalid magic")]
    InvalidMagic,
    #[error("unsupported version {0}")]
    UnsupportedVersion(u16),
    #[error("payload too large: {0} bytes")]
    PayloadTooLarge(u32),
}

pub async fn read_frame_payload<R: AsyncRead + Unpin>(
    reader: &mut R,
    max_size: usize,
) -> Result<Vec<u8>> {
    let mut magic = [0u8; 4];
    reader.read_exact(&mut magic).await?;
    if magic != MAGIC {
        return Err(FramingError::InvalidMagic.into());
    }

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
    Ok(payload)
}

pub async fn write_frame_payload<W: AsyncWrite + Unpin>(
    writer: &mut W,
    payload: &[u8],
) -> Result<()> {
    writer.write_all(&MAGIC).await?;
    writer.write_all(&VERSION.to_le_bytes()).await?;
    writer.write_all(&(payload.len() as u32).to_le_bytes()).await?;
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
    use crate::protocol::{Request, Response};
    use tokio::io::duplex;

    #[tokio::test]
    async fn round_trip_frame() {
        let request = Request::Get;
        let payload = encode_message(&request).unwrap();
        let (mut a, mut b) = duplex(1024);

        write_frame_payload(&mut a, &payload).await.unwrap();
        let received = read_frame_payload(&mut b, 1024).await.unwrap();
        let decoded: Request = decode_message(&received).unwrap();

        assert!(matches!(decoded, Request::Get));
    }

    #[tokio::test]
    async fn rejects_oversized_payload() {
        let response = Response::Ok;
        let payload = encode_message(&response).unwrap();
        let (mut a, mut b) = duplex(1024);

        write_frame_payload(&mut a, &payload).await.unwrap();
        let result = read_frame_payload(&mut b, 0).await;
        assert!(result.is_err());
    }
}
