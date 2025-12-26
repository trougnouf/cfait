// File: src/lib.rs
pub mod cache;
pub mod client;
pub mod color_utils;
pub mod config;
pub mod journal;
pub mod model;
pub mod paths;
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
