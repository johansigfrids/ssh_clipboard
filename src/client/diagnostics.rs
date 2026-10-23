//! Metadata-only session health. Never expose remote stderr or payload bytes.
use super::{
    config::ClientConfig,
    failure::{FailureKind, TransportFailure},
};
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
            ProtocolVersion | InvalidResponse => CheckStage::Protocol,
            TargetMissing | RequestEncoding => CheckStage::Request,
            _ => CheckStage::SshConnection,
        };
        if !matches!(
            failure.kind,
            TargetMissing | SshMissing | SshLaunch | RequestEncoding
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

    pub fn message(&self) -> String {
        match self.hint {
            Some(hint) => format!("{} {hint}", self.summary),
            None => self.summary.into(),
        }
    }
    fn session_result(error: Option<crate::session_protocol::ErrorCode>) -> Self {
        let (summary, hint) =
            error.map_or(("SSH session and shelf daemon responded.", None), |code| {
                let (summary, hint) = server_problem(&code);
                (summary, Some(hint))
            });
        let mut report = Self::unchecked(summary, hint);
        report.healthy = error.is_none();
        for stage in [
            CheckStage::SshExecutable,
            CheckStage::SshConnection,
            CheckStage::Proxy,
            CheckStage::Protocol,
        ] {
            report.mark(
                stage,
                CheckStatus::Passed,
                "The session endpoint returned a correlated reply.",
            );
        }
        let stage = if matches!(
            error,
            Some(crate::session_protocol::ErrorCode::UnsupportedVersion)
        ) {
            CheckStage::Protocol
        } else if matches!(
            error,
            Some(crate::session_protocol::ErrorCode::DaemonUnavailable)
        ) {
            CheckStage::Daemon
        } else {
            CheckStage::Request
        };
        if error.is_none() {
            report.mark(
                CheckStage::Daemon,
                CheckStatus::Passed,
                "The shelf daemon answered Health.",
            );
        }
        report.mark(
            stage,
            if error.is_none() {
                CheckStatus::Passed
            } else {
                CheckStatus::Failed
            },
            summary,
        );
        report
    }
}
pub async fn diagnose_shelf(config: &ClientConfig) -> ConnectionReport {
    use super::{
        session::{Action, SessionError},
        session_transport::{self, Operation, Outcome, RunError},
    };
    match session_transport::run(
        &config.ssh,
        config.policy,
        Operation::Action(Action::Health),
        &[],
    )
    .await
    {
        Ok(Outcome::Reply(_)) => ConnectionReport::session_result(None),
        Err(RunError::Session(SessionError::Remote(code))) => {
            ConnectionReport::session_result(Some(code))
        }
        Err(RunError::Transport(failure)) => ConnectionReport::from_failure(&failure),
        Err(RunError::Session(SessionError::Timeout)) => {
            ConnectionReport::from_failure(&TransportFailure::new(FailureKind::Timeout))
        }
        Err(RunError::Session(SessionError::Local)) => {
            ConnectionReport::from_failure(&TransportFailure::new(FailureKind::RequestEncoding))
        }
        _ => ConnectionReport::from_failure(&TransportFailure::new(FailureKind::InvalidResponse)),
    }
}
pub fn server_problem(code: &crate::session_protocol::ErrorCode) -> (&'static str, &'static str) {
    use crate::session_protocol::ErrorCode;
    match code {
        ErrorCode::DaemonUnavailable => (
            "The session proxy is reachable, but the shelf daemon is unavailable.",
            "Check the shelf service and socket permissions.",
        ),
        ErrorCode::UnsupportedVersion => (
            "The server uses an incompatible session protocol.",
            "Upgrade clients and server together to session version 2; keep existing settings and verify the session-proxy SSH route.",
        ),
        _ => (
            "The shelf server rejected the operation.",
            "Check the server limits and session connection settings.",
        ),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn health_and_remote_errors_have_distinct_evidence() {
        let healthy = ConnectionReport::session_result(None);
        assert!(healthy.healthy);
        let unavailable = ConnectionReport::session_result(Some(
            crate::session_protocol::ErrorCode::DaemonUnavailable,
        ));
        assert!(!unavailable.healthy);
        assert!(
            unavailable
                .checks
                .iter()
                .any(|check| matches!(check.stage, CheckStage::Daemon)
                    && matches!(check.status, CheckStatus::Failed))
        );
    }

    #[tokio::test]
    async fn invalid_policy_does_not_claim_that_ssh_was_started() {
        let report = diagnose_shelf(&ClientConfig {
            ssh: super::super::ssh::SshConfig {
                target: "unused-host".into(),
                port: None,
                identity_file: None,
                ssh_options: vec![],
                ssh_bin: Some("nonexistent-ssh".into()),
            },
            policy: super::super::session::SessionPolicy {
                max_item_bytes: 0,
                ..Default::default()
            },
        })
        .await;
        assert!(!report.healthy);
        assert!(
            report
                .checks
                .iter()
                .any(|check| matches!(check.stage, CheckStage::SshExecutable)
                    && matches!(check.status, CheckStatus::NotChecked))
        );
        assert!(
            report
                .checks
                .iter()
                .any(|check| matches!(check.stage, CheckStage::Request)
                    && matches!(check.status, CheckStatus::Failed))
        );
    }
}
