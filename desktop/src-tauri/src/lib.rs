//! Installation/update policy shared by the desktop and artifact validation tools.
#[cfg(native_update_test)]
#[path = "../build-support/native_update_test.rs"]
mod native_update_test;
pub mod startup;
pub mod update_feed;
#[cfg(feature = "installed-desktop")]
pub mod update_handoff;
#[cfg(feature = "installed-desktop")]
pub mod update_network;
pub mod update_policy;
