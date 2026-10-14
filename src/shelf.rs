//! Platform-independent shelf state. Networking must reserve upload bytes before reading
//! payloads and validate outside the shelf lock. No disk, SSH, or clipboard access here.
use image::{ImageDecoder, ImageFormat, ImageReader, Limits};
use sha2::{Digest, Sha256};
use std::collections::VecDeque;
use std::io::Cursor;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use thiserror::Error;
use uuid::Uuid;

const MIB: usize = 1024 * 1024;
const MAX_NAME_BYTES: usize = 256;

#[derive(Clone, Debug)]
pub struct ShelfLimits {
    pub max_items: usize,
    pub retention: Duration,
    pub max_item_bytes: usize,
    pub max_stored_bytes: usize,
    pub max_resident_bytes: usize,
    pub max_decoded_bytes: usize,
    pub max_retry_records: usize,
    pub retry_retention: Duration,
}

impl Default for ShelfLimits {
    fn default() -> Self {
        Self {
            max_items: 20,
            retention: Duration::from_secs(3600),
            max_item_bytes: 64 * MIB,
            max_stored_bytes: 256 * MIB,
            max_resident_bytes: 384 * MIB,
            max_decoded_bytes: 256 * MIB,
            max_retry_records: 4096,
            retry_retention: Duration::from_secs(600),
        }
    }
}

impl ShelfLimits {
    pub fn validate(&self) -> Result<(), ShelfError> {
        if self.max_items == 0
            || self.max_items > 1024
            || self.retention < Duration::from_millis(1)
            || self.retention > Duration::from_secs(86400)
            || self.max_item_bytes == 0
            || self.max_item_bytes > 256 * MIB
            || self.max_stored_bytes < self.max_item_bytes
            || self.max_stored_bytes > 1024 * MIB
            || self.max_resident_bytes < self.max_stored_bytes.saturating_add(self.max_item_bytes)
            || self.max_resident_bytes > 2048 * MIB
            || self.max_decoded_bytes == 0
            || self.max_decoded_bytes > 256 * MIB
            || self.max_retry_records == 0
            || self.max_retry_records > 16384
            || self.retry_retention < Duration::from_millis(1)
            || self.retry_retention > Duration::from_secs(86400)
        {
            return Err(ShelfError::InvalidLimits);
        }
        Ok(())
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ShelfError {
    #[error("invalid shelf limits")]
    InvalidLimits,
    #[error("item exceeds the encoded payload limit")]
    ItemTooLarge,
    #[error("shelf resources are busy; try again after the current operation finishes")]
    Busy,
    #[error("could not allocate the bounded item buffer")]
    Allocation,
    #[error("invalid UTF-8 text")]
    InvalidText,
    #[error("invalid PNG or decoded image exceeds the limit")]
    InvalidImage,
    #[error("device identity or name is invalid")]
    InvalidOrigin,
    #[error("an upload operation requires a nonempty operation and device identity")]
    InvalidOperation,
    #[error("operation ID was already used for different content or metadata")]
    OperationConflict,
    #[error("server restarted; do not automatically repeat an uncertain share")]
    EpochChanged,
    #[error("retry record window is full; existing deduplication guarantees are preserved")]
    RetryWindowFull,
    #[error("sequence or timestamp cannot be represented")]
    Overflow,
    #[error("monotonic clock moved backwards")]
    ClockReversed,
    #[error("upload was prepared for a different shelf")]
    ForeignUpload,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContentType {
    Text,
    Png,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Origin {
    pub device_id: Uuid,
    pub name: String,
}

#[derive(Clone, Copy, Debug)]
pub struct Now {
    pub instant: Instant,
    pub unix_ms: i64,
}

impl Now {
    pub fn system() -> Result<Self, ShelfError> {
        let unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| ShelfError::Overflow)?
            .as_millis();
        Ok(Self {
            instant: Instant::now(),
            unix_ms: i64::try_from(unix_ms).map_err(|_| ShelfError::Overflow)?,
        })
    }
}

#[derive(Clone)]
pub struct UploadBudget {
    limits: ShelfLimits,
    state: Arc<BudgetState>,
}

#[derive(Default)]
struct BudgetState {
    encoded: AtomicUsize,
    validating_image: AtomicBool,
}

struct Charge {
    state: Arc<BudgetState>,
    bytes: usize,
}
impl Drop for Charge {
    fn drop(&mut self) {
        self.state.encoded.fetch_sub(self.bytes, Ordering::SeqCst);
    }
}

struct DecodePermit(Arc<BudgetState>);
impl Drop for DecodePermit {
    fn drop(&mut self) {
        self.0.validating_image.store(false, Ordering::SeqCst);
    }
}

/// The only way to allocate an upload. Its private buffer cannot grow beyond the reservation.
pub struct IncomingUpload {
    data: Vec<u8>,
    charge: Charge,
    limits: ShelfLimits,
}

impl UploadBudget {
    /// Peer negotiation can narrow item/decode limits, never increase global resources.
    pub fn restricted(
        &self,
        max_item_bytes: usize,
        max_decoded_bytes: usize,
    ) -> Result<Self, ShelfError> {
        if max_item_bytes == 0 || max_decoded_bytes == 0 {
            return Err(ShelfError::InvalidLimits);
        }
        let mut limits = self.limits.clone();
        limits.max_item_bytes = limits.max_item_bytes.min(max_item_bytes);
        limits.max_decoded_bytes = limits.max_decoded_bytes.min(max_decoded_bytes);
        Ok(Self {
            limits,
            state: self.state.clone(),
        })
    }

    pub fn allocate(&self, bytes: usize) -> Result<IncomingUpload, ShelfError> {
        if bytes > self.limits.max_item_bytes {
            return Err(ShelfError::ItemTooLarge);
        }
        self.allocate_reserved(bytes)
    }

    pub(crate) fn allocate_framed(
        &self,
        bytes: usize,
        metadata_allowance: usize,
    ) -> Result<IncomingUpload, ShelfError> {
        if metadata_allowance > 65536
            || bytes
                > self
                    .limits
                    .max_item_bytes
                    .saturating_add(metadata_allowance)
        {
            return Err(ShelfError::ItemTooLarge);
        }
        self.allocate_reserved(bytes)
    }

    fn allocate_reserved(&self, bytes: usize) -> Result<IncomingUpload, ShelfError> {
        let mut current = self.state.encoded.load(Ordering::SeqCst);
        loop {
            let next = current
                .checked_add(bytes)
                .filter(|total| *total <= self.limits.max_resident_bytes)
                .ok_or(ShelfError::Busy)?;
            match self.state.encoded.compare_exchange_weak(
                current,
                next,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => break,
                Err(observed) => current = observed,
            }
        }
        let charge = Charge {
            state: self.state.clone(),
            bytes,
        };
        let mut data = Vec::new();
        data.try_reserve_exact(bytes)
            .map_err(|_| ShelfError::Allocation)?;
        data.resize(bytes, 0);
        Ok(IncomingUpload {
            data,
            charge,
            limits: self.limits.clone(),
        })
    }

    pub fn resident_bytes(&self) -> usize {
        self.state.encoded.load(Ordering::SeqCst)
    }
}

impl IncomingUpload {
    pub fn bytes(&self) -> &[u8] {
        &self.data
    }

    /// Borrowed legacy decoding finds the payload inside its reserved frame. Compact
    /// it in place, retaining the original allocation charge; no second payload Vec.
    pub(crate) fn retain_payload(
        mut self,
        range: std::ops::Range<usize>,
    ) -> Result<Self, ShelfError> {
        if range.start > range.end || range.end > self.data.len() {
            return Err(ShelfError::ForeignUpload);
        }
        let length = range.len();
        if length > self.limits.max_item_bytes {
            return Err(ShelfError::ItemTooLarge);
        }
        self.data.copy_within(range, 0);
        self.data.truncate(length);
        Ok(self)
    }
    pub fn bytes_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }

    /// `operation_id=None` is reserved for legacy shares, which cannot deduplicate retries.
    pub fn validate(
        self,
        content_type: ContentType,
        origin: Option<Origin>,
        created_at_ms: Option<i64>,
        operation_id: Option<Uuid>,
    ) -> Result<ValidatedUpload, ShelfError> {
        if self.data.len() > self.limits.max_item_bytes {
            return Err(ShelfError::ItemTooLarge);
        }
        if let Some(origin) = &origin
            && (origin.device_id.is_nil()
                || origin.name.trim().is_empty()
                || origin.name.len() > MAX_NAME_BYTES
                || origin.name.chars().any(char::is_control))
        {
            return Err(ShelfError::InvalidOrigin);
        }
        if let Some(operation) = operation_id
            && (operation.is_nil() || origin.is_none())
        {
            return Err(ShelfError::InvalidOperation);
        }
        let dimensions = match content_type {
            ContentType::Text => {
                std::str::from_utf8(&self.data).map_err(|_| ShelfError::InvalidText)?;
                None
            }
            ContentType::Png => {
                self.charge
                    .state
                    .validating_image
                    .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                    .map_err(|_| ShelfError::Busy)?;
                let _permit = DecodePermit(self.charge.state.clone());
                Some(validate_png(&self.data, self.limits.max_decoded_bytes)?)
            }
        };
        let fingerprint =
            operation_id.map(|_| fingerprint(content_type, &origin, created_at_ms, &self.data));
        Ok(ValidatedUpload {
            blob: Arc::new(Blob {
                data: self.data,
                _charge: self.charge,
            }),
            content_type,
            origin,
            created_at_ms,
            operation_id,
            fingerprint,
            dimensions: dimensions.map(|info| (info.0, info.1)),
            decoded_bytes: dimensions.map(|info| info.2),
        })
    }
}

fn validate_png(data: &[u8], max_decoded_bytes: usize) -> Result<(u32, u32, usize), ShelfError> {
    let mut reader = ImageReader::with_format(Cursor::new(data), ImageFormat::Png);
    let mut limits = Limits::default();
    limits.max_alloc = Some(max_decoded_bytes as u64);
    reader.limits(limits);
    let decoder = reader
        .into_decoder()
        .map_err(|_| ShelfError::InvalidImage)?;
    let (width, height) = decoder.dimensions();
    let rgba = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|n| n.checked_mul(4))
        .ok_or(ShelfError::InvalidImage)?;
    let native = decoder.total_bytes();
    if width == 0
        || height == 0
        || rgba > max_decoded_bytes as u64
        || native > max_decoded_bytes as u64
    {
        return Err(ShelfError::InvalidImage);
    }
    let mut decoded = Vec::new();
    let length = usize::try_from(native).map_err(|_| ShelfError::InvalidImage)?;
    decoded
        .try_reserve_exact(length)
        .map_err(|_| ShelfError::Allocation)?;
    decoded.resize(length, 0);
    decoder
        .read_image(&mut decoded)
        .map_err(|_| ShelfError::InvalidImage)?;
    Ok((
        width,
        height,
        usize::try_from(native.max(rgba)).map_err(|_| ShelfError::InvalidImage)?,
    ))
}

fn fingerprint(
    content_type: ContentType,
    origin: &Option<Origin>,
    created_at: Option<i64>,
    data: &[u8],
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"ssh-clipboard-upload-v1\0");
    hash.update([match content_type {
        ContentType::Text => 0,
        ContentType::Png => 1,
    }]);
    if let Some(origin) = origin {
        hash.update([1]);
        hash.update(origin.device_id.as_bytes());
        hash.update((origin.name.len() as u64).to_le_bytes());
        hash.update(origin.name.as_bytes());
    } else {
        hash.update([0]);
    }
    if let Some(created_at) = created_at {
        hash.update([1]);
        hash.update(created_at.to_le_bytes());
    } else {
        hash.update([0]);
    }
    hash.update((data.len() as u64).to_le_bytes());
    hash.update(data);
    hash.finalize().into()
}

struct Blob {
    data: Vec<u8>,
    _charge: Charge,
}
impl std::fmt::Debug for Blob {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Blob")
            .field("encoded_bytes", &self.data.len())
            .finish()
    }
}

pub struct ValidatedUpload {
    blob: Arc<Blob>,
    content_type: ContentType,
    origin: Option<Origin>,
    created_at_ms: Option<i64>,
    operation_id: Option<Uuid>,
    fingerprint: Option<[u8; 32]>,
    dimensions: Option<(u32, u32)>,
    decoded_bytes: Option<usize>,
}

impl ValidatedUpload {
    pub fn dimensions(&self) -> Option<(u32, u32)> {
        self.dimensions
    }
    pub fn decoded_bytes(&self) -> Option<usize> {
        self.decoded_bytes
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemMetadata {
    pub id: Uuid,
    pub sequence: u64,
    pub origin: Option<Origin>,
    pub content_type: ContentType,
    pub encoded_bytes: usize,
    pub created_at_ms: Option<i64>,
    pub received_at_ms: i64,
    pub expires_at_ms: i64,
    pub dimensions: Option<(u32, u32)>,
    pub decoded_bytes: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct FetchedItem {
    pub metadata: ItemMetadata,
    blob: Arc<Blob>,
}
impl FetchedItem {
    pub fn bytes(&self) -> &[u8] {
        &self.blob.data
    }
}

struct StoredItem {
    item: FetchedItem,
    expires: Instant,
}
struct RetryRecord {
    key: (Uuid, Uuid),
    fingerprint: [u8; 32],
    metadata: ItemMetadata,
    expires: Instant,
}

#[derive(Debug)]
pub struct CommitOutcome {
    pub metadata: ItemMetadata,
    pub duplicate: bool,
    pub available: bool,
}

#[derive(Debug)]
pub struct Snapshot {
    pub epoch: Uuid,
    pub sequence: u64,
    pub items: Vec<ItemMetadata>,
}

pub struct Shelf {
    budget: UploadBudget,
    epoch: Uuid,
    sequence: u64,
    last_now: Instant,
    items: VecDeque<StoredItem>,
    retries: VecDeque<RetryRecord>,
    stored_bytes: usize,
}

impl Shelf {
    pub fn new(limits: ShelfLimits, now: Now) -> Result<Self, ShelfError> {
        limits.validate()?;
        let mut items = VecDeque::new();
        items
            .try_reserve_exact(limits.max_items)
            .map_err(|_| ShelfError::Allocation)?;
        let mut retries = VecDeque::new();
        retries
            .try_reserve_exact(limits.max_retry_records)
            .map_err(|_| ShelfError::Allocation)?;
        Ok(Self {
            budget: UploadBudget {
                limits,
                state: Arc::default(),
            },
            epoch: Uuid::new_v4(),
            sequence: 0,
            last_now: now.instant,
            items,
            retries,
            stored_bytes: 0,
        })
    }

    pub fn epoch(&self) -> Uuid {
        self.epoch
    }
    pub fn sequence(&self) -> u64 {
        self.sequence
    }
    pub fn limits(&self) -> &ShelfLimits {
        &self.budget.limits
    }
    pub fn upload_budget(&self) -> UploadBudget {
        self.budget.clone()
    }
    pub fn stored_bytes(&self) -> usize {
        self.stored_bytes
    }

    /// Batch expiration is one sequence change. Monotonic time controls retention;
    /// client and wall-clock timestamps never determine ordering or expiry decisions.
    pub fn expire(&mut self, now: Now) -> Result<(), ShelfError> {
        if now.instant < self.last_now {
            return Err(ShelfError::ClockReversed);
        }
        if self
            .items
            .front()
            .is_some_and(|item| item.expires <= now.instant)
        {
            let sequence = self.sequence.checked_add(1).ok_or(ShelfError::Overflow)?;
            while self
                .items
                .front()
                .is_some_and(|item| item.expires <= now.instant)
            {
                self.evict_oldest();
            }
            self.sequence = sequence;
        }
        while self
            .retries
            .front()
            .is_some_and(|record| record.expires <= now.instant)
        {
            self.retries.pop_front();
        }
        self.last_now = now.instant;
        Ok(())
    }

    pub fn commit(
        &mut self,
        expected_epoch: Uuid,
        upload: ValidatedUpload,
        now: Now,
    ) -> Result<CommitOutcome, ShelfError> {
        if expected_epoch != self.epoch {
            return Err(ShelfError::EpochChanged);
        }
        if !Arc::ptr_eq(&upload.blob._charge.state, &self.budget.state) {
            return Err(ShelfError::ForeignUpload);
        }
        let expires_at_ms = now
            .unix_ms
            .checked_add(self.limits().retention.as_millis() as i64)
            .ok_or(ShelfError::Overflow)?;
        let expires = now
            .instant
            .checked_add(self.limits().retention)
            .ok_or(ShelfError::Overflow)?;
        let retry_expires = now
            .instant
            .checked_add(self.limits().retry_retention)
            .ok_or(ShelfError::Overflow)?;
        self.expire(now)?;
        let key = upload
            .operation_id
            .zip(upload.origin.as_ref().map(|origin| origin.device_id))
            .map(|(operation, device)| (device, operation));
        if let Some(key) = key {
            if let Some(record) = self.retries.iter().find(|record| record.key == key) {
                if upload.fingerprint != Some(record.fingerprint) {
                    return Err(ShelfError::OperationConflict);
                }
                return Ok(CommitOutcome {
                    metadata: record.metadata.clone(),
                    duplicate: true,
                    available: self
                        .items
                        .iter()
                        .any(|item| item.item.metadata.id == record.metadata.id),
                });
            }
            if self.retries.len() == self.limits().max_retry_records {
                return Err(ShelfError::RetryWindowFull);
            }
        }
        let sequence = self.sequence.checked_add(1).ok_or(ShelfError::Overflow)?;
        let metadata = ItemMetadata {
            id: Uuid::new_v4(),
            sequence,
            origin: upload.origin,
            content_type: upload.content_type,
            encoded_bytes: upload.blob.data.len(),
            created_at_ms: upload.created_at_ms,
            received_at_ms: now.unix_ms,
            expires_at_ms,
            dimensions: upload.dimensions,
            decoded_bytes: upload.decoded_bytes,
        };
        while self.items.len() >= self.limits().max_items
            || self.stored_bytes + metadata.encoded_bytes > self.limits().max_stored_bytes
        {
            self.evict_oldest();
        }
        self.stored_bytes += metadata.encoded_bytes;
        self.items.push_back(StoredItem {
            item: FetchedItem {
                metadata: metadata.clone(),
                blob: upload.blob,
            },
            expires,
        });
        if let Some(key) = key {
            self.retries.push_back(RetryRecord {
                key,
                fingerprint: upload.fingerprint.unwrap(),
                metadata: metadata.clone(),
                expires: retry_expires,
            });
        }
        self.sequence = sequence;
        Ok(CommitOutcome {
            metadata,
            duplicate: false,
            available: true,
        })
    }

    pub fn snapshot(&mut self, now: Now) -> Result<Snapshot, ShelfError> {
        self.expire(now)?;
        Ok(Snapshot {
            epoch: self.epoch,
            sequence: self.sequence,
            items: self
                .items
                .iter()
                .rev()
                .map(|item| item.item.metadata.clone())
                .collect(),
        })
    }

    pub fn fetch(&mut self, id: Uuid, now: Now) -> Result<Option<FetchedItem>, ShelfError> {
        self.expire(now)?;
        Ok(self
            .items
            .iter()
            .find(|item| item.item.metadata.id == id)
            .map(|item| item.item.clone()))
    }

    pub fn latest(&mut self, now: Now) -> Result<Option<FetchedItem>, ShelfError> {
        self.expire(now)?;
        Ok(self.items.back().map(|item| item.item.clone()))
    }

    pub fn delete(&mut self, id: Uuid, now: Now) -> Result<bool, ShelfError> {
        self.expire(now)?;
        let Some(index) = self
            .items
            .iter()
            .position(|item| item.item.metadata.id == id)
        else {
            return Ok(false);
        };
        let sequence = self.sequence.checked_add(1).ok_or(ShelfError::Overflow)?;
        self.stored_bytes -= self
            .items
            .remove(index)
            .unwrap()
            .item
            .metadata
            .encoded_bytes;
        self.sequence = sequence;
        Ok(true)
    }

    pub fn clear(&mut self, now: Now) -> Result<(), ShelfError> {
        self.expire(now)?;
        if !self.items.is_empty() {
            self.sequence = self.sequence.checked_add(1).ok_or(ShelfError::Overflow)?;
            self.items.clear();
            self.stored_bytes = 0;
        }
        Ok(())
    }

    fn evict_oldest(&mut self) {
        let item = self
            .items
            .pop_front()
            .expect("validated limits allow the new item after eviction");
        self.stored_bytes -= item.item.metadata.encoded_bytes;
    }
}

#[cfg(test)]
mod tests;
