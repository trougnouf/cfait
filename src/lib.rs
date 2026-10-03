// SPDX-License-Identifier: GPL-3.0-or-later
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

#[cfg(test)]
mod i18n_tests {
    // Regression test: rust_i18n parameters must be written as `%{param}` in
    // the locale files, otherwise the raw placeholder text leaks into the UI
    // (e.g. "4 results for '{term}'"). Assertions are locale-agnostic: they
    // only check that the parameters were substituted.
    #[test]
    fn search_result_count_substitutes_parameters() {
        let other = rust_i18n::t!("search_results.other", count = 4, term = "tomatoes").to_string();
        assert!(other.contains('4'), "count missing in: {other}");
        assert!(other.contains("tomatoes"), "term missing in: {other}");
        assert!(
            !other.contains("term}"),
            "unsubstituted placeholder in: {other}"
        );

        let searching = rust_i18n::t!("searching_for", term = "carrots").to_string();
        assert!(
            searching.contains("carrots"),
            "term missing in: {searching}"
        );
        assert!(
            !searching.contains("term}"),
            "unsubstituted placeholder in: {searching}"
        );
    }
}
