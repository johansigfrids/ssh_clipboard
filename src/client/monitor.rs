//! Bounded metadata-only recovery. This module cannot capture or write an OS clipboard.
use super::connection::Connection;
use super::failure::FailureKind;
use super::session::{Action, SessionError, SessionPolicy, Snapshot};
use super::session_transport::RunError;
use super::ssh::SshConfig;
use crate::session_protocol::{ErrorCode, Number, Response};
use serde::Serialize;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use tokio::time::{Instant, sleep, sleep_until};

#[derive(Clone, PartialEq, Eq)]
pub struct Target {
    pub profile_id: String,
    pub revision: String,
    pub ssh: SshConfig,
    pub policy: SessionPolicy,
}

#[derive(Clone, Default)]
pub struct Request {
    pub generation: u64,
    pub target: Option<Target>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Connecting,
    Live,
    Reconnecting,
    NeedsAction,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub phase: Phase,
    pub detail: String,
    pub retry_in_ms: Option<u32>,
}

pub struct Update {
    pub generation: u64,
    pub target: Target,
    pub status: Status,
    pub snapshot: Option<Arc<Snapshot>>,
    pub snapshot_started_at: Option<Instant>,
}

/// Changing profile/revision or explicitly retrying cancels the previous owner's future.
pub fn configure(
    sender: &watch::Sender<Request>,
    target: Option<Target>,
    force: bool,
) -> Result<(), &'static str> {
    let mut exhausted = false;
    sender.send_if_modified(|current| {
        if current.target == target && !force {
            return false;
        }
        let Some(generation) = current.generation.checked_add(1) else {
            exhausted = true;
            return false;
        };
        *current = Request { generation, target };
        true
    });
    if exhausted {
        Err("Reopen the app to reset connection tracking.")
    } else {
        Ok(())
    }
}

pub async fn run(input: watch::Receiver<Request>, output: watch::Sender<Option<Arc<Update>>>) {
    run_with(input, output, |target| async move {
        Connection::connect(target.ssh, target.policy).await
    })
    .await;
}

trait MetadataChannel {
    fn snapshot(&mut self) -> impl Future<Output = Result<Snapshot, RunError>> + Send;
    fn changed(
        &mut self,
        after: Number,
        wait_ms: u16,
    ) -> impl Future<Output = Result<Number, RunError>> + Send;
}
impl MetadataChannel for Connection {
    async fn snapshot(&mut self) -> Result<Snapshot, RunError> {
        Connection::snapshot(self).await
    }
    async fn changed(&mut self, after: Number, wait_ms: u16) -> Result<Number, RunError> {
        let action = if self.supports("watch") {
            Action::Watch {
                after_sequence: after,
                wait_ms,
            }
        } else {
            sleep(Duration::from_millis(u64::from(wait_ms).clamp(100, 2000))).await;
            Action::Health
        };
        match self.execute(action, &[]).await?.response {
            Response::Changed { sequence, .. } | Response::Health { sequence, .. } => Ok(sequence),
            _ => Err(SessionError::Protocol.into()),
        }
    }
}

async fn run_with<C: MetadataChannel, F, Fut>(
    mut input: watch::Receiver<Request>,
    output: watch::Sender<Option<Arc<Update>>>,
    mut connect: F,
) where
    F: FnMut(Target) -> Fut,
    Fut: Future<Output = Result<C, RunError>>,
{
    loop {
        let request = input.borrow_and_update().clone();
        if request.target.is_some() {
            tokio::select! {
                biased;
                changed = input.changed() => { if changed.is_err() {return;} },
                () = follow(&request,&output,&mut connect) => {},
            }
        } else if input.changed().await.is_err() {
            return;
        }
    }
}

fn publish(
    output: &watch::Sender<Option<Arc<Update>>>,
    request: &Request,
    status: Status,
    snapshot: &Option<Arc<Snapshot>>,
    snapshot_started_at: Option<Instant>,
) {
    output.send_replace(Some(Arc::new(Update {
        generation: request.generation,
        target: request.target.as_ref().unwrap().clone(),
        status,
        snapshot: snapshot.clone(),
        snapshot_started_at,
    })));
}

async fn follow<C: MetadataChannel, F, Fut>(
    request: &Request,
    output: &watch::Sender<Option<Arc<Update>>>,
    connect: &mut F,
) where
    F: FnMut(Target) -> Fut,
    Fut: Future<Output = Result<C, RunError>>,
{
    let target = request.target.as_ref().unwrap();
    let mut snapshot = None;
    let mut snapshot_started_at = None;
    let mut failures = 0u32;
    loop {
        publish(
            output,
            request,
            Status {
                phase: if failures == 0 {
                    Phase::Connecting
                } else {
                    Phase::Reconnecting
                },
                detail: "Connecting to shared history…".into(),
                retry_in_ms: None,
            },
            &snapshot,
            snapshot_started_at,
        );
        let result = match connect(target.clone()).await {
            Ok(mut channel) => {
                follow_connected(
                    &mut channel,
                    request,
                    output,
                    &mut snapshot,
                    &mut snapshot_started_at,
                    &mut failures,
                )
                .await
            }
            Err(error) => Err(error),
        };
        let error = result.unwrap_err();
        if !retryable(&error) {
            publish(
                output,
                request,
                Status {
                    phase: Phase::NeedsAction,
                    detail: error.to_string(),
                    retry_in_ms: None,
                },
                &snapshot,
                snapshot_started_at,
            );
            std::future::pending::<()>().await;
        }
        let delay = retry_delay(failures);
        failures = failures.saturating_add(1);
        publish(
            output,
            request,
            Status {
                phase: Phase::Reconnecting,
                detail: error.to_string(),
                retry_in_ms: Some(delay.as_millis() as u32),
            },
            &snapshot,
            snapshot_started_at,
        );
        sleep(delay).await;
    }
}

async fn follow_connected<C: MetadataChannel>(
    channel: &mut C,
    request: &Request,
    output: &watch::Sender<Option<Arc<Update>>>,
    snapshot: &mut Option<Arc<Snapshot>>,
    snapshot_started_at: &mut Option<Instant>,
    failures: &mut u32,
) -> Result<(), RunError> {
    let wait_ms = (request
        .target
        .as_ref()
        .unwrap()
        .policy
        .operation_timeout
        .as_millis()
        / 3)
    .clamp(0, 5000) as u16;
    loop {
        let started = Instant::now();
        let next = match channel.snapshot().await {
            Ok(snapshot) => snapshot,
            Err(RunError::Session(SessionError::Remote(ErrorCode::ResyncRequired))) => {
                sleep(Duration::from_millis(250)).await;
                continue;
            }
            Err(error) => return Err(error),
        };
        let sequence = next.sequence;
        *snapshot = Some(Arc::new(next));
        *snapshot_started_at = Some(started);
        *failures = 0;
        publish(
            output,
            request,
            Status {
                phase: Phase::Live,
                detail: "History is up to date.".into(),
                retry_in_ms: None,
            },
            snapshot,
            *snapshot_started_at,
        );
        loop {
            // Bound even a peer that returns immediate unchanged heartbeats.
            let earliest = Instant::now() + Duration::from_millis(250);
            let current = channel.changed(sequence, wait_ms).await?;
            sleep_until(earliest).await;
            if current != sequence || started.elapsed() >= Duration::from_secs(30) {
                break;
            }
        }
    }
}

fn retry_delay(failures: u32) -> Duration {
    Duration::from_secs((1u64 << failures.min(5)).min(30))
}

fn retryable(error: &RunError) -> bool {
    match error {
        RunError::Transport(error) => matches!(
            error.kind,
            FailureKind::NameResolution
                | FailureKind::ConnectionRefused
                | FailureKind::Network
                | FailureKind::Timeout
                | FailureKind::Io
                | FailureKind::SshExit
        ),
        RunError::Session(SessionError::Closed | SessionError::Timeout | SessionError::Stream) => {
            true
        }
        RunError::Session(SessionError::Remote(
            ErrorCode::Busy
            | ErrorCode::DaemonUnavailable
            | ErrorCode::Internal
            | ErrorCode::EpochChanged
            | ErrorCode::ResyncRequired,
        )) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests;
