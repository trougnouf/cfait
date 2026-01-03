// Module declaration exporting the CalDAV client components.
pub mod auth;
pub mod cert;
pub mod core;

// Use crate path to be safe, or relative
pub use crate::client::core::{GET_CTAG, RustyClient};
