// SPDX-License-Identifier: GPL-3.0-or-later
//! The GUI journal editor debounces its saves: about 500 ms after the last
//! keystroke the whole buffer is re-parsed by `sync_tree_from_markdown`. A
//! line without a `<!-- uid -->` tag mints a fresh uid on every sync, so
//! without re-tagging each intermediate version of a line would accumulate —
//! either as a duplicate, or (now that an absent line is an explicit
//! deletion) as a trashed copy.
//!
//! These tests simulate the editor's save flow as implemented in
//! `flush_journal_save`: sync the buffer, re-tag the minted lines in the
//! buffer with `inject_uid_tags`, then sync the re-tagged buffer. The second
//! sync must match the line by uid instead of minting again, and leave
//! exactly one active component with no trashed intermediates.
use cfait::context::TestContext;
use cfait::model::{CalendarListEntry, DateType, Task};
use cfait::store::{SyncTreeOptions, TaskStore};
use std::collections::HashMap;
use std::sync::Arc;

fn make_store() -> TaskStore {
    let ctx = Arc::new(TestContext::new());
    TaskStore::new(ctx)
}

fn calendars() -> Vec<CalendarListEntry> {
    vec![CalendarListEntry {
        name: "Local".to_string(),
        href: "local://default".to_string(),
        color: None,
        supports_vjournal: Some(true),
    }]
}

fn opts<'a>(
    aliases: &'a HashMap<String, Vec<String>>,
    cals: &'a [CalendarListEntry],
) -> SyncTreeOptions<'a> {
    SyncTreeOptions {
        aliases,
        default_reminder_time: None,
        trash_retention_days: 30,
        calendars: cals,
    }
}

fn daily_root(date: chrono::NaiveDate) -> Task {
    let mut t = Task::new(&date.format("%Y-%m-%d").to_string(), &HashMap::new(), None);
    t.uid = "root-1".to_string();
    t.calendar_href = "local://default".to_string();
    t.is_journal = true;
    t.dtstart = Some(DateType::AllDay(date));
    t
}

fn active_tasks(store: &TaskStore) -> Vec<Task> {
    store
        .calendars
        .values()
        .flat_map(|m| m.values())
        .filter(|t| t.calendar_href != "local://trash")
        .cloned()
        .collect()
}

fn trashed_tasks(store: &TaskStore) -> Vec<Task> {
    store
        .calendars
        .values()
        .flat_map(|m| m.values())
        .filter(|t| t.calendar_href == "local://trash")
        .cloned()
        .collect()
}

#[test]
fn typing_a_checkbox_line_across_debounced_saves() {
    let mut store = make_store();
    store.add_task(daily_root(
        chrono::NaiveDate::from_ymd_opt(2026, 10, 3).unwrap(),
    ));

    let cals = calendars();
    let aliases = HashMap::new();

    // Keystroke batch 1: the user typed "Woke up early.\n- [ ] Buy" and
    // paused past the debounce.
    let buf1 = "Woke up early.\n- [ ] Buy";
    let (_actions, warnings, mints1) = store
        .sync_tree_from_markdown("root-1", buf1, &opts(&aliases, &cals), true)
        .unwrap();
    assert!(warnings.is_empty());
    assert_eq!(
        mints1.len(),
        1,
        "the checkbox line must be minted once: {mints1:?}"
    );

    // The editor re-tags the minted line; the tag is appended at the line
    // end, so the cursor keeps its (line, column).
    let retagged = cfait::model::extractor::inject_uid_tags(buf1, &mints1);
    assert!(
        retagged.contains("<!-- uid:"),
        "the minted line must be re-tagged: {retagged}"
    );

    // Keystroke batch 2: the user finished the line and paused again.
    let buf2 = retagged.replace("- [ ] Buy", "- [ ] Buy milk");
    let (_actions2, warnings2, mints2) = store
        .sync_tree_from_markdown("root-1", &buf2, &opts(&aliases, &cals), true)
        .unwrap();
    assert!(warnings2.is_empty());
    assert!(
        mints2.is_empty(),
        "the re-tagged line must match by uid, not mint again: {mints2:?}"
    );

    let active = active_tasks(&store);
    let subtasks: Vec<_> = active
        .iter()
        .filter(|t| t.parent_uid.as_deref() == Some("root-1"))
        .collect();
    assert_eq!(
        subtasks.len(),
        1,
        "exactly one active subtask expected, got {:?}",
        subtasks.iter().map(|t| &t.summary).collect::<Vec<_>>()
    );
    assert_eq!(subtasks[0].summary, "Buy milk");
    assert_eq!(
        store.get_task_ref("root-1").unwrap().description.trim(),
        "Woke up early.",
        "plain text must stay in the daily note's description"
    );
    assert!(
        trashed_tasks(&store).is_empty(),
        "no intermediate version may end up in the trash: {:?}",
        trashed_tasks(&store)
            .iter()
            .map(|t| &t.summary)
            .collect::<Vec<_>>()
    );
}

#[test]
fn typing_a_page_line_across_debounced_saves() {
    let mut store = make_store();
    store.add_task(daily_root(
        chrono::NaiveDate::from_ymd_opt(2026, 10, 3).unwrap(),
    ));

    let cals = calendars();
    let aliases = HashMap::new();

    // Keystroke batch 1: "Notes.\n- My Pag is:page", paused mid-word.
    let buf1 = "Notes.\n- My Pag is:page";
    let (_actions, warnings, mints1) = store
        .sync_tree_from_markdown("root-1", buf1, &opts(&aliases, &cals), true)
        .unwrap();
    assert!(warnings.is_empty());
    assert_eq!(
        mints1.len(),
        1,
        "the page line must be minted once: {mints1:?}"
    );

    let retagged = cfait::model::extractor::inject_uid_tags(buf1, &mints1);
    assert!(
        retagged.contains("<!-- uid:"),
        "the minted line must be re-tagged: {retagged}"
    );

    // Keystroke batch 2: the user completed the word.
    let buf2 = retagged.replace("My Pag", "My Page");
    let (_actions2, warnings2, mints2) = store
        .sync_tree_from_markdown("root-1", &buf2, &opts(&aliases, &cals), true)
        .unwrap();
    assert!(warnings2.is_empty());
    assert!(
        mints2.is_empty(),
        "the re-tagged line must match by uid, not mint again: {mints2:?}"
    );

    let pages: Vec<_> = active_tasks(&store)
        .into_iter()
        .filter(|t| t.is_journal && t.uid != "root-1")
        .collect();
    assert_eq!(
        pages.len(),
        1,
        "exactly one active sub-page expected, got {:?}",
        pages.iter().map(|t| &t.summary).collect::<Vec<_>>()
    );
    assert_eq!(pages[0].summary, "My Page");
    assert!(
        trashed_tasks(&store).is_empty(),
        "no intermediate version may end up in the trash: {:?}",
        trashed_tasks(&store)
            .iter()
            .map(|t| &t.summary)
            .collect::<Vec<_>>()
    );
}
