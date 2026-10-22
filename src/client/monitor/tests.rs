use super::super::failure::TransportFailure;
use super::*;
use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};
use tokio::sync::mpsc;
use tokio::time::timeout;
use uuid::Uuid;

fn target(profile: &str) -> Target {
    Target {
        profile_id: profile.into(),
        revision: "1".into(),
        ssh: SshConfig {
            target: "synthetic-only".into(),
            port: None,

            identity_file: None,
            ssh_options: vec![],
            ssh_bin: None,
        },
        policy: SessionPolicy::default(),
    }
}

struct Fake {
    epoch: Uuid,
    sequence: Arc<Mutex<u64>>,
    changes: mpsc::UnboundedReceiver<u64>,
    dropped: Arc<AtomicUsize>,
}
impl Drop for Fake {
    fn drop(&mut self) {
        self.dropped.fetch_add(1, Ordering::SeqCst);
    }
}
impl MetadataChannel for Fake {
    async fn snapshot(&mut self) -> Result<Snapshot, RunError> {
        Ok(Snapshot {
            epoch: self.epoch,
            sequence: (*self.sequence.lock().unwrap()).into(),
            items: vec![],
        })
    }
    async fn changed(&mut self, _after: Number, _wait_ms: u16) -> Result<Number, RunError> {
        match self.changes.recv().await {
            Some(sequence) => Ok(sequence.into()),
            None => std::future::pending().await,
        }
    }
}
async fn update_when(
    output: &mut watch::Receiver<Option<Arc<Update>>>,
    predicate: impl Fn(&Update) -> bool,
) -> Arc<Update> {
    timeout(Duration::from_secs(3), async {
        loop {
            let value = output.borrow_and_update().clone();
            if let Some(value) = value
                && predicate(&value)
            {
                return value;
            }
            output.changed().await.unwrap();
        }
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn metadata_changes_resnapshot_and_profile_switch_drops_the_blocked_old_owner() {
    let (input, input_rx) = watch::channel(Request::default());
    let (output, mut output_rx) = watch::channel(None);
    let (changes, changes_rx) = mpsc::unbounded_channel();
    let mut first = Some(changes_rx);
    let sequence = Arc::new(Mutex::new(1));
    let dropped = Arc::new(AtomicUsize::new(0));
    let factory_sequence = sequence.clone();
    let factory_dropped = dropped.clone();
    let running = tokio::spawn(run_with(input_rx, output, move |target| {
        let channel = Fake {
            epoch: Uuid::from_u128(if target.profile_id == "A" { 1 } else { 2 }),
            sequence: factory_sequence.clone(),
            changes: first.take().unwrap_or_else(|| mpsc::unbounded_channel().1),
            dropped: factory_dropped.clone(),
        };
        async { Ok(channel) }
    }));
    configure(&input, Some(target("A")), false).unwrap();
    update_when(&mut output_rx, |update| update.status.phase == Phase::Live).await;
    *sequence.lock().unwrap() = 3;
    changes.send(3).unwrap();
    update_when(&mut output_rx, |update| {
        update
            .snapshot
            .as_ref()
            .is_some_and(|snapshot| snapshot.sequence.get() == 3)
    })
    .await;
    configure(&input, Some(target("B")), false).unwrap();
    let next = update_when(&mut output_rx, |update| {
        update.target.profile_id == "B" && update.status.phase == Phase::Live
    })
    .await;
    assert_eq!(next.snapshot.as_ref().unwrap().epoch, Uuid::from_u128(2));
    assert_eq!(dropped.load(Ordering::SeqCst), 1);
    assert!(
        changes.send(9).is_err(),
        "the old stream must have been dropped"
    );
    drop(input);
    timeout(Duration::from_secs(1), running)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(dropped.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn transient_network_failure_reconnects_only_the_metadata_channel_after_backoff() {
    let (input, input_rx) = watch::channel(Request::default());
    let (output, mut output_rx) = watch::channel(None);
    let calls = Arc::new(AtomicUsize::new(0));
    let factory_calls = calls.clone();
    let dropped = Arc::new(AtomicUsize::new(0));
    let running = tokio::spawn(run_with(input_rx, output, move |_| {
        let attempt = factory_calls.fetch_add(1, Ordering::SeqCst);
        let dropped = dropped.clone();
        async move {
            if attempt == 0 {
                Err(RunError::Transport(TransportFailure::new(
                    FailureKind::Network,
                )))
            } else {
                Ok(Fake {
                    epoch: Uuid::from_u128(1),
                    sequence: Arc::new(Mutex::new(0)),
                    changes: mpsc::unbounded_channel().1,
                    dropped,
                })
            }
        }
    }));
    configure(&input, Some(target("A")), false).unwrap();
    let failure = update_when(&mut output_rx, |update| {
        update.status.phase == Phase::Reconnecting && update.status.retry_in_ms.is_some()
    })
    .await;
    assert_eq!(failure.status.retry_in_ms, Some(1000));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let success = update_when(&mut output_rx, |update| update.status.phase == Phase::Live).await;
    assert!(success.snapshot.as_ref().unwrap().items.is_empty());
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    drop(input);
    running.await.unwrap();
}

#[tokio::test]
async fn authentication_pauses_until_explicit_retry_and_does_not_loop() {
    let (input, input_rx) = watch::channel(Request::default());
    let (output, mut output_rx) = watch::channel(None);
    let calls = Arc::new(AtomicUsize::new(0));
    let factory_calls = calls.clone();
    let running = tokio::spawn(run_with(input_rx, output, move |_| {
        factory_calls.fetch_add(1, Ordering::SeqCst);
        async {
            Err::<Fake, _>(RunError::Transport(TransportFailure::new(
                FailureKind::Authentication,
            )))
        }
    }));
    configure(&input, Some(target("A")), false).unwrap();
    update_when(&mut output_rx, |update| {
        update.status.phase == Phase::NeedsAction
    })
    .await;
    sleep(Duration::from_millis(20)).await;
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    configure(&input, Some(target("A")), true).unwrap();
    update_when(&mut output_rx, |update| {
        update.generation == 2 && update.status.phase == Phase::NeedsAction
    })
    .await;
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    drop(input);
    running.await.unwrap();
}

#[test]
fn backoff_is_bounded_and_trust_or_protocol_failures_need_action() {
    assert_eq!(
        (0..8)
            .map(|attempt| retry_delay(attempt).as_secs())
            .collect::<Vec<_>>(),
        vec![1, 2, 4, 8, 16, 30, 30, 30]
    );
    assert_eq!(retry_delay(u32::MAX), Duration::from_secs(30));
    for kind in [
        FailureKind::Authentication,
        FailureKind::HostTrust,
        FailureKind::ProxyMissing,
        FailureKind::SshConfiguration,
        FailureKind::ProtocolVersion,
        FailureKind::InvalidResponse,
    ] {
        assert!(!retryable(&RunError::Transport(TransportFailure::new(
            kind
        ))));
    }
    assert!(retryable(&SessionError::Stream.into()));
}
