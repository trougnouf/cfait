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

    // The user typed the sub-page's reference line into the page: that is
    // what places the link in the document.
    let typed = "This week in the garden:\n- Water the tomatoes\n- Deadhead the roses\n- Tomatoes is:page <!-- uid:page-2 -->";
    let (place_actions, place_warnings, _place_mints) = store
        .sync_tree_from_markdown("page-1", typed, &opts, true, false)
        .unwrap();
    assert!(place_warnings.is_empty());
    assert!(
        place_actions
            .iter()
            .any(|a| matches!(a, cfait::journal::Action::Update(t) if t.uid == "page-1")),
        "placing the link must record its shadow line in the description: {place_actions:?}"
    );
    assert_eq!(
        store.get_task_ref("page-2").unwrap().description,
        "Stake them before the heat builds up.",
        "placing the link must preserve the sub-page's document"
    );

    // The journal editor serializes with the page's own flag: the description
    // is raw at depth 0 and the placed sub-page is a single reference line,
    // never a nested task-tree form and never its body.
    let md = cfait::model::extractor::serialize_journal_document(&store, "page-1", &cals);
    assert!(
        !md.starts_with("- "),
        "journal page must not serialize its own title as a bullet:\n{md}"
    );
    assert!(md.contains("Water the tomatoes"));
    assert!(md.contains("Tomatoes"));
    assert!(
        !md.contains("Stake them before the heat builds up."),
        "a sub-page serializes as its single reference line, never its body:\n{md}"
    );

    let page_after = store.get_task_ref("page-1").unwrap();
    assert_eq!(
        page_after.description,
        "This week in the garden:\n- Water the tomatoes\n- Deadhead the roses\n- Tomatoes is:page <!-- uid:page-2 -->"
    );
    assert_eq!(children_of(&store, "page-1").len(), 1);

    // Saving the collapsed view is a no-op: the sub-page's document is
    // preserved and its (here empty) subtree is not read as deleted.
    let (actions, warnings, _mints) = store
        .sync_tree_from_markdown("page-1", &md, &opts, true, false)
        .unwrap();
    assert!(warnings.is_empty());
    assert!(
        actions.is_empty(),
        "an unchanged journal page must produce no actions: {actions:?}"
    );

    let md2 = cfait::model::extractor::serialize_journal_document(&store, "page-1", &cals);
    assert_eq!(
        md2, md,
        "the serialized tree must be stable once the shadow is recorded:\n{md2}"
    );
    let (actions2, warnings2, _mints2) = store
        .sync_tree_from_markdown("page-1", &md2, &opts, true, false)
        .unwrap();
    assert!(warnings2.is_empty());
    assert!(
        actions2.is_empty(),
        "an unchanged journal page must produce no actions: {actions2:?}"
    );
    assert_eq!(
        store.get_task_ref("page-2").unwrap().description,
        "Stake them before the heat builds up.",
        "a body-less sub-page line must preserve the page's document"
    );
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

    let md = cfait::model::extractor::serialize_journal_document(&store, "page-1", &cals);
    let (actions, warnings, _mints) = store
        .sync_tree_from_markdown("page-1", &md, &opts, true, false)
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
    let md = cfait::model::extractor::serialize_journal_document(&store, &entry_uid, &cals);
    let (actions, warnings, _mints) = store
        .sync_tree_from_markdown(&entry_uid, &md, &opts, true, false)
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

    // The user typed the sub-page's reference line (with the note token) into
    // the daily note: that is what places the link in the document.
    let typed = "Woke up early, the frost was gone by eight.\n- Tomatoes is:note is:page <!-- uid:page-2 -->";
    let (place_actions, place_warnings, _place_mints) = store
        .sync_tree_from_markdown("page-1", typed, &opts, true, false)
        .unwrap();
    assert!(place_warnings.is_empty());
    assert!(
        place_actions
            .iter()
            .any(|a| matches!(a, cfait::journal::Action::Update(t) if t.uid == "page-1")),
        "placing the link must record its shadow line: {place_actions:?}"
    );
    assert!(
        store.get_task_ref("page-2").unwrap().is_note,
        "placing the link must keep the note state"
    );

    // The note state must be visible in the serialized line, and the list
    // marker must not double up.
    let md = cfait::model::extractor::serialize_journal_document(&store, "page-1", &cals);
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

    // Re-saving the identical tree is a no-op: the shadow line is recorded
    // and the sub-page's document is preserved.
    let (actions, warnings, _mints) = store
        .sync_tree_from_markdown("page-1", &md, &opts, true, false)
        .unwrap();
    assert!(warnings.is_empty());
    assert!(
        actions.is_empty(),
        "a settled round-trip of a journal note must produce no actions: {actions:?}"
    );
    let md2 = cfait::model::extractor::serialize_journal_document(&store, "page-1", &cals);
    assert_eq!(md2, md, "the serialized tree must be stable:\n{md2}");
    let (actions2, warnings2, _mints2) = store
        .sync_tree_from_markdown("page-1", &md2, &opts, true, false)
        .unwrap();
    assert!(warnings2.is_empty());
    assert!(
        actions2.is_empty(),
        "a no-op round-trip of a journal note must produce no actions: {actions2:?}"
    );
    assert!(
        store.get_task_ref("page-2").unwrap().is_note,
        "the note state must survive the round-trip"
    );

    // Removing the token turns the page back into a standard page.
    let unnoted = md.replace(" is:note", "");
    let (actions, warnings, _mints) = store
        .sync_tree_from_markdown("page-1", &unnoted, &opts, true, false)
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
        .sync_tree_from_markdown("page-1", &md, &opts, true, false)
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

#[test]
fn sub_page_serializes_as_a_single_reference_line() {
    // A sub-page is a separate page: the parent page's editor shows its
    // reference line (a followable link), never its body. VTODO children
    // (tasks, notes) still expand inline.
    let mut store = make_store();
    let page = journal_page(
        "page-1",
        "PowerShed journal",
        "See also: [[PowerShed construction]]\nPlan the wiring before the walls go up.",
    );
    store.add_task(page);

    let mut sub = journal_page(
        "page-2",
        "PowerShed floor plan and tools",
        "# Master Workshop Strategy\nMobility and dust segregation.",
    );
    sub.parent_uid = Some("page-1".to_string());
    store.add_task(sub);

    let mut task = Task::new("buy 2x4 studs", &HashMap::new(), None);
    task.uid = "task-1".to_string();
    task.calendar_href = "local://default".to_string();
    task.parent_uid = Some("page-1".to_string());
    task.description = "8ft lengths".to_string();
    store.add_task(task);

    let cals = calendars();
    let aliases = HashMap::new();
    let opts = SyncTreeOptions {
        aliases: &aliases,
        default_reminder_time: None,
        trash_retention_days: 30,
        calendars: &cals,
    };

    // The user referenced the sub-page from the page body: that is what
    // places the link in the document.
    let typed = "See also: [[PowerShed construction]]\nPlan the wiring before the walls go up.\n- PowerShed floor plan and tools is:page <!-- uid:page-2 -->\n- [ ] buy 2x4 studs <!-- uid:task-1 -->\n  8ft lengths";
    let (place_actions, place_warnings, _place_mints) = store
        .sync_tree_from_markdown("page-1", typed, &opts, true, false)
        .unwrap();
    assert!(place_warnings.is_empty());
    assert!(
        place_actions
            .iter()
            .any(|a| matches!(a, cfait::journal::Action::Update(t) if t.uid == "page-1")),
        "placing the link must record its shadow line: {place_actions:?}"
    );
    assert_eq!(
        store.get_task_ref("page-2").unwrap().description,
        "# Master Workshop Strategy\nMobility and dust segregation.",
        "placing the link must preserve the sub-page's document"
    );

    let md = cfait::model::extractor::serialize_journal_document(&store, "page-1", &cals);

    // The sub-page is a single uid-tagged reference line; its body is absent.
    let line = md
        .lines()
        .find(|l| l.contains("PowerShed floor plan and tools"))
        .expect("sub-page reference line in the serialized tree");
    assert!(line.contains("is:page"), "the line keeps its token: {line}");
    assert!(
        line.contains("<!-- uid:page-2 -->"),
        "the line keeps its uid tag: {line}"
    );
    assert!(
        !md.contains("Master Workshop Strategy"),
        "the sub-page body must not be expanded into the parent page:\n{md}"
    );

    // A VTODO child still expands inline, body included.
    assert!(
        md.contains("- [ ] buy 2x4 studs"),
        "VTODO children still show inline: {md}"
    );
    assert!(md.contains("8ft lengths"), "VTODO child body inline: {md}");

    // Saving the collapsed view is a no-op and leaves the sub-page's
    // document untouched.
    let (actions, warnings, _mints) = store
        .sync_tree_from_markdown("page-1", &md, &opts, true, false)
        .unwrap();
    assert!(warnings.is_empty());
    assert!(
        actions.is_empty(),
        "saving the collapsed view must be a no-op: {actions:?}"
    );
    assert_eq!(
        store.get_task_ref("page-2").unwrap().description,
        "# Master Workshop Strategy\nMobility and dust segregation.",
        "a body-less sub-page line must preserve the page's document"
    );

    // The collapsed form round-trips: stable serialization, no-op re-save.
    let md2 = cfait::model::extractor::serialize_journal_document(&store, "page-1", &cals);
    assert_eq!(md2, md, "the collapsed tree must be stable:\n{md2}");
    let (actions2, warnings2, _mints2) = store
        .sync_tree_from_markdown("page-1", &md2, &opts, true, false)
        .unwrap();
    assert!(warnings2.is_empty());
    assert!(
        actions2.is_empty(),
        "re-saving the collapsed view must be a no-op: {actions2:?}"
    );
    assert_eq!(
        store.get_task_ref("page-2").unwrap().description,
        "# Master Workshop Strategy\nMobility and dust segregation.",
        "the sub-page body must survive repeated saves"
    );

    // Deleting the reference line still deletes the sub-page.
    let without_sub: String = md2
        .lines()
        .filter(|l| !l.contains("PowerShed floor plan and tools"))
        .collect::<Vec<_>>()
        .join("\n");
    let (actions3, warnings3, _mints3) = store
        .sync_tree_from_markdown("page-1", &without_sub, &opts, true, false)
        .unwrap();
    assert!(warnings3.is_empty());
    assert!(
        actions3
            .iter()
            .any(|a| matches!(a, cfait::journal::Action::Delete(t) if t.uid == "page-2")),
        "deleting the reference line must soft-delete the sub-page: {actions3:?}"
    );
    assert!(
        store
            .get_task_ref("page-2")
            .is_some_and(|t| t.calendar_href == "local://trash"),
        "the sub-page must land in the trash"
    );

    // An indented block typed under the reference line still rewrites the
    // sub-page's body (the explicit escape hatch).
    let with_body = md2.replace(
        line,
        &format!("{line}\n  Rewritten body typed in the parent editor."),
    );
    let (_actions4, warnings4, _mints4) = store
        .sync_tree_from_markdown("page-1", &with_body, &opts, true, false)
        .unwrap();
    assert!(warnings4.is_empty());
    assert_eq!(
        store.get_task_ref("page-2").unwrap().description,
        "Rewritten body typed in the parent editor.",
        "an indented block under the sub-page line rewrites its body"
    );
}

#[test]
fn sub_page_subtree_survives_parent_page_save() {
    // Regression: a placed sub-page serializes as a single reference line, so
    // its body — and the lines of its own sub-tasks — are absent from the
    // parent page's buffer. The sync must not read that absence as a
    // deletion: the sub-page's subtree is edited in the sub-page's own editor.
    let mut store = make_store();
    let page = journal_page(
        "page-1",
        "PowerShed journal",
        "Plan the wiring before the walls go up.\n- PowerShed floor plan and tools is:page <!-- uid:page-2 -->",
    );
    store.add_task(page);

    let mut sub = journal_page(
        "page-2",
        "PowerShed floor plan and tools",
        "# Master Workshop Strategy\n- [ ] buy 2x4 studs <!-- uid:task-1 -->",
    );
    sub.parent_uid = Some("page-1".to_string());
    store.add_task(sub);

    let mut task = Task::new("buy 2x4 studs", &HashMap::new(), None);
    task.uid = "task-1".to_string();
    task.calendar_href = "local://default".to_string();
    task.parent_uid = Some("page-2".to_string());
    store.add_task(task);

    let cals = calendars();
    let md = cfait::model::extractor::serialize_journal_document(&store, "page-1", &cals);
    assert!(
        md.contains("- PowerShed floor plan and tools is:page <!-- uid:page-2 -->"),
        "the placed sub-page keeps its reference line: {md}"
    );
    assert!(
        !md.contains("Master Workshop Strategy") && !md.contains("buy 2x4 studs"),
        "the sub-page body and its sub-tasks stay out of the parent page: {md}"
    );

    let aliases = HashMap::new();
    let opts = SyncTreeOptions {
        aliases: &aliases,
        default_reminder_time: None,
        trash_retention_days: 30,
        calendars: &cals,
    };
    let (actions, warnings, _mints) = store
        .sync_tree_from_markdown("page-1", &md, &opts, true, false)
        .unwrap();
    assert!(warnings.is_empty());
    assert!(
        actions.is_empty(),
        "an unchanged collapsed view must be a no-op: {actions:?}"
    );

    let task_after = store
        .get_task_ref("task-1")
        .expect("the sub-task must survive");
    assert_eq!(
        task_after.calendar_href, "local://default",
        "the sub-page's sub-task must not be trashed by a parent page save"
    );
    assert_eq!(task_after.parent_uid.as_deref(), Some("page-2"));
    assert_eq!(
        store.get_task_ref("page-2").unwrap().description,
        "# Master Workshop Strategy\n- [ ] buy 2x4 studs <!-- uid:task-1 -->",
        "the sub-page's document (with its shadow line) must be preserved"
    );

    // Deleting the reference line is an explicit deletion: the sub-page and
    // its whole subtree go to the trash.
    let without_sub = md.replace(
        "- PowerShed floor plan and tools is:page <!-- uid:page-2 -->",
        "",
    );
    let without_sub = without_sub.trim().to_string();
    let (actions2, warnings2, _mints2) = store
        .sync_tree_from_markdown("page-1", &without_sub, &opts, true, false)
        .unwrap();
    assert!(warnings2.is_empty());
    assert!(
        actions2
            .iter()
            .any(|a| matches!(a, cfait::journal::Action::Delete(t) if t.uid == "page-2")),
        "deleting the reference line must soft-delete the sub-page: {actions2:?}"
    );
    assert!(
        store
            .get_task_ref("page-2")
            .is_some_and(|t| t.calendar_href == "local://trash"),
        "the sub-page must land in the trash"
    );
    assert!(
        store
            .get_task_ref("task-1")
            .is_some_and(|t| t.calendar_href == "local://trash"),
        "the sub-page's sub-task must be trashed with it"
    );
}

#[test]
fn unplaced_sub_page_is_omitted_and_survives_parent_save() {
    // A sub-page the user never referenced in the parent page (created from
    // the Pages section, yank, or `parent:`) is not part of the parent's
    // document: it is not serialized into the parent's editor, and a parent
    // save must not delete it or its subtree.
    let mut store = make_store();
    let page = journal_page(
        "page-1",
        "PowerShed journal",
        "Plan the wiring before the walls go up.",
    );
    store.add_task(page);

    let mut sub = journal_page(
        "page-2",
        "PowerShed floor plan and tools",
        "# Master Workshop Strategy\n- [ ] buy 2x4 studs <!-- uid:task-1 -->",
    );
    sub.parent_uid = Some("page-1".to_string());
    store.add_task(sub);

    let mut task = Task::new("buy 2x4 studs", &HashMap::new(), None);
    task.uid = "task-1".to_string();
    task.calendar_href = "local://default".to_string();
    task.parent_uid = Some("page-2".to_string());
    store.add_task(task);

    let cals = calendars();
    let md = cfait::model::extractor::serialize_journal_document(&store, "page-1", &cals);
    assert_eq!(
        md, "Plan the wiring before the walls go up.",
        "an unplaced sub-page is not auto-appended to the parent page: {md}"
    );

    let aliases = HashMap::new();
    let opts = SyncTreeOptions {
        aliases: &aliases,
        default_reminder_time: None,
        trash_retention_days: 30,
        calendars: &cals,
    };
    let (actions, warnings, _mints) = store
        .sync_tree_from_markdown("page-1", &md, &opts, true, false)
        .unwrap();
    assert!(warnings.is_empty());
    assert!(
        actions.is_empty(),
        "saving the parent page must not touch the unplaced sub-page: {actions:?}"
    );
    assert!(
        store
            .get_task_ref("page-2")
            .is_some_and(|t| t.calendar_href == "local://default"),
        "the unplaced sub-page must survive the parent save"
    );
    assert!(
        store
            .get_task_ref("task-1")
            .is_some_and(|t| t.calendar_href == "local://default"),
        "the unplaced sub-page's sub-task must survive the parent save"
    );
    assert_eq!(
        store.get_task_ref("page-2").unwrap().description,
        "# Master Workshop Strategy\n- [ ] buy 2x4 studs <!-- uid:task-1 -->",
        "the unplaced sub-page's document must be preserved"
    );
}

#[test]
fn nested_sub_pages_collapse_at_every_level() {
    // page-1 -> page-2 -> page-3, each placed by a reference line in its
    // parent's document. Serializing page-1 shows only page-2's reference
    // line; serializing page-2 shows its own body (it is the root there)
    // plus only page-3's reference line.
    let mut store = make_store();
    let page1 = journal_page(
        "page-1",
        "Level one",
        "Level one body.\n- Level two is:page <!-- uid:page-2 -->",
    );
    store.add_task(page1);
    let mut page2 = journal_page(
        "page-2",
        "Level two",
        "Level two body.\n- Level three is:page <!-- uid:page-3 -->",
    );
    page2.parent_uid = Some("page-1".to_string());
    store.add_task(page2);
    let mut page3 = journal_page("page-3", "Level three", "Level three body.");
    page3.parent_uid = Some("page-2".to_string());
    store.add_task(page3);

    let cals = calendars();
    let md1 = cfait::model::extractor::serialize_journal_document(&store, "page-1", &cals);
    assert!(
        md1.contains("Level two is:page"),
        "the direct sub-page line is present: {md1}"
    );
    assert!(
        !md1.contains("Level two body.") && !md1.contains("Level three"),
        "neither the sub-page's body nor the deeper page leak into the parent: {md1}"
    );

    let md2 = cfait::model::extractor::serialize_journal_document(&store, "page-2", &cals);
    assert!(
        md2.contains("Level two body."),
        "the root page's own body is always shown: {md2}"
    );
    assert!(
        md2.contains("Level three is:page"),
        "the nested sub-page line is present: {md2}"
    );
    assert!(
        !md2.contains("Level three body."),
        "the nested sub-page body stays in its own editor: {md2}"
    );

    // Saving page-1's collapsed view must not delete the deeper page: the
    // whole collapsed subtree is protected.
    let aliases = HashMap::new();
    let opts = SyncTreeOptions {
        aliases: &aliases,
        default_reminder_time: None,
        trash_retention_days: 30,
        calendars: &cals,
    };
    let (actions, warnings, _mints) = store
        .sync_tree_from_markdown("page-1", &md1, &opts, true, false)
        .unwrap();
    assert!(warnings.is_empty());
    assert!(
        actions.is_empty(),
        "saving the collapsed view must be a no-op: {actions:?}"
    );
    assert!(
        store
            .get_task_ref("page-3")
            .is_some_and(|t| t.calendar_href == "local://default"),
        "the nested sub-page must survive the parent save"
    );
}

#[test]
fn tree_editor_expands_sub_page_bodies_and_appends_unplaced_pages() {
    // The tree view (the Edit Task Tree editor, `cfait edit --tree`,
    // `cfait print`, the Android tree markdown) serializes the whole tree:
    // a placed sub-page's body expands at its shadow position, and an
    // unplaced sub-page appends below with its body. Copying the buffer
    // yields the entire tree, journals included.
    let mut store = make_store();
    let page = journal_page(
        "page-1",
        "PowerShed journal",
        "Plan the wiring before the walls go up.\n- PowerShed floor plan and tools is:page <!-- uid:page-2 -->",
    );
    store.add_task(page);

    let mut sub = journal_page(
        "page-2",
        "PowerShed floor plan and tools",
        "# Master Workshop Strategy\n- [ ] buy 2x4 studs <!-- uid:task-1 -->",
    );
    sub.parent_uid = Some("page-1".to_string());
    store.add_task(sub);

    let mut task = Task::new("buy 2x4 studs", &HashMap::new(), None);
    task.uid = "task-1".to_string();
    task.calendar_href = "local://default".to_string();
    task.parent_uid = Some("page-2".to_string());
    store.add_task(task);

    // An unplaced sub-page (parented through the Pages section).
    let mut unplaced = journal_page(
        "page-3",
        "PowerShed shopping list",
        "Copper wire and conduit.",
    );
    unplaced.parent_uid = Some("page-1".to_string());
    store.add_task(unplaced);

    let cals = calendars();
    let md = cfait::model::extractor::serialize_task_tree(&store, "page-1", &cals, true);
    assert!(
        md.contains("# Master Workshop Strategy"),
        "the placed sub-page's body expands in the tree view:\n{md}"
    );
    assert!(
        md.contains("buy 2x4 studs"),
        "the sub-page's sub-task expands too:\n{md}"
    );
    assert!(
        md.contains("PowerShed shopping list") && md.contains("Copper wire and conduit."),
        "the unplaced sub-page appends with its body:\n{md}"
    );

    // The tree buffer is the whole tree: saving it materializes the
    // hierarchy it displayed — the appended sub-page's line is recorded as
    // a reference line (shadow) in the parent's document, exactly as the
    // tree editor behaved before sub-pages collapsed in document view.
    let aliases = HashMap::new();
    let opts = SyncTreeOptions {
        aliases: &aliases,
        default_reminder_time: None,
        trash_retention_days: 30,
        calendars: &cals,
    };
    let (actions, warnings, _mints) = store
        .sync_tree_from_markdown("page-1", &md, &opts, true, true)
        .unwrap();
    assert!(warnings.is_empty());
    assert_eq!(
        actions.len(),
        1,
        "saving the tree view must only record the appended page's line: {actions:?}"
    );
    assert!(
        actions.iter().any(|a| matches!(
            a,
            cfait::journal::Action::Update(t) if t.uid == "page-1"
        )),
        "the parent page records the appended page's reference line: {actions:?}"
    );
    assert!(
        store
            .get_task_ref("page-1")
            .unwrap()
            .description
            .contains("- PowerShed shopping list is:page <!-- uid:page-3 -->"),
        "the unplaced sub-page's line is recorded in the parent's document"
    );

    // The tree view is stable, and every document survives the round-trip.
    let md2 = cfait::model::extractor::serialize_task_tree(&store, "page-1", &cals, true);
    assert_eq!(md2, md, "the tree view must be stable:\n{md2}");
    assert_eq!(
        store.get_task_ref("page-2").unwrap().description,
        "# Master Workshop Strategy\n- [ ] buy 2x4 studs <!-- uid:task-1 -->",
        "the placed sub-page's document must be preserved"
    );
    assert_eq!(
        store.get_task_ref("page-3").unwrap().description,
        "Copper wire and conduit.",
        "the unplaced sub-page's document must be preserved"
    );
    assert_eq!(
        store.get_task_ref("task-1").unwrap().parent_uid.as_deref(),
        Some("page-2"),
        "the sub-task must stay under the sub-page"
    );

    // Once the hierarchy is recorded, re-saving the tree view is a no-op.
    let (actions2, warnings2, _mints2) = store
        .sync_tree_from_markdown("page-1", &md2, &opts, true, true)
        .unwrap();
    assert!(warnings2.is_empty());
    assert!(
        actions2.is_empty(),
        "a settled tree view must be a no-op: {actions2:?}"
    );
}

#[test]
fn tree_editor_deletion_under_a_sub_page_soft_deletes() {
    // In the tree view the buffer is the whole tree, so deleting a line is
    // an explicit deletion — the document-view protections (a body-less
    // sub-page line preserves its subtree) must not apply here.
    let mut store = make_store();
    let page = journal_page(
        "page-1",
        "PowerShed journal",
        "Plan the wiring before the walls go up.\n- PowerShed floor plan and tools is:page <!-- uid:page-2 -->",
    );
    store.add_task(page);

    let mut sub = journal_page(
        "page-2",
        "PowerShed floor plan and tools",
        "# Master Workshop Strategy\n- [ ] buy 2x4 studs <!-- uid:task-1 -->",
    );
    sub.parent_uid = Some("page-1".to_string());
    store.add_task(sub);

    let mut task = Task::new("buy 2x4 studs", &HashMap::new(), None);
    task.uid = "task-1".to_string();
    task.calendar_href = "local://default".to_string();
    task.parent_uid = Some("page-2".to_string());
    store.add_task(task);

    let cals = calendars();
    let md = cfait::model::extractor::serialize_task_tree(&store, "page-1", &cals, true);
    assert!(
        md.contains("buy 2x4 studs"),
        "the tree view expands the sub-task:\n{md}"
    );

    // The user deletes the sub-task's line in the tree editor and saves.
    let edited: String = md
        .lines()
        .filter(|l| !l.contains("buy 2x4 studs"))
        .collect::<Vec<_>>()
        .join("\n");
    let aliases = HashMap::new();
    let opts = SyncTreeOptions {
        aliases: &aliases,
        default_reminder_time: None,
        trash_retention_days: 30,
        calendars: &cals,
    };
    let (actions, warnings, _mints) = store
        .sync_tree_from_markdown("page-1", &edited, &opts, true, true)
        .unwrap();
    assert!(warnings.is_empty());
    assert!(
        actions
            .iter()
            .any(|a| matches!(a, cfait::journal::Action::Delete(t) if t.uid == "task-1")),
        "deleting the line in the tree editor must soft-delete the sub-task: {actions:?}"
    );
    assert!(
        store
            .get_task_ref("task-1")
            .is_some_and(|t| t.calendar_href == "local://trash"),
        "the sub-task must land in the trash"
    );
    assert_eq!(
        store.get_task_ref("page-2").unwrap().description,
        "# Master Workshop Strategy",
        "the sub-page's document loses the deleted line but keeps its body"
    );
    assert!(
        store
            .get_task_ref("page-2")
            .is_some_and(|t| t.calendar_href == "local://default"),
        "the sub-page itself must survive"
    );
}

#[test]
fn tree_editor_on_a_task_includes_an_unplaced_page_with_its_body() {
    // A page parented to a plain task appears in the task's tree editor —
    // line and body — even though the description editor's document view
    // omits it (a plain task stores a prose-only description, so the page
    // cannot be pinned there). Saving the tree view is a no-op.
    let mut store = make_store();
    let mut root = Task::new("Garden plan", &HashMap::new(), None);
    root.uid = "root-1".to_string();
    root.calendar_href = "local://default".to_string();
    root.description = "Beds to rotate this year.".to_string();
    store.add_task(root);

    let mut page = journal_page(
        "page-1",
        "Tomatoes",
        "Stake them before the heat builds up.",
    );
    page.parent_uid = Some("root-1".to_string());
    store.add_task(page);

    let cals = calendars();
    let md = cfait::model::extractor::serialize_task_tree(&store, "root-1", &cals, false);
    assert!(
        md.contains("Tomatoes is:page"),
        "the tree editor shows the unplaced page's line:\n{md}"
    );
    assert!(
        md.contains("Stake them before the heat builds up."),
        "the tree editor expands the page's body:\n{md}"
    );

    // The description editor's merged view omits the page; the tree editor
    // is where the whole tree, journals included, can be copied from.
    let view = cfait::model::extractor::serialize_merged_description(&store, "root-1", &cals);
    assert!(
        !view.contains("Tomatoes"),
        "the description editor's document view omits the unplaced page: {view}"
    );

    let aliases = HashMap::new();
    let opts = SyncTreeOptions {
        aliases: &aliases,
        default_reminder_time: None,
        trash_retention_days: 30,
        calendars: &cals,
    };
    let (actions, warnings, _mints) = store
        .sync_tree_from_markdown("root-1", &md, &opts, false, true)
        .unwrap();
    assert!(warnings.is_empty());
    assert!(
        actions.is_empty(),
        "an unchanged tree view must be a no-op: {actions:?}"
    );
    assert_eq!(
        store.get_task_ref("page-1").unwrap().description,
        "Stake them before the heat builds up.",
        "the page's document must be preserved"
    );
    assert_eq!(
        store.get_task_ref("root-1").unwrap().description,
        "Beds to rotate this year.",
        "the task keeps its prose-only description"
    );
}
