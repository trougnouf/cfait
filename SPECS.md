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
*   **Incremental Remote Fetch:** Remote calendar fetches are two-level incremental. First, the calendar's sync token (`CTag`, falling back to `sync-token`) is compared against the cached one: if they match and there are no unsynced local "ghost" tasks (cached tasks with no server href yet), the cached tasks are returned without any enumeration. Otherwise, a calendar-query enumerates the `VTODO` ETags and only tasks whose ETag differs from the cache are re-downloaded (multiget, with a lower-concurrency fallback path if a batch fails). The cache is then rewritten with the fresh tasks and the latest token.
*   **Local (LAN) address preference:** The per-device `local_url` config names the same CalDAV server at an address reachable only on the local network (e.g. a laptop or phone at home, with `url` pointing at a VPN tunnel address). Whenever it is set, `RustyClient` construction probes it with a bounded TCP connect (500 ms) and prefers it for the whole sync cycle when it answers; otherwise `url` is used unchanged. The reachability decision is cached for 60 seconds so frequent client constructions do not re-probe. Credentials, TLS and all sync semantics are identical for both addresses; a cycle that starts on the local address and loses it mid-way surfaces as a normal transient failure and re-probes on the next cycle. `local_url` is intentionally not part of the synced settings payload (it is network-dependent).
*   **Cross-Instance Change Detection:** The TUI and GUI watch the data and cache directories (via `notify`) for `.json` files written by other cfait processes (e.g. a `cfait sync` in another terminal or the daemon). On an external change, the in-memory store is reloaded from disk without a network round-trip. Writes made by the running instance itself are suppressed via a last-write timestamp so they don't trigger a spurious reload.
*   **Settings Sync:** User configuration (e.g., `default_calendar`, `disabled_calendars`, sorting presets, goals) and aliases sync across devices as a JSON payload (syncable settings + `updated_at` revision). Cfait stores it in one of two ways, chosen per server:
    * *Property mode (preferred):* The payload is written as the custom WebDAV properties `settings` (JSON) and `settings-rev` (revision) in the namespace `http://trougnouf.com/ns/cfait/` on the target calendar collection. Standard CalDAV clients never request or display these properties. Support is detected once per collection by PROPPATCHing a temporary `probe` property with a random UUID, reading it back via PROPFIND, and removing it; the result is cached for 30 days in `settings_meta.json`. A 4xx rejection of a settings PROPPATCH demotes the collection to object mode within the same cycle.
    * *Object mode (fallback):* The payload is stored in the `DESCRIPTION` of a hidden carrier object with UID `cfait-global-settings-v1` — a `CANCELLED` `VEVENT` anchored at the Unix epoch (`DTSTART:19700101T000000Z`, one-minute duration, category `cfait-internal`, summary `⚙ Cfait Settings (Do not delete)`) that most calendar clients filter out by time range. A legacy `VTODO` carrier is re-anchored to the `VEVENT` format on the next sync.
    * *Target collection:* The configured `default_calendar` when it is a remote collection, otherwise the first cached remote collection, otherwise `local://`. The target is re-derived on every sync cycle: when the hosting collection is deleted or the default changes, the old property/carrier is removed (best-effort) and the settings re-sync to the new target.
    * *Conflict resolution:* Last-write-wins on the `updated_at` revision. When both a property payload and a leftover carrier object exist, the newer one wins (migration guard).
    * *Exclusions:* Purely local view state (`hidden_calendars`, window dimensions, UI scale, expanded tree paths) intentionally do not sync so that each device retains its own independent viewing context.
    * *System Tags:* The `cfait-internal` category must be explicitly excluded from user-facing tag lists and autocomplete suggestions.
*   **Write Target (Active Collection):** When a new task is created, it is assigned to the UI's currently "active" collection. In the TUI/GUI, this is the collection currently selected/highlighted in the sidebar (regardless of how many other collections are visible in the main view). On Android, this is the collection tab currently being viewed. Upon app startup, this active collection is initialized to the globally synced `default_calendar`.
*   **Conflict & Error Handling:** 
    *   **If-Match normalization:** Conditional requests (update PUT, delete) send the etag in its strong entity-tag form (RFC 9110 section 8.8.3): a `W/` weakness marker is stripped and missing quotes are added, because `If-Match` is compared strongly and some servers (the United Internet CalDAV stack behind GMX, mail.com and WEB.DE) hand out weak etags in PUT responses and REPORT `getetag`s while rejecting the weak form in `If-Match` — echoing their etags back verbatim 412s every save and cascades Conflict Copies across devices. A pending-refresh or missing etag sends `If-Match: *` (resource exists, version unknown) instead of a malformed empty header.
    *   `412 Precondition Failed` (ETag mismatch): Performs a local 3-way merge. The merge base is the recorded server-agreed state for the queued action's etag: every successful push records the pushed task (fresh etag) in `journal.json` (`bases`, bounded to the last 256 uids), and a queued action etag-matched against a record provably forked from exactly that version — the shared per-calendar cache can be older or newer than an edit's true parent when sibling frontends or in-flight fetches race, which silently reverts or hard-conflicts the merge; the cache remains the fallback for records that are absent or predate the feature. If still unmergeable but the queued edit's `SEQUENCE` is strictly higher than the server's, the server holds an earlier link of our own edit chain (e.g. the "done" of a rapid done/undo pushed by a sibling frontend sharing the journal while our queued action still carried the older etag) and the newer local edit is retried against the server's etag, mirroring the re-parent rule. Only a tie or an older local edit falls through to a "Conflict Copy".
    *   **Re-parent conflicts:** `parent_uid` is the one scalar field exempted from hard-conflict Conflict Copies: when both sides moved a task under different parents, the newer edit wins (higher `SEQUENCE`; ties go to the committed server state), since duplicating a whole task over one relationship would be worse than losing the older move.
    *   **Deterministic Server Rejections (any 4xx except 401, 408, 412, 413, 429):** The server processed the request and rejected it; replaying it cannot succeed, so the task is rescued into a local `local://recovery` calendar to prevent data loss or a stuck queue, with the error appended to its description. Deletes of already-rejected tasks are discarded from the queue.
    *   **Transient failures (network, timeout, 5xx, 401, 408, 429):** The action stays at the head of the queue and is retried on the next sync — an offline server never loses or rescues queued changes, however long the outage. When the queue head fails, the reason and attempt count are persisted in `journal.json` (`last_error`) and shown in the GUI's unsynced tooltip. `last_error` is cleared whenever the queue empties.
    *   **Duplicate UID Resolution:** If a duplicate UID is detected across collections (e.g., during a remote fetch), active collections always take precedence over system collections (`local://trash`, `local://recovery`). Otherwise, the task with the higher sequence number wins, tie-breaking alphabetically by collection HREF.

### 1.2. The Task & Journal Entity (`VTODO` & `VJOURNAL` Mapping)
Tasks map strictly to iCalendar `VTODO` components, while daily notes map to `VJOURNAL` components (RFC 5545). Non-standard metadata is stored via `X-CFAIT-` properties.
*   **VJOURNAL Support:** Each chronological journal entry is a date-anchored note (`DTSTART;VALUE=DATE`). Wiki pages are independent notes and intentionally omit the `DTSTART` property. Stored in the active collection when it advertises VJOURNAL support; otherwise the target is redirected via `resolve_journal_target` (default calendar if capable, else the first visible capable collection, else `local://default`) so the server never receives a VJOURNAL it will refuse. VJOURNAL components are omitted from the main task lists unless they contain child tasks, are children of another task, are explicitly tagged with `is:note` or `is:pinned` to force visibility, or are returned as part of an active search query. Sub-pages (a page whose parent is another page) never appear in the main list — they belong to the journal's Pages section — unless they carry child tasks, are pinned, or match the active search; `is:note` forces visibility only for non-sub-page pages. A page's note state is always explicit: a plain `is:page` line is a standard page, and only an `is:note` token on the line makes it a note. The token is the single source of truth on every tree sync — adding it turns the page into a note, removing it turns it back into a standard page. Both VTODO and VJOURNAL components are included in ICS import/export across all clients (GUI, CLI, Android), enabling migration between CalDAV providers. A collection advertises VJOURNAL support through its `supported-calendar-component-set` property; cfait requests VTODO, VEVENT, and VJOURNAL when creating a collection (MKCALENDAR), so on VJOURNAL-capable servers `cfait collection create` (or the GUI's create button) yields a journal-capable collection. When the journal pane is open and no visible remote collection supports VJOURNAL, a hint reminds the user they can create one from cfait to sync their journal.
*   **Status:** `NeedsAction` (Pending), `InProcess` (Timer running), `Completed`, `Cancelled`.
*   **Manual Block:** Stored via `X-CFAIT-BLOCKED` (boolean) to explicitly mark a task as blocked without dependencies.
*   **Archive:** The `is:archived` flag (see 2.2) is stored via `X-CFAIT-ARCHIVED:TRUE` on both `VTODO` and `VJOURNAL` components, so it survives CalDAV sync and the journal's Archive section (see 5.4).
*   **Manual Sort:** The `sort:manual` flag (see 2.2) is stored via `X-CFAIT-MANUAL-SORT:TRUE` on the parent `VTODO`, and each child's manual position via `X-CFAIT-SORT-POSITION` (a short base-62 fractional index, see 3.2), so both survive CalDAV sync.
*   **Dates (`DateType`):** Start (`DTSTART`) and Due (`DUE`). Supported variants:
    *   *Specific:* Exact DateTime (UTC).
    *   *All-Day:* NaiveDate.
    *   *Fuzzy:* Month/Year precision (stored as All-Day with `X-CFAIT-FUZZY-DUE`/`START` properties).
    *   *Display:* Rendered in UI lists using relative dates for `today`, `tomorrow`, and `yesterday` (including time if specific), falling back to absolute dates otherwise. `Task::to_smart_string()` strictly preserves canonical ISO tokens.
*   **Hierarchy:** `RELATED-TO` establishes the `parent_uid`.
*   **Dependencies:** `RELATED-TO;RELTYPE=DEPENDS-ON` establishes blocking relationships. `RELTYPE=SIBLING` establishes related tasks.
*   **Time Tracking:** Logged via `X-TIME-SPENT` (total seconds), `X-LAST-START` (unix timestamp), and `X-CFAIT-SESSION` (WorkSessions holding Unix start/end timestamps). The duration badge shown in task lists displays the aggregated time across the task's entire subtree (union-merged to avoid double-counting cascade overlaps), not just the task's own tracked time. Detail views and notifications show per-task time.
*   **System Entities:** Local trash uses `local://trash`. Items here are soft-deleted and pruned based on `trash_retention_days`. A soft-deleted task keeps its `parent_uid` (the sub-tree structure survives in the trash) and records its deletion date (`X-TRASHED-DATE`) and origin collection (`X-TRASHED-FROM`). A *Restore* action (GUI/TUI context menus, Android long-press menu, `cfait restore <uid>`) moves a trashed task and its trash-resident sub-tree back to the recorded origin collection — falling back to the parent's collection, the default calendar, then `local://` — stripping the trash bookkeeping properties. `parent_uid` links are left untouched, so the tree lands where it was; dependencies that were cleaned from other tasks on delete are not resurrected.

### 1.3. System Integrations
*   **Keyring:** Passwords are never stored in plaintext `config.toml`. They are vaulted via OS keyrings: Windows Credential Manager, macOS Keychain, Linux Secret Portal (oo7) or Keyutils, Android Keystore. When the keyring is unreachable (e.g. a headless SSH box) and the password exists only as a `password =` line in `config.toml`, Cfait keeps that line on every save and tells the user; once a keyring is reachable, the password is vaulted there and the line is stripped. An optional `password_command` (e.g. `pass show cfait`) can own the credential instead: its stdout (trailing newlines stripped) is used as the password, takes precedence over the keyring and the file, and is never written to disk. The command runs through the system shell with a 10-second timeout and a failed run falls back to keyring/file.
*   **Logging:** Outputs to `cache/cfait.log` (rotating `cfait.old.log`). Terminal stderr logging is enabled for CLI/GUI, but disabled for TUI to prevent screen tearing. Android uses dual logging (File + Logcat).
*   **Crash Reporting (Android):** An `UncaughtExceptionHandler` should write panics to `cache/android_crash.txt` (but it doesn't work in practice).

---

## 2. Smart Syntax & Parsing
Evaluated instantly during text input. Supported across all clients.

### 2.0. Localization & Canonical Storage
*   **Canonical Storage:** The internal data model and `Task::to_smart_string()` MUST always output canonical English/ISO tokens (e.g., `due:`, `@2025-01-01`, `~30m`). This ensures CalDAV sync works seamlessly across devices even if one device is in French and another in English.
*   **Dual-Input Parser:** The parser accepts *both* localized terms AND English canonical terms. This prevents breaking muscle memory for existing users and keeps headless CLI scripts language-agnostic.
*   **Lexicon Cache:** `src/model/parser.rs` uses an `RwLock<ParserLexicon>` to cache valid tokens (O(1) lookups) built from the `rust_i18n` JSON files on startup.
*   **UI Strings (Rust):** `locales/*.json` are the single source of truth. Rust uses `rust_i18n` with `%{param}` placeholders and `t!("key", param = value)`. Parameters MUST be written as `%{param}` — a bare `{param}` is shown literally in the UI.
*   **UI Strings (Android):** The Gradle task `generateStringsXml` (run automatically before any build) converts each `locales/*.json` into `android/app/src/main/res/values*/strings.xml`, rewriting `%{param}` to positional `%1$s`, `%2$s`, ... arguments. Plural keys (objects with `one`/`other`/`zero` subkeys) are emitted as a single `{one=..., other=...}` string and resolved at runtime by `resolvePluralMap()` in `Shared.kt`. Kotlin call sites must therefore pass arguments in the same order as the placeholders appear in the JSON.
*   **Plural Selection:** rust_i18n does NOT select plural forms automatically — `t!("key", count = n)` on a plural key returns the key itself. Callers must pick the subkey, e.g. `i18n::t_plural("key", count)` (see `src/i18n.rs`) or `t!("key.one")` / `t!("key.other")` directly. A regression test (`i18n_tests::t_macro_keys_exist_in_fallback_locale`) fails the build if any `t!()` literal key is missing from `en.json`.

### 2.1. Item Types: Tasks, Notes, Pages & Journal Entries
Every item is created from the same smart-syntax line; the type tokens decide what the line becomes, where it is displayed, and how its body is parsed. The text typed on the input line always becomes the item's *title* — the body is written afterwards in the item's own editor (open a page from the journal's Pages section, a `[[link]]`, or search).

| Type | Token | Component | Checkbox | Displayed in | Created by |
| :--- | :--- | :--- | :--- | :--- | :--- |
| Task | (default) | `VTODO` | yes | Task lists, day planner | Typing in the task input; `cfait add "Water the tomatoes" #gardening` |
| Note / header | `- ` prefix or `is:note` | `VTODO` flagged as a note | no | Task lists, as a structural header that groups the items underneath | `cfait add "Pantry is:note"` |
| Wiki page | `is:page` (alias: `is:journal`) | `VJOURNAL` without `DTSTART` | no | Journal tab → Pages section: a tree of long-form documents (Archive section when archived — see 5.4) | `cfait add "Garden path ideas is:page"`, `[[My Page]]` typed in a page, or `Ctrl+N` in the journal tab |
| Flat document | `is:flat` (alias: `is:doc`) | `VTODO` with `X-CFAIT-FLAT:TRUE` | yes | Task lists; its description is parsed like a page (see 2.4) | `cfait add "Reading list is:flat"` |
| Journal entry (a day's note) | none — the date anchors it | `VJOURNAL` with `DTSTART;VALUE=DATE` | no | Journal tab, under its date | `Enter` on a date in the journal tab (TUI & GUI); CLI: `cfait add "2026-10-17 is:journal start:2026-10-17" --desc "..."` |

*   `is:journal` is an alias of `is:page` — both create a wiki page. The name refers to the component type (`VJOURNAL`), not to the journal tab; a type token alone never creates a dated entry.
*   A journal entry is a page anchored to a date: for each date the journal tab shows the `VJOURNAL` whose `DTSTART` is that date (when several share a date, the one titled with the date string wins — see 5.4). A wiki page given a `start:` date therefore appears as that day's entry, which is how the CLI creates one.
*   `is:note` on a page line (`- My Page is:note is:page`) is the explicit note marker for pages: a plain `is:page` line is a standard page, and the token is the single source of truth on every tree sync.
*   A page's body is a document, never a task tree: headers, plain text, and plain lists stay inside the `DESCRIPTION` (see 2.4); only checkbox lines (`- [ ]`), uid-tagged lines, and typed sub-items become children.

### 2.2. Tokens
| Token | Meaning | Example |
| :--- | :--- | :--- |
| `!1` .. `!9` | Priority (1 is highest/most urgent). | `!1` |
| `@` or `due:` | Due date. | `@now`, `@tomorrow`, `@2025-12-31`, `@fri 2pm`, `@next 8` |
| `^` or `start:` | Start date. | `^next week`, `^next 15` |
| `^@` | Sets *both* Start and Due dates. | `^@tomorrow 9am` |
| `~` or `est:` | Estimated duration (supports ranges). | `~30m`, `~1h-2h` |
| `#` | Tag/Category (Supports brace expansion). | `#work`, `#project{sub1,sub2}` |
| `@@` or `loc:`| Location (Supports multiple via <code>&#124;</code>). | `@@office`,  <code>@@aldi&#124;auchan</code> |
| `url:` | Attach a URL. (Any `scheme://` or `mailto:` is supported. Bare URLs default to `https://`). | `url:perdu.com`, `url:https://example.com` |
| `[[ ]]` | Wiki-link to jump to or create a task/page. Use `:` for absolute paths and `+` for relative sub-items. | `[[Master plan]]`, `[[+Child]]`, `[[Project:Phase 1]]` |
| `dep:` or `depends:`| Set dependency (blocks the task). Supports short UIDs or fuzzy matching by summary. | `dep:"Install foundation"`, `dep:abc1234` |
| `rel:` or `related:`| Set related task (sibling). Supports short UIDs or fuzzy matching by summary. | `rel:"Master plan"`, `rel:abc1234` |
| `parent:` | Make the task a subtask of another task. Supports short UIDs or fuzzy matching by summary (resolved like `dep:`/`rel:` at save time). Replaces any previous parent; unresolvable, self- or cyclic references are kept as typed with a warning, like `dep:`. | `parent:"Home garden"`, `parent:abc1234` |
| `geo:` | Geo-coordinates. | `geo:50.1,4.2`, `geo:here` (Mobile: Fetches GPS) |
| `- ` or `is:note` | Mark task as a note/header (hides checkbox). On journal pages, `is:note` is the explicit note marker in the page line (a plain `is:page` line is a standard page). | `- Pantry`, `is:note` |
| `desc:` | Append text to the description. | `desc:"Buy milk"` or `desc:{...}` |
| `rem:` | Reminder / Alarm. | `rem:10m`, `rem:in 2h20m`, `rem:8pm`, `rem:next friday` |
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
| `sort:manual` | Manual sort: this task's children keep the order the user set (see 3.2) instead of the standard rank/date/priority sort. | `sort:manual` |
| `is:page` | Mark the line as a Wiki page (VJOURNAL component, no `DTSTART` — see 2.1). `is:journal` is an alias. Parses identically from the task interface and the journal tab, and does not imply `is:note`. | `is:page`, `is:journal` |
| `is:flat` | Keep the task a `VTODO` (checkbox stays), but parse its description like a wiki page: Markdown headers and plain list items stay as document text instead of being extracted into subtasks (checkbox items still are). The description also keeps child shadow lines, like a page. | `is:flat`, `is:doc` |
| `is:archived` | Archive a wiki page: it moves from the journal's Pages section to the Archive section (see 5.4). Deleting the token — or toggling the archive button (GUI) / `A` key (TUI) — unarchives it. Searchable with `is:archived` (see 3.1). | `is:archived` |
| `goal:` | Goal tracking target. | `goal:5/w`, `goal:2h/daily`, `goal:weekly` |

Reference resolution (`dep:`/`rel:`/`parent:`): fuzzy references resolve within the collections the UI currently shows (exact UIDs resolve regardless of visibility). UID-prefix matches take priority over summary matches, so a short UID that is a unique prefix resolves even when a quoted summary elsewhere contains the same UID text verbatim (e.g. a pasted task line kept as-is). An ambiguous reference — several visible tasks match the same pool — is kept as typed with a visible warning listing the candidates; disambiguate with a short UID (e.g. `parent:abc12345`).

*Rules:* 
* Double prefixes (`##tag`, `@@@loc`) apply metadata but *keep* the word in the display title.
* Use `\` to escape special characters (e.g., `\#not-a-tag`).
* **Durations** accept compound forms wherever a duration is parsed (`rem:`, `~`, `spent:`, `goal:`, snooze input): every segment carries an explicit unit and segments sum (`2h20m` = 140 minutes, `~1h30m-2h`). Segments without a trailing unit (`2h20`) and clock-time lookalikes (`2:20h`) are rejected.
* **Reminder anchoring:** `rem:in <duration>` counts from now (absolute). `rem:<duration>` counts backwards from the task's due/start date; if the task has no due/start date it falls back to an absolute reminder from now. `rem:<clock time>` (e.g. `rem:2:20`, `rem:8am`) fires at that time on the due/start date, or today/tomorrow when the task has no date.

### 2.3. Aliases (Macros)
Users can define reusable shortcuts that physically expand into multiple tags, locations, or priorities.
*   *Syntax:* `#gardening := #home:outside, @@garden, !4`
*   Aliases act as one-way text macros. They are resolved retroactively across the database upon creation/edit, meaning the target tags are physically appended to matching tasks. Because provenance (whether a tag was added manually or via alias) is not tracked, removing a tag from an alias definition will *not* remove it from existing tasks to prevent data loss. Cycle detection is strictly enforced (max depth 10).

### 2.4. Markdown Subtask Extraction & Round-Trip Editing (Context-Aware)
If a task's description contains Markdown lists, Cfait extracts actionable items into distinct child tasks whenever the task is saved. 
Users can also use the "Edit Tree" action (or `Ctrl+E`) to edit an entire existing task tree—including the root task's summary, metadata, and subtasks—as a single unified Markdown document.
*   **Document Preservation & Transient Metadata:** 
    *   **VTODO (Tasks & Structural Notes):** For standard tasks (`VTODO`), Markdown Headers (`# Header`) and plain bullets (`- plain text`) *are* extracted into distinct structural `VTODO` components (flagged with `is:note`). This allows rapidly building hierarchical task trees without managing indentation spaces. Sub-items of a `VTODO` note are always parsed as independent components.
    *   **VJOURNAL (Wiki Pages & Journals):** For wiki pages (`is:page` / `is:journal`), long-form descriptions (Markdown headers, plain text, and plain lists) are **never** extracted into distinct components. They remain safely intact inside the `DESCRIPTION` property to prevent shredding paragraphs. Sub-items of a `VJOURNAL` are treated as part of the document body unless they contain actionable checkboxes (`- [ ]`), which are always extracted as `VTODO` sub-tasks. The `is:page` token itself is context-independent: a line tagged `is:page` always becomes a sub-page, whether the document was typed in the task interface or the journal tab — only the surrounding plain lines are treated differently (extracted as sub-tasks in task context, kept in the body in journal context).
    *   **Tree Serialization:** When editing an entire tree (`Ctrl+E`), `VJOURNAL` components are serialized explicitly as `- Page Title is:page` (or `is:journal`); a journal note is serialized as `- Page Title is:note is:page` so its note state round-trips through the editor. The journal root itself is serialized as its raw `DESCRIPTION` at depth 0 — its own title line is never emitted as a bullet. This unambiguous token guarantees that re-parsing the tree preserves the component type and correctly scopes the "journal context" for all nested children. `VTODO` structural notes are simply serialized as `- Note Title`. Re-syncing an unchanged tree produces no actions (idempotent round-trip), and a re-pasted sub-page line without a `<!-- uid -->` tag is matched by summary against existing siblings instead of being duplicated. The tree editor's buffer is the *tree view* — the whole tree: a pinned sub-page's body expands in place at its reference line, and an unplaced sub-page (parented through the Pages section) appends below with its body, so copying the buffer yields the entire tree, journals included. Saving that buffer records the hierarchy it displayed: an appended sub-page line becomes a pinned reference line in the parent's document, and a line missing from the buffer is an explicit deletion (see "Absent lines are deletions"). The same tree view backs `cfait edit --tree`, `cfait print` (with `--plain` for tag-free output), and the Android task-tree markdown.
    *   **Sub-pages Are Links, Not Embeds (Document View):** In the *document view* — the journal page editor and the description editors — a sub-page (a `VJOURNAL` child) is a separate page: it renders as its single reference line (marker, title, `is:page`/`is:note` tokens, `dep:`/`rel:` tags, `<!-- uid -->` comment) — a followable link — and its body is never expanded into the parent's document. The body is edited in the sub-page's own editor (open it from the journal's Pages section, search, or a `[[...]]` link). `VTODO` children (tasks, notes) still expand inline with their bodies. A sub-page appears in a parent's document only where the document references it: a sub-page the document does not pin (parented through the Pages section, yank, or `parent:`) is not serialized into the parent's editor at all — the journal's Pages section owns the hierarchy of unplaced sub-pages. On save, a sub-page line without an indented block preserves the page's stored `DESCRIPTION` and inline media untouched — only a line carrying an indented block rewrites the body — and the subtree of such a bare reference line is exempt from deletion (its body is edited in its own editor). Deleting a *pinned* reference line soft-deletes the sub-page (and its subtree); an unplaced sub-page absent from the buffer is never deleted by the parent's save. The tree editor and tree exports use the *tree view* instead (see "Tree Serialization"): sub-page bodies expand and unplaced sub-pages append.
    *   **Flat Documents (`is:flat` / `is:doc`):** A `VTODO` tagged `is:flat` (or `is:doc`) keeps its checkbox and stays in the task lists, but its description is parsed with `VJOURNAL` rules: Markdown headers, plain text, and plain lists are **never** extracted into components and stay intact inside the `DESCRIPTION`, which also keeps child shadow lines (the description is the document, like a page). Only actionable checkboxes (`- [ ]`), uid-tagged lines, and lines carrying `is:note` / `is:page` / `is:journal` / `is:flat` / `is:doc` become subtasks. The token is stored as `X-CFAIT-FLAT:TRUE` on the `VTODO` so it survives CalDAV sync, and it is searchable (`is:flat` / `is:doc`).
    *   **Context-Aware Parser:** The extraction parser tracks the "journal" context dynamically based on indentation. Any sub-tree indented under a `VJOURNAL` node is parsed using `VJOURNAL` rules (no shredding of plain text). The same holds for sub-trees indented under an `is:flat` / `is:doc` node: the token scopes flat parsing to its own sub-tree.
    *   **Transient Metadata:** Any smart syntax tags (`#tag`) or locations (`@@loc`) written inside a journal's description body *are* dynamically extracted into transient memory. This ensures the journal page appears under those categories in the Sidebar and Search results, while preventing ghost tags from polluting the permanent CalDAV `CATEGORIES` property.
*   **Actionable Items:** Bullets containing checkboxes (`- [ ] Action item`) are always extracted into distinct actionable `VTODO` components. Supported checkbox states are:
    *   `[ ]` maps to `NeedsAction` (Pending / Unstarted).
    *   `[/]` or `[<]` maps to `NeedsAction` with `percent_complete` at 50% (Paused).
    *   `[>]` or `[▶]` maps to `InProcess` (Timer running).
    *   `[x]`, `[X]`, or `[*]` maps to `Completed`.
    *   `[-]` or `[~]` maps to `Cancelled`.
    *   In round-trip tree editing, unchecking a completed task (`[x]` → `[ ]`) reopens it: the status returns to `NeedsAction` and the `COMPLETED` date and completion percentage are cleared.
*   **The Structural Parent Rule:** If you indent an actionable task (`- [ ] subtask`) underneath a plain bullet point (`- Folder`), Cfait recognizes the plain bullet as a structural block and extracts it as an `is:note` component. This preserves the proper parent/child hierarchy.
*   **Context-Aware Wiki Links:** Creating a missing link inherits the component type of where it was clicked (creating an actionable `VTODO` from a task, or a `VJOURNAL` from a page). Standard links (`[[My Page]]`) always search globally and default to root-level creation.
    *   *Override Context:* Use `- [ ] [[My Task]]` to explicitly force an actionable task, or `[[My Page is:page]]` to explicitly force a journal page.
    *   *Hierarchy Paths:* Use `[[Project:Phase 1]]` to define an absolute path (dynamically generating missing ancestors). Use `[[+Subpage]]` to explicitly search for or create a child within the current context.
    *   *Ambiguous Links:* A link that matches several existing tasks is never auto-created. The candidates are shown and the user disambiguates, for example by typing the full path (`[[Project:My Page]]`).
*   **Sequential Dependencies:** Numbered lists (`1. [ ]`, `2. [ ]`) create `DEPENDS-ON` blocking relationships. If multiple tasks share the same number at the same indentation level (e.g., two `3. [ ]` tasks), they are extracted as parallel steps that both depend on the previous step (`2. [ ]`). Items do not need to be written in sequential order; out-of-order lists are resolved systematically.
*   **Round-Trip UIDs:** Serialized task trees append an inline HTML comment containing a unique identifier (e.g., `<!-- uid:abc-123 -->`) to the end of each task line. Formatting and text without UID comments are parsed into the `DESCRIPTION`. Inline parsing surfaces the comment as a distinct `UidComment` element: raw editors dim it (gray, italic), while clean rendered views (task rows, summaries) omit it.
    *   **Shadow lines (inline positions):** Extraction never drops an extracted line from its parent's description buffer: the line stays as a *shadow* — the text as typed, tagged with the child's `<!-- uid -->` comment. The child `VTODO`/`VJOURNAL` owns the state; the shadow only pins where the child renders inside the document, so a task typed between two paragraphs reopens exactly there instead of collapsing to the bottom of the page. What gets *stored* depends on the component: a journal page keeps its shadows in the `DESCRIPTION` (the page's description is the document, and the tags pin each child's position), while a plain task stores prose only (`stored_description`) — the tree, not the text, remembers its children — so third-party clients see a clean `DESCRIPTION` with no subtask lines masquerading as notes. Serialization replaces each shadow with the child's fresh serialized line (marker, checkbox, smart syntax, `col:`/`dep:` tags, uid comment) and appends only unreferenced children below the description: unreferenced `VTODO` children (e.g. added through the main list) always append, while unreferenced sub-pages append only in the *tree view* — in the document view they are not appended, since the Pages section owns their hierarchy; for a prose-only task every child is unreferenced, so they append below the prose in tree order. A shadow whose task is no longer a child (moved, trashed, re-parented) is dropped from the view so a later save cannot resurrect its text as a duplicate; a shadow with an unknown uid is kept as typed so the next save binds or mints a child for it. Prose previews (inline list rows, tooltips, detail panes, mobile `descriptionInline`) strip shadow lines via `description_prose` — only user-typed prose is shown — so even a legacy `DESCRIPTION` still carrying shadows (saved by 1.2.0) renders cleanly until the next save rewrites it prose-only.
    *   **Description editors:** Every description editor (GUI summary/description editor, TUI `E`/external editor, `cfait edit` below the first line, Android detail) loads the *merged view* — the full subtree minus the root line (`serialize_merged_description`): shadows render their children inline with fresh state. Sub-pages (`VJOURNAL` children) are the exception: they render as their single reference line (a followable link), never their body. Saving routes through `sync_tree_from_markdown` with the task's own flat-document flag (`is_journal` or `flat`), so shadow-bound children update in place instead of duplicating, deleted lines soft-delete, and unchanged lines are no-ops. The GUI description editor re-tags freshly minted lines like the journal editor; the Android save path compares the editor buffer against `get_task_by_uid`'s merged description, keeping the change-detection symmetric.
    *   **Pasted tagged lines:** Content pasted into a *create* flow (new task/page with description) may carry the uid comments of unrelated existing tasks: every such line is re-minted (`remint_existing_uids`), rewriting its shadow tag and remapping nested parent/dependency references, so the new tree is a copy and never a hijack of the original.
    *   **Editor save cadence:** The GUI journal editor debounces its saves (500 ms after the last keystroke, not per keystroke), and each save re-parses the whole buffer. A line without a UID comment would otherwise be assigned a fresh identifier on every save (accumulating duplicates or trashed intermediate versions while typing), so the editor re-tags every line a save just minted — appending the comment at the line end, which preserves the cursor's (line, column) — and later saves match those components by uid. The other editors hold no buffer across saves: the TUI and mobile reload the tree (with UID comments) after each save, and the GUI tree editor re-serializes from the store, so none of them re-parses a stale buffer.
    *   **Absent lines are deletions:** A document-view buffer (journal page editor, description editors) holds the journal root's description plus every subtask and every *pinned* sub-page's reference line, each carrying its UID comment — so a line missing from the buffer is an explicit deletion. Exemptions: a sub-page's *body* is never part of the parent's buffer (see "Sub-pages Are Links, Not Embeds"), so the whole subtree of a sub-page shown as a bare reference line is protected; and an unplaced sub-page (never referenced by the document) is absent from the buffer by design and is likewise protected — only removing a *pinned* reference line soft-deletes a sub-page (and its subtree), as does deleting its parent page. A *tree-view* buffer (tree editor, `cfait edit --tree`, `cfait print`, Android task-tree markdown) is the whole tree — sub-page bodies expanded, unplaced sub-pages appended — so none of the sub-page exemptions apply: a sub-page line missing from the buffer is an explicit deletion, and deleting a sub-task line under a sub-page soft-deletes it. System-calendar components (trash, recovery) are exempt in both views: they are never serialized into the tree, and deleting them here would hard-delete them and bypass the retention window.

### 2.5. Inline Markdown Formatting
Cfait natively supports rendering basic inline Markdown across task summaries, descriptions, and the raw text editors.
*   **Supported Syntax:** `**bold**`, `__bold__`, `*italic*`, `_italic_`, `~~strikethrough~~`, `` `code` ``, standard Markdown links `[label](url)`, and bare URLs (any `scheme://` or `mailto:`).
*   **Marker Visibility:** Formatting markers (e.g., `**`, `~~`, `` ` ``) are hidden in read-only views (such as the task list, sidebar, and read-only details) to keep the text clean. The markers are preserved and highlighted in the raw text editors and inputs to ensure a seamless text-based editing experience.
*   **Inline `data:` URI stripping:** Inline base64 images (e.g. `![alt](data:image/png;base64,...)`) in CalDAV `DESCRIPTION` properties are stripped at the model boundary (`from_ics`) and replaced with short `cfait-media://UUID` placeholders, so multi-MB blobs never enter UI state, editor buffers, or the text shaper. The original payloads are stored in a separate `inline_media` map and re-injected by `to_ics` for any placeholders that survive in the edited description. Editing text around a placeholder preserves the image; deleting the placeholder drops it. User-pasted `data:` URIs are also stripped at the `extract_markdown_tasks` entry point.

---

## 3. Searching, Filtering, and Sorting

### 3.1. Search Operators & Primitives
The search bar supports a boolean recursive-descent parser.
*   **Logic:** Implicit `AND` (space), `OR` (`|`), `NOT` (`-`), and Grouping `()`.
*   **Primitives:**
    *   *State:* `is:done`, `is:active`, `is:started` / `is:ongoing`, `is:blocked`, `is:note`, `is:page`, `is:flat` / `is:doc` (flat-document task), `is:archived` (manually archived page), `is:canceled` / `is:cancelled`, `is:recurrent` (has a recurrence rule; `-is:recurrent` matches non-recurring tasks).
    *   *Actionable:* `is:ready` (Excludes completed tasks, explicitly/implicitly blocked tasks, tasks starting in the future, and Notes whose children are all unready. `InProcess` bypasses this).
    *   *Comparison:* `~<30m` (duration < 30m), `!<4` (priority < 4).
    *   *Dates:* `@<today` (Overdue), `^>1w` (Starts in > 1 week).
    *   *Collections:* `col:Herbary` (matches the task's collection by name or href, case-insensitive substring).
*   **Match Highlighting:** Search terms are highlighted inline in task titles and descriptions across all clients.
*   **Result Count:** While a search is active, the GUI top bar shows the number of results for the current query instead of the total task count.
*   **Parent Inclusion:** Search matches include parents of matching tasks in the results, so searching for a subtask surfaces its full ancestry.

### 3.2. Multi-Stage Sorting Algorithm
Tasks sort deterministically by rank (0 to 9), then by Overdue -> Priority -> Due Date -> Start Date -> Summary.
*   **Rank 0:** Pinned (`is:pinned`).
*   **Ranks 1-3 (Urgent/Ongoing/Due Soon):** Order dictated by `sort_preset` (e.g., Urgent > Ongoing > Due Soon).
*   **Rank 4 (Actionable):** Due date `<=` `sort_cutoff_days`.
*   **Rank 5 (Deferred):** No due date, or `>` `sort_cutoff_days`.
*   **Rank 6 (Blocked):** Has unresolved dependencies or parent is blocked.
*   **Rank 7 (Future):** Start date is > `start_grace_period_days`.
*   **Rank 8 (Completed):** Done or Cancelled.
*   **Rank 9 (Trash):** In `local://trash`.

*Rule:* If `sort_standard_by_priority` is enabled, Ranks 4 and 5 merge and sort by numeric Priority first, then Date.
*Rule:* Notes (`is:note`) and Journals (`is:journal`) always sort below actionable tasks within the same rank, and automatically drop to Rank 8 if their dates are in the past.
*Rule:* Note and journal sorting is symmetric — overdue notes are ranked as completed to drop them to the bottom of the list, mirroring the behavior for actionable tasks.
*Rule:* Manual sort (`sort:manual`, see 2.2): a task tagged `sort:manual` orders its direct children by explicit position instead of the standard ranks — each child carries a `sort_position` (base-62 fractional index, stored as `X-CFAIT-SORT-POSITION`), and children without a position sort after the positioned ones using the standard comparator. Dependency order still wins: a child with unresolved blockers is surfaced before its blockers regardless of position. Positions are set by moving a sibling — `Alt+Up`/`Alt+Down` in the TUI, or reordering lines in the tree editor (`Ctrl+E` / `cfait edit --tree`). In the TUI, the first move turns on `sort:manual` on the parent automatically; in the tree editor, reordering only takes effect once the parent's line carries the token. Removing the token clears the stored positions.

---

## 4. Core Business Workflows

### 4.0. Undo / Redo & Smart Commands
All clients maintain an active session Undo/Redo stack. Every task mutation applied to the `TaskStore` pushes an `UndoRecord` (containing the exact inverse journal actions derived from pre-mutation snapshots).
*   **Android:** Mutations trigger a transient Snackbar allowing 1-tap Undo. Markdown editors feature explicit ↶/↷ toolbar buttons.
*   **Desktop (GUI/TUI):** Global `Ctrl+Z` (Undo) and `Ctrl+Y` / `Ctrl+Shift+Z` (Redo) shortcuts.
*   **Smart Commands:** If the Add Task input starts with `:` and contains no spaces (e.g., `:undo`, `:redo`, `:empty-trash`, `:delete-all`, `:login`), it is intercepted and executed as a session command rather than creating a task. 
*   **Auto-Complete:** The `CursorContextBanner` acts as a unified auto-complete engine, suggesting commands (when typing `:`), tags (when typing `#`), locations (when typing `@@`), collections (when typing `col:`), and relationships (when typing `dep:`, `rel:`, `parent:`, or `[[`). Task suggestions only draw from currently visible collections (hidden/disabled/system collections never leak their task summaries); when several suggestions share the same summary, each row is labelled with its collection and `dep:`/`rel:`/`parent:` suggestions insert the short UID so the saved reference is unambiguous. Suggestions are typically ordered by exact prefix match, then by descending frequency/usage, and finally alphabetically. In the GUI, the chips are clickable and `Tab` applies the top suggestion; further `Tab` presses cycle through the list (wrapping around) while the completed word stays untouched, the applied chip is highlighted, clicking a chip jumps to it, and the cycle resets as soon as the text or field changes. In the TUI, `Up`/`Down` move the suggestion cursor, `Tab` applies the selected row, and `Esc` dismisses the popup.

### 4.1. The "Yank" Relationship System
Instead of drag-and-drop, Cfait uses a robust "Yank" (Clipboard) system for hierarchy management.
1.  **Yank (`y` / Action Menu):** Copies the selected task's UID to an internal "Yanked" state. UI displays a persistent banner.
2.  **Relate:** Select a *target* task and execute:
    *   `c` (Child): Target becomes a subtask (child) of Yanked. An existing parent is replaced; sync conflicts between two re-parenting devices resolve to the newest relationship (higher `SEQUENCE`, ties to the server, see §1.1) instead of duplicating the task.
    *   `b` (Block): Target becomes blocked by Yanked.
    *   `l` (Link): Target becomes related (sibling) to Yanked.
3.  **Clear (`Esc`):** Clears yank state. (`Y` locks the yanked state for multiple relations). Yanking the already-yanked item also clears the state (toggling), in the task list, the journal header and the TUI.
4.  **Re-parent by text:** The `parent:` token (see §2.2) re-parents via the smart input from any client, accepting the same fuzzy references as `dep:`/`rel:`.

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
        *   **Search & Filter Context:** When searching or filtering by tags/locations, the task tree is filtered to show exact matches alongside their full ancestry and descendants.
            *   *Direct Matches:* Tasks that explicitly match the active filters.
            *   *Descendants:* All subtasks of a direct match are fully visible and treated as matches (inheriting the parent's context).
            *   *Ancestors (Context):* Parent tasks all the way to the root are included to provide structural context, but are visually dimmed (`is_search_context = true`).
            *   *Unrelated Siblings:* Branches without any matches are completely hidden.
        *   During active searches or filtering, all matching task trees are automatically expanded. Users can manually collapse them, but this state is overridden on new queries.
    *   **Android Home-Screen Widget:** The task list widget can optionally respect the user's fold/unfold state via a "Respect fold/unfold state" toggle in the widget configuration screen (default on). When enabled, `respect_tree_collapse = true` is passed in `MobileFilterOptions` so collapsed task trees stay collapsed despite the widget's `is:ready` search query. The fold/unfold indicator is tappable and dispatches `ToggleTreeCollapse`, mirroring the main app's tree state.

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
*   **Subtree Aggregation:** A goal matches a task if the task or any ancestor carries the matching tag/location (or, for `task:`-specific goals, if the task is or descends from the target). A tagged parent's goal naturally covers all work in its subtree — untagged subtasks' time flows up to the nearest tagged ancestor.
*   **Cascade Dedup:** Starting a subtask auto-starts all ancestors, producing overlapping `WorkSession`s on both the subtask and its parents. For Duration goals, all sessions from claimed tasks are union-merged, so overlapping cascade sessions collapse into a single interval — each second of work counts exactly once. For Count goals, sessions are counted per-task: a parent's session fully covered by a descendant's overlapping session is skipped (cascade artifact), but non-overlapping sessions on both parent and child count independently.
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
*   **Fire History:** `alarm_history.json` persists which alarms already fired (keyed by alarm, 24h window). Alarms do not re-fire after an app restart; entries are pruned once they age out of the fire window.
*   **Notifications:** Every fired alarm (explicit or implicit) notifies both the OS and the in-app UI (popup / ringing row).
*   **Snoozing:** Snoozing acknowledges the original alarm and creates a new absolute alarm linked via `RELATED-TO;RELTYPE=SNOOZE`.
*   **Custom Snooze Target:** The custom snooze input is parsed identically in all clients (TUI `s`, the GUI alarm modal, the Android notification reply): a clock time (`16:20`, `4:20pm` — the next time the clock shows that, today if still ahead, otherwise tomorrow), a compact duration (`15m`, `1h`, `16h20m` — from now), a bare minute count (`15`), or a date (`tomorrow`, `monday`, `2026-01-05` — that day at the default reminder time). TUI and GUI report invalid input and keep the alarm ringing; Android falls back to a 10-minute snooze.
*   **Just-In-Time (JIT) Sync:** To prevent phantom alarms across devices, clients must attempt a synchronous network fetch immediately prior to firing an alarm (or within a 15-second pre-fire window). If the task was completed, canceled, or the alarm's trigger time was advanced (via recurrence) on another device, the local alarm is pruned before notifying the user.
*   *Android Implementation:* Uses `AlarmManager.setExactAndAllowWhileIdle`. When an alarm fires, an `AlarmWorker` executes a foreground `api.sync()` before posting a Notification. Notification Actions (Snooze, Done, Pause) are handled via `NotificationActionReceiver` which delegates back to a unique `WorkManager` request to prevent background ANRs.

---

## 5. UI Layout & Platform Specifics

### 5.1. Desktop Graphical User Interface (GUI)
*Powered by `iced`. Optimized for mouse & keyboard.*
*   **Layout:** 3-pane layout (Sidebar, Main List, Markdown Details Pane).
*   **Cursor Context Banner:** Raw text editors feature a dynamic banner that instantly resolves UIDs (`dep:`, `rel:`, `[[...]]`) into task summaries when the text cursor is placed on them.
    *   *Open via Ctrl+Click:* Ctrl+click (Cmd+click on macOS) a `[[wiki link]]` or URL in a text editor to open it, mirroring the TUI's `Ctrl+O`. Wiki links resolve to the target task or create the missing page in context; URLs open externally.
    *   *Click to follow:* When the caret sits on a link that resolves to an existing task (or URL), the banner becomes a clickable chip that navigates to it. Partial links still show autocomplete chips to finish the link; a completed `[[link]]` never re-triggers autocomplete.
*   **Sync Indicator:** The top-bar refresh button reflects sync state: an animated spinner (blue) while a sync is in flight, a red alert when the last sync failed (click to retry), amber pending-changes when local edits are not yet synced, and a neutral refresh icon when everything is up to date.
*   **Window:** Client-Side Decorations (Custom frameless window, resize grips) unless `--force-ssd` is passed.
*   **Zooming:** Global scale via `Ctrl++`, `Ctrl+-`, and `Ctrl+ScrollWheel`. Middle-click resets.
*   **Mouse Interactions:**
    *   *Single Click:* Select row.
    *   *Double Click:* Triggers `EditTaskStart` (focus title input).
    *   *Right Click:* Opens **Full Context Menu** at cursor coordinates; the row highlight (selection) moves to the right-clicked task so the highlighted task is always the one the menu acts on.
    *   *Ellipsis (`...`) Click:* Opens **Partial Context Menu** anchored to the button (shows unpinned actions).
*   **Modals:** Hovering overlays with dimmed backdrops (Move Task, ICS Import, Alarm Notification).
*   **Context Menu keyboard navigation:** `Enter` opens the selected task's context menu with the first entry highlighted; `Up`/`Down` (or `j`/`k`) move the highlight (the menu scrolls to follow), `Enter` activates the highlighted entry, and any other key closes the menu.
*   **Privacy Mode:** When `blur_when_unfocused` is enabled, the window content is blurred when the app loses focus, preventing shoulder-surfing.
*   **Tooltips:** Any GUI button that has an associated keyboard shortcut must include that shortcut in its tooltip (when applicable).

### 5.2. Terminal Interface (TUI)
*Powered by `ratatui`. Keyboard-first, with mouse support.*
*   **Layout:** 2-Pane (Sidebar 20%, Main List 80%). Details view shares vertical space with Main List. Press `Shift+Up/Down` to scroll the active details pane without losing focus on the list. In Journal mode the sidebar is sized to fit the month calendar (7 day columns, or the localized month title when wider) so the full grid is always visible; the content pane absorbs the difference.
*   **Mouse:** Left-clicking a pane moves focus to it (the mouse equivalent of `Tab`); clicking a row in the sidebar or the task list also selects that row. With inline descriptions enabled, task rows can span multiple lines, so a click in the task list only focuses the pane. Scrolling the wheel steps the focused list forward/backward.
*   **Modals/Popups:** Instead of context menus, pressing `Enter` on a task opens a centered **Action Menu** popup with fuzzy filtering. 
*   **Alarm Popup:** When an alarm (explicit, or the implicit due/start auto-reminder) fires, a centered popup shows the task and its key hints. `d`/`Esc`/`Enter` dismiss it without touching the task, `1`/`2` snooze by the preset durations and `s` opens a custom snooze input (see 4.7), `c` marks the task done, `x` cancels the task, and `q`/`Ctrl+C` quit the app.
*   **Details Viewer (`L`):** Unified popup containing the full markdown description, History/Heatmaps, WorkSessions, and relationships (Parents, Children, Blockers, Successors, Siblings) for quick jump navigation.
*   **Session Manager (`T`):** Popup to view/delete `WorkSession` records.
*   **External Editor:** Pressing `E` launches `$VISUAL`/`$EDITOR` (suspending the TUI), falling back to the built-in modal if empty.

### 5.3. Mobile Interface (Android)
*Powered by Jetpack Compose. Touch-optimized.*
*   **Layout:** 
    *   *Top Bar:* Random Jump, Quick Filter, Search toggle, Refresh/Sync, Settings.
*   **Cursor Context Banner:** Raw text editors feature a dynamic banner that instantly resolves UIDs (`dep:`, `rel:`, `[[...]]`) into task summaries when the text cursor is placed on them.
    *   *Open resolved links:* When the caret is on a `[[wiki link]]` that resolves to an existing task, the banner becomes tappable to navigate to it (partial links still show autocomplete to finish the link). Wiki links and URLs in task titles and inline descriptions are also tappable.
    *   *Tabs:* Desktop "Sidebar" is translated into horizontal `HorizontalPager` tabs. Pull-to-refresh triggers manual sync.
    *   *Navigation Drawer:* Swipe from the left edge to switch between Calendars, Tags, Locations, Goals view modes. (Swipe logic uses custom pointer interception to avoid conflicting with tab paging).
*   **Task List Rendering:** `LazyColumn`. Real-time relative duration formatting via coroutines (`liveDurationMins`). Real-time syntax highlighting in input via `VisualTransformation`.
*   **Task Details:** Tapping a task navigates to a dedicated `TaskDetailScreen`. Includes an "Edit Tree" action for full-screen Markdown tree editing.
*   **Context Menu:** Long-pressing a row opens the full Dropdown Menu.
*   **Location Integration (`geo:here`):** If a user types `geo:here`, the UI requests permissions and invokes `LocationManager.getCurrentLocation`. If it fails within 5s, falls back to the last known location.
*   **Notifications:** 
    *   *Ongoing Tasks:* Generate a persistent, swipable notification with a live Chronometer and "Pause"/"Done" actions.
    *   *Alarms:* High-priority. Includes inline "Snooze Custom" via `RemoteInput` text reply, parsed per 4.7.
*   **Intents:** Intercepts `ACTION_VIEW` for `.ics` files to launch the Import Screen.
*   **Debug Export:** UI includes an advanced option to generate a zip of `cache/`, `data/`, `config/`, and `android_crash.txt`, sharing it via `ACTION_SEND`. If a custom `data_dir` is configured, the export also bundles the internal default data directory (`data_internal/`) and the custom one (`data_custom/`) as backups.
*   **AMOLED Black Theme:** A pure-black color variant for OLED screens, selectable from the theme picker.
*   **Top Bar Position:** Configurable to top or bottom via settings (issue #31).

### 5.4. Journal & Wiki Pages (all clients)
Daily notes and wiki pages are `VJOURNAL` components (see 1.2). They share a unified UI across all three clients:
*   **Journal Tab:** A sidebar tab (toggled by `show_journal_tab`) that anchors notes to dates. Selecting a date opens its daily note alongside an activity panel showing tasks due, started, completed, or worked on that day.
*   **Daily Note Resolution:** When several notes share a date (the daily note plus date-anchored wiki pages), the entry whose summary is the date string wins; otherwise the smallest UID wins, so the choice is deterministic.
*   **Wiki Index:** Wiki pages (notes without `DTSTART`) appear in a tree view under the Journal tab. Pages can be nested hierarchically via `[[Parent:Child]]` links and collapsed/expanded with `z` (TUI/GUI). A page whose parent is a regular task (not a journal component) cannot nest in the journal tree; instead it surfaces under dimmed, non-navigable context rows (task-list icon) after the plain root pages, mirroring how search injects ancestor context from outside the scoped set. The ancestor chains of all such pages (each stopping at journal components, system calendars and dangling parents) merge into a single trie: one top-level context group per topmost ancestor task, nested ancestor nodes and pages at increasing depths, each level sorted by summary. Pages whose direct parent is in a system calendar, or whose chain is broken, surface as plain roots.
*   **Archive Section:** Below the Pages (Wiki) and Timeline sections, the journal tab shows an **Archive** section (collapsible with `z`, like the other sections) that collects the pages moved out of the Pages tree. A page is archived when it is flagged `is:archived` (see 2.2), when its direct parent is a completed or cancelled task, or when its parent page is itself archived (the whole sub-tree moves with it) — so finishing a task like "Plan the garden" retires its "Garden path ideas" page out of the wiki index. Dated entries are never auto-archived, and a page parented to a dated entry stays in the Timeline. Archiving is manual and reversible: the `is:archived` token, the archive button in the journal page header (GUI, box-archive icon), or `A` on the selected page (TUI) toggles it, and the flag is stored as `X-CFAIT-ARCHIVED:TRUE` (see 1.2) so it survives sync.
*   **Page Re-parenting:** The journal tab has no re-parent affordance of its own: a `parent:` line typed in a page body stays as prose (journal context never extracts plain lines), and the title input is stored verbatim. Re-parent a page through a smart-input surface (task list title edit, add bar, tree editor) or the journal header's yank button (link icon, same as task rows), which marks the edited page as the yank source and highlights while active; while a yank is active the header also shows a highlighted child-of-yanked button that makes the page being edited a child of the yanked task or page, mirroring the task rows' relation buttons.
*   **Page Creation:** Typing `[[My Page]]` in any text editor creates the page if it doesn't exist. The component type is inherited from the context (actionable `VTODO` from a task, `VJOURNAL` from a page). Use `is:page` or `is:journal` to force the type explicitly.
*   **Page Creation Focus:** Creating a new page focuses the title field first; `Tab` moves focus to the note body.
*   **TUI Journal Navigation:** When the Journal tab is active, `j`/`k` (or arrow keys) navigate the page list in the sidebar, `Enter` opens the selected page for editing, `z` collapses/expands the wiki tree, `A` archives/unarchives the selected page (moving it between the Pages and Archive sections), and `<`/`,` promotes the selected or open page one level up (removing its parent). In the day pane, `←`/`→`/`↑`/`↓` change the selected date and `Enter` opens the editor for the day's entry.
*   **Daily Entry Creation:** The day's entry is a `VJOURNAL` titled with the date string and anchored with `DTSTART;VALUE=DATE` to the selected date (see 2.1). In the GUI, typing in the day's editor creates it on the first keystroke; in the TUI, `Enter` opens the editor and saving non-empty text creates it (typing alone does nothing on an empty day — the TUI is modal). `Ctrl+N` in the journal tab creates a wiki *page* (no date), not a dated entry.
*   **Android Journal:** A month-grid calendar view with daily-note indicators; tapping a day opens its note. Pages can be moved between dates via the context menu.

---

## 6. Keyboard Shortcuts (GUI & TUI)

*   **Navigation:** `j`/`k` or `Up`/`Down` (Select), `Tab` (applies the top autocomplete suggestion in any editor with a suggestion banner and cycles through the suggestion list on repeated presses; with no suggestion it indents the current line inside multi-line editors and cycles focus between Sidebar, List, Input). `1..5` (Switch Sidebar tabs: 1:Collections, 2:Tags, 3:Locations, 4:Goals, 5:Journal). From text fields, use `Ctrl+1..5` instead. `Enter` (GUI) opens the selected task's context menu; while it is open, `Up`/`Down` (or `j`/`k`) move the highlighted entry, `Enter` activates it, and any other key closes the menu.
*   **Main Actions:** 
    *   `Space`: Toggle Done/NeedsAction.
    *   `Shift+Space`: Complete & Shift recurrence (Relative advance).
    *   `s`: Start/Pause timer.
    *   `S`: Stop/Reset timer.
    *   `x`: Cancel task.
    *   `+` / `-`: Increase/Decrease priority.
    *   `e`: Edit title. `E`: Edit description (Markdown). `Ctrl+E`: Edit tree (Markdown) / Switch editor mode. `Ctrl+N`: Create new task with description. `Ctrl+M`: Maximize/Restore description editor.
    *   `Delete`: Move to trash. `Ctrl+Delete`: Delete entire tree.
    *   `M`: Move task (or task tree) to another collection.
    *   `Alt+Up` / `Alt+Down`: Move the selected task up/down among its siblings (manual sort, see 3.2).
    *   `t`: Log time session manually.
    *   `Ctrl+Z` / `Ctrl+Y` / `Ctrl+Shift+Z`: Undo / Redo.
*   **Text Editing (in editors):** `Ctrl+Left` / `Ctrl+Right`: move by word. `Ctrl+Backspace` / `Ctrl+Delete`: delete previous / next word (UAX#29 boundaries). On macOS, use `Option` instead of `Ctrl`. `Enter` continues the current list item (bullets, checkboxes, and numbered lists auto-increment; an empty list item ends the list). `Tab` / `Shift+Tab`: indent / outdent the current line by two spaces.
*   **Tree/Relationships:** 
    *   `z`: Fold/Unfold tree.
    *   `>` / `.` : Demote (Indent / Make child of previous).
    *   `<` / `,` : Promote (Outdent / Move one level up).
    *   `L` : Open relationship browser / toggle details. In the GUI, `Ctrl+Enter` is an alias that also works from text fields.
    *   `o`: Open URL attached to the selected task.
    *   `Ctrl+O` (TUI): Open the wiki link or URL under the text cursor.
*   **App Actions:** 
    *   `/`: Focus search.
    *   `a`: Focus add task.
    *   `w`: Toggle Quick Filter.
    *   `m`: Toggle Match AND/OR logic for sidebar tags.
    *   `H`: Toggle Hide Completed.
    *   `*`: Clear all filters.
    *   `?` (and `F1` in the GUI): Open the shortcuts help.
    *   `Shift+R`: Jump to random actionable task (weighted by priority).
    *   `Ctrl+,`: Settings.
*   **Layout independence (GUI):** Symbol shortcuts (`?`, `*`, `+`, `-`, `>`, `<`, `,`, `.`, `/`) match the character actually typed (`modified_key`, with Shift applied), not the unshifted key, so they fire on layouts where the symbol requires modifiers (e.g. `?` on Turkish-Q). `F1` is a layout-independent way to open the help.

---

## 7. Command Line Interface (CLI)
Used for headless automation, scripting, and piping. Operates directly on the `TaskStore`.

*Note on `<uid>` arguments:* Any CLI command accepting a `<uid>` also accepts partial UIDs, exact titles, partial summaries, or wiki-links (e.g. `[[My Task]]`). If a match is ambiguous, the CLI will output the matching options and exit.

*Global flags:* Most mutation commands accept `-n` / `--no-wait` (queue to journal and exit without syncing) and `-w` / `--wait` (block until network sync completes).

*   `cfait add` (alias: `create`) `<task...>`: Smart input task creation. Inline alias definitions (`#key := #a, @@loc`) are saved and applied retroactively to existing tasks. Pass `--edit` to open the buffer in the external editor (`$VISUAL`/`$EDITOR`) before creating: line 1 is the smart title, the lines below become the description (unless `--desc` is given), and saving an empty buffer cancels. The buffer is seeded with the positional arguments, so saving it unchanged creates the same task as without the flag. Flags: `-c <href>`, `--desc <text>`, `-p <uid>` (set parent), `-n`, `-w`. Type recipes (see 2.1): `cfait add "Water the tomatoes" #gardening` (task), `cfait add "Pantry is:note"` (note/header), `cfait add "Garden path ideas is:page"` (wiki page), `cfait add "Reading list is:flat"` (flat document), `cfait add "Old recipe ideas is:page is:archived"` (archived wiki page), `cfait add "2026-10-17 is:journal start:2026-10-17" --desc "..."` (journal entry for that date).
*   `cfait append <uid> <task...>`: Appends smart syntax tokens (tags, dates, deps, etc.) or text to an existing task. Flags: `--desc <text>` (appends to existing description), `-n`, `-w`.
*   `cfait edit <uid> [--tree]`: Opens an external editor (`$VISUAL`/`$EDITOR`) to edit the task's properties. Pass `--tree` to edit the entire task tree as a single Markdown document.
*   `cfait replace <uid> <task...>`: Replaces the entire task summary and metadata. To safely add tags or dates without losing the title, use `append`. Flags: `--clear-due`, `--clear-start`, `--clear-tags`, `--clear-loc`, `--clear-deps`, `-p <uid>`, `--clear-parent`, `--desc <text>`, `--file <path>` (replaces from markdown file), `--tree` (when used with `--file`, replaces entire tree), `-n`, `-w`.
*   `cfait list [<query>] [--all] [--json] [--fields <a,b,c>] [-c <id>] [-p <uid>]`: Outputs task tree; an optional positional query filters the output (use `-p` to focus on a specific sub-tree). `--fields` prints one TSV line per task with only the requested columns (no header) for scripting; it takes precedence over `--json`. Accepted fields: `uid`, `col` (`colid`, `collection`), `summary` (`title`), `desc` (`description`), `status`, `due`, `start` (`dtstart`), `priority`, `percent` (`pct`), `categories` (`tags`), `locations`, `url`, `parent`, `related`, `dependencies`, `sequence`, `time_spent` (seconds), `estimate` (minutes), `rrule`.
*   `cfait search <query> [--all] [--json] [--fields <a,b,c>] [-c <id>] [-p <uid>]`: Searches and outputs tasks within a specific sub-tree.
*   `cfait view` (alias: `show`) `<uid> [--json]`: Outputs detailed task info.
*   `cfait tree <uid> [--plain]`: Views the task tree starting at `<uid>` serialized into markdown format (same format used by the `Ctrl+E` editor). `--plain` omits the UID comments and `dep:`/`rel:` tags for clean copy-paste into external documents.
*   `cfait start|pause|toggle|done <uid>`: State mutation commands. `done` is idempotent (an already-done task stays done); `toggle` flips the status.
*   `cfait move` (alias: `mv`) `<uid> <collection> [--tree]`: Moves a task to a different collection.
*   `cfait delete` (alias: `rm`) `<uid>`: Moves task to trash.
*   `cfait export [--collection <id>]`: Dumps collection (local or remote) as standard ICS to stdout, including VTODO and VJOURNAL components. The collection `id` can be a local collection name, a full HREF, or a remote collection name.
*   `cfait import <file.ics> [--collection <id>]`: Parses and imports ICS to store. Supports both local and remote collections; for remote collections, tasks are journaled and synced. Handles both VTODO and VJOURNAL components. Without `--collection`, the target resolves like `add` (via `resolve_default_target`): the `default_calendar`, else the first visible, non-disabled, non-system collection, else `local://default`.
*   `cfait sync`: Foreground network sync.
*   `cfait daemon`: Runs a continuous background sync loop based on `auto_refresh_interval_mins`. Acquires a cross-process lock to prevent overlapping syncs with UIs.
*   `cfait collection list [--json]`: Lists CalDAV collections.
*   `cfait collection create <name> [--color #hex]`: Creates a new collection.
*   `cfait collection edit <href> --name <name> [--color #hex]`: Edits a collection's display name or color.
*   `cfait collection default [name-or-href]`: Sets the default collection used when creating tasks without `-c`/`--collection`; accepts a collection name or full HREF and unhides the collection if it was hidden. With no argument, shows the current default.
*   `cfait login <url> <username>`: Configures the CalDAV server and securely prompts for the password to store in the OS keyring. When the keyring is unreachable, the password is kept in `config.toml` instead and the user is told so; when a `password_command` is set, the typed password is not stored at all (the command owns the credential).

---

## 8. Configuration (`config.toml`)
All persistent state and settings live here. Unrecognized TOML keys must not be dropped during serialization.

**Data location:**
*   `data_dir`: String (Optional). Absolute or `~/`-relative path overriding where cfait stores its data files (e.g. `local.json`, `journal.json`, `alarm_index.json`) instead of the XDG default (`~/.local/share/cfait`). Read before any other setting so the data directory can be relocated (e.g. into a syncthing-managed folder) without moving config or cache. Only the data directory is affected; config and cache stay at their XDG defaults.

**Connection & Sync:**
*   `url`, `username`: CalDAV credentials. *(Password vaulted in OS Keyring; kept in this file — with a notice — only while no keyring is reachable.)*
*   `password_command`: String (Optional). Shell command whose stdout (trailing newlines stripped) is used as the CalDAV password (e.g. `pass show cfait`). Takes precedence over the keyring and any `password` line; its output is never stored. Runs through the system shell with a 10-second timeout; on failure Cfait falls back to keyring/file.
*   `tls_client_cert_path`, `tls_client_key_path`: Strings (Optional). Paths to PEM-encoded certificate and private key for mTLS.
*   `allow_insecure_certs`: Boolean.
*   `sync_settings`: Boolean. Enables cross-device settings sync (custom collection properties when the server supports them, otherwise the hidden 1970 CANCELLED VEVENT carrier).
*   `auto_refresh_interval_mins`: Integer. Daemon sync loop interval.
*   `trash_retention_days`: Integer. Days before `local://trash` items are permanently purged. (0 = disable trash).

**UI & Behavior:**
*   `default_calendar`: String HREF.
*   `enable_local_mode`: Boolean. Allow offline `local://` collections.
*   `hide_completed`, `hide_fully_completed_tags`, `hide_aliases_in_sidebar`: Booleans.
*   `blur_when_unfocused`: Boolean. Privacy mode — hides task content when the window loses focus.
*   `strikethrough_completed`: Boolean. Line-through styling for done tasks.
*   `dim_future_tasks`: Boolean (per-device, never synced). Dims the whole row (title, tags, date) of tasks whose start is in the future, in the TUI, GUI and Android.
*   `celebrate_completions`: Boolean (per-device, never synced). Celebrate completing a task: a confetti burst in the GUI, confetti plus a `Confirm` haptic on Android, and a terminal bell in the TUI. Celebrations trigger only on user-initiated completions (never on completions arriving via sync or undo).
*   `show_inline_descriptions`: Boolean. Previews up to 3 lines of the description in the list.
*   `ui_scale`: Float (0.5-3.0). Global zoom.
*   `theme`: Enum (RustyDark, Light, Dracula, Nord, Catppuccin variants, etc.). Tag colors are derived deterministically from the tag name and adapt to the theme's light/dark variant: on light themes the color is darkened (with a luminance cap so bright hues like yellow, green and cyan stay readable), so the same tag keeps its hue but stays legible on light backgrounds.
*   `language`: String (`en`, `fr`). None = system locale.
*   `first_day_of_week`: Enum (`Monday`, `Sunday`). Controls the first day in calendar/journal week views.
*   `date_format`: Enum (`auto`, `ymd`, `dmy`, `mdy`, `ydm`, `myd`, `dym`). How typed dates (e.g. `due:01/02/2026`) are interpreted in task input. `auto` follows the language's format. Both `-` and `/` separators are accepted for the chosen order, and ISO 8601 (`2026-01-02`) plus compact (`20260102`) are always accepted regardless of the setting.
*   `description_editor`: String. CLI command for TUI description editing. `builtin` forces internal UI editor.
*   `show_ongoing_notifications`, `show_priority_numbers`, `sidebar_is_hidden`, `show_task_goals_in_sidebar`: Booleans.
*   `show_calendars_tab`, `show_tags_tab`, `show_locations_tab`, `show_goals_tab`, `show_journal_tab`: Booleans. Toggle individual sidebar tab visibility. On Android, these live under "More settings" (advanced settings), not "Manage collections".
*   `show_undo_snackbar`: Boolean. Show the transient undo notification after task mutations (Android).
*   `pinned_actions`: Array of `TaskAction` enums. Dictates buttons pinned directly to GUI task rows.
*   `log_level`: Enum (`Error`, `Warn`, `Info`, `Debug`, `Trace`). Logging verbosity for both log file and terminal.

**Sorting & Limits:**
*   `sort_preset`: Enum (`UrgentStartedDue`, `UrgentDueStarted`, `StartedUrgentDue`).
*   `paused_sort_behavior`: Enum (`tiebreak`, `top`, `none`). Defines how paused tasks sort against unstarted ones. Default is `tiebreak` (wins ties within the same rank/priority).
*   `sort_tiebreak_recent`: Boolean. If true, ties in sorting are broken by recently modified rather than alphabetical.
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
