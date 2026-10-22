//! Explicit new SSH endpoint, one bounded CLI operation (which may span many pages).
use super::failure::{FailureKind, TransportFailure, classify_stderr};
use super::process::StderrCapture;
use super::session::{Action, Reply, Session, SessionError, SessionPolicy, Snapshot};
use super::ssh::{SshConfig, resolve_target_and_port, spawn_ssh_session_proxy};
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, ReadBuf};
use tokio::process::{Child, ChildStdin, ChildStdout};
use tokio::time::{Duration, timeout};

pub enum Operation {
    Action(Action),
    AtEpoch { epoch: uuid::Uuid, action: Action },
    Snapshot,
}
pub enum Outcome {
    Reply(Reply),
    Snapshot(Snapshot),
}

#[derive(Debug, thiserror::Error)]
pub enum RunError {
    #[error(transparent)]
    Transport(#[from] TransportFailure),
    #[error(transparent)]
    Session(#[from] SessionError),
    #[error(
        "{source} The server outcome is uncertain for epoch {epoch}. Refresh before repeating this action."
    )]
    Uncertain {
        source: SessionError,
        epoch: uuid::Uuid,
        operation_id: Option<uuid::Uuid>,
    },
}

pub async fn run(
    config: &SshConfig,
    policy: SessionPolicy,
    operation: Operation,
    payload: &[u8],
) -> Result<Outcome, RunError> {
    policy.validate()?;
    let target = resolve_target_and_port(config).0;
    if target.trim().is_empty()
        || target.starts_with('-')
        || target.chars().any(char::is_whitespace)
        || target.chars().any(char::is_control)
    {
        return Err(TransportFailure::new(FailureKind::TargetMissing).into());
    }
    let child = spawn_ssh_session_proxy(config).map_err(|error| {
        TransportFailure::new(
            if error
                .downcast_ref::<std::io::Error>()
                .is_some_and(|error| error.kind() == std::io::ErrorKind::NotFound)
            {
                FailureKind::SshMissing
            } else {
                FailureKind::SshLaunch
            },
        )
    })?;
    run_child(child, policy, operation, payload).await
}

async fn run_child(
    child: Child,
    policy: SessionPolicy,
    operation: Operation,
    payload: &[u8],
) -> Result<Outcome, RunError> {
    run_child_version(child, policy, operation, payload, None).await
}

pub(crate) async fn run_child_version(
    mut child: Child,
    policy: SessionPolicy,
    operation: Operation,
    payload: &[u8],
    expected_version: Option<&str>,
) -> Result<Outcome, RunError> {
    let input = child
        .stdin
        .take()
        .ok_or_else(|| TransportFailure::new(FailureKind::SshLaunch))?;
    let output = child
        .stdout
        .take()
        .ok_or_else(|| TransportFailure::new(FailureKind::SshLaunch))?;
    let mut stderr = child
        .stderr
        .take()
        .ok_or_else(|| TransportFailure::new(FailureKind::SshLaunch))?;
    let mut captured = StderrCapture::default();
    let mut finished = None;
    let mut uncertain = false;
    let mut executing_mutation = false;
    let mut epoch = uuid::Uuid::nil();
    let operation_id = match &operation {
        Operation::Action(Action::RetryShare { share, .. })
        | Operation::AtEpoch {
            action: Action::RetryShare { share, .. },
            ..
        } => match share.as_ref() {
            Action::Share { operation_id, .. } => Some(*operation_id),
            _ => None,
        },
        Operation::Action(Action::Share { operation_id, .. })
        | Operation::AtEpoch {
            action: Action::Share { operation_id, .. },
            ..
        } => Some(*operation_id),
        _ => None,
    };
    let exchange = async {
        let mut session = match Session::connect(Pipes { input, output }, policy).await {
            Ok(session) => session,
            Err(error) => {
                finished = Some(Err(error));
                let _ = child.start_kill();
                let _ = child.wait().await;
                return;
            }
        };
        epoch = session.epoch();
        if expected_version.is_some_and(|expected| !matches!(session.welcome(), crate::session_protocol::Response::Welcome {server_version,..} if server_version==expected)) {
            finished = Some(Err(SessionError::Protocol));
            let _ = child.start_kill();
            let _ = child.wait().await;
            return;
        }
        executing_mutation = matches!(
            operation,
            Operation::Action(
                Action::Share { .. }
                    | Action::RetryShare { .. }
                    | Action::Delete { .. }
                    | Action::Clear
            ) | Operation::AtEpoch {
                action: Action::Share { .. }
                    | Action::RetryShare { .. }
                    | Action::Delete { .. }
                    | Action::Clear,
                ..
            }
        );
        let result = match operation {
            Operation::Action(action) => session.execute(action, payload).await.map(Outcome::Reply),
            Operation::AtEpoch { epoch, action } => {
                if epoch != session.epoch() {
                    Err(SessionError::Remote(
                        crate::session_protocol::ErrorCode::EpochChanged,
                    ))
                } else {
                    session.execute(action, payload).await.map(Outcome::Reply)
                }
            }
            Operation::Snapshot => session.snapshot().await.map(Outcome::Snapshot),
        };
        uncertain = session.mutation_uncertain();
        if result.is_err() && !session.is_ready() {
            let _ = child.start_kill();
        }
        finished = Some(result);
        let _ = session.close().await;
        let _ = child.wait().await;
    };
    let drain = async {
        let mut buffer = [0; 4096];
        loop {
            match stderr.read(&mut buffer).await {
                Ok(0) | Err(_) => break,
                Ok(count) => captured.observe(&buffer[..count]),
            }
        }
    };
    if timeout(policy.operation_timeout, async {
        tokio::join!(exchange, drain);
    })
    .await
    .is_err()
    {
        // kill() also waits/reaps. kill_on_drop covers cancellation of this whole call.
        let _ = timeout(Duration::from_secs(2), child.kill()).await;
        if finished.is_none() {
            uncertain = executing_mutation;
        }
    }
    // A validated reply is authoritative even if SSH subsequently fails to exit.
    // Do not turn a confirmed commit into a new, apparently failed upload.
    match finished.unwrap_or(Err(SessionError::Timeout)) {
        Ok(outcome) => Ok(outcome),
        Err(source) if uncertain => Err(RunError::Uncertain {
            source,
            epoch,
            operation_id,
        }),
        Err(SessionError::Stream) => Err(TransportFailure::new(
            classify_stderr(&captured.classification_bytes()).unwrap_or(FailureKind::Io),
        )
        .into()),
        Err(error) => Err(error.into()),
    }
}

pub(super) struct Pipes {
    pub(super) input: ChildStdin,
    pub(super) output: ChildStdout,
}
impl AsyncRead for Pipes {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.output).poll_read(cx, buffer)
    }
}
impl AsyncWrite for Pipes {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.input).poll_write(cx, bytes)
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.input).poll_flush(cx)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.input).poll_shutdown(cx)
    }
}

#[cfg(test)]
mod tests;
