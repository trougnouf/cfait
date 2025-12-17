

![Cfait -- Take control of your TODO list](https://commons.wikimedia.org/wiki/Special:FilePath/Cfait_featureGraphic_(hardcoded_text).svg)

<p align="center">
  <strong>Cfait is a powerful, fast and elegant CalDAV task manager.
</strong>
</p>

<p align="center">
  <a href="https://codeberg.org/trougnouf/cfait/releases"><img src="https://codeberg.org/trougnouf/cfait/badges/release.svg" alt="Releases"></a>
  <a href="https://f-droid.org/packages/com.cfait/"><img src="https://img.shields.io/f-droid/v/com.cfait.svg" alt="F-Droid"></a>
  <a href="https://codeberg.org/trougnouf/cfait/actions"><img src="https://codeberg.org/trougnouf/cfait/badges/workflows/test.yml/badge.svg" alt="Test status"></a>
  <a href="https://codeberg.org/trougnouf/cfait/src/branch/main/LICENSE"><img src="https://img.shields.io/badge/license-GPLv3-gray.svg" alt="License"></a>
</p>

---

**Cfait** is a task manager for people who want speed, efficiency, and ownership of their data.

It connects to any standard **CalDAV** server (Radicale, Xandikos, Nextcloud, iCloud, etc.) so your tasks aren't locked inside a proprietary walled garden. It's written in **Rust**, meaning it starts instantly and handles large lists without stuttering.

You can use it comfortably from the command line (TUI), on your desktop (GUI), or on the go with the native Android app. It's built "offline-first," so you can keep working without an internet connection and Cfait will sync your changes the next time you go online.

## ✨ Features

*   **Smart Input:** Type your tasks naturally. `Buy cookies @tomorrow !1` is parsed instantly into a high-priority task due tomorrow.
*   **Hierarchical Tags:** Organize deeply with tags like `#dev:cfait` or `#cooking:cookies`.
*   **Dependencies:** Block tasks until others are done. You can create parent/child tasks or loose dependencies (`y` to yank, `b` to block).
*   **Recurrence:** Powerful repetition rules for habits and recurrent tasks.
*   **Inline Aliases:** Define shortcuts on the fly; typing `#groceries=#home,#shopping` applies the alias immediately and saves it for future use.
*   **Cross-Platform:** Runs on Linux, Windows, and Android. (Probably on MacOS too.)

## 📸 Screenshots

| Desktop (GUI & TUI) | Mobile (Android) |
| :---: | :---: |
| ![Cfait GUI Screenshot](https://commons.wikimedia.org/wiki/Special:FilePath/Cfait_task_manager_v0.3.0_screenshot_(GUI).png)<br>The Graphical Interface in v0.3.0 <small>([history](https://commons.wikimedia.org/wiki/Category:Screenshots_of_Cfait_(GUI)))</small><br><br>![Cfait TUI Screenshot](https://commons.wikimedia.org/wiki/Special:FilePath/Cfait_task_manager_v0.3.0_screenshot_(TUI).png)<br>The Terminal Interface in v0.3.0 <small>([history](https://commons.wikimedia.org/wiki/Category:Screenshots_of_Cfait_(TUI)))</small> | ![Cfait Android Screenshot](https://commons.wikimedia.org/wiki/Special:FilePath/Cfait_task_manager_v0.3.0_screenshot_(Android).png)<br>The Android client in v0.3.0 <small>([history](https://commons.wikimedia.org/wiki/Category:Screenshots_of_Cfait_(Android)))</small> |

## 🚀 Installation

### Linux
*   **Arch Linux (AUR):** `yay -S cfait` (or `cfait-git`)
*   **Debian/Ubuntu/Mint:** Download the `.deb` file from the [releases page](https://codeberg.org/trougnouf/cfait/releases). (Req. Ubuntu 24.04+ / Mint 22+ / Debian 13+)
*   **Generic:** Download the pre-compiled `.tar.gz` binary tarball from the [releases page](https://codeberg.org/trougnouf/cfait/releases). (Req. `glibc 2.39`, e.g. Fedora 40+)

### Android
*   **F-Droid:** Available in the official repository.
*   **Google Play:** Currently in testing. More testers are needed for inclusion in the Play Store, please contact me.
*   **APK:** Download the latest universal APK from the [releases page](https://codeberg.org/trougnouf/cfait/releases).

### Windows
*   Download the `.zip` archive from the [releases page](https://codeberg.org/trougnouf/cfait/releases). Contains both `cfait.exe` (TUI) and `cfait-gui.exe` (GUI).

### From Source (Rust)
Requires standard system libraries (openssl, alsa, fontconfig, x11, xkbcommon).
```bash
# Install TUI only
cargo install cfait

# Install GUI
cargo install cfait --features gui --bin gui
```
Replace `cfait` with `.` to build locally.


## ⌨️ Smart Input Syntax

You don't need to click through menus to set dates or priorities. Just type them.

### Basics
| Property | Short | Long | Description |
| :--- | :--- | :--- | :--- |
| **Priority** | `!1` | - | 1 is highest (critical), 9 is lowest. 5 is normal. |
| **Due Date** | `@` | `due:` | When the task must be finished. |
| **Start Date** | `^` | `start:` | When you plan to start (hides from "active" views until then). |
| **Recurrence** | `@` | `rec:` | How often the task repeats. |
| **Duration** | `~` | `est:` | Estimated time to complete. |
| **Tag** | `#` | - | Categories. Use `:` for hierarchy (e.g. `#work:admin`). |

### Date & Time Formats
You can use absolute ISO dates or natural language relative offsets.
*   **Keywords:** `today`, `tomorrow`
*   **Offsets:** `1d` (days), `1w` (weeks), `1mo` (months), `1y` (years).
    *   `@2d` = Due in 2 days.
    *   `^1w` = Start in 1 week.

### Recurrence
Recurrence rules determine when the next task is created after you complete the current one.
*   **Presets:** `@daily`, `@weekly`, `@monthly`, `@yearly`.
*   **Custom:** `@every X unit`.
    *   `@every 3 days`
    *   `@every 2 weeks`

### Duration Units
Supported units for `~` duration estimates: `m` (minutes), `h` (hours), `d` (days), `w` (weeks), `mo` (months), `y` (years).
*   `~15m` (15 minutes)
*   `~1.5h` (1 hour 30 minutes)

### Examples
> `"Buy cookies !1 @tomorrow #groceries"`
>
> `"Team meeting @daily ~1h #work"`
>
> `"Update server certificates @2025-12-31 ^2025-12-01 @every 2 years"` (Due Dec 31, start working on it 1 month prior)



## 🔍 Search & Filtering

The search bar isn't just for text. You can use operators (`<`, `>`, `<=`, `>=`) to filter your list precisely.

*   **Status:**
    *   `is:done` / `is:active` / `is:ongoing`
*   **Priority (`!`):**
    *   `!<2` (Priority 1 only - Critical)
    *   `!>=5` (Normal or lower priority)
*   **Dates (`@` / `^`):**
    *   `@<today` (Overdue tasks)
    *   `@>tomorrow` (Due after tomorrow)
    *   `@<=2d` (Due within the next 2 days)
*   **Duration (`~`):**
    *   `~<30m` (Quick tasks, less than 30 mins)
    *   `~>2h` (Long tasks)
*   **Tags:**
    *   `#gardening` (Contains this tag)

You can combine them: `!<4 ~<1h #gardening` (high priority gardening task that takes less than an hour).

## 🎮 TUI Keybindings

If you are using the Terminal interface, here are the essentials (*Press `?` inside the app for the full interactive help menu.*).

**Navigation & Views**
*   `Tab`: Switch focus (Tasks ↔ Sidebar)
*   `j` / `k`: Move selection Down / Up
*   `1` / `2`: Switch Sidebar View (Calendars / Tags)
*   `/`: Search tasks

**Task Management**
*   `a`: **Add** task
*   `e` / `E`: **Edit** title / **Edit** description
*   `Space`: Toggle **Done** status
*   `s` / `x`: Mark **Started** / **Cancelled**
*   `d`: **Delete** task

**Organization & Hierarchy**
*   `y`: **Yank** task ID (Copy)
*   `b`: Mark selection as **Blocked** by yanked task
*   `c`: Make selection a **Child** of yanked task
*   `>` / `<`: Indent / Outdent (visual depth)
*   `+` / `-`: Adjust Priority

**Sidebar Actions**
*   `Enter`: Toggle filter / Select calendar
*   `Space`: Toggle visibility (show/hide layer)
*   `*`: Isolate (hide all others)

The GUI also supports `/` for search and `a` for adding tasks.

## Support

If you enjoy using Cfait, consider supporting the developper:

*   💳 **Liberapay:** [https://liberapay.com/trougnouf](https://liberapay.com/trougnouf)
*   🏦 **Bank (SEPA):** `BE77 9731 6116 6342`
*   ₿ **Bitcoin:** `bc1qc3z9ctv34v0ufxwpmq875r89umnt6ggeclp979`
*   Ł **Litecoin:** `ltc1qv0xcmeuve080j7ad2cj2sd9d22kgqmlxfxvhmg`
*   Ξ **Ethereum:** `0x0A5281F3B6f609aeb9D71D7ED7acbEc5d00687CB`

## Mirrors

*   **[Codeberg](https://codeberg.org/trougnouf/cfait)** (Primary with Linux, Android, and cross-compiled Windows builds)
*   **[GitHub](https://github.com/trougnouf/cfait)** (Mirror with Linux and native Windows builds)
*   **[GitLab](https://gitlab.com/trougnouf/cfait)** (Mirror)

## Privacy Policy

Cfait does not collect data; data is stored on your device and on your CalDAV server.

## License
GPL3