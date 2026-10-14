use super::*;
use image::ImageEncoder;

fn limits() -> ShelfLimits {
    ShelfLimits {
        max_items: 2,
        max_item_bytes: 4,
        max_stored_bytes: 8,
        max_resident_bytes: 12,
        max_decoded_bytes: 1024,
        max_retry_records: 2,
        retention: Duration::from_secs(60),
        retry_retention: Duration::from_secs(10),
    }
}
fn now() -> Now {
    Now {
        instant: Instant::now(),
        unix_ms: 1_000_000,
    }
}
fn later(base: Now, millis: u64) -> Now {
    Now {
        instant: base.instant + Duration::from_millis(millis),
        unix_ms: base.unix_ms + millis as i64,
    }
}
fn origin() -> Origin {
    Origin {
        device_id: Uuid::from_u128(1),
        name: "Laptop".into(),
    }
}
fn upload(shelf: &Shelf, bytes: &[u8], operation: Option<Uuid>) -> ValidatedUpload {
    let mut input = shelf.upload_budget().allocate(bytes.len()).unwrap();
    input.bytes_mut().copy_from_slice(bytes);
    input
        .validate(
            ContentType::Text,
            operation.map(|_| origin()),
            Some(i64::MIN),
            operation,
        )
        .unwrap()
}
fn share(shelf: &mut Shelf, bytes: &[u8], now: Now) -> CommitOutcome {
    let prepared = upload(shelf, bytes, None);
    shelf.commit(shelf.epoch(), prepared, now).unwrap()
}
fn png() -> Vec<u8> {
    let mut data = Vec::new();
    image::codecs::png::PngEncoder::new(&mut data)
        .write_image(&[127; 16], 2, 2, image::ExtendedColorType::Rgba8)
        .unwrap();
    data
}

#[test]
fn exact_bytes_and_server_order_survive_client_clock_skew() {
    let base = now();
    let mut shelf = Shelf::new(limits(), base).unwrap();
    let first = share(&mut shelf, " å\n".as_bytes(), base);
    let mut clock_back = later(base, 1);
    clock_back.unix_ms = 0;
    let second = share(&mut shelf, b"next", clock_back);
    let snapshot = shelf.snapshot(clock_back).unwrap();
    assert_eq!(
        snapshot
            .items
            .iter()
            .map(|item| item.id)
            .collect::<Vec<_>>(),
        [second.metadata.id, first.metadata.id]
    );
    assert_eq!(snapshot.items[0].sequence, 2);
    assert_eq!(first.metadata.created_at_ms, Some(i64::MIN));
    assert_eq!(
        shelf
            .fetch(first.metadata.id, clock_back)
            .unwrap()
            .unwrap()
            .bytes(),
        " å\n".as_bytes()
    );
    assert_eq!(shelf.latest(clock_back).unwrap().unwrap().bytes(), b"next");
}

#[test]
fn count_and_byte_eviction_preserve_the_newest_items_and_exact_boundary() {
    let base = now();
    let mut shelf = Shelf::new(limits(), base).unwrap();
    let first = share(&mut shelf, b"aaaa", base);
    let second = share(&mut shelf, b"bbbb", later(base, 1));
    assert_eq!(shelf.stored_bytes(), 8);
    assert!(matches!(
        shelf.upload_budget().allocate(5),
        Err(ShelfError::ItemTooLarge)
    ));
    assert_eq!(shelf.snapshot(later(base, 1)).unwrap().items.len(), 2);
    let third = share(&mut shelf, b"c", later(base, 2));
    assert!(
        shelf
            .fetch(first.metadata.id, later(base, 2))
            .unwrap()
            .is_none()
    );
    assert_eq!(
        shelf
            .snapshot(later(base, 2))
            .unwrap()
            .items
            .iter()
            .map(|item| item.id)
            .collect::<Vec<_>>(),
        [third.metadata.id, second.metadata.id]
    );
    assert_eq!(shelf.stored_bytes(), 5);

    let mut byte_limits = limits();
    byte_limits.max_items = 10;
    byte_limits.max_stored_bytes = 4;
    byte_limits.max_resident_bytes = 8;
    let mut shelf = Shelf::new(byte_limits, base).unwrap();
    let old = share(&mut shelf, b"abc", base);
    share(&mut shelf, b"de", base);
    assert!(shelf.fetch(old.metadata.id, base).unwrap().is_none());
    assert_eq!(shelf.stored_bytes(), 2);
}

#[test]
fn monotonic_expiry_is_exact_and_batches_changes_without_using_wall_time() {
    let base = now();
    let mut shelf = Shelf::new(limits(), base).unwrap();
    share(&mut shelf, b"a", base);
    share(&mut shelf, b"b", later(base, 1));
    let mut clock_jump = later(base, 59_999);
    clock_jump.unix_ms = i64::MAX;
    assert_eq!(shelf.snapshot(clock_jump).unwrap().items.len(), 2);
    let snapshot = shelf.snapshot(later(base, 60_000)).unwrap();
    assert_eq!(snapshot.items.len(), 1);
    assert_eq!(snapshot.sequence, 3);
    let snapshot = shelf.snapshot(later(base, 60_001)).unwrap();
    assert!(snapshot.items.is_empty());
    assert_eq!(snapshot.sequence, 4);
    assert_eq!(shelf.stored_bytes(), 0);
    assert_eq!(shelf.upload_budget().resident_bytes(), 0);
    assert_eq!(shelf.snapshot(later(base, 60_002)).unwrap().sequence, 4);
    share(&mut shelf, b"x", later(base, 60_002));
    share(&mut shelf, b"y", later(base, 60_002));
    let batch = shelf.snapshot(later(base, 120_002)).unwrap();
    assert!(batch.items.is_empty());
    assert_eq!(batch.sequence, 7);
}

#[test]
fn retries_keep_identity_and_expiry_and_do_not_resurrect_deleted_or_cleared_items() {
    let base = now();
    let mut shelf = Shelf::new(limits(), base).unwrap();
    let operation = Uuid::from_u128(42);
    let prepared = upload(&shelf, b"item", Some(operation));
    let first = shelf.commit(shelf.epoch(), prepared, base).unwrap();
    let prepared = upload(&shelf, b"item", Some(operation));
    let retry = shelf
        .commit(shelf.epoch(), prepared, later(base, 1))
        .unwrap();
    assert!(retry.duplicate && retry.available);
    assert_eq!(retry.metadata, first.metadata);
    assert_eq!(shelf.snapshot(later(base, 1)).unwrap().sequence, 1);
    assert!(shelf.delete(first.metadata.id, later(base, 2)).unwrap());
    let prepared = upload(&shelf, b"item", Some(operation));
    let retry = shelf
        .commit(shelf.epoch(), prepared, later(base, 3))
        .unwrap();
    assert!(retry.duplicate && !retry.available);
    assert!(shelf.snapshot(later(base, 3)).unwrap().items.is_empty());
    let other = Uuid::from_u128(43);
    let prepared = upload(&shelf, b"next", Some(other));
    shelf
        .commit(shelf.epoch(), prepared, later(base, 4))
        .unwrap();
    shelf.clear(later(base, 5)).unwrap();
    let prepared = upload(&shelf, b"next", Some(other));
    let retry = shelf
        .commit(shelf.epoch(), prepared, later(base, 6))
        .unwrap();
    assert!(retry.duplicate && !retry.available);
}

#[test]
fn retry_conflicts_cover_bytes_origin_name_and_captured_timestamp() {
    let base = now();
    let mut shelf = Shelf::new(limits(), base).unwrap();
    let operation = Uuid::from_u128(42);
    let prepared = upload(&shelf, b"item", Some(operation));
    shelf.commit(shelf.epoch(), prepared, base).unwrap();
    for change in 0..3 {
        let bytes = if change == 0 { b"else" } else { b"item" };
        let mut input = shelf.upload_budget().allocate(4).unwrap();
        input.bytes_mut().copy_from_slice(bytes);
        let mut source = origin();
        if change == 1 {
            source.name = "Renamed".into();
        }
        let timestamp = if change == 2 { Some(0) } else { Some(i64::MIN) };
        let prepared = input
            .validate(ContentType::Text, Some(source), timestamp, Some(operation))
            .unwrap();
        assert_eq!(
            shelf.commit(shelf.epoch(), prepared, base).unwrap_err(),
            ShelfError::OperationConflict
        );
    }
    assert_eq!(shelf.snapshot(base).unwrap().sequence, 1);
    assert_eq!(shelf.latest(base).unwrap().unwrap().bytes(), b"item");
}

#[test]
fn a_full_retry_window_rejects_new_operations_without_forgetting_previous_retries() {
    let base = now();
    let mut shelf = Shelf::new(limits(), base).unwrap();
    for id in [1, 2] {
        let prepared = upload(&shelf, b"x", Some(Uuid::from_u128(id)));
        shelf.commit(shelf.epoch(), prepared, base).unwrap();
    }
    let before = shelf.snapshot(base).unwrap();
    let prepared = upload(&shelf, b"new", Some(Uuid::from_u128(3)));
    assert_eq!(
        shelf
            .commit(shelf.epoch(), prepared, later(base, 9999))
            .unwrap_err(),
        ShelfError::RetryWindowFull
    );
    assert_eq!(
        shelf.snapshot(later(base, 9999)).unwrap().sequence,
        before.sequence
    );
    let prepared = upload(&shelf, b"x", Some(Uuid::from_u128(1)));
    assert!(
        shelf
            .commit(shelf.epoch(), prepared, later(base, 9999))
            .unwrap()
            .duplicate
    );
    let prepared = upload(&shelf, b"new", Some(Uuid::from_u128(3)));
    assert!(
        !shelf
            .commit(shelf.epoch(), prepared, later(base, 10000))
            .unwrap()
            .duplicate
    );
    // The old record is now outside the documented guarantee; a caller must not
    // automatically replay it. An explicit new attempt can create another item.
    let prepared = upload(&shelf, b"x", Some(Uuid::from_u128(1)));
    assert!(
        !shelf
            .commit(shelf.epoch(), prepared, later(base, 10000))
            .unwrap()
            .duplicate
    );
}

#[test]
fn restart_epochs_and_foreign_budgets_prevent_cross_session_commits() {
    let base = now();
    let first = Shelf::new(limits(), base).unwrap();
    let mut second = Shelf::new(limits(), base).unwrap();
    assert_ne!(first.epoch(), second.epoch());
    let prepared = upload(&first, b"item", Some(Uuid::from_u128(1)));
    assert_eq!(
        second.commit(first.epoch(), prepared, base).unwrap_err(),
        ShelfError::EpochChanged
    );
    let prepared = upload(&first, b"item", None);
    assert_eq!(
        second.commit(second.epoch(), prepared, base).unwrap_err(),
        ShelfError::ForeignUpload
    );
    assert!(second.snapshot(base).unwrap().items.is_empty());
    assert_eq!(first.upload_budget().resident_bytes(), 0);
}

#[test]
fn fetched_and_inflight_bytes_remain_charged_after_eviction_and_release_on_last_drop() {
    let base = now();
    let mut config = limits();
    config.max_stored_bytes = 4;
    config.max_resident_bytes = 8;
    let mut shelf = Shelf::new(config, base).unwrap();
    let budget = shelf.upload_budget();
    let first = share(&mut shelf, b"old!", base);
    let fetched = shelf.fetch(first.metadata.id, base).unwrap().unwrap();
    let second_lease = fetched.clone();
    share(&mut shelf, b"new!", base);
    assert_eq!(budget.resident_bytes(), 8);
    assert!(matches!(budget.allocate(1), Err(ShelfError::Busy)));
    shelf.clear(base).unwrap();
    assert_eq!(budget.resident_bytes(), 4);
    assert_eq!(fetched.bytes(), b"old!");
    drop(fetched);
    assert_eq!(budget.resident_bytes(), 4);
    drop(second_lease);
    assert_eq!(budget.resident_bytes(), 0);
    let abandoned = budget.allocate(4).unwrap();
    assert_eq!(budget.resident_bytes(), 4);
    drop(abandoned);
    assert_eq!(budget.resident_bytes(), 0);
    let mut invalid = budget.allocate(1).unwrap();
    invalid.bytes_mut()[0] = 255;
    assert_eq!(
        invalid
            .validate(ContentType::Text, None, None, None)
            .err()
            .unwrap(),
        ShelfError::InvalidText
    );
    assert_eq!(budget.resident_bytes(), 0);
}

#[test]
fn concurrent_upload_reservations_cannot_exceed_the_budget() {
    let base = now();
    let shelf = Shelf::new(limits(), base).unwrap();
    let budget = shelf.upload_budget();
    let barrier = Arc::new(std::sync::Barrier::new(5));
    std::thread::scope(|scope| {
        let threads: Vec<_> = (0..4)
            .map(|_| {
                let budget = budget.clone();
                let barrier = barrier.clone();
                scope.spawn(move || {
                    let input = budget.allocate(4);
                    barrier.wait();
                    input
                })
            })
            .collect();
        barrier.wait();
        let results: Vec<_> = threads
            .into_iter()
            .map(|thread| thread.join().unwrap())
            .collect();
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 3);
        assert_eq!(budget.resident_bytes(), 12);
        drop(results);
        assert_eq!(budget.resident_bytes(), 0);
    });
}

#[test]
fn png_validation_decodes_with_limits_but_keeps_the_original_encoded_bytes() {
    let base = now();
    let config = ShelfLimits {
        max_item_bytes: 1024,
        max_stored_bytes: 2048,
        max_resident_bytes: 3072,
        ..limits()
    };
    let mut shelf = Shelf::new(config.clone(), base).unwrap();
    let bytes = png();
    let mut input = shelf.upload_budget().allocate(bytes.len()).unwrap();
    input.bytes_mut().copy_from_slice(&bytes);
    let prepared = input.validate(ContentType::Png, None, None, None).unwrap();
    let outcome = shelf.commit(shelf.epoch(), prepared, base).unwrap();
    assert_eq!(outcome.metadata.dimensions, Some((2, 2)));
    assert_eq!(
        shelf
            .fetch(outcome.metadata.id, base)
            .unwrap()
            .unwrap()
            .bytes(),
        bytes
    );
    let small = Shelf::new(
        ShelfLimits {
            max_decoded_bytes: 8,
            ..config
        },
        base,
    )
    .unwrap();
    let mut input = small.upload_budget().allocate(bytes.len()).unwrap();
    input.bytes_mut().copy_from_slice(&bytes);
    assert_eq!(
        input
            .validate(ContentType::Png, None, None, None)
            .err()
            .unwrap(),
        ShelfError::InvalidImage
    );
    assert_eq!(small.upload_budget().resident_bytes(), 0);
}

#[test]
fn malformed_png_and_busy_decoders_release_their_reservations() {
    let base = now();
    let shelf = Shelf::new(
        ShelfLimits {
            max_item_bytes: 1024,
            max_stored_bytes: 2048,
            max_resident_bytes: 3072,
            ..limits()
        },
        base,
    )
    .unwrap();
    let budget = shelf.upload_budget();
    let input = budget.allocate(3).unwrap();
    assert_eq!(
        input
            .validate(ContentType::Png, None, None, None)
            .err()
            .unwrap(),
        ShelfError::InvalidImage
    );
    assert!(!budget.state.validating_image.load(Ordering::SeqCst));
    budget.state.validating_image.store(true, Ordering::SeqCst);
    let input = budget.allocate(3).unwrap();
    assert_eq!(
        input
            .validate(ContentType::Png, None, None, None)
            .err()
            .unwrap(),
        ShelfError::Busy
    );
    assert_eq!(budget.resident_bytes(), 0);
    budget.state.validating_image.store(false, Ordering::SeqCst);
}

#[test]
fn invalid_origins_and_operation_ids_are_rejected_without_charging_storage() {
    let base = now();
    let shelf = Shelf::new(limits(), base).unwrap();
    let budget = shelf.upload_budget();
    for source in [
        Origin {
            device_id: Uuid::nil(),
            ..origin()
        },
        Origin {
            name: "x\n".into(),
            ..origin()
        },
        Origin {
            name: "å".repeat(129),
            ..origin()
        },
    ] {
        assert_eq!(
            budget
                .allocate(0)
                .unwrap()
                .validate(ContentType::Text, Some(source), None, None)
                .err()
                .unwrap(),
            ShelfError::InvalidOrigin
        );
    }
    for (source, operation) in [(None, Uuid::from_u128(1)), (Some(origin()), Uuid::nil())] {
        assert_eq!(
            budget
                .allocate(0)
                .unwrap()
                .validate(ContentType::Text, source, None, Some(operation))
                .err()
                .unwrap(),
            ShelfError::InvalidOperation
        );
    }
    assert_eq!(budget.resident_bytes(), 0);
}

#[test]
fn invalid_limits_clock_and_sequence_failures_preserve_existing_content() {
    let base = now();
    assert!(matches!(
        Shelf::new(
            ShelfLimits {
                max_items: 0,
                ..limits()
            },
            base
        ),
        Err(ShelfError::InvalidLimits)
    ));
    assert!(matches!(
        Shelf::new(
            ShelfLimits {
                max_resident_bytes: usize::MAX,
                ..limits()
            },
            base
        ),
        Err(ShelfError::InvalidLimits)
    ));
    let mut shelf = Shelf::new(limits(), base).unwrap();
    let first = share(&mut shelf, b"item", later(base, 1));
    assert_eq!(shelf.snapshot(base).unwrap_err(), ShelfError::ClockReversed);
    let prepared = upload(&shelf, b"next", None);
    assert_eq!(
        shelf
            .commit(
                shelf.epoch(),
                prepared,
                Now {
                    unix_ms: i64::MAX,
                    ..later(base, 1)
                }
            )
            .unwrap_err(),
        ShelfError::Overflow
    );
    shelf.sequence = u64::MAX;
    let prepared = upload(&shelf, b"next", None);
    assert_eq!(
        shelf
            .commit(shelf.epoch(), prepared, later(base, 1))
            .unwrap_err(),
        ShelfError::Overflow
    );
    assert_eq!(
        shelf.delete(first.metadata.id, later(base, 1)).unwrap_err(),
        ShelfError::Overflow
    );
    assert_eq!(
        shelf.clear(later(base, 1)).unwrap_err(),
        ShelfError::Overflow
    );
    assert_eq!(
        shelf.expire(later(base, 60_001)).unwrap_err(),
        ShelfError::Overflow
    );
    assert_eq!(shelf.stored_bytes(), 4);
    assert_eq!(
        shelf.latest(later(base, 1)).unwrap().unwrap().bytes(),
        b"item"
    );
}

#[test]
fn an_empty_text_is_an_item_and_an_empty_shelf_is_distinct() {
    let base = now();
    let mut shelf = Shelf::new(limits(), base).unwrap();
    assert!(shelf.latest(base).unwrap().is_none());
    let outcome = share(&mut shelf, b"", base);
    assert_eq!(outcome.metadata.encoded_bytes, 0);
    assert_eq!(shelf.latest(base).unwrap().unwrap().bytes(), b"");
    assert!(!shelf.delete(Uuid::nil(), base).unwrap());
    assert_eq!(shelf.snapshot(base).unwrap().sequence, 1);
}
