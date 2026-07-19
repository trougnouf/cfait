# Cfait Specifications & Developer Guidelines

> **⚠️ INSTRUCTIONS FOR DEVELOPERS AND CONTRIBUTORS:**
> This document is the ultimate source of truth for Cfait's behavior, data model, and architecture.
> 1. **Core-First & DRY:** Business logic, filtering, parsing, and data manipulation MUST live in the Rust core (`src/model`, `src/store.rs`, `src/controller.rs`). UIs (TUI, GUI, Mobile, CLI) must remain as thin as possible and act only as rendering/routing layers.
> 2. **Performance is Critical:** The UI must never lag, even with 100,000+ tasks. Avoid unnecessary cloning (use `&Task` references in filter pipelines). Rely on `TaskStore` indices for O(1) lookups.
> 3. **Keep this updated:** Update this document whenever introducing a new feature, syntax token, setting, or architectural shift. Keep it concise, behavioral, and accurate.

---

## 1. Core Architecture & Persistence

Cfait is an offline-first task manager that seamlessly synchronizes with CalDAV servers and local file storage.

### 1.1. Data Flow & Synchronization
*   **TaskStore (In-Memory):** The single source of truth for the active session. Contains tasks grouped by calendar HREF. Maintains O(1) HashMaps for UID lookups, blocking relationships, and parent-child hierarchies.
*   **Journal (Offline Queue):** All mutations (`Create`, `Update`, `Delete`, `Move`) append to `journal.json` immediately. UIs update optimistically.
*   **TaskController:** Orchestrates all updates. Receives `AppIntent`s from the UIs, applies them to the `TaskStore`, writes to the `Journal`, and signals the background worker.
*   **Background Sync:** 
    *   *Desktop (GUI/CLI daemon):* A background worker reads the Journal and pushes changes via `RustyClient`.
    *   *Android:* Handled via `WorkManager`. `PeriodicSyncWorker` runs based on `auto_refresh_interval_mins` (min 15 mins). Foreground manual syncs trigger immediate updates.
*   **Settings Sync:** User configuration and aliases sync across devices via a hidden `VTODO` task with UID `cfait-global-settings-v1` (status `CANCELLED`, category `cfait-internal`).
*   **Conflict & Error Handling:** 
    *   `412 Precondition Failed` (ETag mismatch): Performs a local 3-way merge. If unmergeable, a "Conflict Copy" is generated.
    *   **Fatal Server Errors (e.g., 400, 403, 415):** The problematic task is rescued into a local `local://recovery` calendar to prevent data loss or sync loop lockups, with the error appended to its description.
    *   **Duplicate UID Resolution:** If a duplicate UID is detected across collections (e.g., during a remote fetch), active collections always take precedence over system collections (`local://trash`, `local://recovery`). Otherwise, the task with the higher sequence number wins, tie-breaking alphabetically by collection HREF.

### 1.2. The Task Entity (`VTODO` Mapping)
Tasks map strictly to iCalendar `VTODO` components (RFC 5545). Non-standard metadata is stored via `X-CFAIT-` properties.
*   **Status:** `NeedsAction` (Pending), `InProcess` (Timer running), `Completed`, `Cancelled`.
*   **Manual Block:** Stored via `X-CFAIT-BLOCKED` (boolean) to explicitly mark a task as blocked without dependencies.
*   **Dates (`DateType`):** Start (`DTSTART`) and Due (`DUE`). Supported variants:
    *   *Specific:* Exact DateTime (UTC).
    *   *All-Day:* NaiveDate.
    *   *Fuzzy:* Month/Year precision (stored as All-Day with `X-CFAIT-FUZZY-DUE`/`START` properties).
*   **Hierarchy:** `RELATED-TO` establishes the `parent_uid`.
*   **Dependencies:** `RELATED-TO;RELTYPE=DEPENDS-ON` establishes blocking relationships. `RELTYPE=SIBLING` establishes related tasks.
*   **Time Tracking:** Logged via `X-TIME-SPENT` (total seconds), `X-LAST-START` (unix timestamp), and `X-CFAIT-SESSION` (WorkSessions holding Unix start/end timestamps).
*   **System Entities:** Local trash uses `local://trash`. Items here are soft-deleted and pruned based on `trash_retention_days`.

### 1.3. System Integrations
*   **Keyring:** Passwords are never stored in plaintext `config.toml`. They are vaulted via OS keyrings: Windows Credential Manager, macOS Keychain, Linux Secret Portal (oo7) or Keyutils, Android Keystore.
*   **Logging:** Outputs to `cache/cfait.log` (rotating `cfait.old.log`). Terminal stderr logging is enabled for CLI/GUI, but disabled for TUI to prevent screen tearing. Android uses dual logging (File + Logcat).
*   **Crash Reporting (Android):** An `UncaughtExceptionHandler` writes panics to `cache/android_crash.txt`.

---

## 2. Smart Syntax & Parsing
Evaluated instantly during text input. Supported across all clients.

### 2.0. Localization & Canonical Storage
*   **Canonical Storage:** The internal data model and `Task::to_smart_string()` MUST always output canonical English/ISO tokens (e.g., `due:`, `@2025-01-01`, `~30m`). This ensures CalDAV sync works seamlessly across devices even if one device is in French and another in English.
*   **Dual-Input Parser:** The parser accepts *both* localized terms AND English canonical terms. This prevents breaking muscle memory for existing users and keeps headless CLI scripts language-agnostic.
*   **Lexicon Cache:** `src/model/parser.rs` uses an `RwLock<ParserLexicon>` to cache valid tokens (O(1) lookups) built from the `rust_i18n` JSON files on startup.

### 2.1. Tokens
| Token | Meaning | Example |
| :--- | :--- | :--- |
| `!1` .. `!9` | Priority (1 is highest/most urgent). | `!1` |
| `@` or `due:` | Due date. | `@now`, `@tomorrow`, `@2025-12-31`, `@fri 2pm`, `@next 8` |
| `^` or `start:` | Start date. | `^next week`, `^next 15` |
| `^@` | Sets *both* Start and Due dates. | `^@tomorrow 9am` |
| `~` or `est:` | Estimated duration (supports ranges). | `~30m`, `~1h-2h` |
| `#` | Tag/Category (Supports brace expansion). | `#work`, `#project{sub1,sub2}` |
| `@@` or `loc:`| Location. | `@@office` |
| `url:` / `[[ ]]`| Attach URL or Wiki-link. | `url:https://example.com`, `[[Master plan]]`, `[[Master plan|Alias]]` |
| `dep:` or `depends:`| Set dependency (blocks the task). Supports short UIDs or fuzzy matching by summary. | `dep:"Install foundation"`, `dep:abc1234` |
| `rel:` or `related:`| Set related task (sibling). Supports short UIDs or fuzzy matching by summary. | `rel:"Master plan"`, `rel:abc1234` |
| `geo:` | Geo-coordinates. | `geo:50.1,4.2`, `geo:here` (Mobile: Fetches GPS) |
| `- ` or `is:note` | Mark task as a note/header (hides checkbox). | `- Pantry`, `is:note` |
| `desc:` | Append text to the description. | `desc:"Buy milk"` or `desc:{...}` |
| `rem:` | Reminder / Alarm. | `rem:10m`, `rem:in 1h`, `rem:8pm`, `rem:next friday` |
| `done:` | Mark completed / Set percentage. | `done:now`, `done:yesterday`, `done:50%` |
| `spent:` | Log time spent manually. | `spent:1h` |
| `rec:` or `@` | Recurrence (`RRULE`). | `@daily`, `rec:every 2 weeks` |
| `@after` | Relative recurrence (shifts from completion). | `@after 1w`, `@after 2mo` |
| `until` | End date for recurrence. | `@daily until 2025-12-31` |
| `except` | Exclusion dates (`EXDATE`). | `@daily except sat,sun` |
| `col:` | Assign task to a specific collection/calendar. | `col:Personal`, `col:"Work Projects"` |
| `+cal` / `-cal` | Force/prevent companion Calendar Event. | `+cal` |
| `is:pinned` | Pin task to the top of the list. | `is:pinned` |
| `is:permanent` | Mark task as a permanent/continuous tracker. | `is:permanent` |
| `goal:` | Goal tracking target. | `goal:5/w`, `goal:2h/daily`, `goal:weekly` |

*Rules:* 
* Double prefixes (`##tag`, `@@@loc`) apply metadata but *keep* the word in the display title.
* Use `\` to escape special characters (e.g., `\#not-a-tag`).

### 2.2. Aliases (Macros)
Users can define reusable shortcuts that expand into multiple tags, locations, or priorities.
*   *Syntax:* `#gardening := #home:outside, @@garden, !4`
*   Aliases are resolved retroactively across the database upon creation/edit. Cycle detection is strictly enforced (max depth 10).

### 2.3. Markdown Subtask Extraction & Round-Trip Editing
If a task's description contains Markdown lists or Headers, Cfait automatically extracts them into distinct child tasks whenever the task is saved. 
Users can also use the "Edit Tree" action (or `Ctrl+E`) to edit an entire existing task tree—including the root task's summary, metadata, and subtasks—as a single unified Markdown document.
*   **Hierarchy:** Tasks nest based on indentation level (for lists) or header depth (`#`, `##`, `###`).
*   **Parallel Tasks & Notes:** Unnumbered lists (`- [ ]`) create independent sibling actionable tasks. If a line is a header (`## Pantry`) or a plain bullet (`- eggs`), it is extracted as a "Note" task (`is_note = true`). Notes act as structural elements, hiding their checkboxes in the UI while retaining hierarchy mapping. Supported checkbox states are:
    *   `[ ]` maps to `NeedsAction` (Pending / Unstarted).
    *   `[/]` or `[<]` maps to `NeedsAction` with `percent_complete` at 50% (Paused).
    *   `[>]` or `[▶]` maps to `InProcess` (Timer running).
    *   `[x]`, `[X]`, or `[*]` maps to `Completed`.
    *   `[-]` or `[~]` maps to `Cancelled`.
*   **Sequential Dependencies:** Numbered lists (`1. [ ]`, `2. [ ]`) create `DEPENDS-ON` blocking relationships. If multiple tasks share the same number at the same indentation level (e.g., two `3. [ ]` tasks), they are extracted as parallel steps that both depend on the previous step (`2. [ ]`). The next step (`4. [ ]`) will automatically depend on *both* parallel tasks.
*   **Cross-Tree Dependencies:** Dependencies that break standard linear sequence are appended to the task string as wiki-links (e.g. `dep:[[Install foundation]]`). The backend resolves these via fuzzy-matching against task summaries.
*   **Cross-Collection Subtasks:** Subtasks belonging to a different collection than the root task append a collection token (e.g., `col:CollectionName`) to preserve their location during round-trip editing.
*   **Round-Trip UIDs:** Extracted tasks append a metadata tag (e.g., `<!-- uid:abc-123 -->`) to their summary. When the tree is re-edited and saved, this tag allows Cfait to diff the text and update existing database entities rather than creating duplicates.

### 2.4. Inline Markdown Formatting
Cfait natively supports rendering basic inline Markdown across task summaries, descriptions, and the raw text editors.
*   **Supported Syntax:** `**bold**`, `__bold__`, `*italic*`, `_italic_`, `~~strikethrough~~`, `` `code` ``, standard Markdown links `[label](url)`, and bare `http(s)://` URLs.
*   **Marker Visibility:** Formatting markers (e.g., `**`, `~~`, `` ` ``) are hidden in read-only views (such as the task list, sidebar, and read-only details) to keep the text clean. The markers are preserved and highlighted in the raw text editors and inputs to ensure a seamless text-based editing experience.

---

## 3. Searching, Filtering, and Sorting

### 3.1. Search Operators & Primitives
The search bar supports a boolean recursive-descent parser.
*   **Logic:** Implicit `AND` (space), `OR` (`|`), `NOT` (`-`), and Grouping `()`.
*   **Primitives:**
    *   *State:* `is:done`, `is:active`, `is:started` / `is:ongoing`, `is:blocked`, `is:note`.
    *   *Actionable:* `is:ready` (Excludes completed tasks, explicitly/implicitly blocked tasks, and tasks starting in the future. `InProcess` bypasses this).
    *   *Comparison:* `~<30m` (duration < 30m), `!<4` (priority < 4).
    *   *Dates:* `@<today` (Overdue), `^>1w` (Starts in > 1 week).

### 3.2. Multi-Stage Sorting Algorithm
Tasks sort deterministically by rank (0 to 9), then by Overdue -> Priority -> Due Date -> Start Date -> Summary.
*   **Rank 0:** Pinned (`is:pinned`).
*   **Ranks 1-3 (Urgent/Started/Due Soon):** Order dictated by `sort_preset` (e.g., Urgent > Started > Due Soon).
*   **Rank 4 (Actionable):** Due date `<=` `sort_cutoff_days`.
*   **Rank 5 (Deferred):** No due date, or `>` `sort_cutoff_days`.
*   **Rank 6 (Blocked):** Has unresolved dependencies or parent is blocked.
*   **Rank 7 (Future):** Start date is > `start_grace_period_days`.
*   **Rank 8 (Completed):** Done or Cancelled.
*   **Rank 9 (Trash):** In `local://trash`.

*Rule:* If `sort_standard_by_priority` is enabled, Ranks 4 and 5 merge and sort by numeric Priority first, then Date.

---

## 4. Core Business Workflows

### 4.1. The "Yank" Relationship System
Instead of drag-and-drop, Cfait uses a robust "Yank" (Clipboard) system for hierarchy management.
1.  **Yank (`y` / Action Menu):** Copies the selected task's UID to an internal "Yanked" state. UI displays a persistent banner.
2.  **Relate:** Select a *target* task and execute:
    *   `c` (Child): Target becomes a subtask (child) of Yanked.
    *   `b` (Block): Target becomes blocked by Yanked.
    *   `l` (Link): Target becomes related (sibling) to Yanked.
3.  **Clear (`Esc`):** Clears yank state. (`Y` locks the yanked state for multiple relations).

### 4.2. Recurrence Recycling & DST Safety
When completing a recurring task:
1.  Running timers commit to `time_spent_seconds`.
2.  A **History Snapshot** is generated (`X-CFAIT-HISTORY-OF: parent_uid`) with the completion date. This snapshot is non-recurring and retains no alarms.
3.  Master task dates advance to the next occurrence based on the `RRULE`.
4.  *DST Rule:* Absolute alarms advance using Local Naive time math. (A 9:00 AM alarm stays 9:00 AM across DST shifts).
5.  *Relative Recurrence:* If `@after 1w` (or Shift+Complete), the master task's base date temporarily shifts to `now` before advancing.
6.  *Completed* subtasks/descendants of the recurring task reset to `NeedsAction`.

### 4.3. Virtualization & Truncation (Completed Groups)
*   If completed subtasks exceed `max_done_subtasks` (or roots exceed `max_done_roots`), the Model injects a **Virtual Expand/Collapse Row** into the flattened task list.
*   Selecting this virtual row toggles visibility of the hidden completed items. State is transient (in-memory only).
*   **Tree Navigation & Expansion (Tags, Locations, Tasks)**
    *   Tags and Locations automatically expand transiently to reveal their active selection, returning to their configured collapsed state when unselected.
        *   **Search Context & Tree Filtering:** When searching, the task tree is filtered to show exact matches alongside their full ancestry and descendants.
            *   *Direct Matches:* Tasks that explicitly match the query.
            *   *Descendants:* All subtasks of a direct match are fully visible and treated as matches.
            *   *Ancestors (Context):* Parent tasks all the way to the root are included to provide structural context, but are visually dimmed.
            *   *Unrelated Siblings:* Branches without any matches are completely hidden.
        *   During searches, all matching tasks are automatically expanded. Users can manually collapse them, but this state is overridden on new searches.

### 4.4. Companion Events (Calendar Integration)
If `create_events_for_tasks` is enabled or `+cal` is used:
*   Generates `.ics` `VEVENT` files and `PUT`s them alongside the `VTODO`.
*   Start/Due ranges > 1 day apart split into `-start` and `-due` events.
*   WorkSessions emit as distinct events (`-session-0`).
*   `EXDATE`s sync to the event so skipped instances disappear from the user's agenda.
*   *Android Note:* Handled reliably via `CalendarSyncWorker` (WorkManager).

### 4.5. Goals & Habit Tracking
Goals act as quotas or habits and can be applied globally or locally.
*   **Global Goals:** Mapped to tags/locations using aliases (e.g., `#reading := goal:2h/w`). They appear in the Goals sidebar tab.
*   **Task-Specific Goals:** Defined directly on a task (e.g., `Read book goal:2h/w`). They replace the task's duration badge with a progress tracker. If `show_task_goals_in_sidebar` is true, they also appear under the Goals tab.
*   **Progress & Heatmaps:** Progress is calculated dynamically by summing `WorkSession` overlaps and completion dates within the calendar interval. The last 7 intervals are evaluated and rendered as a Heatmap sparkline (e.g., `■■□■■■□`) across all UI clients.
*   **Effective Goals:** Recurring tasks (`RRULE`) inherently generate a `1/period` count goal automatically to feed the Heatmap renderer, even if no explicit `goal:` token is set.
*   **Implicit Credit:** If a task with an `estimated_duration` is completed *without* explicitly running a timer, the remaining estimated time is granted instantly as goal progress. Logging a session fulfills "Count" goals if `sessions_count_as_completions` is true.

### 4.6. Permanent / Continuous Tasks
Tasks tagged with `is:permanent` act as endless trackers. When checked off (Completed), they do not change status. Instead:
1. If a timer is running, it is committed as a work session.
2. If no timer is running, a session is logged using the task's `estimated_duration` (or the default goal duration).
3. The task remains in `NeedsAction` state.

### 4.7. Alarms & Reminders
*   **AlarmIndex:** Optimized cache `alarm_index.json` stores upcoming triggers.
*   **Implicit:** Auto-generated alarms for Due / Start dates (if `auto_reminders` is true).
*   **Snoozing:** Snoozing acknowledges the original alarm and creates a new absolute alarm linked via `RELATED-TO;RELTYPE=SNOOZE`.
*   **Just-In-Time (JIT) Sync:** To prevent phantom alarms across devices, clients must attempt a synchronous network fetch immediately prior to firing an alarm (or within a 15-second pre-fire window). If the task was completed, canceled, or the alarm's trigger time was advanced (via recurrence) on another device, the local alarm is pruned before notifying the user.
*   *Android Implementation:* Uses `AlarmManager.setExactAndAllowWhileIdle`. When an alarm fires, an `AlarmWorker` executes a foreground `api.sync()` before posting a Notification. Notification Actions (Snooze, Done, Pause) are handled via `NotificationActionReceiver` which delegates back to a unique `WorkManager` request to prevent background ANRs.

---

## 5. UI Layout & Platform Specifics

### 5.1. Desktop Graphical User Interface (GUI)
*Powered by `iced`. Optimized for mouse & keyboard.*
*   **Layout:** 3-pane layout (Sidebar, Main List, Markdown Details Pane).
*   **Window:** Client-Side Decorations (Custom frameless window, resize grips) unless `--force-ssd` is passed.
*   **Zooming:** Global scale via `Ctrl++`, `Ctrl+-`, and `Ctrl+ScrollWheel`. Middle-click resets.
*   **Mouse Interactions:**
    *   *Single Click:* Select row.
    *   *Double Click:* Triggers `EditTaskStart` (focus title input).
    *   *Right Click:* Opens **Full Context Menu** at cursor coordinates.
    *   *Ellipsis (`...`) Click:* Opens **Partial Context Menu** anchored to the button (shows unpinned actions).
*   **Modals:** Hovering overlays with dimmed backdrops (Move Task, ICS Import, Alarm Notification).

### 5.2. Terminal Interface (TUI)
*Powered by `ratatui`. Keyboard-only paradigm.*
*   **Layout:** 2-Pane (Sidebar 20%, Main List 80%). Details view shares vertical space with Main List. Press `Shift+Up/Down` to scroll the active details pane without losing focus on the list.
*   **Modals/Popups:** Instead of context menus, pressing `Enter` on a task opens a centered **Action Menu** popup with fuzzy filtering. 
*   **Details Viewer (`L`):** Unified popup containing the full markdown description, History/Heatmaps, WorkSessions, and relationships (Parents, Children, Blockers, Successors, Siblings) for quick jump navigation.
*   **Session Manager (`T`):** Popup to view/delete `WorkSession` records.
*   **External Editor:** Pressing `E` launches `$VISUAL`/`$EDITOR` (suspending the TUI), falling back to the built-in modal if empty.

### 5.3. Mobile Interface (Android)
*Powered by Jetpack Compose. Touch-optimized.*
*   **Layout:** 
    *   *Top Bar:* Random Jump, Quick Filter, Search toggle, Refresh/Sync, Settings.
    *   *Tabs:* Desktop "Sidebar" is translated into horizontal `HorizontalPager` tabs. Pull-to-refresh triggers manual sync.
    *   *Navigation Drawer:* Swipe from the left edge to switch between Calendars, Tags, Locations, Goals view modes. (Swipe logic uses custom pointer interception to avoid conflicting with tab paging).
*   **Task List Rendering:** `LazyColumn`. Real-time relative duration formatting via coroutines (`liveDurationMins`). Real-time syntax highlighting in input via `VisualTransformation`.
*   **Task Details:** Tapping a task navigates to a dedicated `TaskDetailScreen`. Includes an "Edit Tree" action for full-screen Markdown tree editing.
*   **Context Menu:** Long-pressing a row opens the full Dropdown Menu.
*   **Location Integration (`geo:here`):** If a user types `geo:here`, the UI requests permissions and invokes `LocationManager.getCurrentLocation`. If it fails within 5s, falls back to the last known location.
*   **Notifications:** 
    *   *Ongoing Tasks:* Generate a persistent, swipable notification with a live Chronometer and "Pause"/"Done" actions.
    *   *Alarms:* High-priority. Includes inline "Snooze Custom" via `RemoteInput` text reply.
*   **Intents:** Intercepts `ACTION_VIEW` for `.ics` files to launch the Import Screen.
*   **Debug Export:** UI includes an advanced option to generate a zip of `cache/`, `data/`, `config/`, and `android_crash.txt`, sharing it via `ACTION_SEND`.

---

## 6. Keyboard Shortcuts (GUI & TUI)

*   **Navigation:** `j`/`k` or `Up`/`Down` (Select), `Tab` (Cycle focus between Sidebar, List, Input). `1..4` (Switch Sidebar tabs).
*   **Main Actions:** 
    *   `Space`: Toggle Done/NeedsAction.
    *   `Shift+Space`: Complete & Shift recurrence (Relative advance).
    *   `s`: Start/Pause timer.
    *   `S`: Stop/Reset timer.
    *   `x`: Cancel task.
    *   `+` / `-`: Increase/Decrease priority.
    *   `e`: Edit title. `E`: Edit description (Markdown). `Ctrl+E`: Edit tree (Markdown) / Switch editor mode. `Ctrl+N`: Create new task with description.
    *   `Delete`: Move to trash. `Ctrl+Delete`: Delete entire tree.
    *   `t`: Log time session manually.
*   **Tree/Relationships:** 
    *   `z`: Fold/Unfold tree.
    *   `>` / `.` : Demote (Indent / Make child of previous).
    *   `<` / `,` : Promote (Outdent / Move one level up).
    *   `L` : Open relationship browser.
*   **App Actions:** 
    *   `/`: Focus search.
    *   `a`: Focus add task.
    *   `w`: Toggle Quick Filter.
    *   `m`: Toggle Match AND/OR logic for sidebar tags.
    *   `H`: Toggle Hide Completed.
    *   `*`: Clear all filters.
    *   `Shift+R`: Jump to random actionable task (weighted by priority).
    *   `Ctrl+,`: Settings.

---

## 7. Command Line Interface (CLI)
Used for headless automation, scripting, and piping. Operates directly on the `TaskStore`.

*Note on `<uid>` arguments:* Any CLI command accepting a `<uid>` also accepts partial UIDs, exact titles, partial summaries, or wiki-links (e.g. `[[My Task]]`). If a match is ambiguous, the CLI will output the matching options and exit.

*   `cfait add <task...>`: Smart input task creation. Flags: `-c <href>`, `--desc <text>`, `-p <uid>` (set parent), `-n` (queue to journal, don't wait for network sync).
*   `cfait append <uid> <task...>`: Appends smart syntax tokens (tags, dates, deps, etc.) or text to an existing task. Flags: `--desc <text>` (appends to existing description), `-n` (no wait).
*   `cfait edit <uid> [--tree]`: Opens an external editor (`$VISUAL`/`$EDITOR`) to edit the task's properties. Pass `--tree` to edit the entire task tree as a single Markdown document.
*   `cfait replace <uid> <task...>`: Replaces the entire task summary and metadata. To safely add tags or dates without losing the title, use `append`. Flags: `--clear-due`, `--clear-start`, `--clear-tags`, `--clear-loc`, `--clear-deps`, `-p <uid>`, `--clear-parent`, `--desc <text>` (overwrites description).
*   `cfait list [--all] [--json] [-c <id>] [-p <uid>]`: Outputs task tree (use `-p` to focus on a specific sub-tree).
*   `cfait search <query> [--all] [--json] [-c <id>] [-p <uid>]`: Searches and outputs tasks within a specific sub-tree.
*   `cfait view <uid> [--json]`: Outputs detailed task info.
*   `cfait tree <uid>`: Views the task tree starting at `<uid>` serialized into markdown format (same format used by the `Ctrl+E` editor).
*   `cfait start|pause|toggle|done|complete|delete <uid>`: State mutation commands.
*   `cfait export [--collection <id>]`: Dumps collection as standard ICS to stdout.
*   `cfait import <file.ics> [--collection <id>]`: Parses and imports ICS to store.
*   `cfait sync`: Foreground network sync.
*   `cfait daemon`: Runs a continuous background sync loop based on `auto_refresh_interval_mins`. Acquires a cross-process lock to prevent overlapping syncs with UIs.
*   `cfait collection list|create|edit`: Manages CalDAV collections.

---

## 8. Configuration (`config.toml`)
All persistent state and settings live here. Unrecognized TOML keys must not be dropped during serialization.

**Connection & Sync:**
*   `url`, `username`: CalDAV credentials. *(Password vaulted in OS Keyring).*
*   `tls_client_cert_path`, `tls_client_key_path`: Strings (Optional). Paths to PEM-encoded certificate and private key for mTLS.
*   `allow_insecure_certs`: Boolean.
*   `sync_settings`: Boolean. Enables the `cfait-global-settings-v1` hidden VTODO sync.
*   `auto_refresh_interval_mins`: Integer. Daemon sync loop interval.
*   `trash_retention_days`: Integer. Days before `local://trash` items are permanently purged. (0 = disable trash).

**UI & Behavior:**
*   `default_calendar`: String HREF.
*   `enable_local_mode`: Boolean. Allow offline `local://` collections.
*   `hide_completed`, `hide_fully_completed_tags`, `hide_aliases_in_sidebar`: Booleans.
*   `strikethrough_completed`: Boolean. Line-through styling for done tasks.
*   `show_inline_descriptions`: Boolean. Previews up to 3 lines of the description in the list.
*   `ui_scale`: Float (0.5-3.0). Global zoom.
*   `theme`: Enum (RustyDark, Light, Dracula, Nord, Catppuccin variants, etc.).
*   `language`: String (`en`, `fr`). None = system locale.
*   `description_editor`: String. CLI command for TUI description editing. `builtin` forces internal UI editor.
*   `show_ongoing_notifications`, `show_priority_numbers`, `sidebar_is_hidden`, `show_goals_tab`, `show_task_goals_in_sidebar`: Booleans.
*   `pinned_actions`: Array of `TaskAction` enums. Dictates buttons pinned directly to GUI task rows.

**Sorting & Limits:**
*   `sort_preset`: Enum (`UrgentStartedDue`, `UrgentDueStarted`, `StartedUrgentDue`).
*   `sort_cutoff_days`: Integer/None. Rank 4 vs 5 divider.
*   `sort_standard_by_priority`: Boolean. Merge ranks 4/5.
*   `urgent_days_horizon`: Integer. Tasks due within X days are "Urgent" (Rank 1-3).
*   `urgent_priority_threshold`: Integer (1-9). Priorities <= X are "Urgent".
*   `default_priority`: Integer (1-9). Maps `!0` to this.
*   `start_grace_period_days`: Integer. Show future tasks X days before they start (Rank 7).
*   `max_done_roots`, `max_done_subtasks`: Integers. Triggers Virtual Expand/Collapse rows.

**Data & Events:**
*   `create_events_for_tasks`, `delete_events_on_completion`: Booleans for VEVENT generation.
*   `default_duration_goal_mins`: Integer. Implicit duration credit for checked-off tasks without an estimate.
*   `sessions_count_as_completions`: Boolean. Logging time counts towards `Count` goals.

**Reminders:**
*   `auto_reminders`: Boolean. Implicit alarms for Due/Start.
*   `default_reminder_time`: String (HH:MM). Default time for all-day date alarms.
*   `snooze_short_mins`, `snooze_long_mins`: Integers for quick snooze preset buttons.

**Quick Filters & State:**
*   `quick_filter_term`, `quick_filter_icon`, `show_quick_filter`: Quick filter button settings.
*   `hidden_calendars`, `disabled_calendars`: Arrays of HREFs.
*   `expanded_tags`, `expanded_locations`: Arrays mapping visual tree expansion states.
*   `tag_aliases`: HashMap of Alias Key -> Array of Tags/Locations.
*   `goals`: HashMap of Goal Key -> Goal Object.
*   `collection_order`: Array of HREFs defining the custom display order of collections.
*   `sort_collections_by_size`: Boolean. Automatically sort collections from most to least tasks. Trash and Recovery collections are always shown below standard collections regardless of their task count.
