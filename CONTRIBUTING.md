# Contributing to Cfait

Thank you for your interest in contributing to Cfait! 

Cfait achieves its incredible speed and cross-platform capability by sharing **one single Rust core** across its TUI, GUI, and Android clients. To maintain this architecture and ensure consistency, please review these guidelines before submitting a Pull Request.

### 📖 The golden rule: use the specs

Before making architectural changes or adding new syntax, please read **[SPECS.md](./SPECS.md)**. 
It is our ultimate source of truth. If you are introducing a new feature, a new setting, or changing how tasks behave, you **must** update `SPECS.md` as part of your Pull Request.

### 🛠️ Core principles

1.  **Core-First / DRY:** All business logic, filtering, parsing, and data manipulation MUST live in the Rust core (`src/model`, `src/store.rs`, `src/controller.rs`). The UIs (TUI, GUI, Mobile) should remain as thin as possible, acting only as rendering and routing layers.
2.  **Feature Parity:** New features should ideally be accessible across all three clients.
3.  **Keep it Fast:** Cfait is built to handle 100,000+ tasks without stuttering. Avoid unnecessary cloning (use references in the filter pipeline) and rely on the `TaskStore` indices for $O(1)$ lookups.
4.  **Update the Help:** If you add a feature, ensure it is documented in the in-app help menus (`src/help.rs`, `src/tui/view.rs`, `src/gui/view/help.rs`, and Android's `HelpScreen.kt`).

### 💻 Local development

*   **TUI:** `cargo run` (Compiles fast, runs well in debug mode).
*   **GUI:** `cargo run --release --features gui --bin cfait-gui`
    * *Note: The GUI relies heavily on rendering optimizations. It will be quite sluggish in standard debug mode. Always use `--release` for a fluid testing experience, even though it takes slightly longer to compile.*

#### Android development
Requires [Android NDK](https://developer.android.com/ndk/downloads) and [cargo-ndk](https://github.com/bbqsrc/cargo-ndk). 

Most of the time, simply updating the Rust code and letting the CI build the Android app is sufficient. However, if you modify `mobile.rs` (the UniFFI boundary), you need to regenerate the Kotlin bindings.

**Script 1: fast binding generation (incremental)**
Use this when you only need to update the Kotlin interface (`cfait.kt`) before committing:
```bash
#!/bin/bash
# 1. Build only the host-native library (Incremental & Debug = Fast)
cargo build --lib --features mobile

# 2. Generate Kotlin bindings immediately
cargo run --features mobile --bin uniffi-bindgen generate \
  --library target/debug/libcfait.so \
  --language kotlin \
  --out-dir ./android/app/src/main/java \
  --config uniffi.toml
```

**Script 2: full Android prep (JNI libs + bindings)**
Use this when you need to actually compile the full Android APK locally:
```bash
#!/bin/bash
export ANDROID_NDK_HOME=/opt/android-ndk
export ANDROID_NDK_ROOT=/opt/android-ndk

# 1. Build the Android binaries (for the APK) — keep these release builds for performance
cargo ndk -t x86_64-linux-android -t aarch64-linux-android -o ./android/app/src/main/jniLibs build --release --features mobile --platform 28

# 2. Build the local host-native library (debug) so metadata extraction works
cargo build --lib --features mobile

# 3. Generate Kotlin bindings
cargo run --features mobile --bin uniffi-bindgen generate \
  --library target/debug/libcfait.so \
  --language kotlin --out-dir ./android/app/src/main/java --config uniffi.toml
```

### ✅ Submitting a Pull Request

**We strongly prefer Pull Requests on Codeberg:** [https://codeberg.org/trougnouf/cfait](https://codeberg.org/trougnouf/cfait)  
*(Our CI pipeline resides on Codeberg. We accept PRs on GitHub, but it requires us to manually port them over).*

Before submitting, please ensure your changes meet the following criteria:

1. **Pass CI Checks:** The CI will fail if these commands don't pass locally:
    ```bash
    cargo fmt --all -- --check
    cargo clippy --all-features --all-targets
    cargo test --all-features --all-targets
    ```
    *Tip: Use `cargo fmt --all` and `cargo clippy --all-features --all-targets --fix --allow-dirty` to automatically fix formatting and linting issues.*

2. **Set up Git Hooks (Optional but recommended):**
   Run `git config core.hooksPath .githooks` to automatically format your code before committing.

### 🌟 Other ways to contribute

Not a Rust developer? We still need your help!
*   **Testing:** We offer "Rolling Releases" (Flatpak, APK, Windows `.exe`). Download the rolling release from the [Releases page](https://codeberg.org/trougnouf/cfait/releases) (or compile it yourself) to test the bleeding edge and report bugs.
*   **Localization:** Help translate the app on [Codeberg Translate](https://translate.codeberg.org/projects/cfait).
*   **Documentation:** The documentation can always be improved. If something is missing and/or confusing then explain it better.
*   **Ideas, design & UI:** Suggestions for improvements are always welcome in the issue tracker.

