use super::{ErrorCode, MAX_METADATA_BYTES, MAX_PAYLOAD_BYTES, Request};
use crate::shelf::{IncomingUpload, ShelfError, UploadBudget};
use serde::{Serialize, de::DeserializeOwned};
use std::io::Write;
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub const MAGIC: [u8; 4] = *b"SCBS";
pub const BOOTSTRAP_VERSION: u16 = 1;
pub const HEADER_BYTES: usize = 16;

#[derive(Debug, Error)]
pub enum WireError {
    #[error("session stream failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid session magic")]
    InvalidMagic,
    #[error("unsupported session bootstrap version")]
    UnsupportedBootstrap,
    #[error("unsupported session flags")]
    ReservedFlags,
    #[error("session metadata exceeds its bound")]
    MetadataTooLarge,
    #[error("session payload exceeds its bound")]
    PayloadTooLarge,
    #[error("invalid session metadata")]
    InvalidMetadata,
    #[error("session payload does not match its metadata")]
    PayloadMismatch,
    #[error("message is invalid for this session state")]
    InvalidState,
    #[error("session negotiation failed: {0:?}")]
    Rejected(ErrorCode),
    #[error(transparent)]
    Resource(#[from] ShelfError),
    #[error("upload rejected: {source}")]
    UploadRejected {
        request_id: super::Number,
        payload_bytes: usize,
        source: ShelfError,
    },
}

#[derive(Clone, Copy)]
pub struct FrameLimits {
    metadata: usize,
    payload: usize,
    decoded: usize,
}
impl FrameLimits {
    pub const fn bootstrap() -> Self {
        Self {
            metadata: 4096,
            payload: 0,
            decoded: 0,
        }
    }
    pub fn session(max_payload: usize, max_decoded: usize) -> Result<Self, WireError> {
        if max_payload == 0 || max_payload > MAX_PAYLOAD_BYTES {
            return Err(WireError::PayloadTooLarge);
        }
        if max_decoded == 0 || max_decoded > MAX_PAYLOAD_BYTES {
            return Err(WireError::InvalidMetadata);
        }
        Ok(Self {
            metadata: MAX_METADATA_BYTES,
            payload: max_payload,
            decoded: max_decoded,
        })
    }
    fn check(&self, metadata: usize, payload: usize) -> Result<(), WireError> {
        if metadata == 0 {
            return Err(WireError::InvalidMetadata);
        }
        if metadata > self.metadata {
            return Err(WireError::MetadataTooLarge);
        }
        if payload > self.payload {
            return Err(WireError::PayloadTooLarge);
        }
        Ok(())
    }
}

pub struct Header {
    metadata: usize,
    payload: usize,
}
impl Header {
    pub fn payload_bytes(&self) -> usize {
        self.payload
    }
    pub fn metadata_bytes(&self) -> usize {
        self.metadata
    }
}

pub async fn read_header(
    reader: &mut (impl AsyncRead + Unpin),
    limits: FrameLimits,
) -> Result<Header, WireError> {
    let mut bytes = [0; HEADER_BYTES];
    reader.read_exact(&mut bytes).await?;
    if bytes[..4] != MAGIC {
        return Err(WireError::InvalidMagic);
    }
    if u16::from_le_bytes([bytes[4], bytes[5]]) != BOOTSTRAP_VERSION {
        return Err(WireError::UnsupportedBootstrap);
    }
    if bytes[6..8] != [0, 0] {
        return Err(WireError::ReservedFlags);
    }
    let metadata = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    let payload = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    limits.check(metadata, payload)?;
    Ok(Header { metadata, payload })
}

pub async fn read_metadata<T: DeserializeOwned>(
    reader: &mut (impl AsyncRead + Unpin),
    header: &Header,
) -> Result<T, WireError> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(header.metadata)
        .map_err(|_| ShelfError::Allocation)?;
    bytes.resize(header.metadata, 0);
    reader.read_exact(&mut bytes).await?;
    serde_json::from_slice(&bytes).map_err(|_| WireError::InvalidMetadata)
}

pub async fn read_hello(reader: &mut (impl AsyncRead + Unpin)) -> Result<Request, WireError> {
    let header = read_header(reader, FrameLimits::bootstrap()).await?;
    let request: Request = read_metadata(reader, &header).await?;
    request.validate(0)?;
    if !matches!(request, Request::Hello { .. }) {
        return Err(WireError::InvalidState);
    }
    Ok(request)
}

pub struct IncomingRequest {
    pub request: Request,
    pub upload: Option<IncomingUpload>,
}

pub async fn read_request(
    reader: &mut (impl AsyncRead + Unpin),
    limits: FrameLimits,
    budget: &UploadBudget,
) -> Result<IncomingRequest, WireError> {
    if limits.payload == 0 {
        return Err(WireError::InvalidState);
    }
    let header = read_header(reader, limits).await?;
    read_request_body(reader, header, limits, budget).await
}

/// The service can apply a separate idle/header deadline before the operation deadline.
pub async fn read_request_body(
    reader: &mut (impl AsyncRead + Unpin),
    header: Header,
    limits: FrameLimits,
    budget: &UploadBudget,
) -> Result<IncomingRequest, WireError> {
    if limits.payload == 0 {
        return Err(WireError::InvalidState);
    }
    limits.check(header.metadata, header.payload)?;
    let request: Request = read_metadata(reader, &header).await?;
    request.validate(header.payload)?;
    if matches!(request, Request::Hello { .. }) {
        return Err(WireError::InvalidState);
    }
    let upload = if matches!(request, Request::Share { .. }) {
        let mut input = budget
            .restricted(limits.payload, limits.decoded)?
            .allocate(header.payload)
            .map_err(|source| WireError::UploadRejected {
                request_id: request.request_id(),
                payload_bytes: header.payload,
                source,
            })?;
        reader.read_exact(input.bytes_mut()).await?;
        Some(input)
    } else {
        None
    };
    Ok(IncomingRequest { request, upload })
}

struct MetadataWriter {
    bytes: Vec<u8>,
    limit: usize,
    exceeded: bool,
}
impl Write for MetadataWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > self.limit - self.bytes.len() {
            self.exceeded = true;
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "session metadata limit",
            ));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub async fn write_message(
    writer: &mut (impl AsyncWrite + Unpin),
    message: &impl Serialize,
    payload: &[u8],
    limits: FrameLimits,
) -> Result<(), WireError> {
    if payload.len() > limits.payload {
        return Err(WireError::PayloadTooLarge);
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(limits.metadata)
        .map_err(|_| ShelfError::Allocation)?;
    let mut metadata = MetadataWriter {
        bytes,
        limit: limits.metadata,
        exceeded: false,
    };
    if serde_json::to_writer(&mut metadata, message).is_err() {
        return Err(if metadata.exceeded {
            WireError::MetadataTooLarge
        } else {
            WireError::InvalidMetadata
        });
    }
    limits.check(metadata.bytes.len(), payload.len())?;
    let mut header = [0; HEADER_BYTES];
    header[..4].copy_from_slice(&MAGIC);
    header[4..6].copy_from_slice(&BOOTSTRAP_VERSION.to_le_bytes());
    header[8..12].copy_from_slice(&(metadata.bytes.len() as u32).to_le_bytes());
    header[12..16].copy_from_slice(&(payload.len() as u32).to_le_bytes());
    writer.write_all(&header).await?;
    writer.write_all(&metadata.bytes).await?;
    writer.write_all(payload).await?;
    writer.flush().await?;
    Ok(())
}
