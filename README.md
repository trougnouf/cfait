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
  <a href="https://liberapay.com/trougnouf/donate"><img src="https://img.shields.io/liberapay/patrons/trougnouf?label=donate" alt="Donate through Liberapay" /></a>
</p>

---

**Cfait** is a task manager for people who want speed, efficiency, and ownership of their data.

It connects to any standard **CalDAV** server (Radicale, Xandikos, Baikal, Nextcloud, iCloud, etc.) so your tasks aren't locked inside a proprietary walled garden. It's written in **Rust**, meaning it starts instantly and handles large lists without stuttering.

You can use it comfortably from the command line (TUI), on your desktop (GUI), or on the go with the native Android app. It's built "offline-first," so you can keep working without an internet connection and Cfait will sync your changes the next time you go online.

<strong>Table of Contents</strong>

- [✨ Features](#features)
- [📸 Screenshots](#screenshots)
- [🚀 Installation](#installation)
  - [🐧 Linux](#linux)
  - [📱 Android](#android)
  - [🪟 Windows](#windows)
  - [⚙️ From Source (Rust)](#from-source-rust)
- [⌨️ Smart Input Syntax](#smart-input-syntax)
- [🔍 Search & Filtering](#search--filtering)
- [🎮 TUI Keybindings](#tui-keybindings)
- [🤗 Support](#support)
- [🪩 Mirrors](#mirrors)
- [🛡️ Privacy Policy](#privacy-policy)
- [⚖️ License](#license)
- [☁️ CalDAV Providers](#caldav-providers)

</details>

<a name="features"></a>
## ✨ Features

*   **Smart Input:** Type your tasks naturally. `Buy cookies @tomorrow @@bakery !1` is parsed instantly into a high-priority task due tomorrow at the bakery.
*   **Hierarchical Tags & Locations:** Organize deeply with tags like `#dev:cfait` or `#cooking:cookies`, and locations like `@@home:office` or `@@store:aldi:downtown`.
*   **Dependencies:** Block tasks until others are done. You can create parent/child tasks or loose dependencies <small>(RFC9253)</small>.
*   **Recurrence:** Powerful repetition rules for habits and recurrent tasks.
*   **Inline Aliases:** Define shortcuts on the fly; typing `#gardening:=#fun,@@home` or `@@aldi:=#groceries,#shopping` applies the alias immediately and saves it for future use (retroactive).
*   **Cross-Platform:** Runs on Linux, Android, and Windows. (Probably on MacOS too.)

<a name="screenshots"></a>
## 📸 Screenshots

| Desktop (GUI & TUI) | Mobile (Android) |
| :---: | :---: |
| ![Cfait GUI Screenshot](https://commons.wikimedia.org/wiki/Special:FilePath/Cfait_task_manager_v0.3.13_screenshot_(GUI).png)<br>The Graphical Interface in v0.3.13 <small>([history](https://commons.wikimedia.org/wiki/Category:Screenshots_of_Cfait_(GUI)))</small><br><br>![Cfait TUI Screenshot](https://commons.wikimedia.org/wiki/Special:FilePath/Cfait_task_manager_v0.3.3_screenshot_(TUI).png)<br>The Terminal Interface in v0.3.3 <small>([history](https://commons.wikimedia.org/wiki/Category:Screenshots_of_Cfait_(TUI)))</small> | ![Cfait Android Screenshot](https://commons.wikimedia.org/wiki/Special:FilePath/Cfait_task_manager_v0.3.3_screenshot_(Android).png)<br>The Android client in v0.3.3 <small>([history](https://commons.wikimedia.org/wiki/Category:Screenshots_of_Cfait_(Android)))</small> |

<a name="installation"></a>
## 🚀 Installation

<a name="linux"></a>
### 🐧 Linux
*   **Arch Linux (AUR):** `yay -S cfait` (or `cfait-git`)
*   **Debian/Ubuntu/Mint:** Download the `.deb` file from the [releases page](https://codeberg.org/trougnouf/cfait/releases). (Req. Ubuntu 24.04+ / Mint 22+ / Debian 13+)
*   **Generic:** Download the pre-compiled `.tar.gz` binary tarball from the [releases page](https://codeberg.org/trougnouf/cfait/releases). (Req. `glibc 2.39`, e.g. Fedora 40+)

<a name="android"></a>
### 📱 Android
*   **<a href="https://f-droid.org/packages/com.cfait/">F-Droid</a>**
*   **Google Play:** Submitted, currently in testing. <span style="color:red;">More testers are needed for inclusion in the Play Store, please <a href='mailto:trougnouf@gmail.com'>contact me</a></span>.
*   **APK:** Download the latest universal APK from the [releases page](https://codeberg.org/trougnouf/cfait/releases).

<a name="windows"></a>
### 🪟 Windows
*   Download the `.zip` archive from the [releases page](https://codeberg.org/trougnouf/cfait/releases). Contains both `cfait.exe` (TUI) and `cfait-gui.exe` (GUI).

<a name="from-source-rust"></a>
### ⚙️ From Source (Rust)
Requires standard system libraries (openssl, alsa, fontconfig, x11, xkbcommon).
```bash
# Install TUI only
cargo install cfait

# Install GUI
cargo install cfait --features gui --bin gui
```
Replace `cfait` with `.` to build locally.

<a name="smart-input-syntax"></a>
## ⌨️ Smart Input Syntax

You don't need to click through menus to set the due/start date, length, priority, recurrence, tags, location,... Just type.

### Basics
| Property | Syntax | Description |
| :--- | :--- | :--- |
| **Priority** | `!1` | 1 is highest (critical), 9 is lowest. 5 is normal. |
| **Due Date** | `@` / `due:` | When the task must be finished. |
| **Start Date** | `^` / `start:` | When you plan to start (hides from "active" views until then). |
| **Recurrence** | `@` / `rec:` | How often the task repeats. |
| **Duration** | `~` / `est:` | Estimated time to complete. |
| **Tag** | `#` | Categories. Use `:` for hierarchy (e.g. `#gardening:tree_planting`). |
| **Location** | `@@` / `loc:` | Where the task happens. Supports hierarchy like tags (e.g. `@@home:office`, `@@store:aldi:downtown`). |
| **Reminder** | `rem:` | Set an notification. (e.g. `rem:10m`, `rem:8am`, `rem:tomorrow 9:00`). |

You can also type url: (e.g. `url:https://www.trougnouf.com`), geo: (e.g. `geo:53.046070, -121.105264`), and desc: (e.g. `desc:"a description"` or `desc:{une description}`)

**Escaping:** If you need to use special characters literally in your task summary (like `#`, `@`, `!`), prefix them with a backslash: `\#not-a-tag \@not-a-date`.

### Date & Time Formats
You can use absolute ISO dates or natural language relative offsets.
*   **Keywords:** `today`, `tomorrow`
*   **Offsets:** `1d` (days), `1w` (weeks), `1mo` (months), `1y` (years).
    *   `@2d` = Due in 2 days.
    *   `^1w` = Start in 1 week.
    *   The word "in" is optional: `@2 weeks` works the same as `@in 2 weeks`
*   **Weekdays:** `@friday`, `@monday`, etc. (or with "next": `@next friday`)
    *   Both forms work identically - they always go to the **next** occurrence of that weekday
*   **Next period:** `@next week`, `@next month`, `@next year`
    *   Goes to the next occurrence of that time period

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

### Reminders
Set alarms to notify you about tasks. Reminders can be **relative** (recalculated when due date changes) or **absolute** (fixed time).
*   **Relative (to due date):** `rem:10m` = 10 minutes before due date, `rem:1h` = 1 hour before due date
    *   These automatically adjust if you change the task's due date
*   **Relative (from now):** `rem:in 5m` = 5 minutes from now, `rem:in 2h` = 2 hours from now
    *   Set as absolute time when task is created (doesn't adjust with due date)
*   **Next occurrence:** `rem:next friday` = Next Friday at default time, `rem:next week` = 7 days from now
    *   Set as absolute time at the next occurrence (doesn't adjust with due date)
*   **Absolute (fixed time):** `rem:8am` = Today (2025-01-15) at 8am, `rem:2025-01-20 9am` = January 20th at 9am
    *   These stay at the specified time regardless of due date changes
*   **Date + Time:** `rem:2025-12-31 10:00` (Absolute: specific date and time)

### Examples
> `"Buy cookies !1 @2025-01-16 #shopping rem:2025-01-16 8am"`
>
> `"Exercise @daily ~30m #health rem:8am"`
>
> `"Update server certificates @2025-12-31 ^2025-12-01 @every 2 years rem:1w"` (Due Dec 31, start working on it 1 month prior, reminder 1 week before)
>
> `"Plant plum tree #tree_planting !3 ~2h"` and `"#tree_planting:=#gardening,@@home"`

The syntax highlighting should visually let you know whether your statements are valid.

### Aliases (Templates)
Define global shortcuts using `:=`. Aliases can inject tags, locations, priorities, or other properties. This applies to the past, present, and future tasks. (It may take some time to update all affected tasks.)

**Tag Aliases:**
*   **Define:** `#tree_planting:=#gardening,@@home,!3`
*   **Use:** Typing `Plant plum tree #tree_planting ~1h` expands to:
    *   Tags: `#tree_planting #gardening`
    *   Location: "home"
    *   Priority: 3

**Location Aliases:**
*   **Define:** `@@aldi:=#groceries,#shopping` or `loc:aldi:=#groceries,#shopping`
*   **Use:** Typing `Buy milk @@aldi` expands to:
    *   Location: "aldi"
    *   Tags: `#groceries #shopping`

**Hierarchical Aliases:**
Both tags and locations support hierarchy. Child locations/tags automatically inherit parent aliases.
*   `#gardening:tree_planting` → matches both `#gardening:tree_planting` and parent `#gardening` aliases
*   `@@store:aldi:downtown` → matches `@@store:aldi:downtown`, `@@store:aldi`, and parent `@@store` aliases

**Note:** If your alias contains spaces, `"`quote it`"` or `{`put it between brockets`}`, e.g. `#"tree planting":=#gardening` or `@@"somewhere else":=#location`. You can define aliases inline while creating tasks, but it's often clearer to define them separately in Settings.

<a name="search--filtering"></a>
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
    *   `#work:project` (Matches tag or any sub-tag like `#work:project:urgent`)
*   **Location:** 
    *   `@@home` (Matches location field)
    *   `@@store:aldi` (Matches location or any sub-location like `@@store:aldi:downtown`)

You can combine them: `!<4 ~<1h #gardening` (high priority gardening task that takes less than an hour).

<a name="tui-keybindings"></a>
## 🎮 TUI Keybindings

If you are using the Terminal interface, here are the essentials (*Press `?` inside the app for the full interactive help menu.*).

**Navigation & Views**
*   `Tab`: Switch focus (Tasks ↔ Sidebar)
*   `j` / `k`: Move selection Down / Up
*   `1` / `2` / `3`: Switch Sidebar (Calendars / Tags / Locations)
*   `/`: Search tasks

**Task Management**
*   `a`: **Add** task
*   `e` / `E`: **Edit** title / **Edit** description
*   `Space`: Toggle **Done** status
*   `s`: Toggle **Start / Pause**
*   `S`: **Stop** (Reset to Needs Action)
*   `x`: **Cancel** task
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

**Note:** The sidebar shows hierarchical tags and locations. For example, if you have tasks with `#work:project:urgent` and `#work:meeting`, they'll be organized under the `#work` parent in the sidebar.

The GUI also supports `/` for search and `a` for adding tasks.

<a name="support"></a>
## 🤗 Support

If you enjoy using Cfait, consider supporting the developper:

*   💳 **Liberapay:** [https://liberapay.com/trougnouf](https://liberapay.com/trougnouf)
*   🏦 **Bank (SEPA):** `BE77 9731 6116 6342`
*   ₿ **Bitcoin:** `bc1qc3z9ctv34v0ufxwpmq875r89umnt6ggeclp979`
*   Ł **Litecoin:** `ltc1qv0xcmeuve080j7ad2cj2sd9d22kgqmlxfxvhmg`
*   Ξ **Ethereum:** `0x0A5281F3B6f609aeb9D71D7ED7acbEc5d00687CB`

<a name="mirrors"></a>
## 🪩 Mirrors

*   **[Codeberg](https://codeberg.org/trougnouf/cfait)** (Primary with Linux, Android, and cross-compiled Windows builds)
*   **[GitHub](https://github.com/trougnouf/cfait)** (Mirror with Linux and native Windows builds)
*   **[GitLab](https://gitlab.com/trougnouf/cfait)** (Mirror)

<a name="privacy-policy"></a>
## 🛡️ Privacy Policy

Cfait does not collect data; data is stored on your device and on your CalDAV server.

<a name="license"></a>
## ⚖️ License
GPL3

<a name="caldav-providers"></a>
## ☁️ CalDAV Providers

Cfait works with any standard CalDAV server. If you don't have one yet, here are some suggestions:

**Self-Hosted**
*   **[Radicale](https://radicale.org/):** One of the easiest, lightweight solution to host on a Raspberry Pi or VPS.
*   **[Nextcloud](https://nextcloud.com/):** A popular full-suite option (files, contacts, and calendars).

**Free & Managed**
*   **[Infomaniak](https://www.infomaniak.com/):** A Swiss provider with a free tier that includes a CalDAV account.
    *   *How to connect:* After signing up, go to [config.infomaniak.com](https://config.infomaniak.com/). Click **"On this device"** followed by **"My Calendars"** to reveal your specific Server URL and Login username. (Use your infomaniak password.)

You can also use the `Local` calendar entirely offline (and there is the possibility to migrate to and synchronize with a CalDAV server at a later time).

## 💬 Community & Support

Have a question, found a bug, a great idea, or just want to chat?

*   **🗨️ Chat:** on [#Cfait:matrix.org](https://matrix.to/#/#Cfait:matrix.org).
*   **🐛 Report a Bug / ✨ Request a Feature:** [Open an issue on Codeberg](https://codeberg.org/trougnouf/cfait/issues) (or [Github](https://github.com/trougnouf/cfait/issues)).
