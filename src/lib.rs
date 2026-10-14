pub mod cli;
pub mod client_actions;
pub mod configuration;
mod file_lock;
pub mod framing;
pub mod protocol;
pub mod session_bridge;
pub mod session_protocol;
pub mod shelf;
pub mod shelf_service;

#[cfg(target_os = "linux")]
pub mod daemon;
#[cfg(target_os = "linux")]
pub mod proxy;
#[cfg(target_os = "linux")]
pub mod server_releases;
#[cfg(target_os = "linux")]
mod server_socket;
#[cfg(target_os = "linux")]
pub mod session_proxy;
#[cfg(target_os = "linux")]
pub mod shelf_daemon;

pub mod client;

#[cfg(all(
    feature = "agent",
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
pub mod agent;
