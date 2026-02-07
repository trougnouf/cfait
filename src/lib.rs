// Crate root library declaration and module exports.
pub mod alarm_index;
pub mod cache;
pub mod cli;
pub mod client;
pub mod color_utils;
pub mod config;
pub mod context;
pub mod controller;
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
