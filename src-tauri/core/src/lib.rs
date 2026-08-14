//! Business logic shared between the desktop Sidecar and the ypm TUI.
//! Nothing in this crate may depend on an HTTP framework.

pub mod auth;
#[cfg(feature = "cache")]
pub mod cache;
pub mod scrobble;
pub mod session;
pub mod unm;
