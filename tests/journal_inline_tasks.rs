// SPDX-License-Identifier: GPL-3.0-or-later
//! Shadow lines: on journal pages, extracted task lines stay in the page's
//! description as uid-tagged placeholders, so children render exactly where
//! the user placed them instead of collapsing to the bottom of the page.
//! Plain tasks store prose-only descriptions instead — the tree, not the
//! text, remembers their children — while the merged description editor
//! (which re-appends the children, uid-tagged) keeps deletions and round-trips
//! working exactly the same.
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

fn journal_page(uid: &str, summary: &str) -> Task {
    let mut t = Task::new(summary, &HashMap::new(), None);
    t.uid = uid.to_string();
    t.calendar_href = "local://default".to_string();
    t.is_journal = true;
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

const PAGE_UID: &str = "11111111-1111-1111-1111-111111111111";

#[test]
fn interleaved_tasks_keep_their_position_across_reopen() {
    let mut store = make_store();
    store.add_task(journal_page(PAGE_UID, "Trip planning"));

    // What the user types into the journal tab editor: tasks interleaved
    // with prose.
    let typed = "We leave on Monday morning, pack the night before.\n\
                 - [ ] book the train tickets\n\
                 Still need to decide on the bakery stop.\n\
                 - [ ] buy a new water bottle\n\
                 That's it for now.";

    let cals = calendars();
    let aliases: HashMap<String, Vec<String>> = HashMap::new();
    let (actions, warnings, _mints) = store
        .sync_tree_from_markdown(PAGE_UID, typed, &opts(&aliases, &cals), true)
        .unwrap();
    assert!(warnings.is_empty());
    assert_eq!(children_of(&store, PAGE_UID).len(), 2);
    assert_eq!(actions.len(), 3, "root + two children: {actions:?}");

    // Reopening the page shows the tasks exactly where they were typed.
    let reopened = cfait::model::extractor::serialize_task_tree(&store, PAGE_UID, &cals, true);
    let lines: Vec<&str> = reopened.lines().collect();
    assert_eq!(
        lines[0],
        "We leave on Monday morning, pack the night before."
    );
    assert!(lines[1].starts_with("- [ ] book the train tickets <!-- uid:"));
    assert_eq!(lines[2], "Still need to decide on the bakery stop.");
    assert!(lines[3].starts_with("- [ ] buy a new water bottle <!-- uid:"));
    assert_eq!(lines[4], "That's it for now.");
}

#[test]
fn shadow_renders_fresh_child_state() {
    let mut store = make_store();
    store.add_task(journal_page(PAGE_UID, "Trip planning"));

    let cals = calendars();
    let aliases: HashMap<String, Vec<String>> = HashMap::new();
    store
        .sync_tree_from_markdown(
            PAGE_UID,
            "- [ ] book the train tickets",
            &opts(&aliases, &cals),
            true,
        )
        .unwrap();
    let child = children_of(&store, PAGE_UID).pop().unwrap();

    // The task is completed through the main list while the page is closed.
    let mut completed = child.clone();
    completed.status = cfait::model::TaskStatus::Completed;
    completed.percent_complete = Some(100);
    store.update_or_add_task(completed);

    // Reopening the page shows the current state at the shadow's position.
    let reopened = cfait::model::extractor::serialize_task_tree(&store, PAGE_UID, &cals, true);
    assert!(
        reopened.starts_with("- [x] book the train tickets"),
        "the shadow must render the child's fresh state: {reopened}"
    );
}

#[test]
fn stale_shadow_of_moved_child_is_dropped() {
    let mut store = make_store();
    store.add_task(journal_page(PAGE_UID, "Trip planning"));

    let cals = calendars();
    let aliases: HashMap<String, Vec<String>> = HashMap::new();
    store
        .sync_tree_from_markdown(
            PAGE_UID,
            "Intro prose.\n- [ ] book the train tickets",
            &opts(&aliases, &cals),
            true,
        )
        .unwrap();
    let child = children_of(&store, PAGE_UID).pop().unwrap();

    // The child is re-parented elsewhere (moved out of the page).
    let mut moved = child.clone();
    moved.parent_uid = None;
    store.update_or_add_task(moved);

    // The stale shadow must not resurrect the task as a duplicate.
    let reopened = cfait::model::extractor::serialize_task_tree(&store, PAGE_UID, &cals, true);
    assert_eq!(
        reopened, "Intro prose.",
        "the stale shadow must be dropped: {reopened}"
    );
    // And a save of that view must not mint a copy either.
    let (actions, _warnings, _mints) = store
        .sync_tree_from_markdown(PAGE_UID, &reopened, &opts(&aliases, &cals), true)
        .unwrap();
    assert!(
        children_of(&store, PAGE_UID).is_empty(),
        "no child may be resurrected"
    );
    assert!(
        actions.iter().all(|a| matches!(
            a,
            cfait::journal::Action::Update(t) if t.uid == PAGE_UID
        )),
        "only the root description may change: {actions:?}"
    );
    assert!(store.get_task_ref(&child.uid).is_some());
}

#[test]
fn unreferenced_child_appends_at_the_bottom() {
    let mut store = make_store();
    store.add_task(journal_page(PAGE_UID, "Trip planning"));

    let cals = calendars();
    let aliases: HashMap<String, Vec<String>> = HashMap::new();
    store
        .sync_tree_from_markdown(
            PAGE_UID,
            "Intro prose.\n- [ ] book the train tickets",
            &opts(&aliases, &cals),
            true,
        )
        .unwrap();

    // A task added through the main UI has no shadow: it appends below the
    // description, as before.
    let mut added = Task::new("water the herbs", &HashMap::new(), None);
    added.calendar_href = "local://default".to_string();
    added.parent_uid = Some(PAGE_UID.to_string());
    store.add_task(added);

    let reopened = cfait::model::extractor::serialize_task_tree(&store, PAGE_UID, &cals, true);
    let lines: Vec<&str> = reopened.lines().collect();
    assert_eq!(lines[0], "Intro prose.");
    assert!(lines[1].starts_with("- [ ] book the train tickets <!-- uid:"));
    assert!(lines[2].starts_with("- [ ] water the herbs <!-- uid:"));
}

#[test]
fn deleting_a_line_deletes_the_task() {
    let mut store = make_store();
    store.add_task(journal_page(PAGE_UID, "Trip planning"));

    let cals = calendars();
    let aliases: HashMap<String, Vec<String>> = HashMap::new();
    store
        .sync_tree_from_markdown(
            PAGE_UID,
            "Intro prose.\n- [ ] book the train tickets",
            &opts(&aliases, &cals),
            true,
        )
        .unwrap();
    let child = children_of(&store, PAGE_UID).pop().unwrap();

    // The user deletes the line and saves.
    store
        .sync_tree_from_markdown(PAGE_UID, "Intro prose.", &opts(&aliases, &cals), true)
        .unwrap();
    assert!(
        store
            .get_task_ref(&child.uid)
            .is_some_and(|t| t.calendar_href == "local://trash"),
        "deleting the line must soft-delete the task to the trash"
    );
    let reopened = cfait::model::extractor::serialize_task_tree(&store, PAGE_UID, &cals, true);
    assert_eq!(reopened, "Intro prose.");
}

#[test]
fn desc_editor_merged_view_round_trips_through_tree_sync() {
    let mut store = make_store();
    let aliases: HashMap<String, Vec<String>> = HashMap::new();
    let cals = calendars();

    let no_aliases: HashMap<String, Vec<String>> = HashMap::new();
    let mut root = Task::new("Garden plan", &no_aliases, None);
    root.uid = "root-1".to_string();
    root.calendar_href = "local://default".to_string();
    root.description = "Beds to rotate this year.\n- [ ] divide the rhubarb".to_string();
    store.add_task(root.clone());

    // Typing in the description editor extracts the subtask and records its
    // shadow (the description-editor flows route through the tree sync).
    let (actions, warnings, _mints) = store
        .sync_tree_from_markdown("root-1", &root.description, &opts(&aliases, &cals), false)
        .unwrap();
    assert!(warnings.is_empty());
    assert!(!actions.is_empty());
    let child = children_of(&store, "root-1").pop().unwrap();
    assert_eq!(child.summary, "divide the rhubarb");

    // The merged description view (what the editors load) keeps the
    // subtask inline; re-saving it is a no-op.
    let view = cfait::model::extractor::serialize_merged_description(&store, "root-1", &cals);
    let lines: Vec<&str> = view.lines().collect();
    assert_eq!(lines[0], "Beds to rotate this year.");
    assert!(lines[1].starts_with("- [ ] divide the rhubarb <!-- uid:"));

    let (actions2, warnings2, _mints2) = store
        .sync_tree_from_markdown("root-1", &view, &opts(&aliases, &cals), false)
        .unwrap();
    assert!(warnings2.is_empty());
    assert!(
        actions2.is_empty(),
        "an unchanged merged view must be a no-op: {actions2:?}"
    );

    // Completing the child elsewhere shows fresh state in the view.
    let mut done = child.clone();
    done.status = cfait::model::TaskStatus::Completed;
    done.percent_complete = Some(100);
    store.update_or_add_task(done);
    let view2 = cfait::model::extractor::serialize_merged_description(&store, "root-1", &cals);
    assert!(
        view2.starts_with("Beds to rotate this year.\n- [x] divide the rhubarb"),
        "merged view: {view2}"
    );
}

#[test]
fn plain_tasks_store_prose_only_descriptions() {
    let mut store = make_store();
    let aliases: HashMap<String, Vec<String>> = HashMap::new();
    let cals = calendars();

    let mut root = Task::new("Garden plan", &aliases, None);
    root.uid = "root-1".to_string();
    root.calendar_href = "local://default".to_string();
    store.add_task(root);

    // Prose plus a subtask that itself carries prose and a nested subtask.
    let typed = "Beds to rotate this year.\n- [ ] divide the rhubarb\n  water deeply\n  - [ ] order the seeds";
    let (actions, warnings, _mints) = store
        .sync_tree_from_markdown("root-1", typed, &opts(&aliases, &cals), false)
        .unwrap();
    assert!(warnings.is_empty());
    assert!(!actions.is_empty());

    // A VTODO stores prose only: no uid-tagged shadow lines in the stored
    // descriptions, for the root and for the child alike.
    let root_after = store.get_task_ref("root-1").unwrap();
    assert_eq!(root_after.description, "Beds to rotate this year.");
    let child = children_of(&store, "root-1").pop().unwrap();
    assert_eq!(child.summary, "divide the rhubarb");
    assert_eq!(child.description, "water deeply");
    let grandchild = children_of(&store, &child.uid).pop().unwrap();
    assert_eq!(grandchild.summary, "order the seeds");
    assert!(grandchild.description.is_empty());

    // The merged view (what the description editor loads) still shows the
    // whole subtree, with the children appended uid-tagged below the prose.
    let view = cfait::model::extractor::serialize_merged_description(&store, "root-1", &cals);
    assert_eq!(
        view,
        format!(
            "Beds to rotate this year.\n- [ ] divide the rhubarb <!-- uid:{} -->\n  water deeply\n    - [ ] order the seeds <!-- uid:{} -->",
            child.uid, grandchild.uid
        ),
        "merged view: {view}"
    );

    // Re-saving that view is a no-op and keeps the stored prose-only
    // descriptions.
    let (actions2, warnings2, _mints2) = store
        .sync_tree_from_markdown("root-1", &view, &opts(&aliases, &cals), false)
        .unwrap();
    assert!(warnings2.is_empty());
    assert!(
        actions2.is_empty(),
        "an unchanged merged view must be a no-op: {actions2:?}"
    );
    assert_eq!(
        store.get_task_ref("root-1").unwrap().description,
        "Beds to rotate this year."
    );
    assert_eq!(
        store.get_task_ref(&child.uid).unwrap().description,
        "water deeply"
    );
}

#[test]
fn plain_task_merged_view_deletion_still_soft_deletes_children() {
    let mut store = make_store();
    let aliases: HashMap<String, Vec<String>> = HashMap::new();
    let cals = calendars();

    let mut root = Task::new("Garden plan", &aliases, None);
    root.uid = "root-1".to_string();
    root.calendar_href = "local://default".to_string();
    store.add_task(root);

    store
        .sync_tree_from_markdown(
            "root-1",
            "Beds to rotate.\n- [ ] divide the rhubarb",
            &opts(&aliases, &cals),
            false,
        )
        .unwrap();
    let child = children_of(&store, "root-1").pop().unwrap();

    // The merged view carries the child's uid-tagged line; the user deletes
    // the line and saves.
    let view = cfait::model::extractor::serialize_merged_description(&store, "root-1", &cals);
    assert!(
        view.contains(&format!("<!-- uid:{} -->", child.uid)),
        "the merged view must carry the child's line: {view}"
    );
    let (actions, _warnings, _mints) = store
        .sync_tree_from_markdown("root-1", "Beds to rotate.", &opts(&aliases, &cals), false)
        .unwrap();

    assert!(
        store
            .get_task_ref(&child.uid)
            .is_some_and(|t| t.calendar_href == "local://trash"),
        "deleting the line must soft-delete the task to the trash"
    );
    assert!(
        actions
            .iter()
            .any(|a| matches!(a, cfait::journal::Action::Delete(t) if t.uid == child.uid)),
        "the deletion must be recorded: {actions:?}"
    );
    assert_eq!(
        store.get_task_ref("root-1").unwrap().description,
        "Beds to rotate.",
        "the root keeps its prose-only description"
    );
}

#[test]
fn prose_preview_strips_shadow_lines() {
    let desc = "Morning pages.\n- [ ] sketch the pear tree <!-- uid:abc -->\nEvening notes.";
    assert_eq!(
        cfait::model::extractor::description_prose(desc),
        "Morning pages.\nEvening notes."
    );
    // Plain list prose the user typed is not tagged, so it survives.
    let plain = "Groceries.\n- olive oil\n- lemons";
    assert_eq!(cfait::model::extractor::description_prose(plain), plain);
}

#[test]
fn pasted_tagged_lines_are_reminted_on_create() {
    let mut store = make_store();
    let aliases: HashMap<String, Vec<String>> = HashMap::new();

    // An unrelated existing task with the uid the pasted line carries.
    let mut existing = Task::new("feed the sourdough starter", &aliases, None);
    existing.uid = "22222222-2222-2222-2222-222222222222".to_string();
    existing.calendar_href = "local://default".to_string();
    store.add_task(existing.clone());

    let mut media: HashMap<String, String> = HashMap::new();
    let (mut root_desc, extracted) = cfait::model::extractor::extract_markdown_tasks(
        "- [ ] sketch the pear tree <!-- uid:22222222-2222-2222-2222-222222222222 -->",
        false,
        &mut media,
    );
    let extracted =
        cfait::model::extractor::remint_existing_uids(&store, &mut root_desc, extracted);
    assert_eq!(extracted.len(), 1);
    assert_ne!(
        extracted[0].uid, existing.uid,
        "the pasted uid must be re-minted, not hijacked"
    );
    assert!(root_desc.contains(&format!("<!-- uid:{} -->", extracted[0].uid)));
    assert!(store.get_task_ref(&existing.uid).is_some());
}
