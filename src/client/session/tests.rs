use super::*;
use crate::shelf::ShelfLimits;
use crate::shelf_service::{ServiceLimits, ShelfService};
use std::sync::Arc;
use tokio::io::DuplexStream;

fn policy() -> SessionPolicy {
    SessionPolicy {
        max_item_bytes: 1024,
        max_decoded_bytes: 1024,
        ..Default::default()
    }
}
fn share(data: &[u8], operation: u128) -> Action {
    Action::Share {
        operation_id: Uuid::from_u128(operation),
        origin: Origin {
            device_id: Uuid::from_u128(99),
            name: "Synthetic".into(),
        },
        representation: Representation {
            id: 0,
            content_type: ContentType::Text,
            encoded_bytes: data.len() as u32,
            dimensions: None,
            decoded_bytes: None,
        },
        created_at_ms: Some(123.into()),
    }
}
async fn connected() -> (
    Session<DuplexStream>,
    tokio::task::JoinHandle<Result<(), wire::WireError>>,
) {
    let service = Arc::new(
        ShelfService::new(
            ShelfLimits {
                max_items: 32,
                max_item_bytes: 1024,
                max_stored_bytes: 32768,
                max_resident_bytes: 65536,
                max_decoded_bytes: 1024,
                ..Default::default()
            },
            ServiceLimits::default(),
        )
        .unwrap(),
    );
    let (client, server) = tokio::io::duplex(64);
    let task = tokio::spawn(async move { service.serve(server).await });
    (Session::connect(client, policy()).await.unwrap(), task)
}
async fn fake_welcome(stream: &mut DuplexStream, client_policy: SessionPolicy) -> Response {
    let hello = framing::read_hello(stream).await.unwrap();
    let limits = ShelfLimits {
        max_item_bytes: client_policy.max_item_bytes as usize,
        max_decoded_bytes: client_policy.max_decoded_bytes as usize,
        ..Default::default()
    };
    wire::negotiate(&hello, Uuid::from_u128(1), 0, &limits).unwrap()
}
async fn request(stream: &mut DuplexStream) -> Request {
    let header = framing::read_header(stream, FrameLimits::session(1024, 1024).unwrap())
        .await
        .unwrap();
    framing::read_metadata(stream, &header).await.unwrap()
}

#[tokio::test]
async fn one_session_shares_pages_fetches_deduplicates_and_deletes_without_clipboard_access() {
    let (mut session, task) = connected().await;
    for operation in 1..=20 {
        let data = format!("  α {operation}\n ");
        session
            .execute(share(data.as_bytes(), operation), data.as_bytes())
            .await
            .unwrap();
    }
    let snapshot = session.snapshot().await.unwrap();
    assert_eq!(snapshot.items.len(), 20);
    assert!(
        snapshot
            .items
            .windows(2)
            .all(|pair| pair[0].sequence.get() > pair[1].sequence.get())
    );
    let selected = snapshot.items[19].id;
    let reply = session
        .execute(Action::Fetch { item_id: selected }, &[])
        .await
        .unwrap();
    assert_eq!(reply.payload, "  α 1\n ".as_bytes());
    assert!(matches!(
        session
            .execute(share("  α 1\n ".as_bytes(), 1), "  α 1\n ".as_bytes())
            .await
            .unwrap()
            .response,
        Response::Shared {
            duplicate: true,
            ..
        }
    ));
    session
        .execute(Action::Delete { item_id: selected }, &[])
        .await
        .unwrap();
    assert!(matches!(
        session
            .execute(Action::Fetch { item_id: selected }, &[])
            .await,
        Err(SessionError::Remote(ErrorCode::NotFound))
    ));
    assert!(session.is_ready());
    session.execute(Action::Clear, &[]).await.unwrap();
    assert!(session.snapshot().await.unwrap().items.is_empty());
    session.close().await.unwrap();
    task.await.unwrap().unwrap();
}

#[tokio::test]
async fn oversized_local_requests_do_not_poison_a_valid_session() {
    let (mut session, task) = connected().await;
    assert!(matches!(
        session
            .execute(share(&[b'x'; 1025], 1), &[b'x'; 1025])
            .await,
        Err(SessionError::Local)
    ));
    assert!(session.is_ready());
    assert!(!session.mutation_uncertain());
    session.execute(Action::Health, &[]).await.unwrap();
    session.close().await.unwrap();
    task.await.unwrap().unwrap();
}

#[tokio::test]
async fn wrong_item_ids_are_rejected_before_waiting_for_the_payload() {
    let (client, mut server) = tokio::io::duplex(4096);
    let task = tokio::spawn(async move {
        let welcome = fake_welcome(&mut server, policy()).await;
        framing::write_message(&mut server, &welcome, &[], FrameLimits::bootstrap())
            .await
            .unwrap();
        let request = request(&mut server).await;
        let item = Item {
            label: "Synthetic".into(),
            id: Uuid::from_u128(3),
            sequence: 1.into(),
            origin: None,
            created_at_ms: None,
            received_at_ms: 100.into(),
            expires_at_ms: 200.into(),
            representations: vec![Representation {
                id: 0,
                content_type: ContentType::Text,
                encoded_bytes: 1000,
                dimensions: None,
                decoded_bytes: None,
            }],
        };
        let response = Response::Value {
            request_id: request.request_id(),
            epoch: Uuid::from_u128(1),
            sequence: 1.into(),
            item,
            representation_id: 0,
        };
        let bytes = serde_json::to_vec(&response).unwrap();
        server.write_all(b"SCBS\x01\0\0\0").await.unwrap();
        server
            .write_all(&(bytes.len() as u32).to_le_bytes())
            .await
            .unwrap();
        server.write_all(&1000u32.to_le_bytes()).await.unwrap();
        server.write_all(&bytes).await.unwrap();
        // Intentionally no body and no EOF: validation must reject before allocation/read.
        std::future::pending::<()>().await;
    });
    let mut session = Session::connect(client, policy()).await.unwrap();
    assert!(matches!(
        timeout(
            Duration::from_secs(1),
            session.execute(
                Action::Fetch {
                    item_id: Uuid::from_u128(2)
                },
                &[]
            )
        )
        .await
        .unwrap(),
        Err(SessionError::Protocol)
    ));
    assert!(!session.is_ready());
    assert!(matches!(
        session.execute(Action::Health, &[]).await,
        Err(SessionError::Closed)
    ));
    task.abort();
}

#[tokio::test]
async fn cancellation_and_timeout_close_partial_sessions_and_preserve_mutation_uncertainty() {
    for cancel in [false, true] {
        let (client, mut server) = tokio::io::duplex(4096);
        let mut local = policy();
        local.operation_timeout = Duration::from_millis(100);
        let task = tokio::spawn(async move {
            let welcome = fake_welcome(&mut server, local).await;
            framing::write_message(&mut server, &welcome, &[], FrameLimits::bootstrap())
                .await
                .unwrap();
            let _ = request(&mut server).await;
            std::future::pending::<()>().await;
        });
        let mut session = Session::connect(client, local).await.unwrap();
        if cancel {
            assert!(
                timeout(
                    Duration::from_millis(20),
                    session.execute(share(b"x", 1), b"x")
                )
                .await
                .is_err()
            );
        } else {
            assert!(matches!(
                session.execute(share(b"x", 1), b"x").await,
                Err(SessionError::Timeout)
            ));
        }
        assert!(!session.is_ready());
        assert!(session.mutation_uncertain());
        task.abort();
    }
}

#[tokio::test]
async fn fetched_content_is_validated_against_actual_bytes_and_claimed_png_metadata() {
    for png in [false, true] {
        let (client, mut server) = tokio::io::duplex(4096);
        let task = tokio::spawn(async move {
            let welcome = fake_welcome(&mut server, policy()).await;
            framing::write_message(&mut server, &welcome, &[], FrameLimits::bootstrap())
                .await
                .unwrap();
            let request = request(&mut server).await;
            let payload = if png {
                let mut bytes = std::io::Cursor::new(Vec::new());
                image::DynamicImage::new_rgba8(1, 1)
                    .write_to(&mut bytes, image::ImageFormat::Png)
                    .unwrap();
                bytes.into_inner()
            } else {
                vec![255]
            };
            let item = Item {
                label: "Synthetic".into(),
                id: Uuid::from_u128(2),
                sequence: 1.into(),
                origin: None,
                created_at_ms: None,
                received_at_ms: 100.into(),
                expires_at_ms: 200.into(),
                representations: vec![Representation {
                    id: 0,
                    content_type: if png {
                        ContentType::Png
                    } else {
                        ContentType::Text
                    },
                    encoded_bytes: payload.len() as u32,
                    dimensions: png.then_some(wire::Dimensions {
                        width: 2,
                        height: 2,
                    }),
                    decoded_bytes: png.then_some(16),
                }],
            };
            framing::write_message(
                &mut server,
                &Response::Value {
                    request_id: request.request_id(),
                    epoch: Uuid::from_u128(1),
                    sequence: 1.into(),
                    item,
                    representation_id: 0,
                },
                &payload,
                FrameLimits::session(1024, 1024).unwrap(),
            )
            .await
            .unwrap();
        });
        let mut session = Session::connect(client, policy()).await.unwrap();
        assert!(matches!(
            session
                .execute(
                    Action::Fetch {
                        item_id: Uuid::from_u128(2)
                    },
                    &[]
                )
                .await,
            Err(SessionError::Content)
        ));
        assert!(!session.is_ready());
        task.await.unwrap();
    }
}

#[tokio::test]
async fn epoch_changes_require_reconnection_and_bootstrap_cannot_raise_client_limits() {
    for raised in [false, true] {
        let (client, mut server) = tokio::io::duplex(4096);
        let task = tokio::spawn(async move {
            let mut welcome = fake_welcome(&mut server, policy()).await;
            if raised && let Response::Welcome { limits, .. } = &mut welcome {
                limits.max_item_bytes = 2048;
            }
            framing::write_message(&mut server, &welcome, &[], FrameLimits::bootstrap())
                .await
                .unwrap();
            if !raised {
                let request = request(&mut server).await;
                framing::write_message(
                    &mut server,
                    &Response::Error {
                        request_id: request.request_id(),
                        code: ErrorCode::EpochChanged,
                        epoch: Some(Uuid::from_u128(2)),
                    },
                    &[],
                    FrameLimits::session(1024, 1024).unwrap(),
                )
                .await
                .unwrap();
            }
        });
        let session = Session::connect(client, policy()).await;
        if raised {
            assert!(matches!(session, Err(SessionError::Protocol)));
        } else {
            let mut session = session.unwrap();
            assert!(matches!(
                session.execute(Action::Health, &[]).await,
                Err(SessionError::Remote(ErrorCode::EpochChanged))
            ));
            assert!(!session.is_ready());
        }
        task.await.unwrap();
    }
}
