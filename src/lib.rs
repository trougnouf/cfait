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
pub mod i18n;
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

    /// Regression test: every literal key passed to `t!()` must exist in the
    /// fallback locale. rust_i18n silently returns the key itself when a
    /// lookup fails — e.g. `t!("import_success", count = 5)` returns
    /// "import_success" because only the subkeys `import_success.one` and
    /// `.other` exist — so a bad key renders as raw text in the UI.
    #[test]
    fn t_macro_keys_exist_in_fallback_locale() {
        let src_dir =
            std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("src");
        let mut missing = Vec::new();
        let mut found = 0;
        collect_missing_t_keys(&src_dir, &mut missing, &mut found);
        assert!(
            found > 50,
            "scanner found only {found} t!() keys, likely broken"
        );
        assert!(
            missing.is_empty(),
            "t!() keys missing from en.json (rust_i18n would show the key itself): {missing:?}"
        );
    }

    fn collect_missing_t_keys(dir: &std::path::Path, missing: &mut Vec<String>, found: &mut usize) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                collect_missing_t_keys(&path, missing, found);
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                let src = std::fs::read_to_string(&path).unwrap();
                for key in extract_t_keys(&src) {
                    *found += 1;
                    if crate::_rust_i18n_try_translate("en", key).is_none() {
                        missing.push(format!("{}: {key}", path.display()));
                    }
                }
            }
        }
    }

    /// Extract the string-literal first argument of every `t!("key", ...)`
    /// call in the source. Occurrences inside strings, comments, and dynamic
    /// (non-literal) keys are skipped.
    fn extract_t_keys(src: &str) -> Vec<&str> {
        let mut keys = Vec::new();
        let bytes = src.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            let c = bytes[i];
            let two = if i + 1 < bytes.len() { bytes[i + 1] } else { 0 };
            match (c, two) {
                // line comment or doc comment
                (b'/', b'/') => match src[i..].find('\n') {
                    Some(n) => i += n + 1,
                    None => break,
                },
                // block comment (Rust block comments nest)
                (b'/', b'*') => {
                    let mut depth = 1;
                    let mut j = i + 2;
                    while j < bytes.len() && depth > 0 {
                        if bytes[j] == b'/' && j + 1 < bytes.len() && bytes[j + 1] == b'*' {
                            depth += 1;
                            j += 2;
                        } else if bytes[j] == b'*' && j + 1 < bytes.len() && bytes[j + 1] == b'/' {
                            depth -= 1;
                            j += 2;
                        } else {
                            j += 1;
                        }
                    }
                    i = j;
                }
                // raw string r#"..."# (no raw strings in this codebase's t! files,
                // but handle the simple form)
                (b'r', b'"') | (b'r', b'#') => {
                    let hashes = bytes[i + 1..].iter().take_while(|&&b| b == b'#').count();
                    let open = i + 1 + hashes + 1;
                    let close = src[open..]
                        .find(&format!("\"{}", "#".repeat(hashes)))
                        .map(|n| open + n + 1 + hashes)
                        .unwrap_or(bytes.len());
                    i = close;
                }
                // regular string literal
                (b'"', _) => {
                    let mut j = i + 1;
                    while j < bytes.len() {
                        if bytes[j] == b'\\' {
                            j += 2;
                            continue;
                        }
                        if bytes[j] == b'"' {
                            break;
                        }
                        j += 1;
                    }
                    i = j + 1;
                }
                // char literal (a `t!` cannot fit inside one)
                (b'\'', _) => {
                    let mut j = i + 1;
                    while j < bytes.len() {
                        if bytes[j] == b'\\' {
                            j += 2;
                            continue;
                        }
                        if bytes[j] == b'\'' {
                            break;
                        }
                        j += 1;
                    }
                    i = j + 1;
                }
                // candidate `t!(`
                (b't', b'!') if i + 2 < bytes.len() && bytes[i + 2] == b'(' => {
                    let preceded_ok =
                        i == 0 || (!bytes[i - 1].is_ascii_alphanumeric() && bytes[i - 1] != b'_');
                    if preceded_ok {
                        let mut j = i + 3;
                        while j < bytes.len() && (bytes[j] == b' ' || bytes[j] == b'\n') {
                            j += 1;
                        }
                        if j < bytes.len() && bytes[j] == b'"' {
                            let start = j + 1;
                            let mut k = start;
                            while k < bytes.len() {
                                if bytes[k] == b'\\' {
                                    k += 2;
                                    continue;
                                }
                                if bytes[k] == b'"' {
                                    break;
                                }
                                k += 1;
                            }
                            if k < bytes.len() {
                                keys.push(&src[start..k]);
                                i = k + 1;
                                continue;
                            }
                        }
                    }
                    i += 1;
                }
                _ => i += 1,
            }
        }
        keys
    }
}
