// Module declaration exporting the CalDAV client components.
pub mod auth;
pub mod cert;
pub mod core;
pub mod manager; // Export the new module

// Use crate path to be safe, or relative
pub use crate::client::core::{GET_CTAG, RustyClient};
pub use crate::client::manager::ClientManager;
