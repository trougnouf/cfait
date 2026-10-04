// SPDX-License-Identifier: GPL-3.0-or-later
//! Journal page and daily-note tree sync behavior.
//!
//! A journal page (a task with `is_journal`) is always serialized and synced
//! with the page's own flag: the description is emitted raw at depth 0 and
//! sub-pages as flat bullets at depth 0. These tests pin the round-trip
//! guarantees that keep a page from repeating itself — its description
//! content being re-extracted as subtasks, or sub-pages being duplicated on
//! re-save — no matter whether the edit came from the task tab or the
//! journal tab.
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

fn journal_page(uid: &str, summary: &str, description: &str) -> Task {
    let mut t = Task::new(summary, &HashMap::new(), None);
    t.uid = uid.to_string();
    t.calendar_href = "local://default".to_string();
    t.is_journal = true;
    t.description = description.to_string();
    t
}

fn children_of(store: &TaskStore, parent: &str) -> Vec<Task> {
    store
        .calendars
        .values()
        .flat_map(|m| m.values())
        .filter(|t| t.parent_uid.as_deref() == Some(parent))
        .cloned()
        .collect()
}

#[test]
fn journal_page_roundtrip_is_idempotent() {
    let mut store = make_store();
    let date = chrono::NaiveDate::from_ymd_opt(2026, 10, 3).unwrap();

    let mut page = journal_page(
        "page-1",
        "Gardening",
        "This week in the garden:\n- Water the tomatoes\n- Deadhead the roses",
    );
    page.dtstart = Some(DateType::AllDay(date));
    store.add_task(page);

    let mut sub = journal_page(
        "page-2",
        "Tomatoes",
        "Stake them before the heat builds up.",
    );
    sub.parent_uid = Some("page-1".to_string());
    store.add_task(sub);

    let cals = calendars();
    let aliases = HashMap::new();
    let opts = SyncTreeOptions {
        aliases: &aliases,
        default_reminder_time: None,
        trash_retention_days: 30,
        calendars: &cals,
    };

    // The journal editor serializes with the page's own flag: the description
    // is raw at depth 0 and sub-pages are flat bullets, never a nested
    // task-tree form.
    let md = cfait::model::extractor::serialize_task_tree(&store, "page-1", &cals, true);
    assert!(
        !md.starts_with("- "),
        "journal page must not serialize its own title as a bullet:\n{md}"
    );
    assert!(md.contains("Water the tomatoes"));
    assert!(md.contains("Tomatoes"));

    let (actions, warnings, _mints) = store
        .sync_tree_from_markdown("page-1", &md, &opts, true)
        .unwrap();
    assert!(warnings.is_empty());
    assert!(
        actions.is_empty(),
        "an unchanged journal page must produce no actions: {actions:?}"
    );

    let page_after = store.get_task_ref("page-1").unwrap();
    assert_eq!(
        page_after.description,
        "This week in the garden:\n- Water the tomatoes\n- Deadhead the roses"
    );
    assert_eq!(children_of(&store, "page-1").len(), 1);
}

#[test]
fn journal_page_description_lists_stay_in_description() {
    // Regression: when a journal page was serialized in task-tree form (the
    // root emitted as a bullet), the list items inside its description were
    // re-parsed as implicit subtasks on the next save, so the page's content
    // "repeated itself" as tasks. Serializing with the page's own flag keeps
    // them in the description.
    let mut store = make_store();
    let page = journal_page(
        "page-1",
        "Reading log",
        "- Finished the novel about the lighthouse keeper\n- Started the one about the beekeeper",
    );
    store.add_task(page);

    let cals = calendars();
    let aliases = HashMap::new();
    let opts = SyncTreeOptions {
        aliases: &aliases,
        default_reminder_time: None,
        trash_retention_days: 30,
        calendars: &cals,
    };

    let md = cfait::model::extractor::serialize_task_tree(&store, "page-1", &cals, true);
    let (actions, warnings, _mints) = store
        .sync_tree_from_markdown("page-1", &md, &opts, true)
        .unwrap();
    assert!(warnings.is_empty());
    assert!(actions.is_empty());
    assert!(
        children_of(&store, "page-1").is_empty(),
        "description list items must not become subtasks"
    );
    let page_after = store.get_task_ref("page-1").unwrap();
    assert!(page_after.description.contains("lighthouse keeper"));
    assert!(page_after.description.contains("beekeeper"));
}

#[test]
fn daily_note_and_date_anchored_page_do_not_collapse() {
    let mut store = make_store();
    let date = chrono::NaiveDate::from_ymd_opt(2026, 10, 3).unwrap();
    let date_str = date.format("%Y-%m-%d").to_string();

    // A date-anchored wiki page whose summary is NOT the date and whose uid
    // is smaller, so a naive "first match" would pick it over the daily note.
    let mut wiki = journal_page(
        "00000000-0000-0000-0000-00000000000a",
        "Gardening",
        "Seasonal notes for the beds.",
    );
    wiki.dtstart = Some(DateType::AllDay(date));
    store.add_task(wiki);

    // The daily note for the same date.
    let mut daily = journal_page(
        "11111111-1111-1111-1111-111111111111",
        &date_str,
        "Woke up early, the frost was gone by eight.",
    );
    daily.dtstart = Some(DateType::AllDay(date));
    store.add_task(daily);

    let entry = store
        .get_journal_entry("local://default", date)
        .expect("an entry exists for the date");
    let entry_uid = entry.uid.clone();
    assert_eq!(
        entry_uid, "11111111-1111-1111-1111-111111111111",
        "the daily note (summary == date) must win over date-anchored pages"
    );

    // Saving the daily note's tree must not touch the wiki page.
    let cals = calendars();
    let aliases = HashMap::new();
    let opts = SyncTreeOptions {
        aliases: &aliases,
        default_reminder_time: None,
        trash_retention_days: 30,
        calendars: &cals,
    };
    let md = cfait::model::extractor::serialize_task_tree(&store, &entry_uid, &cals, true);
    let (actions, warnings, _mints) = store
        .sync_tree_from_markdown(&entry_uid, &md, &opts, true)
        .unwrap();
    assert!(warnings.is_empty());
    assert!(actions.is_empty());
    let wiki_after = store
        .get_task_ref("00000000-0000-0000-0000-00000000000a")
        .expect("wiki page must survive the daily note save");
    assert_eq!(wiki_after.description, "Seasonal notes for the beds.");
}

#[test]
fn get_journal_entry_falls_back_to_smallest_uid() {
    let mut store = make_store();
    let date = chrono::NaiveDate::from_ymd_opt(2026, 10, 3).unwrap();

    let mut b = journal_page(
        "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb",
        "Hiking",
        "Trail notes for the ridge.",
    );
    b.dtstart = Some(DateType::AllDay(date));
    store.add_task(b);

    let mut a = journal_page(
        "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
        "Gardening",
        "Beds and beds.",
    );
    a.dtstart = Some(DateType::AllDay(date));
    store.add_task(a);

    let entry = store
        .get_journal_entry("local://default", date)
        .expect("an entry exists for the date");
    assert_eq!(
        entry.uid, "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
        "without a daily note, the smallest uid wins deterministically"
    );
}

#[test]
fn plain_is_page_line_parses_without_note_flag() {
    // A plain `is:page` line is a standard journal page, not a note. The note
    // state comes only from an explicit `is:note` token (SPECS: pages that
    // appear in the main list are the ones explicitly tagged with is:note).
    let aliases = HashMap::new();
    let task = Task::new(
        "Morning ritual: check the tomato stakes is:page",
        &aliases,
        None,
    );
    assert!(task.is_journal, "is:page must mark a journal page");
    assert!(
        !task.is_note,
        "a plain is:page line must not implicitly become a note"
    );

    // The explicit token sets the note state, in either token order.
    let note_a = Task::new("Morning ritual is:page is:note", &aliases, None);
    let note_b = Task::new("Morning ritual is:note is:page", &aliases, None);
    assert!(note_a.is_journal && note_a.is_note);
    assert!(note_b.is_journal && note_b.is_note);
}

#[test]
fn journal_note_token_round_trips_through_sync() {
    let mut store = make_store();
    let date = chrono::NaiveDate::from_ymd_opt(2026, 10, 3).unwrap();
    let mut root = journal_page(
        "page-1",
        &date.format("%Y-%m-%d").to_string(),
        "Woke up early, the frost was gone by eight.",
    );
    root.dtstart = Some(DateType::AllDay(date));
    store.add_task(root);

    let mut sub = journal_page(
        "page-2",
        "Tomatoes",
        "Stake them before the heat builds up.",
    );
    sub.parent_uid = Some("page-1".to_string());
    sub.is_note = true;
    store.add_task(sub);

    let cals = calendars();
    let aliases = HashMap::new();
    let opts = SyncTreeOptions {
        aliases: &aliases,
        default_reminder_time: None,
        trash_retention_days: 30,
        calendars: &cals,
    };

    // The note state must be visible in the serialized line, and the list
    // marker must not double up.
    let md = cfait::model::extractor::serialize_task_tree(&store, "page-1", &cals, true);
    let line = md
        .lines()
        .find(|l| l.contains("Tomatoes"))
        .expect("sub-page line in the serialized tree");
    assert!(
        line.contains("is:note"),
        "a journal note must serialize its note state as a token: {line}"
    );
    assert!(
        !line.contains("- -"),
        "the list marker must not double up: {line}"
    );

    // Re-saving the identical tree is a no-op.
    let (actions, warnings, _mints) = store
        .sync_tree_from_markdown("page-1", &md, &opts, true)
        .unwrap();
    assert!(warnings.is_empty());
    assert!(
        actions.is_empty(),
        "a no-op round-trip of a journal note must produce no actions: {actions:?}"
    );
    assert!(
        store.get_task_ref("page-2").unwrap().is_note,
        "the note state must survive the round-trip"
    );

    // Removing the token turns the page back into a standard page.
    let unnoted = md.replace(" is:note", "");
    let (actions, warnings, _mints) = store
        .sync_tree_from_markdown("page-1", &unnoted, &opts, true)
        .unwrap();
    assert!(warnings.is_empty());
    assert!(
        actions.iter().any(|a| matches!(
            a,
            cfait::journal::Action::Update(t) if t.uid == "page-2"
        )),
        "removing the token must normalize the page: {actions:?}"
    );
    assert!(
        !store.get_task_ref("page-2").unwrap().is_note,
        "removing the is:note token must clear the note state"
    );

    // Adding it back restores the note state.
    let (_actions, warnings, _mints) = store
        .sync_tree_from_markdown("page-1", &md, &opts, true)
        .unwrap();
    assert!(warnings.is_empty());
    assert!(
        store.get_task_ref("page-2").unwrap().is_note,
        "re-adding the is:note token must restore the note state"
    );
}

#[test]
fn wiki_page_created_in_journal_context_is_a_standard_page() {
    let mut store = make_store();
    let root = journal_page("page-1", "Gardening", "Seasonal notes for the beds.");
    store.add_task(root);

    let aliases = HashMap::new();
    let (uid, actions) = store.walk_or_create_wiki_path(
        "+Tomatoes",
        Some("page-1"),
        true,
        &aliases,
        None,
        Some("local://default".to_string()),
    );
    assert_eq!(actions.len(), 1, "one page should be created");
    let page = store.get_task_ref(&uid).expect("the created page");
    assert!(
        page.is_journal,
        "a wiki page created in journal context is a journal page"
    );
    assert!(
        !page.is_note,
        "a wiki page must be a standard page, not a note"
    );

    // A standard sub-page stays out of the main list and shows up in the
    // journal's Pages section instead.
    let empty: std::collections::HashSet<String> = std::collections::HashSet::new();
    let aliases: HashMap<String, Vec<String>> = HashMap::new();
    let expanded_tags: std::collections::HashSet<String> =
        std::iter::once("j:pages".to_string()).collect();
    let opts = cfait::store::FilterOptions {
        active_cal_href: None,
        hidden_calendars: &empty,
        selected_categories: &empty,
        selected_locations: &empty,
        match_all_categories: false,
        search_term: "",
        hide_completed_global: false,
        hide_fully_completed_tags: false,
        hide_aliases_in_sidebar: false,
        cutoff_date: None,
        min_duration: None,
        max_duration: None,
        include_unset_duration: true,
        urgent_days: 7,
        urgent_prio: 9,
        default_priority: 5,
        start_grace_period_days: 0,
        sort_standard_by_priority: false,
        sort_preset: cfait::config::SortPreset::UrgentStartedDue,
        expanded_done_groups: &empty,
        expanded_tags: &expanded_tags,
        expanded_locations: &empty,
        max_done_roots: 10,
        max_done_subtasks: 10,
        tag_aliases: &aliases,
        search_collapsed_tasks: &empty,
        focused_task_uid: None,
        paused_sort_behavior: cfait::config::PausedSortBehavior::default(),
        sort_tiebreak_recent: false,
        default_reminder_time: chrono::NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
    };
    let res = store.filter(opts);
    let main_uids: Vec<&str> = res
        .items
        .iter()
        .filter_map(|item| match item {
            cfait::store::TaskListItem::Task(t) => Some(t.uid.as_str()),
            _ => None,
        })
        .collect();
    assert!(
        !main_uids.contains(&uid.as_str()),
        "a standard sub-page must not appear in the main list"
    );
    assert!(
        res.journal_pages.iter().any(|p| p.key == uid),
        "the page must appear in the journal Pages section"
    );
}

#[test]
fn is_page_parses_the_same_in_task_and_journal_contexts() {
    let aliases = HashMap::new();
    let doc = "- Morning ritual: check the tomato stakes is:page\n  Water the seedlings before the heat builds up.\n- Plain note about the compost";

    // Task context (created from the task tab): the explicit is:page line
    // becomes a journal subtask, and plain list items become regular subtasks.
    let mut media = HashMap::new();
    let (_, task_ctx) = cfait::model::extract_markdown_tasks(doc, false, &mut media);
    assert_eq!(task_ctx.len(), 2, "task context extracts both lines");
    let page_task = Task::new(&task_ctx[0].raw_text, &aliases, None);
    assert!(
        page_task.is_journal,
        "is:page must mark the subtask as a journal page"
    );
    assert!(
        !page_task.summary.contains("is:page"),
        "the token must be stripped from the summary: {}",
        page_task.summary
    );
    let plain_task = Task::new(&task_ctx[1].raw_text, &aliases, None);
    assert!(!plain_task.is_journal, "a plain line is not a page");

    // Journal context (typed into the journal tab): only the explicit is:page
    // line becomes a sub-page; the plain line stays in the note's description.
    let mut media2 = HashMap::new();
    let (clean, journal_ctx) = cfait::model::extract_markdown_tasks(doc, true, &mut media2);
    assert_eq!(
        journal_ctx.len(),
        1,
        "journal context extracts only the page"
    );
    assert!(
        clean.contains("Plain note about the compost"),
        "the plain line must stay in the description: {clean}"
    );
}
