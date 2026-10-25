// SPDX-License-Identifier: GPL-3.0-or-later
//! Wiki-link binding in journal tree sync.
//!
//! A `[[wiki link]]` line names a task: when a task with that summary
//! exists, the line binds to it ("jump to") instead of minting a duplicate
//! ("create"). A pure reference line only pins the task's position in the
//! document — it must not edit the task it names, so the task's description,
//! tokens, and status survive the save untouched.
use cfait::context::TestContext;
use cfait::model::{CalendarListEntry, Task};
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
        supports_vevent: Some(true),
    }]
}

fn sync_opts<'a>(
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

/// A journal page plus an existing task elsewhere in the garden, with state
/// of its own (a tag, a due date, a description).
fn store_with_page_and_task() -> (
    TaskStore,
    Vec<CalendarListEntry>,
    HashMap<String, Vec<String>>,
) {
    let mut store = make_store();
    let mut page = Task::new("PowerShed timeline is:page", &HashMap::new(), None);
    page.uid = "page-1".to_string();
    page.calendar_href = "local://default".to_string();
    page.is_journal = true;
    page.description = "Plan the wiring before the walls go up.".to_string();
    store.add_task(page);

    let mut task = Task::new(
        "Ventilation for portable tools #shed due:2026-11-01",
        &HashMap::new(),
        None,
    );
    task.uid = "task-1".to_string();
    task.calendar_href = "local://default".to_string();
    task.description = "A small fan on a timer keeps the dust moving.".to_string();
    store.add_task(task);

    (store, calendars(), HashMap::new())
}

fn summary_count(store: &TaskStore, summary: &str) -> usize {
    store
        .calendars
        .values()
        .flat_map(|m| m.values())
        .filter(|t| t.calendar_href != "local://trash" && t.summary == summary)
        .count()
}

#[test]
fn wiki_link_to_existing_task_binds_instead_of_minting() {
    let (mut store, cals, aliases) = store_with_page_and_task();
    let opts = sync_opts(&aliases, &cals);

    // The line the GUI autocomplete produces: a checkbox, then the link,
    // nothing else.
    let typed =
        "Plan the wiring before the walls go up.\n1. [ ] [[Ventilation for portable tools]]";
    let (actions, warnings, _mints) = store
        .sync_tree_from_markdown("page-1", typed, &opts, true, false)
        .unwrap();
    assert!(warnings.is_empty());
    assert!(
        !actions
            .iter()
            .any(|a| matches!(a, cfait::journal::Action::Create(t) if t.summary == "Ventilation for portable tools")),
        "a link to an existing task must not mint a duplicate: {actions:?}"
    );
    assert_eq!(
        summary_count(&store, "Ventilation for portable tools"),
        1,
        "exactly one task with that summary may exist"
    );

    let task = store
        .get_task_ref("task-1")
        .expect("the existing task must survive");
    assert_eq!(
        task.parent_uid.as_deref(),
        Some("page-1"),
        "the link places the task in the page's document"
    );
    assert_eq!(
        task.description, "A small fan on a timer keeps the dust moving.",
        "a pure reference line must not wipe the task's description"
    );
    assert_eq!(task.categories, vec!["shed".to_string()]);
    assert_eq!(
        task.due,
        Some(cfait::model::DateType::AllDay(
            chrono::NaiveDate::from_ymd_opt(2026, 11, 1).unwrap()
        )),
        "a pure reference line must not reset the task's due date"
    );

    // The page's document records the shadow line, re-tagged with the
    // bound task's uid.
    let page = store.get_task_ref("page-1").unwrap();
    assert!(
        page.description
            .contains("1. [ ] [[Ventilation for portable tools]] <!-- uid:task-1 -->"),
        "the shadow line must carry the bound task's uid: {}",
        page.description
    );
}

#[test]
fn wiki_link_roundtrip_is_idempotent() {
    let (mut store, cals, aliases) = store_with_page_and_task();
    let opts = sync_opts(&aliases, &cals);

    let typed =
        "Plan the wiring before the walls go up.\n1. [ ] [[Ventilation for portable tools]]";
    let (_actions, warnings, _mints) = store
        .sync_tree_from_markdown("page-1", typed, &opts, true, false)
        .unwrap();
    assert!(warnings.is_empty());

    // The journal editor serializes the page with the shadow line in place;
    // re-saving that view must never mint a duplicate or edit the linked
    // task. The shadow's text normalizes from the link to the task's
    // expanded form on the first such save, after which the page is settled.
    let md = cfait::model::extractor::serialize_journal_document(&store, "page-1", &cals);
    assert!(
        md.contains("1. [ ] Ventilation for portable tools #shed @2026-11-01 <!-- uid:task-1 -->"),
        "the linked task renders expanded at its reference line: {md}"
    );
    let (actions, warnings, _mints) = store
        .sync_tree_from_markdown("page-1", &md, &opts, true, false)
        .unwrap();
    assert!(warnings.is_empty());
    assert!(
        actions
            .iter()
            .all(|a| matches!(a, cfait::journal::Action::Update(t) if t.uid == "page-1")),
        "re-saving touches only the page's shadow line, never the linked task: {actions:?}"
    );
    assert_eq!(
        store.get_task_ref("task-1").unwrap().description,
        "A small fan on a timer keeps the dust moving.",
        "re-saving must not touch the linked task"
    );
    assert!(
        store.get_task_ref("page-1").unwrap().description.contains(
            "1. [ ] Ventilation for portable tools #shed @2026-11-01 <!-- uid:task-1 -->"
        ),
        "the shadow line settles on the task's expanded form: {}",
        store.get_task_ref("page-1").unwrap().description
    );

    // Settled: the serialized page is stable and re-saving it is a no-op.
    let md2 = cfait::model::extractor::serialize_journal_document(&store, "page-1", &cals);
    assert_eq!(md2, md, "the serialized page must be stable:\n{md2}");
    let (actions2, warnings2, _mints2) = store
        .sync_tree_from_markdown("page-1", &md2, &opts, true, false)
        .unwrap();
    assert!(warnings2.is_empty());
    assert!(
        actions2.is_empty(),
        "re-saving the settled page must be a no-op: {actions2:?}"
    );
    assert_eq!(
        store.get_task_ref("task-1").unwrap().description,
        "A small fan on a timer keeps the dust moving.",
        "the linked task's document must survive repeated saves"
    );
}

#[test]
fn wiki_link_to_unknown_task_still_creates() {
    let (mut store, cals, aliases) = store_with_page_and_task();
    let opts = sync_opts(&aliases, &cals);

    let typed =
        "Plan the wiring before the walls go up.\n- [ ] [[Fresh start on the compost heap]]";
    let (actions, warnings, _mints) = store
        .sync_tree_from_markdown("page-1", typed, &opts, true, false)
        .unwrap();
    assert!(warnings.is_empty());
    assert!(
        actions
            .iter()
            .any(|a| matches!(a, cfait::journal::Action::Create(t) if t.summary == "Fresh start on the compost heap")),
        "a link naming no existing task must create it: {actions:?}"
    );
    let created = store
        .calendars
        .values()
        .flat_map(|m| m.values())
        .find(|t| t.summary == "Fresh start on the compost heap")
        .expect("the created task");
    assert_eq!(
        created.parent_uid.as_deref(),
        Some("page-1"),
        "the created task lands in the page's document"
    );
}

#[test]
fn wiki_link_with_alias_binds_to_the_named_task() {
    let (mut store, cals, aliases) = store_with_page_and_task();
    let opts = sync_opts(&aliases, &cals);

    let typed =
        "Plan the wiring before the walls go up.\n- [ ] [[Ventilation for portable tools|vent]]";
    let (actions, warnings, _mints) = store
        .sync_tree_from_markdown("page-1", typed, &opts, true, false)
        .unwrap();
    assert!(warnings.is_empty());
    assert!(
        !actions
            .iter()
            .any(|a| matches!(a, cfait::journal::Action::Create(t) if t.summary == "Ventilation for portable tools")),
        "the alias must not change what the link names: {actions:?}"
    );
    assert_eq!(
        store.get_task_ref("task-1").unwrap().parent_uid.as_deref(),
        Some("page-1"),
        "the aliased link must bind to the named task"
    );
}

#[test]
fn wiki_link_line_with_tokens_keeps_create_behavior() {
    // A link line carrying tokens beyond the link is an edit, not a pure
    // reference: it keeps the existing create behavior rather than binding
    // (binding it would re-parse the line and wipe the task's description).
    let (mut store, cals, aliases) = store_with_page_and_task();
    let opts = sync_opts(&aliases, &cals);

    let typed = "Plan the wiring before the walls go up.\n- [ ] [[Ventilation for portable tools]] due:2026-11-02";
    let (actions, warnings, _mints) = store
        .sync_tree_from_markdown("page-1", typed, &opts, true, false)
        .unwrap();
    assert!(warnings.is_empty());
    assert!(
        actions
            .iter()
            .any(|a| matches!(a, cfait::journal::Action::Create(t) if t.summary == "Ventilation for portable tools")),
        "a link line with tokens still creates: {actions:?}"
    );
}

#[test]
fn escaped_wiki_link_line_creates_literal_title() {
    // The escaped form names a new task whose title is the link itself; it
    // must not bind to the existing task it happens to reference.
    let (mut store, cals, aliases) = store_with_page_and_task();
    let opts = sync_opts(&aliases, &cals);

    let typed =
        "Plan the wiring before the walls go up.\n1. [ ] \\[[Ventilation for portable tools]]";
    let (actions, warnings, _mints) = store
        .sync_tree_from_markdown("page-1", typed, &opts, true, false)
        .unwrap();
    assert!(warnings.is_empty());
    assert!(
        actions
            .iter()
            .any(|a| matches!(a, cfait::journal::Action::Create(t) if t.summary == "[[Ventilation for portable tools]]")),
        "an escaped link must create a literal-titled task: {actions:?}"
    );
    assert_eq!(
        store.get_task_ref("task-1").unwrap().parent_uid,
        None,
        "the escaped link must not re-parent the existing task"
    );
    assert_eq!(
        summary_count(&store, "Ventilation for portable tools"),
        1,
        "the existing task is untouched"
    );
    assert_eq!(
        summary_count(&store, "[[Ventilation for portable tools]]"),
        1,
        "exactly one literal-titled task is created"
    );
}

#[test]
fn escaped_wiki_link_roundtrip_is_idempotent() {
    // A literal link title serializes in escaped form and re-parses to the
    // same title: re-saving the page must never reduce it to its target.
    let (mut store, cals, aliases) = store_with_page_and_task();
    let opts = sync_opts(&aliases, &cals);

    let typed =
        "Plan the wiring before the walls go up.\n1. [ ] \\[[Ventilation for portable tools]]";
    let (_actions, warnings, _mints) = store
        .sync_tree_from_markdown("page-1", typed, &opts, true, false)
        .unwrap();
    assert!(warnings.is_empty());

    let md = cfait::model::extractor::serialize_journal_document(&store, "page-1", &cals);
    assert!(
        md.contains("\\[[Ventilation for portable tools]]"),
        "the literal link title serializes in escaped form: {md}"
    );
    let (actions, warnings, _mints) = store
        .sync_tree_from_markdown("page-1", &md, &opts, true, false)
        .unwrap();
    assert!(warnings.is_empty());
    assert_eq!(
        summary_count(&store, "[[Ventilation for portable tools]]"),
        1,
        "the literal title must survive a re-save"
    );
    assert_eq!(
        summary_count(&store, "Ventilation for portable tools"),
        1,
        "re-saving must not mint a plain duplicate of the target"
    );
    assert!(
        actions
            .iter()
            .all(|a| matches!(a, cfait::journal::Action::Update(t) if t.summary == "[[Ventilation for portable tools]]" || t.uid == "page-1")),
        "re-saving touches only the page and the literal-titled task: {actions:?}"
    );
}

#[test]
fn escape_pure_wiki_link_resolves_existing_target() {
    let (store, _cals, _aliases) = store_with_page_and_task();
    // A pure link to an existing unique task is re-escaped.
    assert_eq!(
        store.escape_pure_wiki_link("[[Ventilation for portable tools]]"),
        Some("\\[[Ventilation for portable tools]]".to_string())
    );
    // The alias is preserved in the escaped form.
    assert_eq!(
        store.escape_pure_wiki_link("[[Ventilation for portable tools|vent]]"),
        Some("\\[[Ventilation for portable tools|vent]]".to_string())
    );
    // A link naming no task is left for the create path.
    assert_eq!(
        store.escape_pure_wiki_link("[[Fresh start on the compost heap]]"),
        None
    );
    // Anything beyond a pure link is left alone.
    assert_eq!(
        store.escape_pure_wiki_link("[[Ventilation for portable tools]] due:2026-11-02"),
        None
    );
    assert_eq!(
        store.escape_pure_wiki_link("a [[Ventilation for portable tools]]"),
        None
    );
}

#[test]
fn escape_pure_wiki_link_rejects_ambiguous_target() {
    let (mut store, _cals, _aliases) = store_with_page_and_task();
    let mut twin = Task::new(
        "Ventilation for portable tools #attic",
        &HashMap::new(),
        None,
    );
    twin.uid = "task-2".to_string();
    twin.calendar_href = "local://default".to_string();
    store.add_task(twin);

    // Two tasks share the name: never guess, fall through to create.
    assert_eq!(
        store.escape_pure_wiki_link("[[Ventilation for portable tools]]"),
        None
    );
}

#[test]
fn ambiguous_wiki_link_name_falls_through_to_create() {
    // Two tasks share a summary: the link must never silently bind one of
    // them, so it falls through to the create path.
    let (mut store, cals, aliases) = store_with_page_and_task();
    let mut twin = Task::new(
        "Ventilation for portable tools #attic",
        &HashMap::new(),
        None,
    );
    twin.uid = "task-2".to_string();
    twin.calendar_href = "local://default".to_string();
    store.add_task(twin);
    let opts = sync_opts(&aliases, &cals);

    let typed = "Plan the wiring before the walls go up.\n- [ ] [[Ventilation for portable tools]]";
    let (actions, warnings, _mints) = store
        .sync_tree_from_markdown("page-1", typed, &opts, true, false)
        .unwrap();
    assert!(warnings.is_empty());
    assert!(
        actions
            .iter()
            .any(|a| matches!(a, cfait::journal::Action::Create(t) if t.summary == "Ventilation for portable tools")),
        "an ambiguous name must create rather than guess: {actions:?}"
    );
    assert_eq!(
        store.get_task_ref("task-1").unwrap().parent_uid,
        None,
        "neither twin may be re-parented by an ambiguous link"
    );
    assert_eq!(
        store.get_task_ref("task-2").unwrap().parent_uid,
        None,
        "neither twin may be re-parented by an ambiguous link"
    );
}
