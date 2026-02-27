// Crate root library declaration and module exports.

// Make rust_i18n symbols available to the rest of the crate by expanding the
// localization macro before module declarations. Some modules (and their
// compile-time code) may reference the generated `_rust_i18n_t` symbol and
// therefore require the macro to be invoked early in the crate.
rust_i18n::i18n!("locales", fallback = "en");

pub mod alarm_index;
pub mod cache;
pub mod cli;
pub mod client;
pub mod color_utils;
pub mod config;
pub mod context;
pub mod controller;
pub mod help;
pub mod journal;
pub mod model;
pub mod storage;
pub mod store;
pub mod system;

#[cfg(feature = "tui")]
pub mod tui;

#[cfg(feature = "gui")]
pub mod gui;

// --- ANDROID SUPPORT ---
#[cfg(feature = "mobile")]
pub mod mobile;

#[cfg(feature = "mobile")]
uniffi::setup_scaffolding!();
