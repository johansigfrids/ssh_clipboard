//! One reusable SSH child and negotiated stream. Own/drop this across cancelled calls.
use super::failure::{FailureKind, TransportFailure, classify_stderr};
use super::session::{Action, Reply, Session, SessionError, SessionPolicy, Snapshot};
use super::session_transport::{Pipes, RunError};
use super::ssh::{SshConfig, resolve_target_and_port, spawn_ssh_session_proxy};
use super::transport::StderrCapture;
use crate::session_protocol::EffectiveLimits;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::process::Child;
use tokio::task::JoinHandle;
use tokio::time::timeout;
use uuid::Uuid;

pub struct Connection {
    session: Option<Session<Pipes>>,
    child: Option<Child>,
    diagnostic_reader: Option<DiagnosticReader>,
    diagnostics: Arc<Mutex<StderrCapture>>,
    config: SshConfig,
    policy: SessionPolicy,
}
struct DiagnosticReader(JoinHandle<()>);
impl Drop for DiagnosticReader {
    fn drop(&mut self) {
        self.0.abort();
    }
}
impl Connection {
    pub async fn connect(config: SshConfig, policy: SessionPolicy) -> Result<Self, RunError> {
        policy.validate()?;
        let target = resolve_target_and_port(&config).0;
        if target.trim().is_empty()
            || target.starts_with('-')
            || target
                .chars()
                .any(|character| character.is_whitespace() || character.is_control())
        {
            return Err(TransportFailure::new(FailureKind::TargetMissing).into());
        }
        let child = spawn_ssh_session_proxy(&config).map_err(|error| {
            if let Some(error) = error.downcast_ref::<std::io::Error>() {
                tracing::debug!(kind = ?error.kind(), os_code = error.raw_os_error(), "SSH process launch failed");
            }
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
        Self::from_child(child, config, policy).await
    }

    async fn from_child(
        mut child: Child,
        config: SshConfig,
        policy: SessionPolicy,
    ) -> Result<Self, RunError> {
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
        let diagnostics = Arc::new(Mutex::new(StderrCapture::default()));
        let capture = diagnostics.clone();
        let diagnostic_reader = tokio::spawn(async move {
            let mut buffer = [0; 4096];
            loop {
                match stderr.read(&mut buffer).await {
                    Ok(0) | Err(_) => break,
                    Ok(count) => {
                        let Ok(mut capture) = capture.lock() else {
                            break;
                        };
                        capture.observe(&buffer[..count]);
                    }
                }
            }
        });
        // Construct the owner before awaiting so cancellation also aborts the reader
        // and kills the child; no detached stderr task survives a dropped connect.
        let mut connection = Self {
            session: None,
            child: Some(child),
            diagnostic_reader: Some(DiagnosticReader(diagnostic_reader)),
            diagnostics,
            config,
            policy,
        };
        match Session::connect(Pipes { input, output }, policy).await {
            Ok(session) => {
                connection.session = Some(session);
                Ok(connection)
            }
            Err(error) => {
                connection.terminate().await;
                Err(connection.failure(error))
            }
        }
    }

    pub fn matches(&self, config: &SshConfig, policy: SessionPolicy) -> bool {
        self.config == *config && self.policy == policy
    }
    pub fn is_ready(&mut self) -> bool {
        self.session.as_ref().is_some_and(Session::is_ready)
            && self
                .child
                .as_mut()
                .is_some_and(|child| matches!(child.try_wait(), Ok(None)))
    }
    pub fn epoch(&self) -> Option<Uuid> {
        self.session.as_ref().map(Session::epoch)
    }
    pub fn limits(&self) -> Option<&EffectiveLimits> {
        self.session.as_ref().map(Session::limits)
    }

    pub fn supports(&self, capability: &str) -> bool {
        self.session.as_ref().is_some_and(|session| {
            matches!(session.welcome(), crate::session_protocol::Response::Welcome {capabilities,..} if capabilities.iter().any(|value|value==capability))
        })
    }

    /// No action is automatically repeated. The caller owns its captured operation.
    pub async fn execute(&mut self, action: Action, payload: &[u8]) -> Result<Reply, RunError> {
        if !self.is_ready() {
            self.terminate().await;
            return Err(SessionError::Closed.into());
        }
        let operation_id = match &action {
            Action::RetryShare { share, .. } => match share.as_ref() {
                Action::Share { operation_id, .. } => Some(*operation_id),
                _ => None,
            },
            Action::Share { operation_id, .. } => Some(*operation_id),
            _ => None,
        };
        let session = self.session.as_mut().expect("ready session");
        let epoch = session.epoch();
        let result = session.execute(action, payload).await;
        let uncertain = session.mutation_uncertain();
        if !session.is_ready() {
            self.terminate().await;
        }
        match result {
            Ok(reply) => Ok(reply),
            Err(source) if uncertain => Err(RunError::Uncertain {
                source,
                epoch,
                operation_id,
            }),
            Err(error) => Err(self.failure(error)),
        }
    }

    pub async fn snapshot(&mut self) -> Result<Snapshot, RunError> {
        if !self.is_ready() {
            self.terminate().await;
            return Err(SessionError::Closed.into());
        }
        let session = self.session.as_mut().expect("ready session");
        let result = session.snapshot().await;
        if !session.is_ready() {
            self.terminate().await;
        }
        result.map_err(|error| self.failure(error))
    }

    /// Close stdin and allow a bounded normal exit; kill/reap a stuck direct child.
    pub async fn close(mut self) {
        if let Some(session) = self.session.take() {
            let _ = timeout(Duration::from_secs(2), session.close()).await;
        }
        let exited = match self.child.as_mut() {
            Some(child) => timeout(Duration::from_secs(2), child.wait())
                .await
                .is_ok_and(|result| result.is_ok()),
            None => true,
        };
        if !exited {
            self.terminate().await;
        } else {
            self.finish_reader().await;
        }
    }

    async fn terminate(&mut self) {
        self.session = None; // Close both pipes before killing a blocked transport.
        if let Some(child) = self.child.as_mut() {
            let _ = timeout(Duration::from_secs(2), child.kill()).await;
        }
        self.finish_reader().await;
    }
    async fn finish_reader(&mut self) {
        if let Some(mut reader) = self.diagnostic_reader.take()
            && timeout(Duration::from_millis(100), &mut reader.0)
                .await
                .is_err()
        {
            reader.0.abort();
            let _ = (&mut reader.0).await;
        }
    }
    fn failure(&self, error: SessionError) -> RunError {
        if error == SessionError::Stream {
            let classified = self
                .diagnostics
                .lock()
                .ok()
                .and_then(|capture| classify_stderr(&capture.classification_bytes()));
            TransportFailure::new(classified.unwrap_or(FailureKind::Io)).into()
        } else {
            error.into()
        }
    }
}
impl Drop for Connection {
    fn drop(&mut self) {
        // Synchronous invalidation/profile change/cancellation cannot await. Tokio
        // kill-on-drop handles best-effort reaping; explicit failures/close await it.
        if let Some(reader) = self.diagnostic_reader.take() {
            drop(reader);
        }
        if let Some(child) = self.child.as_mut() {
            let _ = child.start_kill();
        }
    }
}

#[cfg(test)]
mod tests;
