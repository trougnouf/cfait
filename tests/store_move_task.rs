use cfait::model::Task;
use cfait::store::TaskStore;
use std::collections::HashMap;

#[test]
fn test_move_task_with_original_returns_original_and_updated_and_updates_store() {
    let mut store = TaskStore::new();

    let mut t = Task::new("Unit Test Task", &HashMap::new(), None);
    t.uid = "test-uid-1".to_string();
    t.calendar_href = "local://cal1".to_string();

    // Insert into store
    store.add_task(t.clone());

    // Move task to a new calendar and get both original and updated
    let res = store.move_task(&t.uid, "local://cal2".to_string());
    assert!(res.is_some(), "Expected move_task to return Some");

    let (original, updated) = res.unwrap();

    // Original should reflect pre-mutation state
    assert_eq!(original.uid, t.uid);
    assert_eq!(original.calendar_href, "local://cal1");

    // Updated should reflect the new calendar
    assert_eq!(updated.uid, t.uid);
    assert_eq!(updated.calendar_href, "local://cal2");

    // Store should no longer contain the task in the original calendar
    let in_old = store
        .calendars
        .get("local://cal1")
        .map(|v| v.iter().any(|x| x.uid == t.uid))
        .unwrap_or(false);
    assert!(!in_old, "Task should not remain in the old calendar");

    // Store should contain the task in the new calendar
    let in_new = store
        .calendars
        .get("local://cal2")
        .map(|v| v.iter().any(|x| x.uid == t.uid))
        .unwrap_or(false);
    assert!(in_new, "Task should exist in the new calendar");
}
