//! Safe transport diagnostics: never return remote stderr or clipboard bytes.
use serde::Serialize;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureKind {
    TargetMissing,
    SshMissing,
    SshLaunch,
    SshConfiguration,
    HostTrust,
    Authentication,
    NameResolution,
    ConnectionRefused,
    Network,
    ProxyMissing,
    Timeout,
    ProtocolVersion,
    InvalidResponse,
    RequestEncoding,
    SshExit,
    Io,
}

impl FailureKind {
    pub fn summary(self) -> &'static str {
        match self {
            Self::TargetMissing => "No SSH connection target is configured.",
            Self::SshMissing => "The SSH executable was not found.",
            Self::SshLaunch => "The SSH executable could not be started.",
            Self::SshConfiguration => "SSH reported a configuration error.",
            Self::HostTrust => "SSH could not verify the server's identity.",
            Self::Authentication => "SSH authentication was rejected.",
            Self::NameResolution => "SSH could not resolve the server name.",
            Self::ConnectionRefused => "The server refused the SSH connection.",
            Self::Network => "The SSH connection could not reach the server.",
            Self::ProxyMissing => "SSH reported that the clipboard proxy command is unavailable.",
            Self::Timeout => "The clipboard operation timed out.",
            Self::ProtocolVersion => "The server uses an incompatible clipboard protocol.",
            Self::InvalidResponse => "The server sent an invalid clipboard reply.",
            Self::RequestEncoding => "The clipboard request could not be prepared.",
            Self::SshExit => "SSH exited before completing the clipboard operation.",
            Self::Io => "The SSH stream closed before the operation completed.",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            Self::TargetMissing => {
                "Save an SSH alias or user@host in Connection settings, or pass --target to the CLI."
            }
            Self::SshMissing | Self::SshLaunch => {
                "Install OpenSSH or correct the SSH executable in Advanced connection settings."
            }
            Self::SshConfiguration => {
                "Check the alias and options in your SSH configuration and Advanced connection settings."
            }
            Self::HostTrust => {
                "Verify the server fingerprint through a trusted source, then resolve the host-key warning in a terminal. Do not disable host verification."
            }
            Self::Authentication => {
                "Unlock your SSH key or agent and verify this connection in a terminal, including its port and identity file."
            }
            Self::NameResolution => "Check the SSH alias, DNS, and VPN connection.",
            Self::ConnectionRefused => {
                "Check the SSH port and that the SSH service is running on the server."
            }
            Self::Network => {
                "Check the server address, network, VPN, and firewall, then test SSH in a terminal."
            }
            Self::ProxyMissing => {
                "Ensure ssh_clipboard is available in the remote SSH session's PATH and the key permits the clipboard proxy."
            }
            Self::Timeout => {
                "Check the network and server. Increase the timeout only if the connection needs longer."
            }
            Self::ProtocolVersion | Self::InvalidResponse => {
                "Check compatible client/server versions and that SSH runs the clipboard proxy."
            }
            Self::RequestEncoding => {
                "Try a supported text or PNG item and check the connection settings."
            }
            Self::SshExit | Self::Io => {
                "Check the SSH connection, remote proxy, and running clipboard daemon. Unrecognized SSH output is kept out of the report."
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransportFailure {
    pub kind: FailureKind,
    pub outcome_uncertain: bool,
}

impl TransportFailure {
    pub(crate) fn new(kind: FailureKind) -> Self {
        Self {
            kind,
            outcome_uncertain: false,
        }
    }
}

impl fmt::Display for TransportFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.kind.summary(), self.kind.hint())?;
        if self.outcome_uncertain {
            write!(
                f,
                " The share may have reached the server. Refresh before sharing it again."
            )?;
        }
        Ok(())
    }
}

impl std::error::Error for TransportFailure {}

pub(crate) fn classify_stderr(bytes: &[u8]) -> Option<FailureKind> {
    let text = String::from_utf8_lossy(bytes).to_ascii_lowercase();
    let any = |patterns: &[&str]| patterns.iter().any(|p| text.contains(p));
    if any(&[
        "host key verification failed",
        "remote host identification has changed",
        "no matching host key type found",
    ]) {
        Some(FailureKind::HostTrust)
    } else if any(&[
        "permission denied",
        "authentication failed",
        "no supported authentication methods",
        "too many authentication failures",
    ]) {
        Some(FailureKind::Authentication)
    } else if any(&[
        "could not resolve hostname",
        "name or service not known",
        "nodename nor servname provided",
    ]) {
        Some(FailureKind::NameResolution)
    } else if any(&["connection refused"]) {
        Some(FailureKind::ConnectionRefused)
    } else if any(&[
        "no route to host",
        "network is unreachable",
        "connection timed out",
        "connection reset by",
        "connection closed by",
    ]) {
        Some(FailureKind::Network)
    } else if any(&[
        "bad configuration option",
        "bad configuration options",
        "bad port",
        "invalid configuration option",
        "unsupported option",
        "bad owner or permissions",
    ]) {
        Some(FailureKind::SshConfiguration)
    } else if text.contains("ssh_clipboard")
        && any(&[
            "command not found",
            "not recognized",
            "no such file",
            "not found",
        ])
    {
        Some(FailureKind::ProxyMissing)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifications_do_not_expose_untrusted_output_or_terminal_controls() {
        for (text, kind) in [
            ("Permission denied (publickey)", FailureKind::Authentication),
            (
                "REMOTE HOST IDENTIFICATION HAS CHANGED",
                FailureKind::HostTrust,
            ),
            (
                "Could not resolve hostname test",
                FailureKind::NameResolution,
            ),
            ("Connection refused", FailureKind::ConnectionRefused),
            ("No route to host", FailureKind::Network),
            (
                "Bad configuration option: test",
                FailureKind::SshConfiguration,
            ),
            (
                "sh: ssh_clipboard: command not found",
                FailureKind::ProxyMissing,
            ),
        ] {
            let untrusted = format!("\x1b[31m{text}\nPRIVATE_SENTINEL\0");
            assert_eq!(classify_stderr(untrusted.as_bytes()), Some(kind));
            let safe = TransportFailure::new(kind).to_string();
            assert!(!safe.contains("PRIVATE_SENTINEL"));
            assert!(!safe.chars().any(char::is_control));
        }
        assert_eq!(classify_stderr(b"unknown PRIVATE_SENTINEL"), None);
    }
}
