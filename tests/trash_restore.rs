// SPDX-License-Identifier: GPL-3.0-or-later
//! Tests for restoring soft-deleted tasks and sub-trees from the trash.
use cfait::config::Config;
use cfait::context::TestContext;
use cfait::journal::Action;
use cfait::model::{AppIntent, Task};
use cfait::store::TaskStore;
use std::collections::HashMap;
use std::sync::Arc;

const TRASH: &str = "local://trash";

fn make_store() -> TaskStore {
    let ctx = Arc::new(TestContext::new());
    TaskStore::new(ctx)
}

fn task(uid: &str, summary: &str, cal: &str, parent: Option<&str>) -> Task {
    let mut t = Task::new(summary, &HashMap::new(), None);
    t.uid = uid.to_string();
    t.calendar_href = cal.to_string();
    t.parent_uid = parent.map(|p| p.to_string());
    t
}

fn config_with_trash() -> Config {
    Config {
        trash_retention_days: 30,
        ..Default::default()
    }
}

fn has_prop(t: &Task, key: &str) -> bool {
    t.unmapped_properties.iter().any(|p| p.key == key)
}

#[test]
fn tree_delete_preserves_hierarchy_in_trash() {
    let mut store = make_store();
    let config = config_with_trash();
    store.add_task(task("garden", "Garden", "cal1", None));
    store.add_task(task("root", "Grow tomatoes", "cal1", Some("garden")));
    store.add_task(task("child", "Buy seeds", "cal1", Some("root")));
    store.add_task(task("grandchild", "Compare prices", "cal1", Some("child")));

    store.apply_task_intent(
        &AppIntent::DeleteTaskTree {
            uid: "root".to_string(),
        },
        &config,
    );

    // The sub-tree must stay intact inside the trash so it can be restored
    // as a whole, and each task must remember where it came from.
    for (uid, parent) in [
        ("root", "garden"),
        ("child", "root"),
        ("grandchild", "child"),
    ] {
        let t = store.get_task_ref(uid).expect("trashed");
        assert_eq!(t.calendar_href, TRASH);
        assert_eq!(t.parent_uid.as_deref(), Some(parent));
        assert!(has_prop(t, "X-TRASHED-DATE"), "{uid} missing trash date");
        assert!(has_prop(t, "X-TRASHED-FROM"), "{uid} missing origin");
        assert_eq!(
            t.unmapped_properties
                .iter()
                .find(|p| p.key == "X-TRASHED-FROM")
                .unwrap()
                .value,
            "cal1"
        );
    }
}

#[test]
fn restore_tree_puts_tasks_back_where_they_were() {
    let mut store = make_store();
    let config = config_with_trash();
    store.add_task(task("garden", "Garden", "cal1", None));
    store.add_task(task("root", "Grow tomatoes", "cal1", Some("garden")));
    store.add_task(task("child", "Buy seeds", "cal1", Some("root")));

    store.apply_task_intent(
        &AppIntent::DeleteTaskTree {
            uid: "root".to_string(),
        },
        &config,
    );

    let (actions, reverse, desc, primary) = store.apply_task_intent(
        &AppIntent::RestoreTaskTree {
            uid: "root".to_string(),
        },
        &config,
    );

    assert_eq!(primary.as_deref(), Some("root"));
    assert!(
        desc.contains("tree"),
        "undo description should describe the restore: {desc}"
    );
    assert_eq!(actions.len(), 4, "delete+create per restored task");
    // trash -> remote is journaled as Delete (trash copy) + Create (new copy)
    for a in &actions {
        match a {
            Action::Delete(t) => assert_eq!(t.calendar_href, TRASH),
            Action::Create(t) => {
                assert_eq!(t.calendar_href, "cal1");
                assert!(!has_prop(t, "X-TRASHED-DATE"));
                assert!(!has_prop(t, "X-TRASHED-FROM"));
            }
            other => panic!("unexpected journal action: {other:?}"),
        }
    }

    for (uid, parent) in [("root", "garden"), ("child", "root")] {
        let t = store.get_task_ref(uid).expect("restored");
        assert_eq!(t.calendar_href, "cal1");
        assert_eq!(t.parent_uid.as_deref(), Some(parent));
        assert!(!has_prop(t, "X-TRASHED-DATE"));
        assert!(!has_prop(t, "X-TRASHED-FROM"));
    }
    let garden = store.get_task_ref("garden").unwrap();
    assert_eq!(garden.calendar_href, "cal1");
    assert!(garden.parent_uid.is_none());

    // Undo: the reverse actions put the tree back in the trash.
    store.apply_actions(&reverse);
    for uid in ["root", "child"] {
        let t = store.get_task_ref(uid).expect("re-trashed by undo");
        assert_eq!(t.calendar_href, TRASH);
    }
    assert!(store.get_task_ref("garden").unwrap().calendar_href == "cal1");
}

#[test]
fn restore_single_deleted_task_reattaches_to_live_parent() {
    let mut store = make_store();
    let config = config_with_trash();
    store.add_task(task("garden", "Garden", "cal1", None));
    store.add_task(task("root", "Grow tomatoes", "cal1", Some("garden")));
    store.add_task(task("child", "Buy seeds", "cal1", Some("root")));

    // Deleting only the middle task promotes its live children...
    store.apply_task_intent(
        &AppIntent::DeleteTask {
            uid: "root".to_string(),
        },
        &config,
    );
    let trashed = store.get_task_ref("root").unwrap();
    assert_eq!(trashed.calendar_href, TRASH);
    assert_eq!(trashed.parent_uid.as_deref(), Some("garden"));

    // ...and restoring it places it back under its parent.
    store.apply_task_intent(
        &AppIntent::RestoreTaskTree {
            uid: "root".to_string(),
        },
        &config,
    );
    let restored = store.get_task_ref("root").unwrap();
    assert_eq!(restored.calendar_href, "cal1");
    assert_eq!(restored.parent_uid.as_deref(), Some("garden"));
}

#[test]
fn restore_task_without_trash_origin_uses_fallbacks() {
    let mut store = make_store();
    let mut config = config_with_trash();

    // A live parent in cal2 anchors the fallback for unrecorded trash tasks.
    store.add_task(task("anchor", "Garden journal", "cal2", None));
    // Old trash item: trashed before origins were recorded, still linked to
    // its live parent.
    store.add_task(task("legacy", "Repot the fern", TRASH, Some("anchor")));
    // Old trash item with no parent either: falls back to the default calendar.
    store.add_task(task("orphan", "Sharpen shears", TRASH, None));
    config.default_calendar = Some("cal3".to_string());
    store.add_task(task("cal3-dummy", "Prune roses", "cal3", None));

    let (actions, _, _, _) = store.apply_task_intent(
        &AppIntent::RestoreTaskTree {
            uid: "legacy".to_string(),
        },
        &config,
    );
    assert!(!actions.is_empty());
    assert_eq!(store.get_task_ref("legacy").unwrap().calendar_href, "cal2");
    assert_eq!(
        store.get_task_ref("legacy").unwrap().parent_uid.as_deref(),
        Some("anchor")
    );

    let (actions, _, _, _) = store.apply_task_intent(
        &AppIntent::RestoreTaskTree {
            uid: "orphan".to_string(),
        },
        &config,
    );
    assert!(!actions.is_empty());
    assert_eq!(store.get_task_ref("orphan").unwrap().calendar_href, "cal3");
}

#[test]
fn restore_task_outside_trash_is_a_no_op() {
    let mut store = make_store();
    let config = config_with_trash();
    store.add_task(task("root", "Grow tomatoes", "cal1", None));

    let (actions, _, _, _) = store.apply_task_intent(
        &AppIntent::RestoreTaskTree {
            uid: "root".to_string(),
        },
        &config,
    );
    assert!(actions.is_empty());
    assert_eq!(store.get_task_ref("root").unwrap().calendar_href, "cal1");
}

#[test]
fn restore_local_trash_to_local_calendar_journals_a_move() {
    let mut store = make_store();
    let config = config_with_trash();
    // A task originally in the local calendar, then trashed.
    store.add_task(task("note", "Water the ferns", "local://default", None));
    store.apply_task_intent(
        &AppIntent::DeleteTask {
            uid: "note".to_string(),
        },
        &config,
    );
    assert_eq!(store.get_task_ref("note").unwrap().calendar_href, TRASH);

    let (actions, _, _, _) = store.apply_task_intent(
        &AppIntent::RestoreTaskTree {
            uid: "note".to_string(),
        },
        &config,
    );
    // local -> local is a plain Move, not delete+create.
    assert!(matches!(
        actions.as_slice(),
        [Action::Move(t, target)]
            if t.calendar_href == TRASH && target == "local://default"
    ));
    assert_eq!(
        store.get_task_ref("note").unwrap().calendar_href,
        "local://default"
    );
}
