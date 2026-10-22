//! One RAM-only frozen share. Never recapture, renew its retry window, or cross epochs.
use crate::clipboard_payload::ClipboardPayload;
use serde::Serialize;
use ssh_clipboard::client::session::Action;
use ssh_clipboard::session_protocol::{Number, Origin, Representation};
use tokio::time::Instant;
use uuid::Uuid;

pub struct PendingShare {
    pub epoch: Uuid,
    pub operation_id: Uuid,
    pub origin: Origin,
    pub representation: Representation,
    pub value: Option<ClipboardPayload>,
    deadline: Instant,
    server_deadline: Option<Number>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    pub id: Uuid,
    pub can_retry: bool,
}
impl PendingShare {
    pub fn new(
        epoch: Uuid,
        operation_id: Uuid,
        origin: Origin,
        representation: Representation,
        value: ClipboardPayload,
        retry_ms: u32,
        server_time: Option<Number>,
    ) -> Self {
        let window = std::time::Duration::from_millis(retry_ms.into())
            .saturating_sub(std::time::Duration::from_secs(1));
        Self {
            epoch,
            operation_id,
            origin,
            representation,
            value: Some(value),
            deadline: Instant::now() + window,
            server_deadline: server_time
                .and_then(|time| time.get().checked_add(u64::from(retry_ms)).map(Into::into)),
        }
    }
    pub fn view(&self) -> View {
        View {
            id: self.operation_id,
            can_retry: self.value.is_some()
                && self.server_deadline.is_some()
                && Instant::now() < self.deadline,
        }
    }
    pub fn expire(&mut self) -> bool {
        if Instant::now() >= self.deadline {
            self.value.take().is_some()
        } else {
            false
        }
    }
    pub fn expire_server(&mut self, now: Number) {
        if self
            .server_deadline
            .is_some_and(|deadline| now.get() >= deadline.get())
        {
            self.deadline = Instant::now();
            self.value = None;
        }
    }
    pub fn action(&self) -> Result<Action, String> {
        if !self.view().can_retry {
            return Err("The captured share's safe retry window has expired. Check its status or discard recovery data.".into());
        }
        Ok(self.initial_action())
    }
    pub fn initial_action(&self) -> Action {
        let value = self.value.as_ref().unwrap();
        let share = Action::Share {
            operation_id: self.operation_id,
            origin: self.origin.clone(),
            representation: self.representation.clone(),
            created_at_ms: Some(value.created_at.into()),
        };
        if let Some(not_after_ms) = self.server_deadline {
            Action::RetryShare {
                share: Box::new(share),
                not_after_ms,
            }
        } else {
            share
        }
    }
    pub fn guard(&self, id: Uuid, epoch: Uuid) -> Result<(), String> {
        if id != self.operation_id {
            return Err(
                "This recovery action refers to a different share. Reload the panel.".into(),
            );
        }
        if epoch != self.epoch {
            return Err("The server restarted or this is a different server. The earlier outcome is uncertain; captured bytes will not be sent to this epoch.".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retry_keeps_original_identity_bytes_timestamp_and_deadline_and_rejects_other_epochs() {
        let value = ClipboardPayload {
            content_type: crate::clipboard_payload::CONTENT_TYPE_TEXT,
            data: b"original bytes".to_vec(),
            created_at: 123,
        };
        let mut pending = PendingShare::new(
            Uuid::from_u128(1),
            Uuid::from_u128(3),
            Origin {
                device_id: Uuid::from_u128(2),
                name: "Synthetic".into(),
            },
            Representation {
                id: 0,
                content_type: ssh_clipboard::session_protocol::ContentType::Text,
                encoded_bytes: value.data.len() as u32,
                dimensions: None,
                decoded_bytes: None,
            },
            value,
            10_000,
            Some(0.into()),
        );
        let deadline = pending.deadline;
        assert!(
            !serde_json::to_string(&pending.view())
                .unwrap()
                .contains("original bytes")
        );
        for _ in 0..2 {
            assert!(
                matches!(pending.action().unwrap(),Action::RetryShare {share,not_after_ms} if not_after_ms.get()==10_000 && matches!(share.as_ref(),Action::Share {operation_id,created_at_ms:Some(time),..} if *operation_id==pending.operation_id && time.get()==123))
            );
            assert_eq!(pending.value.as_ref().unwrap().data, b"original bytes");
            assert_eq!(pending.deadline, deadline);
        }
        assert!(
            pending
                .guard(pending.operation_id, Uuid::from_u128(3))
                .is_err()
        );
        assert!(pending.guard(Uuid::new_v4(), pending.epoch).is_err());
        pending.deadline = Instant::now();
        assert!(pending.expire());
        assert!(pending.value.is_none());
        assert!(pending.action().is_err());
        assert!(!pending.expire());
    }
}
