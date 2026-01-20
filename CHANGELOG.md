# Changelog

## [0.4.8] - 2026-01-20

### 🚀 Features

- *(tasks)* Implement time estimation ranges (e.g. ~10m-3h) with compatible search logic
- *(gui)* Initial work on keyboard shortcuts ( https://codeberg.org/trougnouf/cfait/issues/9 )
- *(gui)* Towards keyboard shortcuts
- *(gui)* Expandable description field with scroll bar
- *(android)* Parse new alias definitions in task input bar
- *(gui)* Double click to edit a task ( https://codeberg.org/trougnouf/cfait/issues/9 )
- *(tui)* Improved task editing experience with enter for newline, basic markdown highlighting, and markdown viewer in the task details view (with plain-text auto-detection to maintain basic newlines) ( https://codeberg.org/trougnouf/cfait/issues/14 )
- *(tui)* Support horizontal scrolling in the description editor and multi-byte characters ( https://codeberg.org/trougnouf/cfait/issues/14 )
- *(config)* Differentiate between corrupted and missing config files ( https://codeberg.org/trougnouf/cfait/issues/13 )

### 🐛 Bug Fixes

- *(android)* Maintain keyboard after adding task and jump to task instantly instead of with animation
- *(recurrence)* Preserve task UID on cancellation to prevent duplication
- *(tui)* Restore onboarding ( https://codeberg.org/trougnouf/cfait/issues/13 )
- *(gui)* Load iced_aw fonts to render correct icons in color picker (reported by beh bah)
- *(android)* Disable keyboard autocorrect for login fields and enable url / password mode ( https://codeberg.org/trougnouf/cfait/issues/15 )
- *(android)* Fix cursor position in new task field and adjust size of task description field when keyboard is out
- *(tests)* Properly isolate all write tests in a temporary directory (reported by Geoffrey Frogeye)
- *(model)* Fix stuck recurring tasks by normalizing AllDay/EXDATE calculations to UTC
- *(gui)* Auto-scroll to task (linked or up/down)

### ⚡ Performance

- Pre-calculate and cache is_blocked status and parent attributes (fixes O(N^2) rendering lag)

### 🎨 Styling

- Use consistant style for start - due date(time) (hourglass_empty - hourglass_full), avoid identical start/due date(time) repetitions
- *(gui)* Restore calendar icon for due date(time) when it's the only date(time) shown

### ⚙️ Miscellaneous Tasks

- *(android)* Bump min SDK to 28 (Android 9) due to uniffi v0.31 update
- Move Cargo.lock update to every release and disable check
## [0.4.7] - 2026-01-13

### 🚀 Features

- Show start date (if not yet started) in tasks list
- Add ^@ syntax (sets both start and due date(time)). sorting: if parent is canceled/paused/done, use that for the whole group
- Implement multi-select filtering and focus for tags & locations, make the clear icon always visible, improve UI and make it consistent between GUI and Android
-  feat: implement multi-select filtering and focus for tags & locations and make the clear icon always visible on Android (feature parity with GUI)

### 🐛 Bug Fixes

- Ignore Release KeyEvents (fixes doubled characters on Windows reported by Christian Meixner
- *(adapter)* Use X-ESTIMATED-DURATION for all-day tasks to fix RFC 5545 / iCalendar standard compliance
- *(android)* Hide keyboard after adding task s.t. it is visible when we jump to it ( https://codeberg.org/trougnouf/cfait/issues/7 ), ensure that the jump-to-top icon hiden after 3-seconds and after getting pressed

### 📚 Documentation

- Confirm MacOS compatibility and add CONTRIBUTING.md file

### 🎨 Styling

- *(gui)* Improve tags list UI
- *(gui)* Remove empty space between title bar and input bar

### ⚙️ Miscellaneous Tasks

- Release cfait version 0.4.7
## [0.4.6] - 2026-01-11

### 🚀 Features

- *(gui)* Navigate back to tasks list after moving a task
- *(sorting)* Sort task and its sub-task(s) according to the first sort_rank of the whole group
- *(sorting)* Propagate more tie-breaker fields from sub-tasks
- *(gui)* Ensure resize handles are available in every panel
- *(parser)* Parse time in recurrences
- *(calendar)* Move completed tasks to the time they were done (and add undated done tasks)

### 🐛 Bug Fixes

- Moving a task no longer leaves the old calendar event
- *(windows)* Avoid spawning a console window with the GUI ( https://codeberg.org/trougnouf/cfait/issues/8 )
- Fix respawn logic and timezone handling for recurring tasks (reported by Antoine Bargel)
- *(android)* Correct cursor position in high DPI screens
- Fix legacy data migration and floating time parsing for all-day tasks
- *(recurrence)* Fix respawn logic and timezone handling for recurring tasks (for realz this time)

### 🚜 Refactor

- *(gui)* Reduce theme repetition, rm fragile manual count and position-dependent code

### 📚 Documentation

- *(readme)* Update F-Droid link to com.trougnouf.cfait

### 🎨 Styling

- *(gui)* Add a bunch of themes
- *(gui)* Improve themes
- *(gui)* Add Random theme and make it the default
- *(gui)* Switch to fastrand randomness generator
- *(gui)* Add two rounded corners
- *(gui)* Allow server-side decorations with --force-ssd flag ( https://codeberg.org/trougnouf/cfait/issues/11 )
- Show calendar's name in its own color, add ++... with each calendar's color (GUI, Android, https://codeberg.org/trougnouf/cfait/issues/4 proposed by devinside)
- *(android)* Add theme switch w/ light, dark, dynamic light, dynamic dark, default to dynamic auto-detect ( https://codeberg.org/trougnouf/cfait/issues/6 proposed by devinside )
- *(gui)* Only show calendar+++ when not in calendar list view. chore: update screenshots

### ⚙️ Miscellaneous Tasks

- *(flatpak)* Add changelog to future releases
- *(flathub)* Add release notes, categories, keywords, controls
- Update rust licenses
- Release cfait version 0.4.6
## [0.4.5] - 2026-01-06

### 🚀 Features

- Allow canceling single occurrence of recurring task
- Allow grace period before pushing tasks with a future date in the future/bottom bin
- *(gui)* Make window draggable in the settings and help panels
- Improve Android related-tasks navigation
- Show related-task icon in the tasks list (Android, GUI)
- *(android)* Add jump back up shortcut button
- Allow default sorting priority > 9

### 🐛 Bug Fixes

- Test concurrency issue
- *(android)* Rm dead scroll zone in Settings > Manage calendars

### 📚 Documentation

- *(installation)* Mention MacOS binaries provided by Martin Stut and Flatpak Linux binaries
- *(readme)* Add Android build instructions

### 🎨 Styling

- *(gui)* Merge local calendars and data management settings

### ⚙️ Miscellaneous Tasks

- *(android)* Rename com.cfait to com.trougnouf.cfait
- *(flathub)* Attempt auto-release
- Fix prepare_release.py
- Release cfait version 0.4.5
- Fix flathub-release CI script
## [0.4.3] - 2026-01-05

### 🚀 Features

- Add import ics local store function and warn Android users about upcoming name change
- Allow opening ics files from outside the app. Show Android migration warning max. once a day.
- Allow changing default priority for sorting ( https://github.com/trougnouf/cfait/discussions/3 )

### 🐛 Bug Fixes

- *(android)* Local store backward compatibility: add missing #[serde(default)] to Task fields

### 📚 Documentation

- *(comments)* Add a short descriptive header to each source file
- *(comments)* Comment on possible required cache / local store version bumps (and bump cache version)

### ⚙️ Miscellaneous Tasks

- *(flathub)* Update org.codeberg.trougnouf.cfait.yml per request and automate prepare_release
- *(flathub)* Rename to com.trougnouf.Cfait
- *(flatpak)* Try to auto-update
- *(flathub)* Update cargo-sources.yml
- *(flathub)* Fix linting issues
- Prep for v0.4.3 release (last before com.cfait to com.trougnouf.cfait migration on Android)
- Release cfait version 0.4.3
## [0.4.2] - 2026-01-03

### 🐛 Bug Fixes

- *(parser)* Restore RFC5545 duration compliance (use custom tag for DUE+DURATION)

### 💼 Other

- Add filesystem permission for app data directory

### ⚙️ Miscellaneous Tasks

- Update Flatpak manifest for v0.4.1 release
- Set GUI as default command for Flatpak
- Release cfait version 0.4.2
## [0.4.1] - 2026-01-03

### 🚀 Features

- *(tui)* Implement custom snooze (and change default snooze durations from 15m,1h to 1h,1d)
- Include sub-tasks in search results when parent matches
- *(sorting)* Sort started tasks like standard tasks (due date then priority), replace is:ongoing with is:started
- *(android)* Allow pressing enter to edit a task's title
- *(local)* Support multiple local collections
- Support manual #blocked tag, move blocked tasks away from bins 1,2,3
- *(ui)* Click on a linked task to jump to it (GUI, TUI (L), Android)
- *(gui)* Decouple exit button from scrollable area in settings and help
- *(android)* Make url and geo icons more accessible (new action in tasks list and task details)
- *(parser)* Parse "until" (date(time)) and "except" (date(time), month, year, comma-separated) in recurrence. Fix timezone issue.

### 🐛 Bug Fixes

- *(android)* Show canceled tasks with the correct checkbox (previously appeared as done)
- *(priorities)* Per https://github.com/trougnouf/cfait/discussions/3
- Move tasks between calendars
- *(android)* Force visibility on write calendar
- *(android)* No longer showing disabled calendars when moving a task from the description screen
- *(sorting)* Treat paused tasks like unstarted tasks
- *(parser)* Fix some parser recurrence edge cases

### 🎨 Styling

- *(help)* Add hugicon in help header (GUI, Android)
- *(help)* Add more hugicons (GUI, Android)

### ⚙️ Miscellaneous Tasks

- Add 10" tablet screenshots, complete Cargo.toml exclude
- Prepare for release v0.4.1
- Release cfait version 0.4.1
## [0.4.0] - 2025-12-30

### 🚀 Features

- *(search)* Add work mode ("is:ready") suggested by Martin Stut, relative start time ("^<2d"), or not set (append !, e.g. "^<2d!")
- Calendar integration
- Export local store to ics

### 🐛 Bug Fixes

- *(Local)* Add migration path to avoid catastrophic data loss (reported by montherlant)

### ⚡ Performance

- *(calendars, migration)* Implement concurrent calendar sync with Android WorkManager support

### 🎨 Styling

- *(gui)* Show dependency tree even in search mode
- *(calendar)* Event description <- put task description before managed event warning

### ⚙️ Miscellaneous Tasks

- Release cfait version 0.4.0
## [0.3.14] - 2025-12-29

### 🚀 Features

- Sub-locations (e.g. @@home:office, like sub-tags)
- Location-based aliases (e.g. @@aldi:=#groceries)
- Alias shadowing: not showing alias expansions in tasks list view
- Add generic related-to relationships
- *(tui)* Show yank and its actions in the main help bar

### 🐛 Bug Fixes

- Multiple relationships issue
- *(tui)* Restore priority color in tasks list (regression)
- *(tui)* Allow editing long titles with horizontal scrolling

### 📚 Documentation

- Fix reminder wording (not an alarm atm)
- Update screenshots to upcoming v0.3.14

### ⚡ Performance

- Eliminate disk I/O on every action, excessive cloning in hierarchy, and redundant sidebar recalculation

### 🎨 Styling

- Don't show sub-task's location in the tasks list when it is the same as its parent's
- *(tui)* Align tags to the right
- *(tui)* If a task title is too long, truncate it... and repeat it in the description
- *(tui)* Show title in the description only if it does not fit in the tasks list

### ⚙️ Miscellaneous Tasks

- Prepare for flathub release
- Release cfait version 0.3.14
## [0.3.13] - 2025-12-28

### 🚀 Features

- Add core logic for all-day/specific-time events, reminders, and alarm snoozing
- Add reminder UI elements
- Implement TUI and GUI notifications (rem:TIME), and fix(sync): robustly handle MOVE conflicts and no-op moves (otherwise journal could get stuck)
- Implement configurable auto-reminders and fix parsing for rem:, today, and tomorrow
- *(reminders)* Implement Android notifications, fix GUI snooze, enhance reminder parser (incl. rem:in TIME, relative to now)
- *(android)* Migrate alarms to WorkManager and add alarm index cache
- *(notifications)* GUI and TUI wait until 1st network sync attempt is done before firing notifications
- *(android notifications)* Add two snooze options
- *(android)* Add "Create subtask" action
- Parse time units in snooze settings presets
- *(parser)* Add rem:in ... and rem:next ...

### 🐛 Bug Fixes

- Invisible(->pushed back) tasks after cutoff date
- Local time shift
- *(android)* Working Android notifications, still need to fix Dismiss issue
- *(android)* Dismissing reminders is taken into account right away
- *(android)* Snooze

### 📚 Documentation

- Update documentation with due:datetime
- Update GUI screenshot to v0.3.13

### 🎨 Styling

- Do not show timestamp for all-day tasks
- Show appropriate time unit when editing a task
- Properly syntax-highlight rem:date time
- *(settings)* Move "Manage calendars" right below the connexion settings (Android & GUI)

### ⚙️ Miscellaneous Tasks

- Lint
- Rm strip and debug from Arch packages
- Release cfait version 0.3.13
## [0.3.12] - 2025-12-24

### 🚀 Features

- *(sort)* Prioritize urgent tasks and make urgency rules configurable (priority and days to overdue).
- *(parser)* Handle escape character

### 🐛 Bug Fixes

- *(android)* Use i32 for token indices to match Kotlin/JVM Int type

### 🚜 Refactor

- *(android)* Unify syntax highlighting with core Rust parser via UniFFI.

### ⚙️ Miscellaneous Tasks

- Remove dead code
- Lint
- Release cfait version 0.3.12
## [0.3.11] - 2025-12-24

### 🚜 Refactor

- *(parser)* Improve smart input engine for robust date, recurrence, and metadata parsing. (Fixes bug where short dates were parsed as start dates.)

### 🧪 Testing

- Improve test coverage

### ⚙️ Miscellaneous Tasks

- Release cfait version 0.3.11
## [0.3.10] - 2025-12-24

### 🐛 Bug Fixes

- *(parser)* Correct recurrence parsing, fix highlighting, and add comprehensive README compliance tests

### 📚 Documentation

- Fix README syntax error & minor update
- Add Matrix chatroom to README

### ⚙️ Miscellaneous Tasks

- Release cfait version 0.3.10
## [0.3.9] - 2025-12-23

### 🚀 Features

- *(android)* Improve (multi-words) highlighting, add pull down to refresh, add move in task menu, add description indicator in tasks list
- *(ui)* Add clickable URL/Geo icons and randomized location tab
- *(ui)* Count only active tasks (header)

### 🐛 Bug Fixes

- *(cache)* Add versioning to invalidate stale caches on upgrade
- *(geo)* Allow entering spaced geocoordinates (e.g. geo:53.046880, -121.105042)

### 🎨 Styling

- *(android)* Only show Move if there are multiple calendars
- Add some random icons to the mix, include Android

### ⚙️ Miscellaneous Tasks

- Release cfait version 0.3.9
## [0.3.8] - 2025-12-23

### 🚀 Features

- *(core/ui)* Add locations, URLs, recursive aliases with cycle detection, quoted string support, safer alias syntax (:=), and filter views by location
- *(search)* Implement implicit location search and jump-to-location navigation

### 🐛 Bug Fixes

- *(journal)* Skip ghost pruning for local calendar to prevent data loss
- *(core)* Optimize sync, fix GUI inputs, and improve alias expansion
- *(core)* Resolve timezone and multi-word date parsing bugs

### ⚡ Performance

- Compact journal on sync

### 🎨 Styling

- Add location icons and color

### ⚙️ Miscellaneous Tasks

- Release cfait version 0.3.8
## [0.3.7] - 2025-12-21

### 🚀 Features

- *(ui)* Refine smart input syntax highlighting (different color per type), add (optional) "in" date keyword, set default cutoff to 2 months

### 🐛 Bug Fixes

- *(sync)* Correctly journal offline task moves to prevent duplication
- *(android)* Preserve tag filter and sidebar state on back navigation using rememberSaveable

### 🚜 Refactor

- *(core)* Centralize journal application logic, improve sort consistency, and fix sync race conditions

### 📚 Documentation

- Mention Baikal in README since it has been explicitly tested

### ⚙️ Miscellaneous Tasks

- Release cfait version 0.3.7
## [0.3.6] - 2025-12-21

### 🚀 Features

- *(client)* Add Digest authentication and fix Android SSL

### 📚 Documentation

- *(android)* Published on F-Droid

### ⚙️ Miscellaneous Tasks

- Lint
- Release cfait version 0.3.6
## [0.3.5] - 2025-12-20

### 🚀 Features

- *(ui)* Decorate @^special statements and #tags in real time in the input bar
- *(android)* Press once to search

### 🐛 Bug Fixes

- *(android)* Default sortMonths = 2 months
- *(android)* Decorate only valid special statements in the smart bar

### 📚 Documentation

- Add suggested CalDAV providers, TOC to the README

### ⚙️ Miscellaneous Tasks

- Release cfait version 0.3.5
## [0.3.4] - 2025-12-19

### 🚀 Features

- *(android)* Improve offline remote calendar handling, show current state of connexion
- *(ux)* Refactor settings for instant save, adjust layout, and enhance Android options

### 🐛 Bug Fixes

- *(sync)* Resolve journal deadlock by handling 412 conflict on task creation

### ⚙️ Miscellaneous Tasks

- *(skip)* Fix overeager app store inclusion in README
- *(release)* Prepare for v0.3.4
- Release cfait version 0.3.4
## [0.3.3] - 2025-12-19

### 🚀 Features

- *(android)* Add loading state to task save (visual feedback) and handle coroutine cancellation
- *(workflow)* Implement pause/stop states
- *(android)* Implement optimistic UI updates for instant task creation and modification
- *(android)* If no calendars are setup (Local-only) then default sideview to Tags rather than Calendars
- *(android)* Show task duration

### 🐛 Bug Fixes

- Resolve infinite sync loop and implement optimistic save with auto-scroll to task in Android
- *(tui)* Fix double flipping logic

### 📚 Documentation

- Update README & TUI help w/ start/stop/pause shortcuts
- Update screenshots to upcoming v0.3.3

### 🎨 Styling

- *(gui)* Make RustyDark the default theme
- *(tui)* Dynamic details height

### ⚙️ Miscellaneous Tasks

- *(fdroid)* Simplify screenshot names
- Prepare for release v0.3.3
- Disable signed commit
- Release cfait version 0.3.3
## [0.3.2] - 2025-12-18

### 🐛 Bug Fixes

- Don't include uniffi/mobile for TUI/GUI desktop builds
- *(android)* Replace target_os = "android" with feature = "mobile"

### 📚 Documentation

- Update README

### ⚙️ Miscellaneous Tasks

- *(android)* Generate static Android version number for F-Droid release
- Release cfait version 0.3.2
- Auto-generate changelogs (fastlane and CHANGELOG.md), backfill previous ones
- Add Cargo.lock
- *(android)* Rm i686-linux-android
- *(fdroid)* Work on Reproducible Builds
- *(fdroid)* Pin rust version for F-Droid build
- *(fdroid)* Work on reproducible build (locked ndkVersion, RUSTFLAGS)
- *(fdroid)* Strip dependenciesInfo
- *(fdroid)* Set Android rust toolchain in rust_toolchain.toml
## [0.3.1] - 2025-12-17

### 🐛 Bug Fixes

- *(android)* Prevent crash on "Show all calendars" toggle on first run by handling missing config file
- *(android)* Prevent sync conflicts by writing post-sync ETag/sequence back to store

### ⚙️ Miscellaneous Tasks

- Prepare for 0.3.1
- Release cfait version 0.3.1
## [0.3.0] - 2025-12-17

### 🚀 Features

- *(ux)* Unhide and activate default calendar on startup
- *(core)* Implement hierarchical tag filtering and sidebar aggregation
- *(gui)* Retroactively apply new aliases to existing tasks
- *(core)* Support inline alias definition (#a=#b,#c) with retroactive application in TUI and GUI
- *(core)* Implement tag inheritance for child tasks (GUI+TUI) and hide redundant tags in the GUI task list, fix GUI yank button
- *(gui)* Add promote button for child tasks
- *(workflow)* Auto-clear yanked state after linking (child/block) in GUI and TUI
- *(ui)* Implement priority gradient
- Fix alias expansion for sub-categories, hide expanded alias tags in list view
- *(ui)* Display CalDAV calendar colors in TUI and GUI
- *(android)* Implement native Android client using Jetpack Compose and UniFFI
- *(android)* Polish UI with NerdFonts, add calendar selection, and automate asset copying
- *(android)* Implement sidebar tabs (Calendars/Tags), advanced search, and task hierarchy
- *(android)* Optimize startup with optimistic caching, fix tag layout, add back navigation, and implement task action menu
- *(android)* Compact UI layout, custom state checkbox with (currently wrong) calendar colors, blocked-by details, and expanded action menu
- *(android)* Align UI styling with desktop (calendar colors, state backgrounds, priority text) and fix icon alignment
- *(android)* Streamline task creation and editing workflow
- *(android)* Add an arrow_right / isolate button next to each calendar
- *(gui)* Refine task row layout for title and tags
- *(android)* Auto-scroll to newly created task, tag jumping, migration tool, write calendar color
- *(android)* Add help screen (under settings)
- *(tui)* Add auto-jump on create, fix search focus, wrap long titles, reduce sidebar/list ratio
- *(gui)* Improve search UX, task row layout, and add auto-scroll
- *(gui)* Add keyboard shortcuts for focusing input fields ("/" to search, "a" for smart input)

### 🐛 Bug Fixes

- Allow starting newly created tasks
- *(store)* Ensure index update on recurring task creation and network sync
- *(core)* Prevent data loss by routing alias updates through journal instead of direct cache overwrite
- *(core)* Fail loudly on corrupted local storage instead of overwriting with empty state
- *(android)* Align play icon, implement disabled calendars in settings, and resolve kotlin deprecation warnings
- *(core)* Prevent data loss by correcting relation parsing and sync ETag propagation
- *(android)* Implement full sync on connect/refresh
- *(core)* Fix Android locking, sync consistency, and model logic
- *(client)* Prevent data loss by verifying journal queue head identity before removal
- *(mobile)* Resolve lifecycle, timezone, and data safety issues
- *(Android)* Fix build
- *(core)* Resolve persistent ghost tasks by forcing sync on empty ETags
- *(core)* Recycle recurring tasks on completion to prevent duplication
- *(ci)* Downgrade artifact actions to v3 for forgejo compatibility
- *(ci)* Grant builder user permissions for AUR generation
- *(android)* Correct kotlin dsl syntax for signing config
- *(android)* Add R8 rules for JNA/UniFFI
- *(ci)* Use absolute path for keystore to fix signing

### 🚜 Refactor

- *(core)* Deduplicate task logic and centralize path resolution
- *(gui&core)* Restore edit functionality and optimize store lookups
- *(android)* Split UI into submodules and implement task relationships

### 📚 Documentation

- Add support options and improve help
- Improve TUI help
- Add Android screenshot

### 🎨 Styling

- Convert UI text and documentation from title case to normal case capitalization
- *(android)* Modify header with active calendar info, compact tag list layout, and add sidebar footer logo
- *(gui)* Highlight task on hover, display details on click
- *(android)* Reduce space between tasks, highlight current task, allow long press for more actions
- *(android)* Dark background icon
- *(gui)* Make recurrence_icon gray
- *(gui)* Implement theme switcher with custom 'Rusty Dark' option
- *(gui)* Rusty Dark: make selector yellow-amber instead of default blue
- FeatureGraphic v1
- FeatureGraphic v2

### 🧪 Testing

- *(model)* Add unit tests for ICS relation parsing and case-insensitivity
- *(sync)* Add sync safety and concurrency tests

### ⚙️ Miscellaneous Tasks

- Lint
- Lint
- *(android)* Update versions
- Switch deb/generic-linux build from Arch to to Ubuntu 24.04 (glibc 2.39), update documentation
- Update licenses
- Fastlane stuff for Android release
- *(release)* Add signed Android build pipeline and prepare for v0.3.0
- Release cfait version 0.3.0
- *(refactor)* Centralize release logic in arch container and build in parallel
- *(android)* Update gradle to 9.2, build apk in next release
## [0.2.9] - 2025-12-08

### 🚀 Features

- *(sync)* Preserve recurring task exceptions written by other clients
- *(gui)* Click on tag jumps to it
- *(gui)* Help on hover

### ⚡ Performance

- *(sync)* Optimize VTODO parsing and exception preservation to speedup startup from empty cache

### 🎨 Styling

- *(gui)* Always align tags to the right and try to share a line with the title
- *(gui)* Switch calendar highlight from blue to amber
- *(gui)* (deterministically) randomize tag color
- *(gui)* Switch Calendars/Tags header from blue to amber
- *(gui)* Move logo/icon to the sidebar when space permits

### ⚙️ Miscellaneous Tasks

- Switch to iced 0.14.0 (dev->release)
- *(forgejo)* Build once for different Linux releases
- *(release)* Update readme and changelog for 0.2.9, add version / license to GUI help
- Release cfait version 0.2.9
## [0.2.8] - 2025-12-08

### 🚀 Features

- *(ui)* Implement smart tag navigation, search result jumping, and implicit tag matching
- *(sync)* Implement safe 3-way merge for 412 conflicts to reduce duplicate tasks
- *(core)* Safe unmapped property handling
- *(gui)* Implement optimistic cache loading for instant startup

### 🐛 Bug Fixes

- *(gui)* Reset child creation mode when unlinking/canceling the parent reference
- *(tui)* Use default color for default text for white bg terminals compatibility
- *(core)* Optimize unmapped property parsing and ensure backward compatibility

### 📚 Documentation

- *(readme)* Mentian Mint

### 🎨 Styling

- *(gui)* "select" active task

### ⚙️ Miscellaneous Tasks

- Lint
- Update CHANGELOG
- Lint
- Release cfait version 0.2.8
## [0.2.7] - 2025-12-06

### 🚀 Features

- *(ui)* Display active task count next to each tag in GUI and TUI sidebars
- *(core)* Implement Start Date (DTSTART) with smart input parsing, sorting, and recurrence compatibility
- *(tui)* Implement PageUp/PageDown scrolling for sidebar lists
- *(tui)* Re-use '*' keybinding to clear all selected tags in tags view
- *(gui)* Add 'Clear All Tags' button to sidebar
- *(GUI)* Add help screen, use icons for help and settings
- *(gui)* Implement custom draggable and resizable client-side decorations
- *(gui)* Make the entire window header draggable

### 🐛 Bug Fixes

- *(tui)* Enable cursor movement in task creation input field
- *(gui)* Swap delete/cancel icon positions and adjust icons padding to prevent cropping

### 🚜 Refactor

- *(gui)* Decompose monolithic update logic into domain-specific modules
- *(gui)* Upgrade to iced 0.14-dev for native window resizing support

### 📚 Documentation

- Add icon to README

### 🎨 Styling

- *(tui)* Improve highlight contrast and right-align tags for readability
- Update logo (nerd-fonts cat -> Font Awesome, CC-BY-SA-4.0, license in LICENSES/nerd-fonts)
- Cleanup new logo
- Fix cropped cat outline
- *(gui)* Use ghost buttons for task actions and highlight destructive operations
- *(gui)* Add padding right of scroll bar to separate it from resizing
- *(gui)* Reduce vertical spacing between header and task list
- *(gui)* Reduce spacing between input bar and 1st task

### ⚙️ Miscellaneous Tasks

- *(release)* Update changelog for v0.2.6
- *(release)* Update screenshots for v0.2.6
- Release cfait version 0.2.6
- Fix Cargo.toml (too many keywords)
- Release cfait version 0.2.6
- Lint
- *(release)* Update readme and changelog for 0.2.7
- Release cfait version 0.2.7
## [0.2.5] - 2025-12-04

### 🚀 Features

- *(workflow)* Streamline child task creation from parent in GUI and TUI
- *(tui)* Add toggleable, dynamic, and comprehensive help screen
- *(ui)* Implement auto-jump to new tasks (TUI & GUI) and better scrollable logic
- *(GUI)* Tab between fields in the settings window

### 🚜 Refactor

- Split model, client, and gui view into granular submodules
- Modularize TUI logic into network actor and event handlers
- *(core)* Decouple search matching logic from store to model domain

### 🎨 Styling

- *(GUI)* Allow main content area to expand with window width

### ⚙️ Miscellaneous Tasks

- *(release)* Update changelog and screenshots for v0.2.5
- Release cfait version 0.2.5
## [0.2.4] - 2025-12-03

### 🚀 Features

- *(core)* Implement robust file locking with fs2, atomic journal processing, and isolated tests to prevent data corruption
- *(ui)* Enable multiline task descriptions in GUI and TUI (Alt+Enter), fix visual corruption in TUI, and propagate sync errors

### 📚 Documentation

- Move main mirror from github to codeberg

### ⚙️ Miscellaneous Tasks

- Add Codeberg Actions for testing and release builds
- Add Rust toolchain to Codeberg
- Lint
- Use lld linker to fix OOM errors and install clippy component
- Reduce memory usage by not compiling cargo-deb, 2-threads
- Self-hosted runner
- *(release)* Add cmake and nasm to fix windows cross-compilation and fix shell script syntax
- *(release)* Update changelog for v0.2.4
- Release cfait version 0.2.4
## [0.2.3] - 2025-12-01

### 🚀 Features

- *(core)* Implement layered calendars, disabled state, and robust tui visibility toggles

### 🐛 Bug Fixes

- *(gui)* Add close button to error banner and clear on success
- *(sync)* Implement safe conflict resolution (copy on 412), atomic file writes, and atomic move operations
- *(gui)* Preserve active calendar on refresh, always inject local calendar, and show duration metadata for untagged tasks
- *(model)* Treat no priority as implied normal priority (5) for sorting

### 🚜 Refactor

- *(sync)* Implement CTag caching, optimize fetch, and fix journal atomicity bugs

### ⚡ Performance

- *(net)* Constrain concurrent calendar fetches to 4 to prevent server overload
- *(core)* Implement bounded concurrency and delta sync for task fetching

### ⚙️ Miscellaneous Tasks

- Add licenses
- Lint
- *(release)* Update changelog
- Release cfait version 0.2.3
## [0.2.2] - 2025-11-29

### 🐛 Bug Fixes

- *(sync)* Handle 412 Precondition Failed by refreshing ETag and retrying

### 🎨 Styling

- [GUI] align tags with titles

### ⚙️ Miscellaneous Tasks

- Lint
- Update changelog
- Release cfait version 0.2.2
## [0.2.1] - 2025-11-29

### 🚀 Features

- *(security)* Implement secure TLS with insecure toggle and improve connection UX
- *(config)* Add setting to hide specific calendars from view
- *(core)* Implement moving tasks between calendars in GUI and TUI
- *(core)* Introduce a local-only calendar with an option to migrate tasks to a CalDAV server
- *(journaling)* Implement offline task queue and UI indicators
- *(gui)* Embed Symbols Nerd Font, iconify UI and compact task-row layout

### 📚 Documentation

- Replace #urgent with !<4 in Advanced search example
- Installation instructions (Arch, deb, Windows, generic-Linux, Rust crate)
- Update README

### 🎨 Styling

- *(gui)* Overhaul task row with a space-saving layout

### ⚙️ Miscellaneous Tasks

- Lint
- Auto-add changelog to release notes
- Release cfait version 0.2.1
## [0.2.0] - 2025-11-27

### 💼 Other

- Initial implementation of ongoing & canceled tasks (Need custom checkboxes)
- Custom checkmark icons (V,>,X)
- Implement GTD workflow and advanced search parser
- Lint
- Lint
- More linting and update screenshots for next release

### 🎨 UI/UX Improvements

- [TUI] Show [E]dit description in help bar
- [TUI] Refresh on error and add refresh key
- [GUI] add remove dependency button(s) in the task description

### ⚙️ Miscellaneous Tasks

- Automate changelog with git-cliff
- *(release)* Update changelog for v"${TAG}"
- Release cfait version 0.2.0
## [0.1.9] - 2025-11-26

### 💼 Other

- Update funding sources in FUNDING.yml
- Mention sorting
- Preparing for crate release
- Preparing for crate release
- Default to True
- Set cutoff date s.t. timed tasks are not always on top (default: 6-months). Add scroll wheel in GUI settings.
- Tags were saved with comma / not fully compatible w/ other clients
- Change hide_completed_in_tags setting to hide_fully_completed_tags (i.e. hide the tags, not the tasks within)

### 🎨 UI/UX Improvements

- [GUI] remove "<" and ">" buttons (replaced w/ Link functionality)

### ⚙️ Miscellaneous Tasks

- Release cfait version 0.1.9
## [0.1.7] - 2025-11-25

### 💼 Other

- Rename GUI window
- Mention categories
- Attempt Windows build (in next release)
- Groundwork to support RFC 9253 (DEPENDS-ON) in model
- Support RFC 9253 (DEPENDS-ON) in both TUI and GUI, and improve children dependency handling
- Add options=('!lto') to Arch PKGBUILD (fix issues when lto is enabled in makepkg.conf)
- Support aliases (set in the config file and/or in the GUI settings)
- Add subtitle
- Manually allow multiple RELATED-TO fields (not supported by icalendar library)
- Add unit tests
- Release 0.1.7

### 🎨 UI/UX Improvements

- [TUI] add space after [ ]
- [TUI] support RFC 9253 (DEPENDS-ON)
- [GUI] set window name in Linux only (fix Windows build?)
## [0.1.6] - 2025-11-24

### 💼 Other

- Rm warning
- Add icon, replace milk w/ cat food
- [README] add screenshots
- [README] add screenshots
- [README] use raw Wikimedia Commons URL for screenshots; gitlab does not support redirects
- Add cfait-git Arch PKGBUILD
- Uncomment icon
- Fix missing icon in opened application
- Refactor GUI, add support for #categories
- Refactor TUI
- Add unit tests
- Fix hide completed tasks in tab view, fix GUI save settings
- Fix bug where completed tags remained selected but invisible / hiding all tasks. Add uncategorized tag
- Fix TUI build error on CI, update screenshots

### 🎨 UI/UX Improvements

- [GUI] add Tags (categories) view (pulling from all calendars), add settings to hide completed tasks
- [TUI] Browse by category/tags, restore cache
- [GUI] fix cutoff tags AND/OR text
## [0.1.5] - 2025-11-22

### 💼 Other

- Fix github release
- Optimize binary size
- Add onboarding prompt
- Rename fairouille->cfait, automate Arch Linux PKGBUILD
- Add .deb release

### 🎨 UI/UX Improvements

- [TUI] respond to --help
## [0.1.4] - 2025-11-22

### 💼 Other

- Add license file, bump version
- Add Arch Linux PKGBUILD
- Rustache -> ferouille
- Rustache -> ferouille
## [0.1.3] - 2025-11-22

### 💼 Other

- Add recurrence support, recurrence symbol, and expand relative dates
- Add unit tests to model.rs
- Add caching (fast inter-calendar switching)
## [0.1.2] - 2025-11-21

### 💼 Other

- Allow viewing + editing description in GUI + TUI
- Bump version up to 0.1.2

### 🎨 UI/UX Improvements

- [GUI] Add priority / subtask / edit / delete buttons
- [GUI] show tasks description
## [0.1.1] - 2025-11-21

### 💼 Other

- Initial commit (working TUI with create/add/complete/delete, sorted by date+priorities
- Add multiple calendars support (from the same server)
- Add README
- Rename to rustache
- Add edit support
- Support moving cursor
- Prep for GUI
- Basic GUI (single-calendar)
- Update README

### 🎨 UI/UX Improvements

- [TUI] add scrolling
- [GUI] multi-calendar support
- [GUI] sub-tasks support
- [TUI] sub-tasks support
- [GUI] search function
- [GUI] show date. Bump to version 0.1.1
