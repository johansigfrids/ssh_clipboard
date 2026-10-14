//! Metadata history and selected preview. Clipboard writes remain in panel orchestration.
use crate::model::ItemPreview;
use serde::Serialize;
use ssh_clipboard::client::session::{Action, SessionPolicy, Snapshot};
use ssh_clipboard::client::session_transport::{self as transport, Operation, Outcome};
use ssh_clipboard::client::ssh::SshConfig;
use ssh_clipboard::protocol::{CONTENT_TYPE_PNG, CONTENT_TYPE_TEXT, ClipboardValue};
use ssh_clipboard::session_protocol::{ContentType, Item, Response};
use uuid::Uuid;

#[derive(Default)]
pub struct History {
    pub epoch: Option<Uuid>,
    pub items: Vec<Item>,
    pub selected: Option<Uuid>,
    pub preview: Option<ItemPreview>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    epoch: Option<Uuid>,
    selected_id: Option<Uuid>,
    items: Vec<Row>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Row {
    id: Uuid,
    label: String,
    kind: &'static str,
    encoded_bytes: u32,
    source: Option<String>,
    received_at: String,
    expires_at: String,
}
impl History {
    pub fn item_preview(&self, show_preview: bool) -> Option<ItemPreview> {
        self.preview.clone().map(|mut preview| {
            if !show_preview {
                preview.label = if preview.kind == "image" {
                    "Image".into()
                } else {
                    "Text".into()
                };
                preview.text = None;
                preview.image_url = None;
            }
            preview
        })
    }
    pub fn view(&self, show_preview: bool) -> View {
        View {
            epoch: self.epoch,
            selected_id: self.selected,
            items: self
                .items
                .iter()
                .map(|item| {
                    let representation = &item.representations[0];
                    let image = representation.content_type == ContentType::Png;
                    Row {
                        id: item.id,
                        label: if show_preview {
                            item.label.clone()
                        } else if image {
                            "Image".into()
                        } else {
                            "Text".into()
                        },
                        kind: if image { "image" } else { "text" },
                        encoded_bytes: representation.encoded_bytes,
                        source: item.origin.as_ref().map(|origin| origin.name.clone()),
                        received_at: item.received_at_ms.get().to_string(),
                        expires_at: item.expires_at_ms.get().to_string(),
                    }
                })
                .collect(),
        }
    }
    pub fn accept_snapshot(&mut self, snapshot: Snapshot) {
        let retain = (self.epoch == Some(snapshot.epoch))
            .then_some(self.selected)
            .flatten();
        self.selected = retain
            .filter(|id| snapshot.items.iter().any(|item| item.id == *id))
            .or_else(|| snapshot.items.first().map(|item| item.id));
        self.epoch = Some(snapshot.epoch);
        self.items = snapshot.items;
        self.preview = None;
    }
    pub fn guard(&self, epoch: Uuid) -> Result<(), String> {
        if self.epoch != Some(epoch) {
            return Err("The server history changed. Refresh before continuing.".into());
        }
        Ok(())
    }
    pub async fn refresh(
        &mut self,
        config: &SshConfig,
        policy: SessionPolicy,
    ) -> Result<(), String> {
        let Outcome::Snapshot(snapshot) = transport::run(config, policy, Operation::Snapshot, &[])
            .await
            .map_err(|error| error.to_string())?
        else {
            return Err("Unexpected history reply.".into());
        };
        self.accept_snapshot(snapshot);
        Ok(())
    }
    pub async fn fetch(
        &mut self,
        config: &SshConfig,
        policy: SessionPolicy,
        id: Uuid,
        show_preview: bool,
    ) -> Result<ClipboardValue, String> {
        let epoch = self.epoch.ok_or("Refresh history first.")?;
        if !self.items.iter().any(|item| item.id == id) {
            return Err("This item is no longer in the loaded history. Refresh first.".into());
        }
        self.preview = None;
        self.selected = Some(id);
        let result = transport::run(
            config,
            policy,
            Operation::AtEpoch {
                epoch,
                action: Action::Fetch { item_id: id },
            },
            &[],
        )
        .await;
        let Outcome::Reply(reply) =
            result.map_err(|error| format!("{error} Your clipboard was left unchanged."))?
        else {
            return Err("Unexpected item reply.".into());
        };
        let Response::Value { item, .. } = reply.response else {
            return Err("Unexpected item reply.".into());
        };
        let value = ClipboardValue {
            content_type: match item.representations[0].content_type {
                ContentType::Text => CONTENT_TYPE_TEXT,
                ContentType::Png => CONTENT_TYPE_PNG,
            }
            .into(),
            data: reply.payload,
            created_at: item.created_at_ms.unwrap_or(item.received_at_ms).get(),
        };
        self.preview = Some(ItemPreview::from_value_with_limit(
            &value,
            show_preview,
            policy.max_decoded_bytes as usize,
        )?);
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn item(id: u128, label: &str) -> Item {
        Item {
            id: Uuid::from_u128(id),
            sequence: (id as u64).into(),
            label: label.into(),
            origin: None,
            created_at_ms: None,
            received_at_ms: 100.into(),
            expires_at_ms: 200.into(),
            representations: vec![ssh_clipboard::session_protocol::Representation {
                id: 0,
                content_type: ContentType::Text,
                encoded_bytes: 10,
                dimensions: None,
                decoded_bytes: None,
            }],
        }
    }
    #[test]
    fn private_labels_never_cross_the_frontend_boundary_when_previews_are_hidden() {
        let history = History {
            epoch: Some(Uuid::from_u128(1)),
            selected: Some(Uuid::from_u128(2)),
            items: vec![item(2, "PRIVATE-SENTINEL")],
            preview: None,
        };
        let hidden = serde_json::to_string(&history.view(false)).unwrap();
        assert!(!hidden.contains("PRIVATE-SENTINEL"));
        assert!(hidden.contains("Text"));
        assert!(
            serde_json::to_string(&history.view(true))
                .unwrap()
                .contains("PRIVATE-SENTINEL")
        );
    }
    #[test]
    fn selection_is_retained_by_id_only_within_the_same_epoch_and_live_snapshot() {
        let epoch = Uuid::from_u128(1);
        let mut history = History::default();
        history.accept_snapshot(Snapshot {
            epoch,
            sequence: 3.into(),
            items: vec![item(3, "New"), item(2, "Old")],
        });
        history.selected = Some(Uuid::from_u128(2));
        history.accept_snapshot(Snapshot {
            epoch,
            sequence: 3.into(),
            items: vec![item(3, "New"), item(2, "Old")],
        });
        assert_eq!(history.selected, Some(Uuid::from_u128(2)));
        history.accept_snapshot(Snapshot {
            epoch,
            sequence: 4.into(),
            items: vec![item(3, "New")],
        });
        assert_eq!(history.selected, Some(Uuid::from_u128(3)));
        history.accept_snapshot(Snapshot {
            epoch: Uuid::from_u128(10),
            sequence: 2.into(),
            items: vec![item(2, "Different server")],
        });
        assert!(history.guard(epoch).is_err());
        assert_eq!(history.selected, Some(Uuid::from_u128(2)));
        assert!(history.preview.is_none());
    }
}
