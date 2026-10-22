//! Resolved session connection shared by desktop, diagnostics and CLI.
use super::{session::SessionPolicy, ssh::SshConfig};
#[derive(Clone, Debug)]
pub struct ClientConfig {
    pub ssh: SshConfig,
    pub policy: SessionPolicy,
}
