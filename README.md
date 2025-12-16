# Cfait
> Take control of your TODO list

**Cfait** is a powerful, elegant and fast CalDAV task manager, written in Rust.

It features a modern **GUI (Graphical UI)**, an efficient **TUI (Terminal UI)**, and a native **Android** client.

![featurethingy](https://commons.wikimedia.org/wiki/Special:FilePath/Cfait_featureGraphic_(hardcoded_text).svg)

![Cfait GUI Screenshot](https://commons.wikimedia.org/wiki/Special:FilePath/Cfait_task_manager_v0.3.0_screenshot_(GUI).png)
> The Graphical Interface in v0.3.0 <small>([history](https://commons.wikimedia.org/wiki/Category:Screenshots_of_Cfait_(GUI)))</small>

![Cfait TUI Screenshot](https://commons.wikimedia.org/wiki/Special:FilePath/Cfait_task_manager_v0.3.0_screenshot_(TUI).png)
> The Terminal Interface in v0.3.0 <small>([history](https://commons.wikimedia.org/wiki/Category:Screenshots_of_Cfait_(TUI)))</small>

![Cfait Android Screenshot](https://commons.wikimedia.org/wiki/Special:FilePath/Cfait_task_manager_v0.3.0_screenshot_(Android).png)
> The Android client in v0.3.0 <small>([history](https://commons.wikimedia.org/wiki/Category:Screenshots_of_Cfait_(Android)))</small>


## Features

*   **Triple interface:** TUI (Terminal), GUI (Windowed), and Native Android.
*   **Smart input:** Add tasks naturally: `Buy cat food !1 @tomorrow ~15m` sets priority, due date, and duration automatically.
*   **GTD workflow:** Mark tasks as **in process** (`>`), **cancelled** (`x`), or **done**.
*   **Duration estimation:** Estimate time (`~2h`) and filter tasks by duration (`~<30m`).
*   **Syncs everywhere:** Fully compatible with standard CalDAV servers (Radicale, Xandikos, Nextcloud, iCloud, etc.).
*   **Tag support:** Organize tasks using tags (`#gardening`) and sub-tags (`#gaming:coop` is automatically included in `#gaming`).
*   **Inline Aliases:** Define shortcuts on the fly; typing `#groceries=#home,#shopping` applies the alias immediately and saves it for future use.
*   **Dependencies:** Link tasks using RFC 9253 logic (Blocked-by / Child-of).
*   **Hierarchy support:** Create sub-tasks directly from parents, promote children, and organize nested lists.
*   **Multiple calendars:** Seamlessly switch between "Work", "Personal", and other lists, or move tasks between them.
*   **Offline & local first:** Optimistic UI updates mean you never wait for the server. A persistent "Local" calendar allows offline use with a 1-click migration tool to push tasks to a CalDAV server later.
*   **Sane sorting:** Tasks are sorted by Status > Start Date > Due Date > Priority.


## Installation

### A. Pre-built packages (Recommended for Linux/Windows)

The build pipeline generates binaries for Linux and Windows automatically.

*   **Linux (Generic / Debian / Ubuntu):**
    *   **Compatibility:** Binaries are built on **Ubuntu 24.04**. They require **glibc 2.39** or newer.
    *   **Supported Distros:** Ubuntu 24.04+, Linux Mint 22+, Fedora 40+, Debian 13 (Trixie), Arch Linux.
    *   **Older Distros:** If you are on Debian 12 (Bookworm) or Ubuntu 22.04, please [build from source](#c-from-cratesio-via-cargo).
    *   **Download:** Get the `.deb` or `.tar.gz` from [**Codeberg Releases**](https://codeberg.org/trougnouf/cfait/releases).

*   **Arch Linux:**
    ```bash
    yay -S cfait      # Stable release
    # or
    yay -S cfait-git  # Latest git version
    ```

*   **Windows:**
    *   Download the `.zip` archive from [**Codeberg Releases**](https://codeberg.org/trougnouf/cfait/releases).
    *   Extract and run `cfait-gui.exe` (or `cfait.exe` for the terminal).

*  **Android** releases will be available on F-Droid and the Play Store starting with v0.3.0

### B. macOS

We do not provide pre-built `.dmg` or `.app` bundles, but Cfait runs natively on macOS (Apple Silicon & Intel). Please install via Cargo:

```bash
# Install Rust (if not installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install Cfait
cargo install cfait --features gui
```

### C. From crates.io (via Cargo)

```bash
# Install both TUI and GUI
cargo install cfait --features gui

# Or, install only the TUI
cargo install cfait
```

### D. From source (Development)

```bash
git clone https://codeberg.org/trougnouf/cfait.git
cd cfait

# Run the TUI
cargo run

# Run the GUI
cargo run --bin gui --no-default-features --features gui
```

## Configuration

The GUI includes a configuration dialog. The TUI has an onboarding screen to set credentials.

The config file is located at:
*   **Linux:** `~/.config/cfait/config.toml`
*   **macOS:** `~/Library/Application Support/com.cfait.cfait/config.toml`
*   **Windows:** `%APPDATA%\cfait\config.toml`

```toml
url = "https://localhost:5232/user/"
username = "myuser"
password = "mypassword"

# Security: Allow self-signed certificates
# Default: false
allow_insecure_certs = true 

default_calendar = "todo"

# Hide completed tasks globally?
hide_completed = false

# Hide tags in the sidebar if they only contain completed tasks?
hide_fully_completed_tags = true

# Sorting: Tasks due more than X months away are sorted by priority only (not date)
# Default: 6
sort_cutoff_months = 6

# Tag Aliases: Automatically expand one tag into multiple
[tag_aliases]
groceries = ["shopping", "home"]
```

## TUI Keybindings

| Context | Key | Action |
| :--- | :--- | :--- |
| **Global** | `Tab` | Switch focus (Tasks ↔ Sidebar) |
| | `q` | Quit |
**Sidebar (Cals)** | `Enter` | **Set target** (Add to view) |
| | `Right` | **Focus** (Set target + Hide others) |
  | | `Space` | **Toggle visibility** (Show/Hide layer) |
| | `*` | **Toggle all** (Show all / Hide others) |
| **Sidebar (Tags)** | `Enter` | Toggle tag filter |
| | `m` | Toggle tag match mode (AND / OR) |
| | `*` | **Clear all tags** (Show all tasks) |
| **Task List** | `j` / `k` | Move down / up |
| | `Space` | **Toggle** completion |
| | `s` | **Start / Pause** (Mark in-process) |
| | `x` | **Cancel** task |
| | `a` | **Add** task (Type name, press Enter) |
| | `C` | **Create child** (Create new task linked as child of current, Shift+c) |
| | `e` | **Edit** task title |
| | `E` | **Edit** task description (Shift+e) |
| | `d` | **Delete** task |
| | `M` | **Move** task to another calendar (Shift+m) |
| | `y` | **Yank** (Copy ID for linking) |
| | `b` | **Block** (Mark current task as blocked by Yanked task) |
| | `c` | **Child** (Mark current task as child of Yanked task) |
| | `r` | **Refresh** (Force sync) |
| | `X` | **Export** (Migrate all tasks from Local to remote, Shift+x) |
| | `H` | Toggle **hide completed** tasks |
| | `/` | **Search** / Filter tasks |
| | `+` / `-` | Increase / Decrease **priority** |
| | `>` / `<` | **Indent** / **Outdent** (Visual sub-tasks depth) |
| **Sidebar** | `Enter` | Select calendar / Toggle tag |
| | `1` | Switch to **Calendars** view |
| | `2` | Switch to **Tags** view |
| | `m` | Toggle tag match mode (AND / OR) |

## Input Syntax
When adding (`a`) or editing (`e`) a task:

*   `!1` to `!9`: **Priority** (1 is high, 9 is low).
*   `@DATE` or `due:DATE`: **Due date** (`2025-12-31`, `today`, `tomorrow`, `1w`, `2d`, 3mo, 4y).
*   `^DATE` or `start:DATE`: **Start date**. Pushes to bottom until date.
*   `~DURATION` or `est:DURATION`: **Estimate** (`~30m`, `~1h`, `~4d`).
*   `rec:INTERVAL` or `@every X`: **Recurrence** (`@daily`, `@weekly`, `@monthly`, `@yearly`, or `@every 2 weeks`, ...).
*   `#tag`: **Tag**. Supports hierarchy (`#project:backend`).
    *   **Define Alias:** `#alias=#tag1,#tag2` (e.g., `#shop=#home,#buy`).

## Advanced Search
The search bar supports specific filters:

*   `#tag`: Filter by tag.
*   `is:done` / `is:active` / `is:ongoing`: Filter by status.
*   `~<30m`: Duration less than 30 mins (using `>`, `>=`, `<`, `<=` operators and `m`, `h`, `d`, `mo`, `y`).
*   `!<3`: Priority higher than 3 (1 or 2).
*   `@<today`: Overdue tasks, `@<1w`: due within 1 week, `@>=2d`: due at least 2 days from now.

## Android Development

Cfait uses a native Android UI (Jetpack Compose) backed by the shared Rust core via [UniFFI](https://github.com/mozilla/uniffi-rs).

Android will be made available on F-Droid and the Play store starting with v0.3.0.

### Prerequisites
1.  **Android Studio** (with NDK installed).
2.  **Rust Targets**:
    ```bash
    rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
    ```
3.  **Cargo NDK**:
    ```bash
    cargo install cargo-ndk
    ```

### Building & Running
1.  **Compile Rust Library:**
    Set `ANDROID_NDK_HOME` and `ANDROID_NDK_ROOT` to your NDK path (e.g., inside `/opt/android-ndk`).
    ```bash
    export ANDROID_NDK_HOME=/path/to/your/ndk
    
    # Build the shared libraries (.so) and place them in the Android project
    cargo ndk -t aarch64-linux-android -t x86_64-linux-android -o ./android/app/src/main/jniLibs build --release --lib
    ```

2.  **Generate Kotlin Bindings:**
    ```bash
    cargo run --bin uniffi-bindgen generate \
      --library target/aarch64-linux-android/release/libcfait.so \
      --language kotlin \
      --out-dir ./android/app/src/main/java \
      --config uniffi.toml
    ```

3.  **Run:** Open the `android` folder in Android Studio and click **Run**.

## Support

If you enjoy using Cfait, consider supporting the developper:

*   💳 **Liberapay:** [https://liberapay.com/trougnouf](https://liberapay.com/trougnouf)
*   🏦 **Bank (SEPA):** `BE77 9731 6116 6342`
*   ₿ **Bitcoin:** `bc1qpecezwmlnzxcqye6nfwv5hn075f7vjf0w3g6gr`
*   Ł **Litecoin:** `ltc1q3xjajxhgmvsth0hwtaz085pr3qml7z8ytjnmkd`
*   Ξ **Ethereum:** `0x0A5281F3B6f609aeb9D71D7ED7acbEc5d00687CB`

## Mirrors

*   **[Codeberg](https://codeberg.org/trougnouf/cfait)** (Primary with Linux Builds and cross-compiled Windows builds)
*   **[GitHub](https://github.com/trougnouf/cfait)** (Mirror with Linux and native Windows builds)
*   **[GitLab](https://gitlab.com/trougnouf/cfait)** (Mirror)

## License
GPL3
