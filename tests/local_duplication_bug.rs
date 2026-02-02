use cfait::context::TestContext;
use cfait::model::{Task, TaskStatus};
use cfait::storage::{LOCAL_CALENDAR_HREF, LocalStorage};
use cfait::store::{FilterOptions, TaskStore};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

#[test]
fn test_reproduce_android_local_revert_bug() {
    let ctx = Arc::new(TestContext::new());

    // 1. SIMULATE CORRUPTION: Create a local storage file with duplicate UIDs.
    // This is what the old import_from_ics logic did.
    let uid = "corrupted-uid-123";
    let mut t1 = Task::new("Original Summary", &HashMap::new(), None);
    t1.uid = uid.to_string();
    t1.calendar_href = LOCAL_CALENDAR_HREF.to_string();

    let mut t2 = t1.clone();
    t2.summary = "Stale Duplicate".to_string();

    // Manually save a list with duplicates to disk
    let corrupted_list = vec![t1.clone(), t2.clone()];
    LocalStorage::save_for_href(ctx.as_ref(), LOCAL_CALENDAR_HREF, &corrupted_list).unwrap();

    // 2. SIMULATE ANDROID BRIDGE UPDATE:
    // This mimics the logic in CfaitMobile::apply_store_mutation or toggle_task.
    let mut local_list = LocalStorage::load_for_href(ctx.as_ref(), LOCAL_CALENDAR_HREF).unwrap();

    // The bridge finds the task to update.
    // .position() finds the FIRST occurrence (the one with "Original Summary").
    if let Some(idx) = local_list.iter().position(|t| t.uid == uid) {
        local_list[idx].summary = "Updated via Bridge".to_string();
        local_list[idx].status = TaskStatus::Completed;

        // Save the list back to disk.
        // The file now contains:
        // [0] UID:123 Summary:"Updated via Bridge"
        // [1] UID:123 Summary:"Stale Duplicate"
        LocalStorage::save_for_href(ctx.as_ref(), LOCAL_CALENDAR_HREF, &local_list).unwrap();
    }

    // 3. SIMULATE UI RELOAD:
    // This mimics how the app starts up or refreshes.
    let mut store = TaskStore::new(ctx.clone());
    let reloaded_list = LocalStorage::load_for_href(ctx.as_ref(), LOCAL_CALENDAR_HREF).unwrap();
    store.insert(LOCAL_CALENDAR_HREF.to_string(), reloaded_list);

    // Filter to get the tasks as the UI would see them
    let filtered = store.filter(FilterOptions {
        active_cal_href: None,
        hidden_calendars: &HashSet::new(),
        selected_categories: &HashSet::new(),
        selected_locations: &HashSet::new(),
        match_all_categories: false,
        search_term: "",
        hide_completed_global: false, // Don't hide so we can check
        cutoff_date: None,
        min_duration: None,
        max_duration: None,
        include_unset_duration: true,
        urgent_days: 1,
        urgent_prio: 1,
        default_priority: 5,
        start_grace_period_days: 1,
    });

    let visible_task = filtered
        .iter()
        .find(|t| t.uid == uid)
        .expect("Task should exist");

    // VERIFICATION:
    // If the bug is present:
    // The TaskStore (HashMap) took the LAST entry from the file ("Stale Duplicate").
    // The UI shows "Stale Duplicate" and Status::NeedsAction.
    // The user's change ("Updated via Bridge") is effectively hidden/reverted.

    assert_eq!(
        visible_task.summary, "Updated via Bridge",
        "BUG: Task summary was reverted to the stale duplicate!"
    );
    assert!(
        visible_task.status == TaskStatus::Completed,
        "BUG: Task status was reverted to the stale duplicate!"
    );
}

