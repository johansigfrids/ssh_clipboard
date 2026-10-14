use super::framing::*;
use super::*;
use crate::shelf::{Now, Shelf};

const HELLO: &[u8] = br#"{"type":"hello","request_id":"1","versions":[1],"max_item_bytes":67108864,"max_decoded_bytes":268435456}"#;

fn hello() -> Request {
    Request::Hello {
        request_id: 1.into(),
        versions: vec![1],
        max_item_bytes: 64 * 1024 * 1024,
        max_decoded_bytes: 256 * 1024 * 1024,
    }
}
fn share(bytes: usize) -> Request {
    Request::Share {
        request_id: 2.into(),
        epoch: Uuid::from_u128(1),
        operation_id: Uuid::from_u128(2),
        origin: Origin {
            device_id: Uuid::from_u128(3),
            name: "Laptop".into(),
        },
        representation: Representation {
            id: 0,
            content_type: ContentType::Text,
            encoded_bytes: bytes as u32,
            dimensions: None,
            decoded_bytes: None,
        },
        created_at_ms: Some(i64::MIN.into()),
    }
}
fn shelf() -> Shelf {
    Shelf::new(
        ShelfLimits {
            max_items: 2,
            max_item_bytes: 4,
            max_stored_bytes: 8,
            max_resident_bytes: 12,
            ..Default::default()
        },
        Now::system().unwrap(),
    )
    .unwrap()
}

#[tokio::test]
async fn hello_wire_fixture_freezes_the_independent_bootstrap_and_json() {
    let mut frame = Vec::new();
    write_message(&mut frame, &hello(), &[], FrameLimits::bootstrap())
        .await
        .unwrap();
    assert_eq!(
        &frame[..16],
        &[83, 67, 66, 83, 1, 0, 0, 0, 104, 0, 0, 0, 0, 0, 0, 0]
    );
    assert_eq!(&frame[16..], HELLO);
    assert_eq!(read_hello(&mut frame.as_slice()).await.unwrap(), hello());
    assert_ne!(MAGIC, crate::protocol::MAGIC);
}

#[test]
fn decimal_fields_preserve_full_precision_and_reject_noncanonical_forms() {
    for value in [0, 1, (1u64 << 53) + 1, u64::MAX] {
        let encoded = serde_json::to_string(&Number::from(value)).unwrap();
        assert_eq!(
            serde_json::from_str::<Number>(&encoded).unwrap().get(),
            value
        );
        assert_eq!(encoded, format!("\"{value}\""));
    }
    for value in [i64::MIN, -1, 0, i64::MAX] {
        let encoded = serde_json::to_string(&Timestamp::from(value)).unwrap();
        assert_eq!(
            serde_json::from_str::<Timestamp>(&encoded).unwrap().get(),
            value
        );
    }
    for bad in [
        "1",
        "1.5",
        "\"01\"",
        "\"+1\"",
        "\"-1\"",
        "\"18446744073709551616\"",
        "\" 1\"",
    ] {
        assert!(serde_json::from_str::<Number>(bad).is_err());
    }
    assert!(serde_json::from_str::<Timestamp>("\"-0\"").is_err());
}

#[test]
fn negotiation_intersects_policy_and_never_advertises_pending_capabilities() {
    let request = Request::Hello {
        request_id: u64::MAX.into(),
        versions: vec![3, 1],
        max_item_bytes: 1024,
        max_decoded_bytes: 2048,
    };
    let reply = negotiate(
        &request,
        Uuid::from_u128(1),
        u64::MAX,
        &ShelfLimits::default(),
    )
    .unwrap();
    let Response::Welcome {
        request_id,
        sequence,
        session_version,
        capabilities,
        limits,
        ..
    } = reply
    else {
        panic!("wrong reply")
    };
    assert_eq!(request_id.get(), u64::MAX);
    assert_eq!(sequence.get(), u64::MAX);
    assert_eq!(session_version, 1);
    assert_eq!(limits.max_item_bytes, 1024);
    assert_eq!(limits.max_decoded_bytes, 2048);
    assert_eq!(limits.max_metadata_bytes, 65536);
    assert_eq!(limits.max_list_items, 16);
    assert_eq!(
        capabilities,
        [
            "list",
            "fetch",
            "delete",
            "clear",
            "deduplicated_share",
            "watch"
        ]
    );
    let unsupported = Request::Hello {
        request_id: 1.into(),
        versions: vec![7],
        max_item_bytes: 1,
        max_decoded_bytes: 1,
    };
    assert!(matches!(
        negotiate(&unsupported, Uuid::from_u128(1), 0, &ShelfLimits::default()),
        Err(WireError::Rejected(ErrorCode::UnsupportedVersion))
    ));
    for versions in [vec![], vec![1, 1], vec![1; 9]] {
        let request = Request::Hello {
            request_id: 1.into(),
            versions,
            max_item_bytes: 1,
            max_decoded_bytes: 1,
        };
        assert!(negotiate(&request, Uuid::from_u128(1), 0, &ShelfLimits::default()).is_err());
    }
}

#[tokio::test]
async fn metadata_space_does_not_reduce_the_exact_payload_budget() {
    let shelf = shelf();
    let budget = shelf.upload_budget();
    let limits = FrameLimits::session(4, 1024).unwrap();
    let mut frame = Vec::new();
    write_message(&mut frame, &share(4), b"test", limits)
        .await
        .unwrap();
    assert!(frame.len() > 4 + 16);
    let received = read_request(&mut frame.as_slice(), limits, &budget)
        .await
        .unwrap();
    assert_eq!(received.request, share(4));
    assert!(received.upload.is_some());
    assert_eq!(budget.resident_bytes(), 4);
    let validated = received
        .upload
        .unwrap()
        .validate(shelf::ContentType::Text, None, None, None)
        .unwrap();
    drop(validated);
    assert_eq!(budget.resident_bytes(), 0);
}

#[tokio::test]
async fn forged_headers_and_metadata_fail_before_upload_allocation() {
    let mut frame = Vec::new();
    write_message(&mut frame, &hello(), &[], FrameLimits::bootstrap())
        .await
        .unwrap();
    for (range, value) in [(8..12, u32::MAX), (12..16, u32::MAX)] {
        let mut forged = frame.clone();
        forged[range].copy_from_slice(&value.to_le_bytes());
        assert!(read_hello(&mut forged.as_slice()).await.is_err());
    }
    for (index, value) in [(0, b'X'), (4, 2), (6, 1)] {
        let mut forged = frame.clone();
        forged[index] = value;
        assert!(read_hello(&mut forged.as_slice()).await.is_err());
    }
    let shelf = shelf();
    let budget = shelf.upload_budget();
    let limits = FrameLimits::session(4, 1024).unwrap();
    let mut frame = Vec::new();
    write_message(&mut frame, &share(3), b"test", limits)
        .await
        .unwrap();
    assert!(matches!(
        read_request(&mut frame.as_slice(), limits, &budget).await,
        Err(WireError::PayloadMismatch)
    ));
    assert_eq!(budget.resident_bytes(), 0);
    let request = Request::Health {
        request_id: 3.into(),
        epoch: Uuid::from_u128(1),
    };
    let mut frame = Vec::new();
    write_message(&mut frame, &request, b"test", limits)
        .await
        .unwrap();
    assert!(matches!(
        read_request(&mut frame.as_slice(), limits, &budget).await,
        Err(WireError::PayloadMismatch)
    ));
    assert_eq!(budget.resident_bytes(), 0);
}

#[tokio::test]
async fn truncated_uploads_release_the_reservation_and_errors_do_not_echo_metadata() {
    let shelf = shelf();
    let budget = shelf.upload_budget();
    let limits = FrameLimits::session(4, 1024).unwrap();
    let mut frame = Vec::new();
    write_message(&mut frame, &share(4), b"test", limits)
        .await
        .unwrap();
    frame.pop();
    assert!(matches!(
        read_request(&mut frame.as_slice(), limits, &budget).await,
        Err(WireError::Io(_))
    ));
    assert_eq!(budget.resident_bytes(), 0);
    let mut frame = Vec::new();
    write_message(
        &mut frame,
        &serde_json::json!({"type": "PRIVATE_INVALID_TYPE", "request_id":"1"}),
        &[],
        FrameLimits::bootstrap(),
    )
    .await
    .unwrap();
    let error = read_hello(&mut frame.as_slice()).await.err().unwrap();
    assert!(!error.to_string().contains("PRIVATE"));
    assert!(matches!(error, WireError::InvalidMetadata));
}

#[tokio::test]
async fn startup_and_ready_helpers_reject_the_wrong_message_state() {
    let limits = FrameLimits::session(4, 1024).unwrap();
    let shelf = shelf();
    let request = Request::Health {
        request_id: 3.into(),
        epoch: Uuid::from_u128(1),
    };
    let mut frame = Vec::new();
    write_message(&mut frame, &request, &[], FrameLimits::bootstrap())
        .await
        .unwrap();
    assert!(matches!(
        read_hello(&mut frame.as_slice()).await,
        Err(WireError::InvalidState)
    ));
    let mut frame = Vec::new();
    write_message(&mut frame, &hello(), &[], limits)
        .await
        .unwrap();
    assert!(matches!(
        read_request(&mut frame.as_slice(), limits, &shelf.upload_budget()).await,
        Err(WireError::InvalidState)
    ));
    for raw in [br#"{"type":"hello","request_id":1,"versions":[1],"max_item_bytes":1,"max_decoded_bytes":1}"#.as_slice(),
        br#"{"type":"hello","request_id":"1","versions":[1],"max_item_bytes":1,"max_decoded_bytes":1,"unknown":true}"#] {
        assert!(serde_json::from_slice::<Request>(raw).is_err());
    }
}

#[tokio::test]
async fn oversized_metadata_and_payload_are_rejected_before_any_output() {
    let mut output = Vec::new();
    let huge = serde_json::json!({"type": "PRIVATE".repeat(10000)});
    assert!(matches!(
        write_message(&mut output, &huge, &[], FrameLimits::bootstrap()).await,
        Err(WireError::MetadataTooLarge)
    ));
    assert!(output.is_empty());
    assert!(matches!(
        write_message(&mut output, &hello(), b"x", FrameLimits::bootstrap()).await,
        Err(WireError::PayloadTooLarge)
    ));
    assert!(output.is_empty());
}

#[test]
fn response_payloads_and_list_page_boundaries_are_checked() {
    let mut shelf = shelf();
    let now = Now::system().unwrap();
    let mut input = shelf.upload_budget().allocate(4).unwrap();
    input.bytes_mut().copy_from_slice(b"test");
    let prepared = input
        .validate(shelf::ContentType::Text, None, None, None)
        .unwrap();
    let committed = shelf.commit(shelf.epoch(), prepared, now).unwrap();
    let item = Item::try_from(&committed.metadata).unwrap();
    let response = Response::Value {
        request_id: 1.into(),
        epoch: shelf.epoch(),
        sequence: 1.into(),
        item: item.clone(),
        representation_id: 0,
    };
    assert!(response.validate(4).is_ok());
    assert!(matches!(
        response.validate(3),
        Err(WireError::PayloadMismatch)
    ));
    let page = |offset, total, next_offset| Response::Listed {
        request_id: 1.into(),
        epoch: shelf.epoch(),
        sequence: 1.into(),
        items: vec![item.clone()],
        offset,
        total,
        next_offset,
    };
    assert!(page(0, 2, Some(1)).validate(0).is_ok());
    assert!(page(1, 2, None).validate(0).is_ok());
    for response in [page(0, 2, None), page(2, 2, None), page(0, 2, Some(2))] {
        assert!(response.validate(0).is_err());
    }
}

#[tokio::test]
async fn negotiated_decode_budget_reaches_the_actual_png_validator() {
    use image::ImageEncoder;
    let mut bytes = Vec::new();
    image::codecs::png::PngEncoder::new(&mut bytes)
        .write_image(&[127; 16], 2, 2, image::ExtendedColorType::Rgba8)
        .unwrap();
    let shelf = Shelf::new(ShelfLimits::default(), Now::system().unwrap()).unwrap();
    let mut request = share(bytes.len());
    if let Request::Share { representation, .. } = &mut request {
        representation.content_type = ContentType::Png;
    }
    let limits = FrameLimits::session(1024, 8).unwrap();
    let mut frame = Vec::new();
    write_message(&mut frame, &request, &bytes, limits)
        .await
        .unwrap();
    let received = read_request(&mut frame.as_slice(), limits, &shelf.upload_budget())
        .await
        .unwrap();
    assert!(matches!(
        received
            .upload
            .unwrap()
            .validate(shelf::ContentType::Png, None, None, None),
        Err(shelf::ShelfError::InvalidImage)
    ));
    assert_eq!(shelf.upload_budget().resident_bytes(), 0);
}

#[tokio::test]
async fn a_full_page_with_maximum_names_and_integers_fits_the_metadata_budget() {
    let epoch = Uuid::from_u128(1);
    let items: Vec<_> = (0..16)
        .map(|index| Item {
            label: "世".repeat(80),
            id: Uuid::from_u128(index + 1),
            sequence: (u64::MAX - index as u64).into(),
            origin: Some(Origin {
                device_id: Uuid::from_u128(1),
                name: "\\\"".repeat(128),
            }),
            created_at_ms: Some(i64::MIN.into()),
            received_at_ms: i64::MAX.into(),
            expires_at_ms: i64::MAX.into(),
            representations: vec![Representation {
                id: 0,
                content_type: ContentType::Png,
                encoded_bytes: MAX_PAYLOAD_BYTES as u32,
                dimensions: Some(Dimensions {
                    width: 8192,
                    height: 8192,
                }),
                decoded_bytes: Some(MAX_PAYLOAD_BYTES as u32),
            }],
        })
        .collect();
    let reply = Response::Listed {
        request_id: u64::MAX.into(),
        epoch,
        sequence: u64::MAX.into(),
        items: items.clone(),
        offset: 0,
        total: 1024,
        next_offset: Some(16),
    };
    reply.validate(0).unwrap();
    let mut frame = Vec::new();
    let limits = FrameLimits::session(MAX_PAYLOAD_BYTES, MAX_PAYLOAD_BYTES).unwrap();
    write_message(&mut frame, &reply, &[], limits)
        .await
        .unwrap();
    assert!(frame.len() < MAX_METADATA_BYTES);
    let header = read_header(&mut frame.as_slice(), limits).await.unwrap();
    assert_eq!(header.payload_bytes(), 0);
    let mut duplicate = reply.clone();
    if let Response::Listed { items, .. } = &mut duplicate {
        items[1] = items[0].clone();
    }
    assert!(duplicate.validate(0).is_err());
    let mut wrong_order = reply;
    if let Response::Listed { items, .. } = &mut wrong_order {
        items.swap(0, 1);
    }
    assert!(wrong_order.validate(0).is_err());
}
