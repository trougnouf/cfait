# Contributing to Cfait

Cfait shares one Rust core across TUI, GUI, and Android clients. To maintain consistency, please follow these guidelines.

### Core Principles

1.  **Feature Parity:** New features should be implemented across all three clients (TUI, GUI, Android). If you can't, please note it in your PR.
2.  **Tests:** Test your changes on all three clients to ensure nothing breaks.
3.  **Update Documentation:** If you add a feature, update both the `README.md` and the in-app help for all clients (common: `src/help.rs`, TUI: `src/tui/view.rs`, GUI: `src/gui/view/help.rs`, Android: `android/app/src/main/java/com/trougnouf/cfait/ui/HelpScreen.kt`).
4.  **Keep it Light:** Aim for simplicity to minimize the codebase and maintenance burden and to keep the program fast.


### Submitting a Pull Request

Before submitting, please ensure your changes meet the following criteria:

*   **Add Unit Tests** for any new logic where applicable.
*   **Pass CI Checks.** The CI will fail if these commands don't pass locally:
    ```bash
    cargo fmt --all -- --check
    cargo clippy --all-features --all-targets
    cargo test --all-features --all-targets
    ```
  * Set `git config core.hooksPath .githooks` to automatically format the code before committing.
  * Use `cargo fmt --all` and `cargo clippy --all-features --all-targets --fix --allow-dirty` to automatically fix formatting & most linting issues.
  * Regenerate the Kotlin bindings if you make changes to `mobile.rs`:
    * `cargo build --lib --features mobile`
    * `cargo run --features mobile --bin uniffi-bindgen generate --library target/debug/libcfait.so --language kotlin --out-dir ./android/app/src/main/java --config uniffi.toml`

### Local Development

*   **TUI:** `cargo run`
*   **GUI:** `cargo run --features gui --bin cfait-gui`
*   **Android:** See build instructions in `README.md`.

### Other ways to contribute

*   **Testing:** Use the current development version of Cfait (e.g. `cfait-git` on Arch Linux or the rolling release available on https://codeberg.org/trougnouf/cfait/releases ) and report any bug before the next release.
*   **Documentation:** Could always be improved
*   **Localization:** https://translate.codeberg.org/projects/cfait
