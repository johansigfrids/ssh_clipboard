pub mod config;
pub mod connection;
pub mod diagnostics;
pub mod failure;
pub mod image;
pub mod monitor;
mod process;
pub mod session;
pub mod session_transport;
pub mod ssh;

// Share admission across pipe fixtures: per-module limits still let several
// PowerShell hosts compete with each other's short fault deadlines on Windows.
#[cfg(test)]
pub(crate) static SUBPROCESS_PROBES: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(2);
