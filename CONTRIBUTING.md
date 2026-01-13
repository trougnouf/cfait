# Contributing to Cfait

Cfait shares one Rust core across TUI, GUI, and Android clients. To maintain consistency, please follow these guidelines.

### Core Principles

1.  **Feature Parity:** New features should be implemented across all three clients (TUI, GUI, Android). If you can't, please note it in your PR.
2.  **Tests:** Test your changes on all three clients to ensure nothing breaks.
3.  **Update Documentation:** If you add a feature, update both the `README.md` and the in-app help for all clients.
    *   **TUI:** `src/tui/view.rs`
    *   **GUI:** `src/gui/view/help.rs`
    *   **Android:** `android/app/src/main/java/com/trougnouf/cfait/ui/HelpScreen.kt`
4.  **Keep it Light:** Aim for simplicity to minimize the codebase and maintenance burden.


### Submitting a Pull Request

Before submitting, please ensure your changes meet the following criteria:

*   **Add Unit Tests** for any new logic where applicable.
*   **Pass CI Checks.** The CI will fail if these commands don't pass locally:
    ```bash
    cargo clippy --all-features --all-targets
    cargo test --all-features --all-targets
    ```

### Local Development

*   **TUI:** `cargo run`
*   **GUI:** `cargo run --features gui --bin gui`
*   **Android:** See build instructions in `README.md`.

### Other ways to contribute

*   **Testing:** Use the current development version of Cfait and report any bug before the next release.
*   **Documentation:** Could always be improved
*   **Localization:** This isn't implemented yet as of when this document was written, but feel free to contact me s.t. I will recontact you when it's ready.
