// File: ./src/client/mod.rs
// Module declaration exporting the CalDAV client components.
pub mod auth;
pub mod cert;
pub mod core;
pub mod redirect; // Added redirect module

// Use crate path to be safe, or relative
pub use crate::client::core::{GET_CTAG, RustyClient};
