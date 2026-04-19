// SPDX-License-Identifier: GPL-3.0-or-later
// File: ./src/client/mod.rs
// Module declaration exporting the CalDAV client components.
pub mod auth;
pub mod cert;
pub mod core;
pub mod middleware;
pub mod redirect;
pub mod sync; // Restore this module

// Restore exports from local module
pub use crate::client::redirect::{FollowRedirectLayer, FollowRedirectService};

pub use crate::client::core::{GET_CTAG, RustyClient};
