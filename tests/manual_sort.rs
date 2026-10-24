// SPDX-License-Identifier: GPL-3.0-or-later
//! Tests for manual task sorting: the `is:manual` token, fractional
//! `sort_position` keys, sibling moves, and tree-editor reordering.
use cfait::config::{Config, PausedSortBehavior, SortPreset};
use cfait::context::TestContext;
use cfait::model::{AppIntent, Task};
use cfait::store::{HierarchyOptions, TaskListItem, TaskStore};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

fn make_store() -> TaskStore {
    let ctx = Arc::new(TestContext::new());
    TaskStore::new(ctx)
}

fn parent_task(uid: &str, summary: &str, manual: bool) -> Task {
    let mut t = Task::new(summary, &HashMap::new(), None);
    t.uid = uid.to_string();
    t.calendar_href = "cal1".to_string();
    t.manual_sort = manual;
    t
}

fn child(uid: &str, summary: &str, parent: &str, position: &str) -> Task {
    let mut t = Task::new(summary, &HashMap::new(), None);
    t.uid = uid.to_string();
    t.calendar_href = "cal1".to_string();
    t.parent_uid = Some(parent.to_string());
    t.sort_position = position.to_string();
    t
}

fn organize(tasks: Vec<Task>) -> Vec<String> {
    let empty: HashSet<String> = HashSet::new();
    let items = cfait::store::organize_hierarchy(
        tasks,
        HierarchyOptions {
            default_priority: 5,
            sort_standard_by_priority: false,
            expanded_groups: &empty,
            max_done_roots: 100,
            max_done_subtasks: 100,
            search_active: false,
            sort_preset: SortPreset::default(),
            search_collapsed_tasks: &empty,
            focused_task_uid: None,
            paused_sort_behavior: PausedSortBehavior::default(),
            sort_tiebreak_recent: false,
        },
    );
    items
        .into_iter()
        .filter_map(|item| match item {
            TaskListItem::Task(t) => Some(t.uid),
            _ => None,
        })
        .collect()
}

/// A manual "weekend garden" tree: standard (alphabetical) order is
/// Mulch < Prune < Water, but the manual positions say Water, Mulch, Prune.
fn manual_garden() -> Vec<Task> {
    let mut tasks = vec![parent_task("p", "Plan the garden", true)];
    tasks.push(child("water", "Water the tomatoes", "p", "V"));
    tasks.push(child("mulch", "Mulch the beds", "p", "W"));
    tasks.push(child("prune", "Prune the roses", "p", "X"));
    tasks
}

#[test]
fn is_manual_token_roundtrips_through_parser() {
    let mut t = Task::new("Plan the garden", &HashMap::new(), None);
    cfait::model::parser::apply_smart_input(
        &mut t,
        "Plan the garden is:manual",
        &HashMap::new(),
        None,
    );
    assert!(t.manual_sort, "is:manual must set the flag");
    assert!(
        t.to_smart_string().contains("is:manual"),
        "the flag must serialize back: {}",
        t.to_smart_string()
    );

    // Re-parsing a line without the token restores the standard sort.
    let mut t2 = Task::new("Plan the garden", &HashMap::new(), None);
    t2.manual_sort = true;
    cfait::model::parser::apply_smart_input(&mut t2, "Plan the garden", &HashMap::new(), None);
    assert!(!t2.manual_sort, "removing the token must clear the flag");
    assert!(!t2.to_smart_string().contains("is:manual"));
}

#[test]
fn manual_sort_roundtrips_through_ics() {
    let parent = parent_task("p", "Plan the garden", true);
    let ics = parent.to_ics();
    assert!(
        ics.contains("X-CFAIT-MANUAL-SORT:TRUE"),
        "manual flag must be stored: {ics}"
    );
    let parsed = Task::from_ics(&ics, "e".into(), "h".into(), "cal".into()).unwrap();
    assert!(parsed.manual_sort);

    let c = child("c", "Water the tomatoes", "p", "V");
    let ics = c.to_ics();
    assert!(
        ics.contains("X-CFAIT-SORT-POSITION:V"),
        "position must be stored: {ics}"
    );
    let parsed = Task::from_ics(&ics, "e".into(), "h".into(), "cal".into()).unwrap();
    assert_eq!(parsed.sort_position, "V");

    // Neither property is emitted when the feature is off.
    let plain = parent_task("q", "Read a chapter", false);
    assert!(!plain.to_ics().contains("X-CFAIT-MANUAL-SORT"));
    let plain_c = child("r", "Feed the fish", "q", "");
    assert!(!plain_c.to_ics().contains("X-CFAIT-SORT-POSITION"));
}

#[test]
fn compare_manual_sort_orders_positioned_first() {
    let a = child("a", "Alpha", "p", "W");
    let b = child("b", "Beta", "p", "V");
    let c = child("c", "Gamma", "p", "");

    assert_eq!(b.compare_manual_sort(&a), std::cmp::Ordering::Less);
    assert_eq!(c.compare_manual_sort(&a), std::cmp::Ordering::Greater);
    assert_eq!(a.compare_manual_sort(&c), std::cmp::Ordering::Less);

    // Equal keys fall back to summary, then uid, for a stable order.
    let d = child("d", "Alpha", "p", "W");
    assert_eq!(a.compare_manual_sort(&d), std::cmp::Ordering::Less);
}

#[test]
fn organize_hierarchy_orders_manual_children_by_position() {
    // Standard order would be Mulch, Prune, Water (alphabetical); the manual
    // positions must win.
    assert_eq!(
        organize(manual_garden()),
        vec!["p", "water", "mulch", "prune"]
    );

    // The same tree without the flag keeps the standard order.
    let mut tasks = manual_garden();
    tasks[0].manual_sort = false;
    assert_eq!(organize(tasks), vec!["p", "mulch", "prune", "water"]);
}

#[test]
fn organize_hierarchy_unpositioned_children_sort_last() {
    let mut tasks = vec![parent_task("p", "Plan the garden", true)];
    tasks.push(child("water", "Water the tomatoes", "p", "V"));
    tasks.push(child("mulch", "Mulch the beds", "p", ""));
    tasks.push(child("prune", "Prune the roses", "p", "W"));
    assert_eq!(
        organize(tasks),
        vec!["p", "water", "prune", "mulch"],
        "unpositioned children sort after the keyed ones"
    );
}

#[test]
fn organize_hierarchy_dependencies_still_win_over_manual_order() {
    // Manual order says Prune first, but Prune is blocked by Water, so the
    // topological pass must surface Water before it.
    let mut tasks = vec![parent_task("p", "Plan the garden", true)];
    tasks.push(child("prune", "Prune the roses", "p", "V"));
    tasks.push(child("water", "Water the tomatoes", "p", "W"));
    tasks.push(child("mulch", "Mulch the beds", "p", "X"));
    tasks[1].dependencies.push("water".to_string());
    assert_eq!(organize(tasks), vec!["p", "water", "prune", "mulch"]);
}

#[test]
fn move_manual_node_rewrites_a_single_position() {
    let mut store = make_store();
    store.add_task(parent_task("p", "Plan the garden", true));
    store.add_task(child("water", "Water the tomatoes", "p", "V"));
    store.add_task(child("mulch", "Mulch the beds", "p", "W"));
    store.add_task(child("prune", "Prune the roses", "p", "X"));

    // Move Prune up between Water ("V") and Mulch ("W"): one key between
    // them, and only the moved task is rewritten.
    let actions = store.move_manual_node("prune", true);
    assert_eq!(actions.len(), 1, "only the moved task should be updated");
    assert_eq!(store.get_task_ref("prune").unwrap().sort_position, "VV");
    assert_eq!(store.get_task_ref("water").unwrap().sort_position, "V");
    assert_eq!(store.get_task_ref("mulch").unwrap().sort_position, "W");

    // Move it back down: the open end after Mulch ("W") extends to "X".
    let actions = store.move_manual_node("prune", false);
    assert_eq!(actions.len(), 1);
    assert_eq!(store.get_task_ref("prune").unwrap().sort_position, "X");
}

#[test]
fn move_manual_node_self_heals_the_parent_flag() {
    let mut store = make_store();
    store.add_task(parent_task("p", "Plan the garden", false));
    store.add_task(child("water", "Water the tomatoes", "p", ""));
    store.add_task(child("mulch", "Mulch the beds", "p", ""));
    store.add_task(child("prune", "Prune the roses", "p", ""));

    // Moving Mulch down (before Prune) turns the parent manual and assigns
    // fresh positions to the whole, previously unpositioned group.
    let actions = store.move_manual_node("mulch", false);
    assert!(
        store.get_task_ref("p").unwrap().manual_sort,
        "an explicit move must enable manual sorting on the parent"
    );
    // Parent update + three child updates.
    assert_eq!(actions.len(), 4, "actions: {actions:?}");
    assert_eq!(store.get_task_ref("prune").unwrap().sort_position, "V");
    assert_eq!(store.get_task_ref("mulch").unwrap().sort_position, "W");
    assert_eq!(store.get_task_ref("water").unwrap().sort_position, "X");
}

#[test]
fn move_manual_node_boundaries_are_noops() {
    let mut store = make_store();
    store.add_task(parent_task("p", "Plan the garden", true));
    store.add_task(child("water", "Water the tomatoes", "p", "V"));
    store.add_task(child("mulch", "Mulch the beds", "p", "W"));

    assert!(store.move_manual_node("water", true).is_empty());
    assert!(store.move_manual_node("mulch", false).is_empty());
    assert_eq!(store.get_task_ref("water").unwrap().sort_position, "V");
    assert_eq!(store.get_task_ref("mulch").unwrap().sort_position, "W");

    // Roots have no sibling group to move in.
    assert!(store.move_manual_node("p", true).is_empty());
}

#[test]
fn tree_sync_reassigns_positions_in_line_order() {
    let mut store = make_store();
    store.add_task(parent_task("p", "Plan the garden", true));
    store.add_task(child("water", "Water the tomatoes", "p", "V"));
    store.add_task(child("mulch", "Mulch the beds", "p", "W"));
    store.add_task(child("prune", "Prune the roses", "p", "X"));

    // Serialize the tree (manual order: Water, Mulch, Prune) and swap the
    // Mulch and Prune lines.
    let markdown = cfait::model::extractor::serialize_task_tree(&store, "p", &[], false);
    let mut lines: Vec<&str> = markdown.lines().collect();
    let i_mulch = lines.iter().position(|l| l.contains("uid:mulch")).unwrap();
    let i_prune = lines.iter().position(|l| l.contains("uid:prune")).unwrap();
    lines.swap(i_mulch, i_prune);
    let reordered = lines.join("\n");

    let options = cfait::store::SyncTreeOptions {
        aliases: &HashMap::new(),
        default_reminder_time: None,
        trash_retention_days: 30,
        calendars: &[],
    };
    let (actions, warnings, _mints) = store
        .sync_tree_from_markdown("p", &reordered, &options, false, true)
        .expect("tree sync should succeed");
    assert!(warnings.is_empty());

    // New line order Water, Prune, Mulch gets positions V, W, X.
    assert_eq!(store.get_task_ref("water").unwrap().sort_position, "V");
    assert_eq!(store.get_task_ref("prune").unwrap().sort_position, "W");
    assert_eq!(store.get_task_ref("mulch").unwrap().sort_position, "X");
    assert_eq!(
        actions.len(),
        2,
        "only the two moved children update: {actions:?}"
    );

    // Re-saving the same buffer is a no-op.
    let markdown2 = cfait::model::extractor::serialize_task_tree(&store, "p", &[], false);
    let (actions2, warnings2, _mints2) = store
        .sync_tree_from_markdown("p", &markdown2, &options, false, true)
        .expect("tree sync should succeed");
    assert!(warnings2.is_empty());
    assert!(
        actions2.is_empty(),
        "an unchanged manual tree must round-trip without actions: {actions2:?}"
    );
}

#[test]
fn tree_sync_clears_positions_when_manual_is_removed() {
    let mut store = make_store();
    store.add_task(parent_task("p", "Plan the garden", true));
    store.add_task(child("water", "Water the tomatoes", "p", "V"));
    store.add_task(child("mulch", "Mulch the beds", "p", "W"));
    store.add_task(child("prune", "Prune the roses", "p", "X"));

    // Drop the token from the root line.
    let markdown = cfait::model::extractor::serialize_task_tree(&store, "p", &[], false);
    let unflagged = markdown.replace(" is:manual", "");
    assert!(
        !unflagged.contains("is:manual"),
        "the serialized tree must carry the token"
    );

    let options = cfait::store::SyncTreeOptions {
        aliases: &HashMap::new(),
        default_reminder_time: None,
        trash_retention_days: 30,
        calendars: &[],
    };
    let (actions, warnings, _mints) = store
        .sync_tree_from_markdown("p", &unflagged, &options, false, true)
        .expect("tree sync should succeed");
    assert!(warnings.is_empty());

    assert!(!store.get_task_ref("p").unwrap().manual_sort);
    for uid in ["water", "mulch", "prune"] {
        assert_eq!(
            store.get_task_ref(uid).unwrap().sort_position,
            "",
            "{uid} must lose its stale position"
        );
    }
    // Root update + three child updates.
    assert_eq!(actions.len(), 4, "actions: {actions:?}");
}

#[test]
fn extractor_serializes_manual_order() {
    let mut store = make_store();
    store.add_task(parent_task("p", "Plan the garden", true));
    store.add_task(child("water", "Water the tomatoes", "p", "V"));
    store.add_task(child("mulch", "Mulch the beds", "p", "W"));
    store.add_task(child("prune", "Prune the roses", "p", "X"));

    // Alphabetical order would be Mulch, Prune, Water; the buffer must show
    // the manual order instead.
    let markdown = cfait::model::extractor::serialize_task_tree(&store, "p", &[], false);
    let i_water = markdown
        .lines()
        .position(|l| l.contains("uid:water"))
        .unwrap();
    let i_mulch = markdown
        .lines()
        .position(|l| l.contains("uid:mulch"))
        .unwrap();
    let i_prune = markdown
        .lines()
        .position(|l| l.contains("uid:prune"))
        .unwrap();
    assert!(
        i_water < i_mulch && i_mulch < i_prune,
        "buffer:\n{markdown}"
    );
}

#[test]
fn apply_task_intent_moves_a_manual_node() {
    let mut store = make_store();
    store.add_task(parent_task("p", "Plan the garden", true));
    store.add_task(child("water", "Water the tomatoes", "p", "V"));
    store.add_task(child("mulch", "Mulch the beds", "p", "W"));
    store.add_task(child("prune", "Prune the roses", "p", "X"));
    let config = Config {
        trash_retention_days: 0,
        ..Default::default()
    };

    let (actions, undo, desc, primary) = store.apply_task_intent(
        &AppIntent::MoveNodeUp {
            uid: "prune".into(),
        },
        &config,
    );
    assert_eq!(actions.len(), 1, "only the moved task should update");
    assert_eq!(
        store.get_task_ref("prune").unwrap().sort_position,
        "VV",
        "MoveNodeUp must rewrite the position"
    );
    assert_eq!(desc, "Moved up: Prune the roses");
    assert_eq!(primary, Some("prune".to_string()));
    assert!(!undo.is_empty(), "undo actions must be recorded");

    let (actions, _undo, desc, _primary) = store.apply_task_intent(
        &AppIntent::MoveNodeDown {
            uid: "prune".into(),
        },
        &config,
    );
    assert_eq!(actions.len(), 1);
    assert_eq!(desc, "Moved down: Prune the roses");
    assert_eq!(store.get_task_ref("prune").unwrap().sort_position, "X");
}
