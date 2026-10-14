//! Transport-independent session service. Native socket ownership and authentication
//! belong to the listener adapter; this layer owns admission, deadlines and shelf actions.
use crate::session_protocol::framing::{self, FrameLimits, IncomingRequest, WireError};
use crate::session_protocol::{self as wire, EffectiveLimits, ErrorCode, Request, Response};
use crate::shelf::{FetchedItem, Now, Origin, Shelf, ShelfError, ShelfLimits, UploadBudget};
use std::sync::Mutex;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::sync::Semaphore;
use tokio::time::{Instant, timeout, timeout_at};
use uuid::Uuid;

#[derive(Clone, Copy)]
pub struct ServiceLimits {
    pub max_connections: usize,
    pub handshake_timeout: Duration,
    pub operation_timeout: Duration,
    pub idle_timeout: Duration,
}
impl Default for ServiceLimits {
    fn default() -> Self {
        Self {
            max_connections: 16,
            handshake_timeout: Duration::from_secs(5),
            operation_timeout: Duration::from_secs(30),
            idle_timeout: Duration::from_secs(300),
        }
    }
}

pub struct ShelfService {
    shelf: Mutex<Shelf>,
    budget: UploadBudget,
    slots: Semaphore,
    limits: ServiceLimits,
}
struct Outgoing {
    response: Response,
    item: Option<FetchedItem>,
}
impl Outgoing {
    fn control(response: Response) -> Self {
        Self {
            response,
            item: None,
        }
    }
}

impl ShelfService {
    pub fn new(shelf_limits: ShelfLimits, limits: ServiceLimits) -> Result<Self, WireError> {
        if limits.max_connections == 0
            || limits.max_connections > 64
            || limits.handshake_timeout.is_zero()
            || limits.handshake_timeout > Duration::from_secs(30)
            || limits.operation_timeout.is_zero()
            || limits.operation_timeout > Duration::from_secs(300)
            || limits.idle_timeout.is_zero()
            || limits.idle_timeout > Duration::from_secs(3600)
        {
            return Err(ShelfError::InvalidLimits.into());
        }
        let shelf = Shelf::new(shelf_limits, Now::system()?)?;
        let budget = shelf.upload_budget();
        Ok(Self {
            shelf: Mutex::new(shelf),
            budget,
            slots: Semaphore::new(limits.max_connections),
            limits,
        })
    }

    fn with_shelf<T>(
        &self,
        action: impl FnOnce(&mut Shelf, Now) -> Result<T, WireError>,
    ) -> Result<T, WireError> {
        let mut shelf = self
            .shelf
            .lock()
            .map_err(|_| WireError::Rejected(ErrorCode::Internal))?;
        let now = Now::system()?;
        action(&mut shelf, now)
    }

    /// The listener must call this periodically even when no requests arrive.
    pub fn maintenance(&self) -> Result<(Uuid, u64), WireError> {
        self.with_shelf(|shelf, now| {
            shelf.expire(now)?;
            Ok((shelf.epoch(), shelf.sequence()))
        })
    }

    pub fn resident_bytes(&self) -> usize {
        self.budget.resident_bytes()
    }

    pub async fn serve(
        &self,
        mut stream: impl AsyncRead + AsyncWrite + Unpin,
    ) -> Result<(), WireError> {
        // No waiting connection queue and no frame allocation before admission.
        let _slot = self.slots.try_acquire().map_err(|_| ShelfError::Busy)?;
        let result = self.serve_inner(&mut stream).await;
        let _ = timeout(self.limits.operation_timeout, stream.shutdown()).await;
        result
    }

    async fn serve_inner(
        &self,
        stream: &mut (impl AsyncRead + AsyncWrite + Unpin),
    ) -> Result<(), WireError> {
        let deadline = Instant::now() + self.limits.handshake_timeout;
        let hello = timeout_at(deadline, framing::read_hello(stream))
            .await
            .map_err(|_| timed_out())??;
        let welcome = self.with_shelf(|shelf, now| {
            shelf.expire(now)?;
            wire::negotiate(&hello, shelf.epoch(), shelf.sequence(), shelf.limits())
        });
        let welcome = match welcome {
            Ok(welcome) => welcome,
            Err(WireError::Rejected(code)) => {
                let error = Response::Error {
                    request_id: hello.request_id(),
                    code,
                    epoch: None,
                };
                timeout_at(
                    deadline,
                    framing::write_message(stream, &error, &[], FrameLimits::bootstrap()),
                )
                .await
                .map_err(|_| timed_out())??;
                return Ok(());
            }
            Err(error) => return Err(error),
        };
        let Response::Welcome { limits: peer, .. } = &welcome else {
            unreachable!()
        };
        let frame_limits = peer.frame_limits()?;
        timeout_at(
            deadline,
            framing::write_message(stream, &welcome, &[], FrameLimits::bootstrap()),
        )
        .await
        .map_err(|_| timed_out())??;
        loop {
            let header = match timeout(
                self.limits.idle_timeout,
                framing::read_header(stream, frame_limits),
            )
            .await
            .map_err(|_| timed_out())?
            {
                Ok(header) => header,
                Err(WireError::Io(error)) if error.kind() == std::io::ErrorKind::UnexpectedEof => {
                    return Ok(());
                }
                Err(error) => return Err(error),
            };
            let deadline = Instant::now() + self.limits.operation_timeout;
            timeout_at(deadline, async {
                let incoming =
                    match framing::read_request_body(stream, header, frame_limits, &self.budget)
                        .await
                    {
                        Ok(incoming) => incoming,
                        Err(WireError::UploadRejected {
                            request_id,
                            payload_bytes,
                            source,
                        }) => {
                            // This length already passed the peer/frame limits. Drain in fixed
                            // chunks under the operation deadline, then reply without committing.
                            let mut remaining = payload_bytes;
                            let mut discard = [0; 8192];
                            while remaining > 0 {
                                let take = remaining.min(discard.len());
                                tokio::io::AsyncReadExt::read_exact(stream, &mut discard[..take])
                                    .await?;
                                remaining -= take;
                            }
                            let reply = self.with_shelf(|shelf, _| {
                                Ok(error(request_id, code(&source), shelf.epoch()))
                            })?;
                            framing::write_message(stream, &reply.response, &[], frame_limits)
                                .await?;
                            return Ok(());
                        }
                        Err(error) => return Err(error),
                    };
                let outgoing = self.handle(incoming, peer).await?;
                let payload = outgoing.item.as_ref().map_or(&[][..], FetchedItem::bytes);
                outgoing.response.validate(payload.len())?;
                framing::write_message(stream, &outgoing.response, payload, frame_limits).await
            })
            .await
            .map_err(|_| timed_out())??;
        }
    }

    async fn handle(
        &self,
        incoming: IncomingRequest,
        peer: &EffectiveLimits,
    ) -> Result<Outgoing, WireError> {
        let IncomingRequest { request, upload } = incoming;
        let request_id = request.request_id();
        if let Request::Share {
            epoch,
            operation_id,
            origin,
            representation,
            created_at_ms,
            ..
        } = request
        {
            if let Some(error) = self.with_shelf(|shelf, _| {
                Ok((epoch != shelf.epoch())
                    .then(|| error(request_id, ErrorCode::EpochChanged, shelf.epoch())))
            })? {
                return Ok(error);
            }
            let input = upload.ok_or(WireError::InvalidState)?;
            // Validation jobs never mutate the shelf. A timed-out/dropped waiter cannot
            // later commit a detached validation result.
            let prepared = tokio::task::spawn_blocking(move || {
                input.validate(
                    representation.content_type.into(),
                    Some(Origin {
                        device_id: origin.device_id,
                        name: origin.name,
                    }),
                    created_at_ms.map(|time| time.get()),
                    Some(operation_id),
                )
            })
            .await
            .map_err(|_| WireError::Rejected(ErrorCode::Internal))?;
            let prepared = match prepared {
                Ok(prepared) => prepared,
                Err(failure) => {
                    return self.with_shelf(|shelf, _| {
                        Ok(error(request_id, code(&failure), shelf.epoch()))
                    });
                }
            };
            if representation
                .dimensions
                .is_some_and(|size| Some((size.width, size.height)) != prepared.dimensions())
                || representation
                    .decoded_bytes
                    .is_some_and(|size| Some(size as usize) != prepared.decoded_bytes())
            {
                return self.with_shelf(|shelf, _| {
                    Ok(error(request_id, ErrorCode::InvalidContent, shelf.epoch()))
                });
            }
            return self.with_shelf(|shelf, now| match shelf.commit(epoch, prepared, now) {
                Ok(committed) => Ok(Outgoing::control(Response::Shared {
                    request_id,
                    epoch: shelf.epoch(),
                    sequence: shelf.sequence().into(),
                    item: wire::Item::try_from(&committed.metadata)?,
                    duplicate: committed.duplicate,
                    available: committed.available,
                })),
                Err(failure) => Ok(error(request_id, code(&failure), shelf.epoch())),
            });
        }
        self.with_shelf(|shelf, now| {
            shelf.expire(now)?;
            let epoch = match &request {
                Request::List { epoch, .. }
                | Request::Fetch { epoch, .. }
                | Request::Delete { epoch, .. }
                | Request::Clear { epoch, .. }
                | Request::Health { epoch, .. } => *epoch,
                _ => return Err(WireError::InvalidState),
            };
            if epoch != shelf.epoch() {
                return Ok(error(request_id, ErrorCode::EpochChanged, shelf.epoch()));
            }
            let reply = match request {
                Request::List {
                    offset,
                    limit,
                    expected_sequence,
                    ..
                } => {
                    let snapshot = shelf.snapshot(now)?;
                    if expected_sequence.is_some_and(|sequence| sequence.get() != snapshot.sequence)
                    {
                        return Ok(error(request_id, ErrorCode::ResyncRequired, epoch));
                    }
                    if usize::from(offset) > snapshot.items.len()
                        || (offset != 0 && expected_sequence.is_none())
                    {
                        return Ok(error(request_id, ErrorCode::InvalidRequest, epoch));
                    }
                    let end = (usize::from(offset) + usize::from(limit)).min(snapshot.items.len());
                    Response::Listed {
                        request_id,
                        epoch,
                        sequence: snapshot.sequence.into(),
                        items: snapshot.items[usize::from(offset)..end]
                            .iter()
                            .map(wire::Item::try_from)
                            .collect::<Result<_, _>>()?,
                        offset,
                        total: snapshot.items.len() as u16,
                        next_offset: (end < snapshot.items.len()).then_some(end as u16),
                    }
                }
                Request::Fetch {
                    item_id,
                    representation_id,
                    ..
                } => {
                    let Some(item) = shelf.fetch(item_id, now)? else {
                        return Ok(error(request_id, ErrorCode::NotFound, epoch));
                    };
                    if item.metadata.encoded_bytes > peer.max_item_bytes as usize
                        || item
                            .metadata
                            .decoded_bytes
                            .is_some_and(|size| size > peer.max_decoded_bytes as usize)
                    {
                        return Ok(error(request_id, ErrorCode::PayloadTooLarge, epoch));
                    }
                    let response = Response::Value {
                        request_id,
                        epoch,
                        sequence: shelf.sequence().into(),
                        item: wire::Item::try_from(&item.metadata)?,
                        representation_id,
                    };
                    return Ok(Outgoing {
                        response,
                        item: Some(item),
                    });
                }
                Request::Delete { item_id, .. } => Response::Deleted {
                    request_id,
                    epoch,
                    removed: shelf.delete(item_id, now)?,
                    sequence: shelf.sequence().into(),
                },
                Request::Clear { .. } => {
                    shelf.clear(now)?;
                    Response::Cleared {
                        request_id,
                        epoch,
                        sequence: shelf.sequence().into(),
                    }
                }
                Request::Health { .. } => Response::Health {
                    request_id,
                    epoch,
                    sequence: shelf.sequence().into(),
                    items: shelf.snapshot(now)?.items.len() as u16,
                    stored_bytes: shelf.stored_bytes() as u32,
                },
                _ => return Err(WireError::InvalidState),
            };
            Ok(Outgoing::control(reply))
        })
    }
}

fn error(request_id: wire::Number, code: ErrorCode, epoch: Uuid) -> Outgoing {
    Outgoing::control(Response::Error {
        request_id,
        code,
        epoch: Some(epoch),
    })
}
fn timed_out() -> WireError {
    std::io::Error::new(std::io::ErrorKind::TimedOut, "session deadline exceeded").into()
}
fn code(error: &ShelfError) -> ErrorCode {
    match error {
        ShelfError::ItemTooLarge => ErrorCode::PayloadTooLarge,
        ShelfError::Busy | ShelfError::Allocation => ErrorCode::Busy,
        ShelfError::InvalidText | ShelfError::InvalidImage => ErrorCode::InvalidContent,
        ShelfError::InvalidOrigin | ShelfError::InvalidOperation => ErrorCode::InvalidRequest,
        ShelfError::EpochChanged => ErrorCode::EpochChanged,
        ShelfError::OperationConflict => ErrorCode::OperationConflict,
        ShelfError::RetryWindowFull => ErrorCode::RetryWindowFull,
        _ => ErrorCode::Internal,
    }
}

mod legacy;
#[cfg(test)]
mod tests;
