//! Installation/update policy shared by the desktop and artifact validation tools.
pub mod startup;
pub mod update_feed;
#[cfg(feature = "installed-desktop")]
pub mod update_handoff;
#[cfg(feature = "installed-desktop")]
pub mod update_network;
pub mod update_policy;
