// File: ./src/client/mod.rs
// Module declaration exporting the CalDAV client components.
pub mod auth;
pub mod cert;
pub mod core;

// Re-export follow-redirect middleware from tower-http so the rest of the crate
// can import the same types it previously expected from the `redirect` module.
pub use tower_http::follow_redirect::{
    FollowRedirect as FollowRedirectService, FollowRedirectLayer, RequestUri,
};

// Use crate path to be safe, or relative
pub use crate::client::core::{GET_CTAG, RustyClient};
