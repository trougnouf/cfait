// SPDX-License-Identifier: GPL-3.0-or-later
// File: ./src/i18n.rs
// Helpers for working with the rust_i18n locale files across clients.

/// Translate a plural key that has `one`/`other` subkeys and a `count`
/// parameter, selecting the subkey for the given count.
///
/// rust_i18n does not select plural forms automatically: `t!("key", count = n)`
/// on a plural key returns the key itself (e.g. `"import_success"`), so the
/// subkey must be chosen by the caller.
pub fn t_plural(key: &str, count: usize) -> String {
    if count == 1 {
        rust_i18n::t!(format!("{key}.one")).to_string()
    } else {
        rust_i18n::t!(format!("{key}.other"), count = count).to_string()
    }
}
