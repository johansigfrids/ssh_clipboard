pub mod client_actions;
pub mod framing;
pub mod protocol;

#[cfg(target_os = "linux")]
pub mod daemon;
#[cfg(target_os = "linux")]
pub mod proxy;

pub mod client;

#[cfg(all(
    feature = "agent",
    any(target_os = "windows", target_os = "macos", target_os = "linux")
))]
pub mod agent;
