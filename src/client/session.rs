//! Sequential shelf client over any owned stream. No implicit clipboard access or retries.
use crate::session_protocol::framing::{self, FrameLimits};
use crate::session_protocol::{
    self as wire, ContentType, EffectiveLimits, ErrorCode, Item, Number, Origin, Representation,
    Request, Response, Timestamp,
};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::time::timeout;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SessionPolicy {
    pub max_item_bytes: u32,
    pub max_decoded_bytes: u32,
    pub operation_timeout: Duration,
}
impl Default for SessionPolicy {
    fn default() -> Self {
        Self {
            max_item_bytes: 64 * 1024 * 1024,
            max_decoded_bytes: 256 * 1024 * 1024,
            operation_timeout: Duration::from_millis(7000),
        }
    }
}
impl SessionPolicy {
    pub fn validate(&self) -> Result<(), SessionError> {
        FrameLimits::session(
            self.max_item_bytes as usize,
            self.max_decoded_bytes as usize,
        )
        .map_err(|_| SessionError::Local)?;
        if self.operation_timeout.is_zero() || self.operation_timeout > Duration::from_secs(300) {
            return Err(SessionError::Local);
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SessionError {
    #[error("Shelf session is closed; reconnect before another operation.")]
    Closed,
    #[error("Shelf operation timed out; reconnect before another operation.")]
    Timeout,
    #[error("Shelf stream failed; reconnect before another operation.")]
    Stream,
    #[error("The server sent an invalid or mismatched shelf reply.")]
    Protocol,
    #[error("The request exceeds negotiated limits or has invalid metadata.")]
    Local,
    #[error("The server does not support this shelf action.")]
    Unsupported,
    #[error("The server returned invalid or oversized clipboard content.")]
    Content,
    #[error("{}", remote_error(.0))]
    Remote(ErrorCode),
}

fn remote_error(code: &ErrorCode) -> String {
    if *code == ErrorCode::UnsupportedVersion {
        "Client and server session versions differ. Upgrade both clients and server to file-capable builds (session version 2); existing settings can be kept.".into()
    } else {
        format!("The server rejected the shelf operation: {code:?}.")
    }
}

/// Caller retains the original operation ID and captured bytes for any deliberate reconciliation.
pub enum Action {
    Share {
        operation_id: Uuid,
        origin: Origin,
        representation: Representation,
        created_at_ms: Option<Timestamp>,
    },
    List {
        offset: u16,
        limit: u16,
        expected_sequence: Option<Number>,
    },
    Fetch {
        item_id: Uuid,
    },
    Delete {
        item_id: Uuid,
    },
    Clear,
    Health,
    OperationStatus {
        device_id: Uuid,
        operation_id: Uuid,
    },
    RetryShare {
        share: Box<Action>,
        not_after_ms: Number,
    },
    Watch {
        after_sequence: Number,
        wait_ms: u16,
    },
}
impl Action {
    fn request(self, request_id: Number, epoch: Uuid) -> Request {
        match self {
            Self::RetryShare {
                share,
                not_after_ms,
            } => Request::RetryShare {
                share: Box::new(share.request(request_id, epoch)),
                not_after_ms,
            },
            Self::Share {
                operation_id,
                origin,
                representation,
                created_at_ms,
            } => Request::Share {
                request_id,
                epoch,
                operation_id,
                origin,
                representation,
                created_at_ms,
            },
            Self::List {
                offset,
                limit,
                expected_sequence,
            } => Request::List {
                request_id,
                epoch,
                offset,
                limit,
                expected_sequence,
            },
            Self::Fetch { item_id } => Request::Fetch {
                request_id,
                epoch,
                item_id,
                representation_id: 0,
            },
            Self::Delete { item_id } => Request::Delete {
                request_id,
                epoch,
                item_id,
            },
            Self::Clear => Request::Clear { request_id, epoch },
            Self::Health => Request::Health { request_id, epoch },
            Self::OperationStatus {
                device_id,
                operation_id,
            } => Request::OperationStatus {
                request_id,
                epoch,
                device_id,
                operation_id,
            },
            Self::Watch {
                after_sequence,
                wait_ms,
            } => Request::Watch {
                request_id,
                epoch,
                after_sequence,
                wait_ms,
            },
        }
    }
}

pub struct Reply {
    pub response: Response,
    pub payload: Vec<u8>,
}
#[derive(Clone)]
pub struct Snapshot {
    pub epoch: Uuid,
    pub sequence: Number,
    pub items: Vec<Item>,
}

pub struct Session<T> {
    stream: T,
    policy: SessionPolicy,
    welcome: Response,
    limits: EffectiveLimits,
    epoch: Uuid,
    sequence: Number,
    next_id: u64,
    ready: bool,
    mutation_uncertain: bool,
}
impl<T: AsyncRead + AsyncWrite + Unpin> Session<T> {
    pub async fn connect(mut stream: T, policy: SessionPolicy) -> Result<Self, SessionError> {
        policy.validate()?;
        let hello = Request::Hello {
            request_id: 1.into(),
            versions: vec![wire::SESSION_VERSION],
            max_item_bytes: policy.max_item_bytes,
            max_decoded_bytes: policy.max_decoded_bytes,
        };
        let welcome = timeout(policy.operation_timeout, async {
            framing::write_message(&mut stream, &hello, &[], FrameLimits::bootstrap())
                .await
                .map_err(stream_error)?;
            let header = framing::read_header(&mut stream, FrameLimits::bootstrap())
                .await
                .map_err(stream_error)?;
            let response: Response = framing::read_metadata(&mut stream, &header)
                .await
                .map_err(stream_error)?;
            response.validate(0).map_err(|_| SessionError::Protocol)?;
            if response.request_id() != 1.into() {
                return Err(SessionError::Protocol);
            }
            match &response {
                Response::Error { code, .. } => return Err(SessionError::Remote(*code)),
                Response::Welcome {
                    limits,
                    capabilities,
                    ..
                } if limits.max_item_bytes <= policy.max_item_bytes
                    && limits.max_decoded_bytes <= policy.max_decoded_bytes =>
                {
                    if capabilities.len() > 32
                        || capabilities.iter().enumerate().any(|(index, name)| {
                            name.is_empty()
                                || name.len() > 64
                                || name.chars().any(char::is_control)
                                || capabilities[..index].contains(name)
                        })
                    {
                        return Err(SessionError::Protocol);
                    }
                }
                _ => return Err(SessionError::Protocol),
            }
            Ok(response)
        })
        .await
        .map_err(|_| SessionError::Timeout)??;
        let Response::Welcome {
            epoch,
            sequence,
            limits,
            ..
        } = &welcome
        else {
            unreachable!()
        };
        Ok(Self {
            stream,
            policy,
            limits: limits.clone(),
            epoch: *epoch,
            sequence: *sequence,
            welcome,
            next_id: 2,
            ready: true,
            mutation_uncertain: false,
        })
    }
    pub fn welcome(&self) -> &Response {
        &self.welcome
    }
    pub fn epoch(&self) -> Uuid {
        self.epoch
    }
    pub fn limits(&self) -> &EffectiveLimits {
        &self.limits
    }
    pub fn is_ready(&self) -> bool {
        self.ready
    }
    pub fn mutation_uncertain(&self) -> bool {
        self.mutation_uncertain
    }

    pub async fn execute(&mut self, action: Action, payload: &[u8]) -> Result<Reply, SessionError> {
        if !self.ready {
            return Err(SessionError::Closed);
        }
        let next = self.next_id.checked_add(1).ok_or(SessionError::Closed)?;
        let request = action.request(self.next_id.into(), self.epoch);
        request
            .validate(payload.len())
            .map_err(|_| SessionError::Local)?;
        if payload.len() > self.limits.max_item_bytes as usize {
            return Err(SessionError::Local);
        }
        let capability = match &request {
            Request::RetryShare { .. } => Some("deadline_share"),
            Request::Share { .. } => Some("deduplicated_share"),
            Request::List { .. } => Some("list"),
            Request::Fetch { .. } => Some("fetch"),
            Request::Delete { .. } => Some("delete"),
            Request::Clear { .. } => Some("clear"),
            Request::Watch { .. } => Some("watch"),
            Request::OperationStatus { .. } => Some("operation_status"),
            _ => None,
        };
        if let (Some(capability), Response::Welcome { capabilities, .. }) =
            (capability, &self.welcome)
            && !capabilities.iter().any(|item| item == capability)
        {
            return Err(SessionError::Unsupported);
        }
        let share_request = if let Request::RetryShare { share, .. } = &request {
            share.as_ref()
        } else {
            &request
        };
        if let Request::Share { representation, .. } = share_request
            && representation
                .decoded_bytes
                .is_some_and(|bytes| bytes > self.limits.max_decoded_bytes)
        {
            return Err(SessionError::Local);
        }
        // If this future is cancelled, partially consumed framing cannot be reused.
        self.ready = false;
        self.mutation_uncertain = matches!(
            request,
            Request::Share { .. }
                | Request::RetryShare { .. }
                | Request::Delete { .. }
                | Request::Clear { .. }
        );
        self.next_id = next;
        let result = timeout(
            self.policy.operation_timeout,
            self.exchange(&request, payload),
        )
        .await
        .map_err(|_| SessionError::Timeout)?;
        if let Ok(reply) = &result {
            self.ready = true;
            self.mutation_uncertain = false;
            if let Some(sequence) = response_sequence(&reply.response) {
                self.sequence = sequence;
            }
            if let Response::Error { code, .. } = reply.response {
                if code == ErrorCode::EpochChanged {
                    self.ready = false;
                }
                return Err(SessionError::Remote(code));
            }
        }
        result
    }

    async fn exchange(&mut self, request: &Request, payload: &[u8]) -> Result<Reply, SessionError> {
        let frame_limits = self
            .limits
            .frame_limits()
            .map_err(|_| SessionError::Protocol)?;
        framing::write_message(&mut self.stream, request, payload, frame_limits)
            .await
            .map_err(stream_error)?;
        let header = framing::read_header(&mut self.stream, frame_limits)
            .await
            .map_err(stream_error)?;
        let response: Response = framing::read_metadata(&mut self.stream, &header)
            .await
            .map_err(stream_error)?;
        response
            .validate(header.payload_bytes())
            .map_err(|_| SessionError::Protocol)?;
        check_reply(request, &response, self.epoch, self.sequence, &self.limits)?;
        let mut payload = Vec::new();
        payload
            .try_reserve_exact(header.payload_bytes())
            .map_err(|_| SessionError::Content)?;
        payload.resize(header.payload_bytes(), 0);
        self.stream
            .read_exact(&mut payload)
            .await
            .map_err(|_| SessionError::Stream)?;
        if let Response::Value { item, .. } = &response {
            let representation = item.representations[0].clone();
            let max_decoded = self.limits.max_decoded_bytes as usize;
            payload = tokio::task::spawn_blocking(move || {
                match representation.content_type {
                    ContentType::Files => {
                        crate::files::validate(&payload).map_err(|_| SessionError::Content)?;
                    }
                    ContentType::Text => {
                        std::str::from_utf8(&payload).map_err(|_| SessionError::Content)?;
                    }
                    ContentType::Png => {
                        let (width, height, decoded) =
                            crate::shelf::validate_png(&payload, max_decoded)
                                .map_err(|_| SessionError::Content)?;
                        if representation.dimensions != Some(wire::Dimensions { width, height })
                            || representation.decoded_bytes.map(|value| value as usize)
                                != Some(decoded)
                        {
                            return Err(SessionError::Content);
                        }
                    }
                }
                Ok(payload)
            })
            .await
            .map_err(|_| SessionError::Content)??;
        }
        Ok(Reply { response, payload })
    }

    /// One bounded, consistent enumeration. A mutation returns ResyncRequired for explicit refresh.
    pub async fn snapshot(&mut self) -> Result<Snapshot, SessionError> {
        timeout(self.policy.operation_timeout, self.snapshot_inner())
            .await
            .map_err(|_| SessionError::Timeout)?
    }

    async fn snapshot_inner(&mut self) -> Result<Snapshot, SessionError> {
        let mut items = Vec::<Item>::new();
        let mut offset = 0;
        let mut sequence = None;
        let mut expected_total = None;
        loop {
            let reply = self
                .execute(
                    Action::List {
                        offset,
                        limit: self.limits.max_list_items,
                        expected_sequence: sequence,
                    },
                    &[],
                )
                .await?;
            let Response::Listed {
                items: page,
                sequence: current,
                next_offset,
                total,
                ..
            } = reply.response
            else {
                return Err(SessionError::Protocol);
            };
            if expected_total.is_some_and(|expected| expected != total) {
                self.ready = false;
                return Err(SessionError::Protocol);
            }
            expected_total = Some(total);
            if let (Some(last), Some(first)) = (items.last(), page.first())
                && (last.sequence.get() <= first.sequence.get()
                    || items
                        .iter()
                        .any(|previous| page.iter().any(|item| previous.id == item.id)))
            {
                self.ready = false;
                return Err(SessionError::Protocol);
            }
            items.extend(page);
            sequence = Some(current);
            match next_offset {
                Some(next) => offset = next,
                None => {
                    return Ok(Snapshot {
                        epoch: self.epoch,
                        sequence: current,
                        items,
                    });
                }
            }
        }
    }

    pub async fn close(mut self) -> Result<(), SessionError> {
        self.ready = false;
        timeout(self.policy.operation_timeout, self.stream.shutdown())
            .await
            .map_err(|_| SessionError::Timeout)?
            .map_err(|_| SessionError::Stream)
    }
}

fn response_sequence(response: &Response) -> Option<Number> {
    match response {
        Response::Welcome { sequence, .. }
        | Response::Shared { sequence, .. }
        | Response::Listed { sequence, .. }
        | Response::Value { sequence, .. }
        | Response::Deleted { sequence, .. }
        | Response::Cleared { sequence, .. }
        | Response::Health { sequence, .. }
        | Response::Changed { sequence, .. }
        | Response::Operation { sequence, .. } => Some(*sequence),
        Response::Error { .. } => None,
    }
}
fn check_reply(
    request: &Request,
    response: &Response,
    epoch: Uuid,
    sequence: Number,
    limits: &EffectiveLimits,
) -> Result<(), SessionError> {
    if response.request_id() != request.request_id()
        || response_sequence(response).is_some_and(|current| current.get() < sequence.get())
    {
        return Err(SessionError::Protocol);
    }
    if let Response::Error {
        code,
        epoch: returned,
        ..
    } = response
    {
        if returned.is_some_and(|returned| returned != epoch) && *code != ErrorCode::EpochChanged {
            return Err(SessionError::Protocol);
        }
        return Ok(());
    }
    let inner_request = if let Request::RetryShare { share, .. } = request {
        share.as_ref()
    } else {
        request
    };
    let matches = match (inner_request, response) {
        (
            Request::OperationStatus {
                device_id,
                operation_id,
                ..
            },
            Response::Operation {
                epoch: returned,
                device_id: actual_device,
                operation_id: actual_operation,
                ..
            },
        ) => *returned == epoch && device_id == actual_device && operation_id == actual_operation,
        (
            Request::Watch { after_sequence, .. },
            Response::Changed {
                epoch: returned,
                sequence: current,
                ..
            },
        ) => *returned == epoch && current.get() >= after_sequence.get(),
        (
            Request::Share {
                origin,
                representation,
                created_at_ms,
                ..
            },
            Response::Shared {
                item,
                epoch: returned,
                ..
            },
        ) => {
            *returned == epoch
                && item.origin.as_ref() == Some(origin)
                && item.created_at_ms == *created_at_ms
                && item.representations[0].content_type == representation.content_type
                && item.representations[0].encoded_bytes == representation.encoded_bytes
                && representation
                    .dimensions
                    .is_none_or(|value| item.representations[0].dimensions == Some(value))
                && representation
                    .decoded_bytes
                    .is_none_or(|value| item.representations[0].decoded_bytes == Some(value))
        }
        (
            Request::List {
                offset,
                limit,
                expected_sequence,
                ..
            },
            Response::Listed {
                epoch: returned,
                offset: actual,
                items,
                total,
                sequence,
                ..
            },
        ) => {
            *returned == epoch
                && offset == actual
                && items.len() <= *limit as usize
                && *total <= limits.max_items
                && expected_sequence.is_none_or(|expected| expected == *sequence)
        }
        (
            Request::Fetch {
                item_id,
                representation_id,
                ..
            },
            Response::Value {
                item,
                epoch: returned,
                representation_id: actual,
                ..
            },
        ) => {
            *returned == epoch
                && item_id == &item.id
                && representation_id == actual
                && item.representations[0]
                    .decoded_bytes
                    .is_none_or(|bytes| bytes <= limits.max_decoded_bytes)
        }
        (
            Request::Delete { .. },
            Response::Deleted {
                epoch: returned, ..
            },
        )
        | (
            Request::Clear { .. },
            Response::Cleared {
                epoch: returned, ..
            },
        ) => *returned == epoch,
        (
            Request::Health { .. },
            Response::Health {
                epoch: returned,
                items,
                stored_bytes,
                ..
            },
        ) => {
            *returned == epoch
                && *items <= limits.max_items
                && *stored_bytes <= limits.max_stored_bytes
        }
        _ => false,
    };
    if !matches {
        return Err(SessionError::Protocol);
    }
    Ok(())
}
fn stream_error(error: wire::WireError) -> SessionError {
    match error {
        wire::WireError::Io(_) => SessionError::Stream,
        _ => SessionError::Protocol,
    }
}

#[cfg(test)]
mod tests;
