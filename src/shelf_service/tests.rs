use super::*;
use crate::session_protocol::{
    ContentType, Dimensions, Number, Origin as WireOrigin, Representation,
};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, DuplexStream};
use tokio::task::JoinHandle;

fn service(limits: ServiceLimits) -> Arc<ShelfService> {
    Arc::new(
        ShelfService::new(
            ShelfLimits {
                max_items: 32,
                max_item_bytes: 1024,
                max_stored_bytes: 8192,
                max_resident_bytes: 16384,
                max_decoded_bytes: 1024,
                max_retry_records: 64,
                ..Default::default()
            },
            limits,
        )
        .unwrap(),
    )
}

#[tokio::test]
async fn expired_retry_deadlines_cannot_recreate_a_removed_item_after_dedup_expiry() {
    let service = Arc::new(
        ShelfService::new(
            ShelfLimits {
                retry_retention: Duration::from_millis(10),
                ..ShelfLimits::default()
            },
            ServiceLimits::default(),
        )
        .unwrap(),
    );
    let (mut peer, task) = Peer::open(service.clone(), 1024).await;
    let operation = Uuid::new_v4();
    let request = Request::OperationStatus {
        request_id: peer.id(),
        epoch: peer.epoch,
        device_id: Uuid::from_u128(7),
        operation_id: operation,
    };
    let Response::Operation { epoch_time_ms, .. } = peer.exchange(request, &[]).await.0 else {
        panic!("no clock");
    };
    let deadline = (epoch_time_ms.get() + 10).into();
    let request = peer.share(operation, b"frozen");
    peer.exchange(request, b"frozen").await;
    let request = Request::Clear {
        request_id: peer.id(),
        epoch: peer.epoch,
    };
    peer.exchange(request, &[]).await;
    tokio::time::sleep(Duration::from_millis(20)).await;
    let inner = peer.share(operation, b"frozen");
    let reply = peer
        .exchange(
            Request::RetryShare {
                share: Box::new(inner),
                not_after_ms: deadline,
            },
            b"frozen",
        )
        .await
        .0;
    assert!(matches!(
        reply,
        Response::Error {
            code: ErrorCode::RetryExpired,
            ..
        }
    ));
    let request = Request::List {
        request_id: peer.id(),
        epoch: peer.epoch,
        offset: 0,
        limit: 16,
        expected_sequence: None,
    };
    assert!(matches!(
        peer.exchange(request, &[]).await.0,
        Response::Listed { total: 0, .. }
    ));
    assert_eq!(service.resident_bytes(), 0);
    drop(peer);
    task.await.unwrap().unwrap();
}

#[tokio::test]
async fn watch_wakes_on_mutation_coalesces_gaps_and_retains_request_association() {
    let service = service(ServiceLimits::default());
    let (mut observer, observer_task) = Peer::open(service.clone(), 1024).await;
    let (mut writer, writer_task) = Peer::open(service.clone(), 1024).await;
    let request = Request::Watch {
        request_id: observer.id(),
        epoch: observer.epoch,
        after_sequence: 0.into(),
        wait_ms: 5000,
    };
    let watching = tokio::spawn(async move {
        let response = observer.exchange(request, &[]).await;
        (observer, response)
    });
    // A writer must remain available while the watcher waits. Multiple updates may
    // coalesce; the subsequent complete snapshot is the convergence boundary.
    for value in [b"one", b"two", b"end"] {
        let request = writer.share(Uuid::new_v4(), value);
        writer.exchange(request, value).await;
    }
    let (mut observer, (response, bytes)) = timeout(Duration::from_secs(2), watching)
        .await
        .unwrap()
        .unwrap();
    assert!(bytes.is_empty());
    assert!(
        matches!(response,Response::Changed{request_id,sequence,..} if request_id.get()==2 && sequence.get()>=1)
    );
    let request = Request::Watch {
        request_id: observer.id(),
        epoch: observer.epoch,
        after_sequence: 0.into(),
        wait_ms: 5000,
    };
    let (response, _) = timeout(Duration::from_secs(1), observer.exchange(request, &[]))
        .await
        .unwrap();
    assert!(matches!(response,Response::Changed{sequence,..} if sequence.get()==3));
    let request = Request::Watch {
        request_id: observer.id(),
        epoch: observer.epoch,
        after_sequence: 99.into(),
        wait_ms: 0,
    };
    assert!(matches!(
        observer.exchange(request, &[]).await.0,
        Response::Error {
            code: ErrorCode::ResyncRequired,
            ..
        }
    ));
    drop(observer);
    drop(writer);
    observer_task.await.unwrap().unwrap();
    writer_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn watch_heartbeat_is_bounded_and_expiry_notifies_without_payload_access() {
    let service = Arc::new(
        ShelfService::new(
            ShelfLimits {
                retention: Duration::from_millis(30),
                ..ShelfLimits::default()
            },
            ServiceLimits::default(),
        )
        .unwrap(),
    );
    let (mut writer, writer_task) = Peer::open(service.clone(), 1024).await;
    let request = writer.share(Uuid::new_v4(), b"expires");
    writer.exchange(request, b"expires").await;
    let (mut observer, observer_task) = Peer::open(service.clone(), 1024).await;
    let request = Request::Watch {
        request_id: observer.id(),
        epoch: observer.epoch,
        after_sequence: 1.into(),
        wait_ms: 50,
    };
    let (response, bytes) = observer.exchange(request, &[]).await;
    assert!(bytes.is_empty());
    assert!(matches!(response,Response::Changed{sequence,..} if sequence.get()==2));
    let request = Request::Watch {
        request_id: observer.id(),
        epoch: observer.epoch,
        after_sequence: 2.into(),
        wait_ms: 5,
    };
    assert!(
        matches!(timeout(Duration::from_secs(1),observer.exchange(request,&[])).await.unwrap().0,Response::Changed{sequence,..} if sequence.get()==2)
    );
    drop(writer);
    drop(observer);
    writer_task.await.unwrap().unwrap();
    observer_task.await.unwrap().unwrap();
}
struct Peer {
    stream: DuplexStream,
    epoch: Uuid,
    limits: FrameLimits,
    next_id: u64,
}
impl Peer {
    async fn open(
        service: Arc<ShelfService>,
        max_decoded: u32,
    ) -> (Self, JoinHandle<Result<(), WireError>>) {
        let (mut client, server) = tokio::io::duplex(64);
        let task = tokio::spawn(async move { service.serve(server).await });
        let hello = Request::Hello {
            request_id: 1.into(),
            versions: vec![1],
            max_item_bytes: 1024,
            max_decoded_bytes: max_decoded,
        };
        framing::write_message(&mut client, &hello, &[], FrameLimits::bootstrap())
            .await
            .unwrap();
        let header = framing::read_header(&mut client, FrameLimits::bootstrap())
            .await
            .unwrap();
        let welcome: Response = framing::read_metadata(&mut client, &header).await.unwrap();
        welcome.validate(0).unwrap();
        let Response::Welcome { epoch, limits, .. } = welcome else {
            panic!("not welcomed")
        };
        (
            Self {
                stream: client,
                epoch,
                limits: limits.frame_limits().unwrap(),
                next_id: 2,
            },
            task,
        )
    }
    fn id(&mut self) -> Number {
        let id = self.next_id;
        self.next_id += 1;
        id.into()
    }
    async fn exchange(&mut self, request: Request, data: &[u8]) -> (Response, Vec<u8>) {
        let id = request.request_id();
        request.validate(data.len()).unwrap();
        framing::write_message(&mut self.stream, &request, data, self.limits)
            .await
            .unwrap();
        let header = framing::read_header(&mut self.stream, self.limits)
            .await
            .unwrap();
        let response: Response = framing::read_metadata(&mut self.stream, &header)
            .await
            .unwrap();
        response.validate(header.payload_bytes()).unwrap();
        assert_eq!(response.request_id(), id);
        let mut payload = vec![0; header.payload_bytes()];
        self.stream.read_exact(&mut payload).await.unwrap();
        (response, payload)
    }
    fn share(&mut self, operation: Uuid, data: &[u8]) -> Request {
        Request::Share {
            request_id: self.id(),
            epoch: self.epoch,
            operation_id: operation,
            origin: WireOrigin {
                device_id: Uuid::from_u128(7),
                name: "Synthetic device".into(),
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
    async fn close(mut self, task: JoinHandle<Result<(), WireError>>) {
        self.stream.shutdown().await.unwrap();
        timeout(Duration::from_secs(2), task)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
    }
}

#[tokio::test]
async fn multiple_requests_share_list_fetch_delete_and_clear_exact_bytes() {
    let service = service(ServiceLimits::default());
    let (mut peer, task) = Peer::open(service.clone(), 1024).await;
    let text = " α\n  ".as_bytes();
    let request = peer.share(Uuid::from_u128(1), text);
    let (response, _) = peer.exchange(request, text).await;
    let Response::Shared {
        item, duplicate, ..
    } = response
    else {
        panic!("not shared")
    };
    assert!(!duplicate);
    let request = Request::List {
        request_id: peer.id(),
        epoch: peer.epoch,
        offset: 0,
        limit: 16,
        expected_sequence: None,
    };
    let (response, payload) = peer.exchange(request, &[]).await;
    assert!(payload.is_empty());
    let Response::Listed {
        items,
        total,
        next_offset,
        ..
    } = response
    else {
        panic!("not listed")
    };
    assert_eq!(items.as_slice(), std::slice::from_ref(&item));
    assert_eq!(total, 1);
    assert_eq!(next_offset, None);
    let request = Request::Fetch {
        request_id: peer.id(),
        epoch: peer.epoch,
        item_id: item.id,
        representation_id: 0,
    };
    assert_eq!(peer.exchange(request, &[]).await.1, text);
    let request = Request::Delete {
        request_id: peer.id(),
        epoch: peer.epoch,
        item_id: item.id,
    };
    assert!(matches!(
        peer.exchange(request, &[]).await.0,
        Response::Deleted { removed: true, .. }
    ));
    let request = peer.share(Uuid::from_u128(1), text);
    assert!(matches!(
        peer.exchange(request, text).await.0,
        Response::Shared {
            duplicate: true,
            available: false,
            ..
        }
    ));
    let request = peer.share(Uuid::from_u128(2), b"next");
    peer.exchange(request, b"next").await;
    let request = Request::Clear {
        request_id: peer.id(),
        epoch: peer.epoch,
    };
    peer.exchange(request, &[]).await;
    let request = Request::Fetch {
        request_id: peer.id(),
        epoch: peer.epoch,
        item_id: item.id,
        representation_id: 0,
    };
    assert!(matches!(
        peer.exchange(request, &[]).await.0,
        Response::Error {
            code: ErrorCode::NotFound,
            ..
        }
    ));
    assert_eq!(service.resident_bytes(), 0);
    peer.close(task).await;
}

#[tokio::test]
async fn simultaneous_identical_uploads_commit_once_across_connections() {
    let service = service(ServiceLimits::default());
    let (mut first, first_task) = Peer::open(service.clone(), 1024).await;
    let (mut second, second_task) = Peer::open(service.clone(), 1024).await;
    assert_eq!(first.epoch, second.epoch);
    let operation = Uuid::from_u128(1);
    let a = first.share(operation, b"same");
    let b = second.share(operation, b"same");
    let (a, b) = tokio::join!(first.exchange(a, b"same"), second.exchange(b, b"same"));
    let Response::Shared {
        item: a_item,
        duplicate: a_duplicate,
        sequence: a_sequence,
        ..
    } = a.0
    else {
        panic!("not shared")
    };
    let Response::Shared {
        item: b_item,
        duplicate: b_duplicate,
        sequence: b_sequence,
        ..
    } = b.0
    else {
        panic!("not shared")
    };
    assert_eq!(a_item, b_item);
    assert_ne!(a_duplicate, b_duplicate);
    assert_eq!(a_sequence.get(), 1);
    assert_eq!(b_sequence.get(), 1);
    assert_eq!(service.resident_bytes(), 4);
    first.close(first_task).await;
    second.close(second_task).await;
}

#[tokio::test]
async fn pagination_refuses_mixed_sequences_and_unanchored_later_pages() {
    let service = service(ServiceLimits::default());
    let (mut peer, task) = Peer::open(service.clone(), 1024).await;
    for id in 1..=3 {
        let request = peer.share(Uuid::from_u128(id), b"x");
        peer.exchange(request, b"x").await;
    }
    let request = Request::List {
        request_id: peer.id(),
        epoch: peer.epoch,
        offset: 0,
        limit: 1,
        expected_sequence: None,
    };
    let Response::Listed {
        sequence,
        total,
        next_offset,
        ..
    } = peer.exchange(request, &[]).await.0
    else {
        panic!("not listed")
    };
    assert_eq!(total, 3);
    assert_eq!(next_offset, Some(1));
    let request = peer.share(Uuid::from_u128(4), b"x");
    peer.exchange(request, b"x").await;
    for expected_sequence in [Some(sequence), None] {
        let request = Request::List {
            request_id: peer.id(),
            epoch: peer.epoch,
            offset: 1,
            limit: 1,
            expected_sequence,
        };
        let response = peer.exchange(request, &[]).await.0;
        assert!(matches!(
            response,
            Response::Error {
                code: ErrorCode::ResyncRequired | ErrorCode::InvalidRequest,
                ..
            }
        ));
    }
    peer.close(task).await;
}

#[tokio::test]
async fn wrong_epochs_and_invalid_text_cannot_change_the_shelf() {
    let service = service(ServiceLimits::default());
    let (mut peer, task) = Peer::open(service.clone(), 1024).await;
    let mut request = peer.share(Uuid::from_u128(1), &[255]);
    if let Request::Share { epoch, .. } = &mut request {
        *epoch = Uuid::from_u128(42);
    }
    assert!(matches!(
        peer.exchange(request, &[255]).await.0,
        Response::Error {
            code: ErrorCode::EpochChanged,
            ..
        }
    ));
    let request = peer.share(Uuid::from_u128(1), &[255]);
    assert!(matches!(
        peer.exchange(request, &[255]).await.0,
        Response::Error {
            code: ErrorCode::InvalidContent,
            ..
        }
    ));
    let request = Request::Health {
        request_id: peer.id(),
        epoch: peer.epoch,
    };
    assert!(matches!(
        peer.exchange(request, &[]).await.0,
        Response::Health {
            items: 0,
            stored_bytes: 0,
            ..
        }
    ));
    assert_eq!(service.resident_bytes(), 0);
    peer.close(task).await;
}

#[tokio::test]
async fn admission_and_handshake_deadlines_release_connection_slots() {
    let service = service(ServiceLimits {
        max_connections: 1,
        handshake_timeout: Duration::from_millis(100),
        ..Default::default()
    });
    let (peer, task) = Peer::open(service.clone(), 1024).await;
    let (_client, server) = tokio::io::duplex(64);
    assert!(matches!(
        service.serve(server).await,
        Err(WireError::Resource(ShelfError::Busy))
    ));
    peer.close(task).await;
    let (_client, server) = tokio::io::duplex(64);
    assert!(
        matches!(timeout(Duration::from_secs(2), service.serve(server)).await.unwrap(), Err(WireError::Io(error)) if error.kind() == std::io::ErrorKind::TimedOut)
    );
    let (peer, task) = Peer::open(service.clone(), 1024).await;
    peer.close(task).await;
}

#[tokio::test]
async fn incomplete_upload_times_out_and_releases_reserved_bytes() {
    let service = service(ServiceLimits {
        operation_timeout: Duration::from_millis(60),
        ..Default::default()
    });
    let (mut peer, task) = Peer::open(service.clone(), 1024).await;
    let request = peer.share(Uuid::from_u128(1), b"test");
    let mut frame = Vec::new();
    framing::write_message(&mut frame, &request, b"test", peer.limits)
        .await
        .unwrap();
    peer.stream
        .write_all(&frame[..frame.len() - 2])
        .await
        .unwrap();
    let result = timeout(Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap();
    assert!(
        matches!(result, Err(WireError::Io(error)) if error.kind() == std::io::ErrorKind::TimedOut)
    );
    assert_eq!(service.resident_bytes(), 0);
}

#[tokio::test]
async fn rejected_versions_get_a_correlated_error_without_opening_a_ready_session() {
    let service = service(ServiceLimits::default());
    let (mut client, server) = tokio::io::duplex(64);
    let task = tokio::spawn(async move { service.serve(server).await });
    let hello = Request::Hello {
        request_id: 19.into(),
        versions: vec![9],
        max_item_bytes: 1,
        max_decoded_bytes: 1,
    };
    framing::write_message(&mut client, &hello, &[], FrameLimits::bootstrap())
        .await
        .unwrap();
    let header = framing::read_header(&mut client, FrameLimits::bootstrap())
        .await
        .unwrap();
    let response: Response = framing::read_metadata(&mut client, &header).await.unwrap();
    assert!(
        matches!(response, Response::Error {request_id, code: ErrorCode::UnsupportedVersion, ..} if request_id.get() == 19)
    );
    timeout(Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn png_descriptors_are_derived_and_native_decode_size_controls_fetch_admission() {
    use image::ImageEncoder;
    let service = service(ServiceLimits::default());
    let (mut peer, task) = Peer::open(service.clone(), 1024).await;
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png)
        .write_image(&[0; 18], 3, 1, image::ExtendedColorType::Rgb16)
        .unwrap();
    let mut request = peer.share(Uuid::from_u128(1), &png);
    if let Request::Share { representation, .. } = &mut request {
        representation.content_type = ContentType::Png;
        representation.dimensions = Some(Dimensions {
            width: 1,
            height: 1,
        });
    }
    assert!(matches!(
        peer.exchange(request, &png).await.0,
        Response::Error {
            code: ErrorCode::InvalidContent,
            ..
        }
    ));
    let mut request = peer.share(Uuid::from_u128(1), &png);
    if let Request::Share { representation, .. } = &mut request {
        representation.content_type = ContentType::Png;
    }
    let Response::Shared { item, .. } = peer.exchange(request, &png).await.0 else {
        panic!("not shared")
    };
    assert_eq!(item.representations[0].decoded_bytes, Some(18));
    assert_eq!(
        item.representations[0].dimensions,
        Some(Dimensions {
            width: 3,
            height: 1
        })
    );
    let (mut small, small_task) = Peer::open(service.clone(), 16).await;
    let request = Request::Fetch {
        request_id: small.id(),
        epoch: small.epoch,
        item_id: item.id,
        representation_id: 0,
    };
    assert!(matches!(
        small.exchange(request, &[]).await.0,
        Response::Error {
            code: ErrorCode::PayloadTooLarge,
            ..
        }
    ));
    small.close(small_task).await;
    peer.close(task).await;
}

#[tokio::test]
async fn busy_uploads_are_drained_with_bounded_memory_and_can_retry_the_same_capture() {
    let service = service(ServiceLimits::default());
    let held: Vec<_> = (0..16)
        .map(|_| service.budget.allocate(1024).unwrap())
        .collect();
    let (mut peer, task) = Peer::open(service.clone(), 1024).await;
    let bytes = [b'x'; 1024];
    let operation = Uuid::from_u128(1);
    let request = peer.share(operation, &bytes);
    assert!(matches!(
        peer.exchange(request, &bytes).await.0,
        Response::Error {
            code: ErrorCode::Busy,
            ..
        }
    ));
    assert_eq!(service.resident_bytes(), 16384);
    drop(held);
    assert_eq!(service.resident_bytes(), 0);
    let request = peer.share(operation, &bytes);
    assert!(matches!(
        peer.exchange(request, &bytes).await.0,
        Response::Shared {
            duplicate: false,
            ..
        }
    ));
    peer.close(task).await;
}

#[tokio::test]
async fn blocked_fetch_writes_release_pinned_expired_bytes_at_the_deadline() {
    let service = Arc::new(
        ShelfService::new(
            ShelfLimits {
                max_item_bytes: 1024,
                max_stored_bytes: 2048,
                max_resident_bytes: 3072,
                retention: Duration::from_millis(200),
                ..Default::default()
            },
            ServiceLimits {
                operation_timeout: Duration::from_secs(1),
                ..Default::default()
            },
        )
        .unwrap(),
    );
    let (mut peer, task) = Peer::open(service.clone(), 1024).await;
    let bytes = [b'x'; 512];
    let request = peer.share(Uuid::from_u128(1), &bytes);
    let Response::Shared { item, .. } = peer.exchange(request, &bytes).await.0 else {
        panic!("not shared")
    };
    let request = Request::Fetch {
        request_id: peer.id(),
        epoch: peer.epoch,
        item_id: item.id,
        representation_id: 0,
    };
    framing::write_message(&mut peer.stream, &request, &[], peer.limits)
        .await
        .unwrap();
    let header = framing::read_header(&mut peer.stream, peer.limits)
        .await
        .unwrap();
    let response: Response = framing::read_metadata(&mut peer.stream, &header)
        .await
        .unwrap();
    assert!(matches!(response, Response::Value { .. }));
    assert_eq!(header.payload_bytes(), 512);
    // Leave the 512-byte body unread in a 64-byte stream, pinning the fetched lease.
    tokio::time::sleep(Duration::from_millis(250)).await;
    service.maintenance().unwrap();
    assert_eq!(service.resident_bytes(), 512);
    assert!(
        matches!(timeout(Duration::from_secs(2), task).await.unwrap().unwrap(), Err(WireError::Io(error)) if error.kind() == std::io::ErrorKind::TimedOut)
    );
    assert_eq!(service.resident_bytes(), 0);
}
