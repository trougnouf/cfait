// File: ./src/client/mod.rs
// Module declaration exporting the CalDAV client components.
pub mod auth;
pub mod cert;
pub mod core;
pub mod middleware;
pub mod sync;
pub mod redirect; // Restore this module

// Restore exports from local module
pub use crate::client::redirect::{
    FollowRedirectService, FollowRedirectLayer,
};

pub use crate::client::core::{GET_CTAG, RustyClient};
