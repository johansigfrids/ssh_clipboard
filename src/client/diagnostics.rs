//! A single metadata-only health check, including for proxy-only forced-command keys.
use super::failure::{FailureKind, TransportFailure};
use super::transport::{ClientConfig, make_request, send_request};
use crate::protocol::{ErrorCode, Request, RequestKind, Response, ResponseKind};
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    Passed,
    Failed,
    NotChecked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckStage {
    SshExecutable,
    SshConnection,
    Proxy,
    Daemon,
    Protocol,
    Request,
}

impl CheckStage {
    pub fn label(self) -> &'static str {
        match self {
            Self::SshExecutable => "SSH executable",
            Self::SshConnection => "SSH connection",
            Self::Proxy => "Remote clipboard proxy",
            Self::Daemon => "Clipboard daemon",
            Self::Protocol => "Clipboard protocol",
            Self::Request => "Metadata check",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticCheck {
    pub stage: CheckStage,
    pub label: &'static str,
    pub status: CheckStatus,
    pub detail: &'static str,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionReport {
    pub healthy: bool,
    pub summary: &'static str,
    pub hint: Option<&'static str>,
    pub checks: Vec<DiagnosticCheck>,
}

impl ConnectionReport {
    fn unchecked(summary: &'static str, hint: Option<&'static str>) -> Self {
        Self {
            healthy: false,
            summary,
            hint,
            checks: [
                CheckStage::SshExecutable,
                CheckStage::SshConnection,
                CheckStage::Proxy,
                CheckStage::Daemon,
                CheckStage::Protocol,
                CheckStage::Request,
            ]
            .into_iter()
            .map(|stage| DiagnosticCheck {
                stage,
                label: stage.label(),
                status: CheckStatus::NotChecked,
                detail: "Not verified by this check.",
            })
            .collect(),
        }
    }

    fn mark(&mut self, stage: CheckStage, status: CheckStatus, detail: &'static str) {
        if let Some(check) = self.checks.iter_mut().find(|check| check.stage == stage) {
            check.status = status;
            check.detail = detail;
        }
    }

    pub fn from_failure(failure: &TransportFailure) -> Self {
        let mut report = Self::unchecked(failure.kind.summary(), Some(failure.kind.hint()));
        use FailureKind::*;
        let stage = match failure.kind {
            SshMissing | SshLaunch => CheckStage::SshExecutable,
            ProxyMissing => CheckStage::Proxy,
            ProtocolVersion | ProtocolFraming | InvalidResponse | ResponseMismatch
            | ResponseTooLarge => CheckStage::Protocol,
            TargetMissing | InvalidTimeout | RequestEncoding => CheckStage::Request,
            _ => CheckStage::SshConnection,
        };
        if !matches!(
            failure.kind,
            TargetMissing | SshMissing | SshLaunch | InvalidTimeout | RequestEncoding
        ) {
            report.mark(
                CheckStage::SshExecutable,
                CheckStatus::Passed,
                "The SSH process started.",
            );
        }
        report.mark(stage, CheckStatus::Failed, failure.kind.summary());
        report
    }

    fn from_response(response: Response) -> Self {
        let success = matches!(
            response.kind,
            ResponseKind::Meta { .. } | ResponseKind::Empty
        );
        let (summary, hint) = match &response.kind {
            ResponseKind::Meta { .. } | ResponseKind::Empty => {
                ("SSH proxy and clipboard daemon responded.", None)
            }
            ResponseKind::Error { code, .. } => {
                let (summary, hint) = server_problem(code);
                (summary, Some(hint))
            }
            _ => (
                "The metadata check received an unexpected reply.",
                Some(
                    "Check compatible client/server versions and that SSH runs the clipboard proxy.",
                ),
            ),
        };
        let mut report = Self::unchecked(summary, hint);
        report.healthy = success;
        report.mark(
            CheckStage::SshExecutable,
            CheckStatus::Passed,
            "The SSH process exchanged clipboard protocol data.",
        );
        report.mark(
            CheckStage::SshConnection,
            CheckStatus::Passed,
            "A correlated clipboard reply arrived over SSH.",
        );
        report.mark(
            CheckStage::Proxy,
            CheckStatus::Passed,
            "The remote endpoint returned a clipboard frame.",
        );
        report.mark(
            CheckStage::Protocol,
            CheckStatus::Passed,
            "The reply uses the supported framing and request ID.",
        );
        report.mark(
            CheckStage::Request,
            if success {
                CheckStatus::Passed
            } else {
                CheckStatus::Failed
            },
            summary,
        );
        if success {
            report.mark(
                CheckStage::Daemon,
                CheckStatus::Passed,
                "The clipboard daemon answered the metadata request.",
            );
        } else if let ResponseKind::Error { code, .. } = response.kind {
            match code {
                ErrorCode::DaemonNotRunning => report.mark(
                    CheckStage::Daemon,
                    CheckStatus::Failed,
                    "The proxy could not reach the clipboard daemon.",
                ),
                ErrorCode::VersionMismatch => report.mark(
                    CheckStage::Protocol,
                    CheckStatus::Failed,
                    "The endpoint reported a protocol version mismatch.",
                ),
                _ => (),
            }
        }
        report
    }

    pub fn message(&self) -> String {
        match self.hint {
            Some(hint) => format!("{} {hint}", self.summary),
            None => self.summary.into(),
        }
    }
}

/// New endpoint health has no payload or preview; retain the same evidence stages.
pub async fn diagnose_shelf(
    config: &ClientConfig,
    policy: super::session::SessionPolicy,
) -> ConnectionReport {
    use super::session::{Action, SessionError};
    use super::session_transport::{self, Operation, Outcome, RunError};
    use crate::session_protocol::ErrorCode as ShelfCode;
    match session_transport::run(&config.ssh, policy, Operation::Action(Action::Health), &[]).await
    {
        Ok(Outcome::Reply(_)) => ConnectionReport::from_response(Response {
            request_id: 1,
            kind: ResponseKind::Empty,
        }),
        Ok(_) => {
            ConnectionReport::from_failure(&TransportFailure::new(FailureKind::InvalidResponse))
        }
        Err(RunError::Transport(failure)) => ConnectionReport::from_failure(&failure),
        Err(RunError::Session(SessionError::Remote(code))) => {
            ConnectionReport::from_response(Response {
                request_id: 1,
                kind: ResponseKind::Error {
                    code: match code {
                        ShelfCode::DaemonUnavailable => ErrorCode::DaemonNotRunning,
                        ShelfCode::UnsupportedVersion => ErrorCode::VersionMismatch,
                        _ => ErrorCode::InvalidRequest,
                    },
                    message: String::new(),
                },
            })
        }
        Err(error) => {
            let kind = match error {
                RunError::Session(SessionError::Timeout) => FailureKind::Timeout,
                RunError::Session(SessionError::Local) => FailureKind::RequestEncoding,
                _ => FailureKind::InvalidResponse,
            };
            ConnectionReport::from_failure(&TransportFailure::new(kind))
        }
    }
}

/// Ignore server-supplied error text: it can include private data or terminal controls.
pub fn server_problem(code: &ErrorCode) -> (&'static str, &'static str) {
    match code {
        ErrorCode::DaemonNotRunning => (
            "The SSH proxy is reachable, but the clipboard daemon is unavailable.",
            "Check the clipboard daemon service and socket permissions on the server, then retry.",
        ),
        ErrorCode::PayloadTooLarge => (
            "The server rejected the request because it exceeds its transfer limit.",
            "Use a smaller item or coordinate matching client, proxy, and daemon limits.",
        ),
        ErrorCode::InvalidUtf8 => (
            "The server rejected text that is not valid UTF-8.",
            "Copy supported plain text and share it again.",
        ),
        ErrorCode::VersionMismatch => (
            "The server reported an incompatible clipboard protocol.",
            "Check compatible client/server versions and the remote proxy installation.",
        ),
        ErrorCode::InvalidRequest => (
            "The server rejected the clipboard request.",
            "Check compatible client/server versions and the selected transfer limits.",
        ),
        ErrorCode::Internal => (
            "The clipboard service reported an internal error.",
            "Check the proxy and daemon on the server, then retry.",
        ),
    }
}

pub async fn diagnose(config: &ClientConfig) -> ConnectionReport {
    // This remains usable with an account key restricted to `ssh_clipboard proxy`.
    // Do not add shell `true`, --help, service, or installation commands here.
    let probe = probe_config(config);
    diagnose_with(|request| send_request(&probe, request)).await
}

fn probe_config(config: &ClientConfig) -> ClientConfig {
    let mut probe = config.clone();
    if probe.ssh.ssh_options.first().map(String::as_str) != Some("BatchMode=yes") {
        probe.ssh.ssh_options.insert(0, "BatchMode=yes".into());
    }
    probe
}

async fn diagnose_with<F, Fut>(send: F) -> ConnectionReport
where
    F: FnOnce(Request) -> Fut,
    Fut: std::future::Future<Output = Result<Response, TransportFailure>>,
{
    let request = make_request(RequestKind::PeekMeta);
    let request_id = request.request_id;
    match send(request).await {
        Ok(response) if response.request_id == request_id => {
            ConnectionReport::from_response(response)
        }
        Ok(_) => {
            ConnectionReport::from_failure(&TransportFailure::new(FailureKind::ResponseMismatch))
        }
        Err(failure) => ConnectionReport::from_failure(&failure),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostics_force_batch_mode_without_changing_saved_options_or_trust_policy() {
        let original = ClientConfig {
            ssh: crate::client::ssh::SshConfig {
                target: "fixture-only".into(),
                port: Some(2222),
                user: None,
                host: None,
                identity_file: Some("test/key".into()),
                ssh_options: vec!["BatchMode=no".into(), "StrictHostKeyChecking=yes".into()],
                ssh_bin: None,
            },
            max_size: 1234,
            timeout_ms: 9000,
            resync_frames: false,
            resync_max_bytes: 4096,
        };
        let probe = probe_config(&original);
        assert_eq!(
            probe.ssh.ssh_options,
            ["BatchMode=yes", "BatchMode=no", "StrictHostKeyChecking=yes"]
        );
        assert_eq!(
            original.ssh.ssh_options,
            ["BatchMode=no", "StrictHostKeyChecking=yes"]
        );
        assert_eq!(probe.timeout_ms, 9000);
        assert_eq!(probe.ssh.port, Some(2222));
    }

    #[tokio::test]
    async fn health_uses_only_metadata_and_accepts_an_empty_server_without_fetching_content() {
        let report = diagnose_with(|request| async move {
            assert!(matches!(request.kind, RequestKind::PeekMeta));
            Ok(Response {
                request_id: request.request_id,
                kind: ResponseKind::Empty,
            })
        })
        .await;
        assert!(report.healthy);
        assert!(
            report
                .checks
                .iter()
                .all(|check| check.status == CheckStatus::Passed)
        );
    }

    #[tokio::test]
    async fn missing_daemon_verifies_ssh_proxy_and_wire_but_never_reports_a_healthy_service() {
        let report = diagnose_with(|request| async move {
            Ok(Response {
                request_id: request.request_id,
                kind: ResponseKind::Error {
                    code: ErrorCode::DaemonNotRunning,
                    message: "PRIVATE_SENTINEL\x1b[31m".into(),
                },
            })
        })
        .await;
        assert!(!report.healthy);
        assert_eq!(report.checks[1].status, CheckStatus::Passed);
        assert_eq!(report.checks[2].status, CheckStatus::Passed);
        assert_eq!(report.checks[3].status, CheckStatus::Failed);
        assert_eq!(report.checks[4].status, CheckStatus::Passed);
        assert!(
            !serde_json::to_string(&report)
                .unwrap()
                .contains("PRIVATE_SENTINEL")
        );
    }

    #[tokio::test]
    async fn unexpected_values_and_unmatched_replies_are_not_health_success_or_content_previews() {
        let report = diagnose_with(|request| async move {
            Ok(Response {
                request_id: request.request_id,
                kind: ResponseKind::Value {
                    value: crate::protocol::ClipboardValue {
                        content_type: "text/plain".into(),
                        data: b"PRIVATE_SENTINEL".to_vec(),
                        created_at: 0,
                    },
                },
            })
        })
        .await;
        assert!(!report.healthy);
        assert!(
            !serde_json::to_string(&report)
                .unwrap()
                .contains("PRIVATE_SENTINEL")
        );
        let report = diagnose_with(|request| async move {
            Ok(Response {
                request_id: request.request_id.wrapping_add(1),
                kind: ResponseKind::Empty,
            })
        })
        .await;
        assert!(!report.healthy);
        assert_eq!(report.checks[4].status, CheckStatus::Failed);
    }

    #[test]
    fn unknown_transport_failures_do_not_claim_to_have_verified_the_remote_layers() {
        let report = ConnectionReport::from_failure(&TransportFailure::new(FailureKind::Io));
        assert_eq!(report.checks[0].status, CheckStatus::Passed);
        assert_eq!(report.checks[1].status, CheckStatus::Failed);
        assert_eq!(report.checks[2].status, CheckStatus::NotChecked);
        assert_eq!(report.checks[3].status, CheckStatus::NotChecked);
        assert_eq!(report.checks[4].status, CheckStatus::NotChecked);
    }
}
