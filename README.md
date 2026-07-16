![Cfait -- Take control of your TODO list](https://commons.wikimedia.org/wiki/Special:FilePath/Cfait_featureGraphic_(hardcoded_text).svg)

<p align="center">
  <strong>A fast and powerful, offline-first task manager for your terminal, desktop, and phone.</strong>
</p>

<p align="center">
  <a href="https://codeberg.org/trougnouf/cfait/releases"><img src="https://codeberg.org/trougnouf/cfait/badges/release.svg" alt="Releases"></a>
  <a href="https://f-droid.org/packages/com.trougnouf.cfait/"><img src="https://img.shields.io/f-droid/v/com.trougnouf.cfait.svg" alt="F-Droid"></a>
  <a href="https://codeberg.org/trougnouf/cfait/actions"><img src="https://codeberg.org/trougnouf/cfait/badges/workflows/test_roll.yml/badge.svg" alt="Test status"></a>
  <a href="https://codeberg.org/trougnouf/cfait/src/branch/main/LICENSE"><img src="https://img.shields.io/badge/license-GPLv3-gray.svg" alt="License"></a>
  <a href="https://liberapay.com/trougnouf/donate"><img src="https://img.shields.io/liberapay/patrons/trougnouf?label=donate" alt="Donate through Liberapay" /></a>
</p>

---

**Cfait** is built for people who want keyboard-centric efficiency.

You can use it comfortably from the command line (TUI), on your desktop (GUI), or on the go with the native Android app. Because it is built entirely **offline-first**, you can manage your tasks perfectly without ever connecting to a server. 

If you *do* want to sync your tasks across devices, Cfait connects seamlessly to any standard **CalDAV** server (Nextcloud, Radicale, Baikal, ...) without locking your data inside a proprietary walled garden. Backed by a shared Rust core, it starts instantly and handles thousands of tasks without stuttering.

### ✨ Highlights

* ⚡ **Think it, type it:** No clicking through menus. Type `Buy green tea @tomorrow !1 @@dharma_city` to create a high-priority task, due tomorrow, at a specific location.
* 🧠 **Deep organization:** Go beyond flat lists with hierarchical tags (`#gardening:kiwai`), blocking dependencies, and parent/child task trees.
* ⏱️ **Time & goals:** Start/pause tasks to track time spent. Set estimated durations (`~2h`) or recurring habit goals (`#read:book:=goal:5/y`).
* 🪄 **Dynamic aliases:** Define shortcuts on the fly. Typing `#hiking:=#exercise,@@outside` applies the alias instantly and saves it for future use.

### 📸 Glimpse

| Desktop (GUI & TUI) | Mobile (Android) |
| :---: | :---: |
| ![Cfait GUI Screenshot](https://commons.wikimedia.org/wiki/Special:FilePath/Cfait_task_manager_v0.5.2_screenshot_(GUI).png)<br>The Graphical Interface in v0.5.2 <small>([history](https://commons.wikimedia.org/wiki/Category:Screenshots_of_Cfait_(GUI)))</small><br><br>![Cfait TUI Screenshot](https://commons.wikimedia.org/wiki/Special:FilePath/Cfait_task_manager_v0.5.0_screenshot_(TUI).png)<br>The Terminal Interface in v0.5.0 <small>([history](https://commons.wikimedia.org/wiki/Category:Screenshots_of_Cfait_(TUI)))</small> | ![Cfait Android Screenshot](https://commons.wikimedia.org/wiki/Special:FilePath/Cfait_task_manager_v0.5.2_screenshot_(Android).png)<br>The Android client in v0.5.2 <small>([history](https://commons.wikimedia.org/wiki/Category:Screenshots_of_Cfait_(Android)))</small> |

### 📖 Documentation & usage

Cfait is designed to keep you in flow. Instead of a massive wiki, **the app is entirely self-documenting.**

* **Everyday usage:** Press `?` (in the TUI/GUI) or navigate to the Help tab (Android) to open the interactive syntax guide and keyboard shortcut cheat sheet.
* **Configuration:** The CLI/TUI configuration file (`~/.config/cfait/config.toml`) is completely self-documenting. Open it in any text editor to see all available options, headless daemon setup, and UI tweaks.
* **Under the hood:** Want to see the exact sorting algorithms, how we handle CalDAV merge conflicts, or how our Markdown subtask extraction works? Read our [SPECS.md](./SPECS.md) — it is the ultimate, up-to-date source of truth for Cfait's architecture.

### 🚀 Installation

We offer both **Stable** and **Rolling** releases. We highly encourage users to try the rolling release to get the latest features and report bugs! 

*(Note: On our Codeberg releases page, the "Rolling" tag stays pinned at the very top. To find the latest stable version, scroll past it and click "Downloads" on the numbered release).*

* **🐧 Linux:** Available on [Flathub](https://flathub.org/apps/com.trougnouf.Cfait), the AUR (`yay -S cfait`), or via `.deb` / `.tar.gz` on our [Releases page](https://codeberg.org/trougnouf/cfait/releases).
* **👺 FreeBSD:** Available in [Ports/Packages](https://www.freshports.org/deskutils/cfait/), install with `pkg install cfait`
* **📱 Android:** Get it on [F-Droid](https://f-droid.org/packages/com.trougnouf.cfait/), [Google Play](https://play.google.com/store/apps/details?id=com.trougnouf.cfait), or download the APK.
* **🪟 Windows:** Check the [Releases page](https://codeberg.org/trougnouf/cfait/releases) for binaries.
* **🍎 MacOS:** Download pre-compiled binaries provided by Martin Stut on https://static.stut.de/cfait-macos/
* **⚙️ Rust (Cargo):** `cargo install cfait` (TUI) or `cargo install cfait --features gui --bin cfait-gui` (GUI).

### ☁️ CalDAV providers

You can use the default `Local` collection entirely offline. But if you want to sync, Cfait works with standard CalDAV servers. We recommend:
* **Self-hosted:** [Radicale](https://radicale.org/) (lightweight and feature-complete, runs on anything including Raspberry Pi) or [Nextcloud](https://nextcloud.com/).
* **Free & managed:** [Disroot](https://disroot.org/) offers a privacy-focused platform with free nextcloud-based CalDAV access.

### 🛡️ Privacy Policy

Cfait does not collect data; data is stored on your device and on your CalDAV server.

### 🤗 Community & support

Have a question, found a bug, or want to contribute?
* **🗨️ Chat:** Join us on Matrix at [#Cfait:matrix.org](https://matrix.to/#/#Cfait:matrix.org).
* **🐛 Bugs / ✨ Features:** [Open an issue on Codeberg](https://codeberg.org/trougnouf/cfait/issues).
* **🛠️ Contribute code:** Check out our [CONTRIBUTING.md](./CONTRIBUTING.md) to get started!
* **🌐 Translate:** Help translate Cfait into your language on [Codeberg Translate](https://translate.codeberg.org/projects/cfait/).

If Cfait helps you stay on top of your life, consider supporting development:

*   💳 **Liberapay:** [https://liberapay.com/trougnouf](https://liberapay.com/trougnouf)
*   💳 **Ko-fi:** [https://ko-fi.com/trougnouf](https://ko-fi.com/trougnouf)
*   🏦 **Bank (SEPA):** `BE77 9731 6116 6342`
*   ₿ **Bitcoin:** `bc1qc3z9ctv34v0ufxwpmq875r89umnt6ggeclp979`
*   Ł **Litecoin:** `ltc1qv0xcmeuve080j7ad2cj2sd9d22kgqmlxfxvhmg`
*   Ξ **Ethereum:** `0x0A5281F3B6f609aeb9D71D7ED7acbEc5d00687CB`

<p align="center">
  <small>Released under the <strong>GPL-3.0 License</strong></small>
</p>
