![Cfait -- Take control of your TODO list](https://commons.wikimedia.org/wiki/Special:FilePath/Cfait_featureGraphic_(hardcoded_text).svg)

<p align="center">
  <strong>Cfait is a powerful, fast and elegant task manager. (CalDAV and local, GUI, TUI, and Android clients)
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

**Cfait** is a task manager / TODO list for people who want speed, efficiency, and ownership of their data.

It connects to any standard **CalDAV** server (Radicale, Xandikos, Baikal, Nextcloud, iCloud, etc.) so your tasks aren't locked inside a proprietary walled garden. It's written in **Rust**, meaning it starts instantly and handles large lists without stuttering.

You can use it comfortably from the command line (TUI), on your desktop (GUI), or on the go with the native Android app. It's built "offline-first," so you can keep working without an internet connection and Cfait will sync your changes the next time you go online.

<strong>Table of Contents</strong>

- [✨ Features](#features)
- [📸 Screenshots](#screenshots)
- [🚀 Installation](#installation)
  - [🐧 Linux](#linux)
  - [📱 Android](#android)
  - [🪟 Windows](#windows)
  - [🍎 MacOS](#macos)
  - [⚙️ From Source (Rust)](#from-source-rust)
- [⌨️ Smart Input Syntax](#smart-input-syntax)
- [🔍 Search & Filtering](#search--filtering)
- [📊 Task Sorting](#task-sorting)
- [📅 Calendar Events for Tasks](#calendar-events-for-tasks)
- [💾 Export & Backup](#export-backup)
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
*   **Dependencies:** Block tasks until others are done. You can create parent/child tasks or loose dependencies <small>(RFC9253)</small> (or manually block with `#blocked`).
*   **Recurrence:** Powerful repetition rules for habits and recurrent tasks.
*   **Inline Aliases:** Define shortcuts on the fly; typing `#gardening:=#fun,@@home` or `@@aldi:=#groceries,#shopping` applies the alias immediately and saves it for future use (retroactive).
*   **Cross-Platform:** Runs on Linux, Android, and Windows. (Probably on MacOS too.)

<a name="screenshots"></a>
## 📸 Screenshots

| Desktop (GUI & TUI) | Mobile (Android) |
| :---: | :---: |
| ![Cfait GUI Screenshot](https://commons.wikimedia.org/wiki/Special:FilePath/Cfait_task_manager_v0.3.14_screenshot_(GUI).png)<br>The Graphical Interface in v0.3.14 <small>([history](https://commons.wikimedia.org/wiki/Category:Screenshots_of_Cfait_(GUI)))</small><br><br>![Cfait TUI Screenshot](https://commons.wikimedia.org/wiki/Special:FilePath/Cfait_task_manager_v0.3.14_screenshot_(TUI).png)<br>The Terminal Interface in v0.3.14 <small>([history](https://commons.wikimedia.org/wiki/Category:Screenshots_of_Cfait_(TUI)))</small> | ![Cfait Android Screenshot](https://commons.wikimedia.org/wiki/Special:FilePath/Cfait_task_manager_v0.3.14_screenshot_(Android).png)<br>The Android client in v0.3.14 <small>([history and more](https://commons.wikimedia.org/wiki/Category:Screenshots_of_Cfait_(Android)))</small> |

<a name="installation"></a>
## 🚀 Installation

<a name="linux"></a>
### 🐧 Linux
*   **Flatpak:** Available on [Flathub](https://flathub.org/apps/com.trougnouf.Cfait)
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

<a name="macos"></a>
### 🍎 MacOS
*   Download pre-compiled binaries provided by Martin Stut on https://static.stut.de/cfait-macos/

<a name="from-source-rust"></a>
### ⚙️ From Source (Rust)

#### Desktop (TUI/GUI)
Requires Rust (latest stable version recommended).
```bash
# Install TUI only
cargo install cfait

# Install GUI
cargo install cfait --features gui --bin gui
```
Replace `cfait` with `.` to build locally.

#### Android
Requires [Android NDK](https://developer.android.com/ndk/downloads) and [cargo-ndk](https://github.com/bbqsrc/cargo-ndk).

```bash
# Set up Android NDK environment variables
export ANDROID_NDK_HOME=/path/to/android-ndk
export ANDROID_NDK_ROOT=/path/to/android-ndk

# Build native libraries for Android architectures
cargo ndk -t aarch64-linux-android -t x86_64-linux-android \
  -o ./android/app/src/main/jniLibs build --release --lib --features mobile

# Generate Kotlin bindings
cargo run --features mobile --bin uniffi-bindgen generate \
  --library target/aarch64-linux-android/release/libcfait.so \
  --language kotlin --out-dir ./android/app/src/main/java --config uniffi.toml

# Build APK using Gradle
cd android
./gradlew assembleRelease
```

The APK will be in `android/app/build/outputs/apk/release/`.

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
| **Calendar Event** | `+cal` / `-cal` | Override calendar event creation (per-task). `+cal` forces event creation, `-cal` prevents it. |

You can also type url: (e.g. `url:https://trougnouf.com`), geo: (e.g. `geo:53.046070, -121.105264`), and desc: (e.g. `desc:"a description"` or `desc:{une description}`)

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
*   **Specific weekdays:** `@every <weekday>` or `@every <weekday1>,<weekday2>,...`
    *   `@every monday` (every Monday)
    *   `@every monday,wednesday,friday` (Mon/Wed/Fri)
    *   `@every tue,thu` (Tuesday and Thursday)
    *   Supports short (`mo,tu`), abbreviated (`mon,tue`), or full names (`monday,tuesday`)
*   **Auto-dates:** If you specify recurrence without any dates, both start and due dates are automatically set to the first occurrence.
    *   `Morning routine @daily` → starts and is due today, repeats daily
    *   `Yoga class @every monday` → starts and is due next Monday, repeats weekly
    *   For `@daily`, `@weekly`, etc., the first occurrence is today
    *   For `@every monday`, `@every monday,wednesday,friday`, etc., the first occurrence is the next matching day
*   **End Date:** `until <date>` - Sets an end date for the recurrence (RRULE UNTIL). The end date is **inclusive** (the task will occur on that date).
    *   `@daily until 2025-12-31` (repeats daily until December 31st)
    *   `@every 2 weeks until 2026-06-30` (repeats every 2 weeks until June 30th)
*   **Exception Dates:** `except <value>` - Skips specific occurrences.
    *   **Specific dates:** `@weekly except 2025-01-20` (skips January 20th)
    *   **Comma-separated dates:** `@daily except 2025-12-25,2026-01-01` (skips multiple dates)
    *   **Weekdays:** `@daily except mo,tue` or `@daily except monday,tuesday` or `@daily except saturdays,sundays`
    *   **Months:** `@monthly except oct,nov,dec` or `@weekly except march` (excludes entire months)
    *   **Mixed:** `@monthly except oct,november,dec,january` (short and long forms work together)

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
> `"Water plants @every 3 days until 2025-06-30"` (Every 3 days until end of June)
>
> `"Practice handstands @daily except saturdays,sundays"` (Daily practice, weekdays only)
>
> `"Yoga class @every tue,thu until 2025-12-31"` (Tue/Thu classes until end of year)
>
> `"Water plants @@home @monthly except oct,nov,dec,jan,feb,mar"` (Monthly watering, skip winter months)
>
> `"Gardening @saturday @weekly except march"` (Saturday gardening, skip March entirely)
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

**Note:** If your alias contains spaces, `"`quote it`"` or `{`put it between brockets`}`, e.g. `#"tree planting":=#gardening` or `@@"somewhere else":=#location`. You can define aliases inline while creating tasks, as standalone statements, or in the Settings.

<a name="search--filtering"></a>
## 🔍 Search & Filtering

The search bar isn't just for text. You can use operators (`<`, `>`, `<=`, `>=`) to filter your list precisely.

### Status Filters
*   **`is:ready`** - Shows only actionable tasks right now (not completed/cancelled, start date passed or not set, or blocked)
*   **`is:blocked`** - Shows only blocked tasks (blocked by dependencies or `#blocked` tag - excluded from urgent/due soon/started bins)
*   `is:done` / `is:active` / `is:started`
*   Combine with other filters: `is:ready #work`, `is:ready ~<1h`

### Priority Filters (`!`)
*   `!<2` (Priority 1 only - Critical)
*   `!>=5` (Normal or lower priority)

### Date Filters (`@` / `^`)
Date filters support **relative dates** for both due (`@`) and start (`^`) dates, plus a **"not set" operator** (`!`):

*   **Overdue/Past:**
    *   `@<today` (Overdue tasks)
    *   `^<today` (Started before today)
*   **Future:**
    *   `@>tomorrow` (Due after tomorrow)
    *   `^>1w` (Start more than 1 week from now)
*   **Relative dates:**
    *   `@<=2d` (Due within the next 2 days)
    *   `^<5d` (Start within the next 5 days)
*   **"Not Set" operator** (trailing `!`):
    *   `@<today!` (Overdue OR no due date)
    *   `^>1w!` (Start later than 1 week OR no start date)
    *   `@<=2025-12-31!` (Due before Dec 31 OR no due date)

### Duration Filters (`~`)
*   `~<30m` (Quick tasks, less than 30 mins)
*   `~>2h` (Long tasks)

### Tag Filters
*   `#gardening` (Contains this tag)
*   `#work:project` (Matches tag or any sub-tag like `#work:project:urgent`)

### Location Filters
*   `@@home` (Matches location field)
*   `@@store:aldi` (Matches location or any sub-location like `@@store:aldi:downtown`)

### Combining Filters
You can combine multiple filters: `is:ready !<4 ~<1h #gardening` (actionable high-priority gardening tasks under an hour).

<a name="task-sorting"></a>
## 📊 Task Sorting

Cfait organizes tasks in the following order:

1. **🔴 Urgent tasks** (priority ≤ 1 by default)
2. **⏰ Due soon** (due today or tomorrow by default)
3. **▶️ Started tasks** (status: in-process) - Sorted by due date, then priority
4. **📅 Standard tasks** (within sorting cutoff) - Sorted by due date, then priority
5. **📋 Remaining tasks** (outside cutoff or no date) - Sorted by priority, then name
6. **🔮 Future tasks** (start date not yet reached)
7. **✅ Done/Cancelled** - Completed or cancelled tasks

**Within each rank:** Tasks sort by priority → due date → name (except ranks 2, 3 & 4 which sort by due date first).

**Notes:**
- Priority 0 (unset) is treated as priority 5 (medium)
- Future start dates move tasks to rank 6, even if they have urgent priority
- Thresholds for "urgent", "due soon", and "cutoff" are configurable in settings

<a name="calendar-events-for-tasks"></a>
## 📅 Calendar Events for Tasks

Cfait can automatically create calendar events (VEVENT) for tasks with dates, making them visible in any CalDAV calendar app.

**Enable:** 
- **GUI/Android:** Toggle "Create calendar events for tasks with dates" in Settings
- **TUI:** Add `create_events_for_tasks = true` to `~/.config/cfait/config.toml`

When you toggle this setting on, events will be retroactively created for all existing tasks with start and/or due dates.

**Per-Task Control:** Use `+cal` to force enable or `-cal` to disable for specific tasks:
```
Playing Terraforming Mars ^tomorrow 2pm ~4h +cal
Very private task @tomorrow -cal
```

**Behavior:**
- Creates/updates events when tasks have dates
- Events are always deleted (or moved) when tasks are deleted (or moved)
- Optional: Delete events when tasks are completed or cancelled (toggle in Settings, default: keep)

**Events Cleanup:**
- Use the "Delete all calendar events" button in the GUI or Android Settings to remove all auto-generated events

<a name="export-backup"></a>
## 💾 Export & Backup

Export your local tasks to standard `.ics` (iCalendar) format for backup or sharing with other applications.

**TUI (Command Line):**
```bash
# Export to file
cfait export > backup.ics

# View export content
cfait export

# Pipe to other tools
cfait export | grep 'SUMMARY'
```

**GUI (Desktop):**
1. Open Settings (gear icon)
2. Scroll to "Data Management" section
3. Click "Export Local Tasks (.ics)"
4. Choose save location in file dialog

**Android:**
1. Open Settings
2. Scroll to "Data Management" section  
3. Tap "Export Local Tasks (.ics)"
4. Choose where to save/share (Google Drive, Email, Files, etc.)

The exported `.ics` any CalDAV-compatible application.

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
*   `l`: **Link** selection as **Related** to yanked task
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
