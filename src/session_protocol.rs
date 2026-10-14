//! New, separately selected shelf endpoint. Never serialize these messages through V2.
use crate::shelf::{self, ShelfLimits};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub mod framing;
pub use framing::WireError;
pub const SESSION_VERSION: u16 = 1;
pub const MAX_LIST_ITEMS: u16 = 16;
pub const MAX_METADATA_BYTES: usize = 65536;
pub const MAX_PAYLOAD_BYTES: usize = 256 * 1024 * 1024;

macro_rules! decimal {
    ($name:ident, $integer:ty) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name($integer);
        impl $name {
            pub fn get(self) -> $integer {
                self.0
            }
        }
        impl From<$integer> for $name {
            fn from(value: $integer) -> Self {
                Self(value)
            }
        }
        impl From<$name> for String {
            fn from(value: $name) -> Self {
                value.0.to_string()
            }
        }
        impl TryFrom<String> for $name {
            type Error = &'static str;
            fn try_from(value: String) -> Result<Self, Self::Error> {
                let number: $integer = value.parse().map_err(|_| "invalid decimal integer")?;
                if number.to_string() != value {
                    return Err("noncanonical decimal integer");
                }
                Ok(Self(number))
            }
        }
    };
}
decimal!(Number, u64);
decimal!(Timestamp, i64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContentType {
    #[serde(rename = "text/plain; charset=utf-8")]
    Text,
    #[serde(rename = "image/png")]
    Png,
}
impl From<ContentType> for shelf::ContentType {
    fn from(value: ContentType) -> Self {
        match value {
            ContentType::Text => Self::Text,
            ContentType::Png => Self::Png,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Origin {
    pub device_id: Uuid,
    pub name: String,
}

impl Origin {
    fn validate(&self) -> Result<(), WireError> {
        if self.device_id.is_nil()
            || self.name.trim().is_empty()
            || self.name.len() > 256
            || self.name.chars().any(char::is_control)
        {
            return Err(WireError::InvalidMetadata);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dimensions {
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Representation {
    pub id: u8,
    pub content_type: ContentType,
    pub encoded_bytes: u32,
    pub dimensions: Option<Dimensions>,
    pub decoded_bytes: Option<u32>,
}
impl Representation {
    fn validate(&self) -> Result<(), WireError> {
        if self.id != 0
            || self.encoded_bytes as usize > MAX_PAYLOAD_BYTES
            || (self.content_type == ContentType::Text
                && (self.dimensions.is_some() || self.decoded_bytes.is_some()))
            || self
                .decoded_bytes
                .is_some_and(|size| size == 0 || size as usize > MAX_PAYLOAD_BYTES)
            || self
                .dimensions
                .is_some_and(|size| size.width == 0 || size.height == 0)
        {
            return Err(WireError::InvalidMetadata);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Item {
    pub id: Uuid,
    pub sequence: Number,
    pub origin: Option<Origin>,
    pub created_at_ms: Option<Timestamp>,
    pub received_at_ms: Timestamp,
    pub expires_at_ms: Timestamp,
    pub representations: Vec<Representation>,
    pub label: String,
}

impl Item {
    fn validate(&self) -> Result<(), WireError> {
        if self.id.is_nil()
            || self.sequence.get() == 0
            || self.representations.len() != 1
            || self.label.is_empty()
            || self.label.len() > 256
            || self.label.chars().any(char::is_control)
        {
            return Err(WireError::InvalidMetadata);
        }
        if let Some(origin) = &self.origin {
            origin.validate()?;
        }
        let representation = &self.representations[0];
        if representation.content_type == ContentType::Png {
            let dimensions = representation
                .dimensions
                .ok_or(WireError::InvalidMetadata)?;
            let decoded = representation
                .decoded_bytes
                .ok_or(WireError::InvalidMetadata)?;
            let rgba = u64::from(dimensions.width)
                .checked_mul(u64::from(dimensions.height))
                .and_then(|bytes| bytes.checked_mul(4))
                .ok_or(WireError::InvalidMetadata)?;
            if rgba > u64::from(decoded) {
                return Err(WireError::InvalidMetadata);
            }
        }
        self.representations[0].validate()
    }
}

impl TryFrom<&shelf::ItemMetadata> for Item {
    type Error = WireError;
    fn try_from(item: &shelf::ItemMetadata) -> Result<Self, Self::Error> {
        let item = Self {
            label: item.label.clone(),
            id: item.id,
            sequence: item.sequence.into(),
            origin: item.origin.as_ref().map(|origin| Origin {
                device_id: origin.device_id,
                name: origin.name.clone(),
            }),
            created_at_ms: item.created_at_ms.map(Into::into),
            received_at_ms: item.received_at_ms.into(),
            expires_at_ms: item.expires_at_ms.into(),
            representations: vec![Representation {
                id: 0,
                content_type: match item.content_type {
                    shelf::ContentType::Text => ContentType::Text,
                    shelf::ContentType::Png => ContentType::Png,
                },
                encoded_bytes: item
                    .encoded_bytes
                    .try_into()
                    .map_err(|_| WireError::InvalidMetadata)?,
                dimensions: item
                    .dimensions
                    .map(|(width, height)| Dimensions { width, height }),
                decoded_bytes: item
                    .decoded_bytes
                    .map(u32::try_from)
                    .transpose()
                    .map_err(|_| WireError::InvalidMetadata)?,
            }],
        };
        item.validate()?;
        Ok(item)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    Hello {
        request_id: Number,
        versions: Vec<u16>,
        max_item_bytes: u32,
        max_decoded_bytes: u32,
    },
    Share {
        request_id: Number,
        epoch: Uuid,
        operation_id: Uuid,
        origin: Origin,
        representation: Representation,
        created_at_ms: Option<Timestamp>,
    },
    List {
        request_id: Number,
        epoch: Uuid,
        offset: u16,
        limit: u16,
        expected_sequence: Option<Number>,
    },
    Fetch {
        request_id: Number,
        epoch: Uuid,
        item_id: Uuid,
        representation_id: u8,
    },
    Delete {
        request_id: Number,
        epoch: Uuid,
        item_id: Uuid,
    },
    Clear {
        request_id: Number,
        epoch: Uuid,
    },
    Health {
        request_id: Number,
        epoch: Uuid,
    },
    Watch {
        request_id: Number,
        epoch: Uuid,
        after_sequence: Number,
        wait_ms: u16,
    },
}

impl Request {
    pub fn request_id(&self) -> Number {
        match self {
            Self::Hello { request_id, .. }
            | Self::Share { request_id, .. }
            | Self::List { request_id, .. }
            | Self::Fetch { request_id, .. }
            | Self::Delete { request_id, .. }
            | Self::Clear { request_id, .. }
            | Self::Health { request_id, .. }
            | Self::Watch { request_id, .. } => *request_id,
        }
    }
    pub fn validate(&self, payload_bytes: usize) -> Result<(), WireError> {
        if self.request_id().get() == 0 {
            return Err(WireError::InvalidMetadata);
        }
        let expected = if let Self::Share { representation, .. } = self {
            representation.encoded_bytes as usize
        } else {
            0
        };
        if expected != payload_bytes {
            return Err(WireError::PayloadMismatch);
        }
        match self {
            Self::Hello {
                versions,
                max_item_bytes,
                max_decoded_bytes,
                ..
            } => {
                if versions.is_empty()
                    || versions.len() > 8
                    || versions
                        .iter()
                        .enumerate()
                        .any(|(index, version)| versions[..index].contains(version))
                    || *max_item_bytes == 0
                    || *max_decoded_bytes == 0
                {
                    return Err(WireError::InvalidMetadata);
                }
                return Ok(());
            }
            Self::Share {
                operation_id,
                origin,
                representation,
                ..
            } => {
                if operation_id.is_nil() {
                    return Err(WireError::InvalidMetadata);
                }
                origin.validate()?;
                representation.validate()?;
            }
            Self::List { limit, offset, .. }
                if *limit == 0 || *limit > MAX_LIST_ITEMS || *offset > 1024 =>
            {
                return Err(WireError::InvalidMetadata);
            }
            Self::Fetch {
                item_id,
                representation_id,
                ..
            } if item_id.is_nil() || *representation_id != 0 => {
                return Err(WireError::InvalidMetadata);
            }
            Self::Delete { item_id, .. } if item_id.is_nil() => {
                return Err(WireError::InvalidMetadata);
            }
            Self::Watch { wait_ms, .. } if *wait_ms > 10_000 => {
                return Err(WireError::InvalidMetadata);
            }
            _ => {}
        }
        let epoch = match self {
            Self::Hello { .. } => unreachable!(),
            Self::Share { epoch, .. }
            | Self::List { epoch, .. }
            | Self::Fetch { epoch, .. }
            | Self::Delete { epoch, .. }
            | Self::Clear { epoch, .. }
            | Self::Health { epoch, .. }
            | Self::Watch { epoch, .. } => epoch,
        };
        if epoch.is_nil() {
            return Err(WireError::InvalidMetadata);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectiveLimits {
    pub max_item_bytes: u32,
    pub max_decoded_bytes: u32,
    pub max_metadata_bytes: u32,
    pub max_items: u16,
    pub retention_ms: u32,
    pub max_stored_bytes: u32,
    pub max_resident_bytes: u32,
    pub max_retry_records: u16,
    pub retry_retention_ms: u32,
    pub max_list_items: u16,
}

impl EffectiveLimits {
    pub fn validate(&self) -> Result<(), WireError> {
        if self.max_metadata_bytes as usize != MAX_METADATA_BYTES
            || self.max_list_items != MAX_LIST_ITEMS
        {
            return Err(WireError::InvalidMetadata);
        }
        ShelfLimits {
            max_items: self.max_items.into(),
            retention: std::time::Duration::from_millis(self.retention_ms.into()),
            max_item_bytes: self.max_item_bytes as usize,
            max_decoded_bytes: self.max_decoded_bytes as usize,
            max_stored_bytes: self.max_stored_bytes as usize,
            max_resident_bytes: self.max_resident_bytes as usize,
            max_retry_records: self.max_retry_records.into(),
            retry_retention: std::time::Duration::from_millis(self.retry_retention_ms.into()),
        }
        .validate()
        .map_err(|_| WireError::InvalidMetadata)
    }

    pub fn frame_limits(&self) -> Result<framing::FrameLimits, WireError> {
        self.validate()?;
        framing::FrameLimits::session(
            self.max_item_bytes as usize,
            self.max_decoded_bytes as usize,
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    InvalidRequest,
    UnsupportedVersion,
    InvalidState,
    PayloadTooLarge,
    InvalidContent,
    Busy,
    RetryWindowFull,
    OperationConflict,
    EpochChanged,
    NotFound,
    ResyncRequired,
    DaemonUnavailable,
    Internal,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Response {
    Welcome {
        request_id: Number,
        session_version: u16,
        server_version: String,
        epoch: Uuid,
        sequence: Number,
        capabilities: Vec<String>,
        limits: EffectiveLimits,
    },
    Shared {
        request_id: Number,
        epoch: Uuid,
        sequence: Number,
        item: Item,
        duplicate: bool,
        available: bool,
    },
    Listed {
        request_id: Number,
        epoch: Uuid,
        sequence: Number,
        items: Vec<Item>,
        offset: u16,
        total: u16,
        next_offset: Option<u16>,
    },
    Value {
        request_id: Number,
        epoch: Uuid,
        sequence: Number,
        item: Item,
        representation_id: u8,
    },
    Deleted {
        request_id: Number,
        epoch: Uuid,
        sequence: Number,
        removed: bool,
    },
    Cleared {
        request_id: Number,
        epoch: Uuid,
        sequence: Number,
    },
    Health {
        request_id: Number,
        epoch: Uuid,
        sequence: Number,
        items: u16,
        stored_bytes: u32,
    },
    Changed {
        request_id: Number,
        epoch: Uuid,
        sequence: Number,
    },
    Error {
        request_id: Number,
        code: ErrorCode,
        epoch: Option<Uuid>,
    },
}

impl Response {
    pub fn request_id(&self) -> Number {
        match self {
            Self::Welcome { request_id, .. }
            | Self::Shared { request_id, .. }
            | Self::Listed { request_id, .. }
            | Self::Value { request_id, .. }
            | Self::Deleted { request_id, .. }
            | Self::Cleared { request_id, .. }
            | Self::Health { request_id, .. }
            | Self::Changed { request_id, .. }
            | Self::Error { request_id, .. } => *request_id,
        }
    }

    pub fn validate(&self, payload_bytes: usize) -> Result<(), WireError> {
        let expected = if let Self::Value {
            item,
            representation_id,
            ..
        } = self
        {
            item.validate()?;
            item.representations
                .iter()
                .find(|representation| representation.id == *representation_id)
                .ok_or(WireError::InvalidMetadata)?
                .encoded_bytes as usize
        } else {
            0
        };
        if expected != payload_bytes {
            return Err(WireError::PayloadMismatch);
        }
        if self.request_id().get() == 0 && !matches!(self, Self::Error { .. }) {
            return Err(WireError::InvalidMetadata);
        }
        let epoch = match self {
            Self::Welcome { epoch, .. }
            | Self::Shared { epoch, .. }
            | Self::Listed { epoch, .. }
            | Self::Value { epoch, .. }
            | Self::Deleted { epoch, .. }
            | Self::Cleared { epoch, .. }
            | Self::Health { epoch, .. }
            | Self::Changed { epoch, .. } => Some(*epoch),
            Self::Error { epoch, .. } => *epoch,
        };
        if epoch.is_some_and(|epoch| epoch.is_nil()) {
            return Err(WireError::InvalidMetadata);
        }
        match self {
            Self::Welcome {
                session_version,
                server_version,
                limits,
                ..
            } => {
                if *session_version != SESSION_VERSION
                    || server_version.is_empty()
                    || server_version.len() > 128
                    || server_version.chars().any(char::is_control)
                {
                    return Err(WireError::InvalidMetadata);
                }
                limits.validate()?;
            }
            Self::Shared {
                item,
                duplicate,
                available,
                sequence,
                ..
            } => {
                item.validate()?;
                if (!duplicate && !available) || item.sequence.get() > sequence.get() {
                    return Err(WireError::InvalidMetadata);
                }
            }
            Self::Listed {
                items,
                offset,
                total,
                next_offset,
                sequence,
                ..
            } => {
                if items.len() > MAX_LIST_ITEMS as usize || *total > 1024 {
                    return Err(WireError::InvalidMetadata);
                }
                let end = usize::from(*offset) + items.len();
                if end > usize::from(*total)
                    || next_offset.map(usize::from) != (end < usize::from(*total)).then_some(end)
                {
                    return Err(WireError::InvalidMetadata);
                }
                if items.is_empty() && end < usize::from(*total) {
                    return Err(WireError::InvalidMetadata);
                }
                for (index, item) in items.iter().enumerate() {
                    item.validate()?;
                    if item.sequence.get() > sequence.get()
                        || items[..index].iter().any(|previous| previous.id == item.id)
                        || (index > 0 && items[index - 1].sequence.get() <= item.sequence.get())
                    {
                        return Err(WireError::InvalidMetadata);
                    }
                }
            }
            Self::Value { item, sequence, .. } if item.sequence.get() > sequence.get() => {
                return Err(WireError::InvalidMetadata);
            }
            _ => {}
        }
        Ok(())
    }
}

pub fn negotiate(
    request: &Request,
    epoch: Uuid,
    sequence: u64,
    server: &ShelfLimits,
) -> Result<Response, WireError> {
    request.validate(0)?;
    let Request::Hello {
        request_id,
        versions,
        max_item_bytes,
        max_decoded_bytes,
    } = request
    else {
        return Err(WireError::InvalidState);
    };
    if !versions.contains(&SESSION_VERSION) {
        return Err(WireError::Rejected(ErrorCode::UnsupportedVersion));
    }
    server.validate()?;
    if epoch.is_nil() {
        return Err(WireError::InvalidMetadata);
    }
    Ok(Response::Welcome {
        request_id: *request_id,
        session_version: SESSION_VERSION,
        server_version: env!("CARGO_PKG_VERSION").into(),
        epoch,
        sequence: sequence.into(),
        capabilities: [
            "list",
            "fetch",
            "delete",
            "clear",
            "deduplicated_share",
            "watch",
        ]
        .map(String::from)
        .to_vec(),
        limits: EffectiveLimits {
            max_item_bytes: (*max_item_bytes).min(server.max_item_bytes as u32),
            max_decoded_bytes: (*max_decoded_bytes).min(server.max_decoded_bytes as u32),
            max_metadata_bytes: MAX_METADATA_BYTES as u32,
            max_items: server.max_items as u16,
            retention_ms: server.retention.as_millis() as u32,
            max_stored_bytes: server.max_stored_bytes as u32,
            max_resident_bytes: server.max_resident_bytes as u32,
            max_retry_records: server.max_retry_records as u16,
            retry_retention_ms: server.retry_retention.as_millis() as u32,
            max_list_items: MAX_LIST_ITEMS,
        },
    })
}

#[cfg(test)]
mod tests;
