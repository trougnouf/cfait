// Unit tests for export functionality and full roundtrip chain (create → export → import)
use cfait::model::{Alarm, DateType, Task, TaskStatus};
use cfait::storage::LocalStorage;
use cfait::store::TaskStore;
use chrono::{Datelike, NaiveDate, TimeZone, Utc};
use serial_test::serial;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::time::SystemTime;

fn setup_test_env(test_name: &str) -> String {
    let thread_id = std::thread::current().id();
    let timestamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let test_dir = env::temp_dir().join(format!(
        "cfait_export_test_{}_{:?}_{}",
        test_name, thread_id, timestamp
    ));
    let _ = fs::remove_dir_all(&test_dir);
    fs::create_dir_all(&test_dir).unwrap();
    unsafe {
        env::set_var("CFAIT_TEST_DIR", test_dir.to_str().unwrap());
    }
    test_dir.to_str().unwrap().to_string()
}

fn cleanup_test_env() {
    unsafe {
        env::remove_var("CFAIT_TEST_DIR");
    }
}

// ==================== Export Tests ====================

#[test]
#[serial]
fn test_export_single_task() {
    setup_test_env("export_single");

    let task = Task::new("Test Task", &HashMap::new(), None);
    let tasks = vec![task];

    let ics = LocalStorage::to_ics_string(&tasks);

    // Verify ICS structure
    assert!(ics.contains("BEGIN:VCALENDAR"));
    assert!(ics.contains("END:VCALENDAR"));
    assert!(ics.contains("VERSION:2.0"));
    assert!(ics.contains("PRODID:-//Cfait//Export//EN"));
    assert!(ics.contains("BEGIN:VTODO"));
    assert!(ics.contains("END:VTODO"));
    assert!(ics.contains("SUMMARY:Test Task"));

    // Verify only one VTODO block
    assert_eq!(ics.matches("BEGIN:VTODO").count(), 1);

    cleanup_test_env();
}

#[test]
#[serial]
fn test_export_multiple_tasks() {
    setup_test_env("export_multiple");

    let task1 = Task::new("Task One", &HashMap::new(), None);
    let task2 = Task::new("Task Two", &HashMap::new(), None);
    let task3 = Task::new("Task Three", &HashMap::new(), None);
    let tasks = vec![task1, task2, task3];

    let ics = LocalStorage::to_ics_string(&tasks);

    // Verify structure
    assert!(ics.contains("BEGIN:VCALENDAR"));
    assert!(ics.contains("END:VCALENDAR"));

    // Verify all three tasks are present
    assert_eq!(ics.matches("BEGIN:VTODO").count(), 3);
    assert!(ics.contains("SUMMARY:Task One"));
    assert!(ics.contains("SUMMARY:Task Two"));
    assert!(ics.contains("SUMMARY:Task Three"));

    cleanup_test_env();
}

#[test]
#[serial]
fn test_export_empty_task_list() {
    setup_test_env("export_empty");

    let tasks: Vec<Task> = vec![];
    let ics = LocalStorage::to_ics_string(&tasks);

    // Should still have valid VCALENDAR structure
    assert!(ics.contains("BEGIN:VCALENDAR"));
    assert!(ics.contains("END:VCALENDAR"));
    assert!(ics.contains("VERSION:2.0"));

    // But no VTODO blocks
    assert_eq!(ics.matches("BEGIN:VTODO").count(), 0);

    cleanup_test_env();
}

#[test]
#[serial]
fn test_export_task_with_due_date() {
    setup_test_env("export_due");

    let mut task = Task::new("Task with Due", &HashMap::new(), None);
    task.due = Some(DateType::Specific(
        Utc.with_ymd_and_hms(2026, 3, 15, 14, 30, 0).unwrap(),
    ));
    let tasks = vec![task];

    let ics = LocalStorage::to_ics_string(&tasks);

    assert!(ics.contains("DUE:20260315T143000Z"));

    cleanup_test_env();
}

#[test]
#[serial]
fn test_export_task_with_alarm() {
    setup_test_env("export_alarm");

    let mut task = Task::new("Task with Alarm", &HashMap::new(), None);
    task.due = Some(DateType::Specific(
        Utc.with_ymd_and_hms(2026, 3, 15, 14, 0, 0).unwrap(),
    ));
    task.alarms.push(Alarm::new_relative(15));
    let tasks = vec![task];

    let ics = LocalStorage::to_ics_string(&tasks);

    assert!(ics.contains("BEGIN:VALARM"));
    assert!(ics.contains("END:VALARM"));
    assert!(ics.contains("TRIGGER:-PT15M"));
    assert!(ics.contains("ACTION:DISPLAY"));

    cleanup_test_env();
}

#[test]
#[serial]
fn test_export_task_with_priority() {
    setup_test_env("export_priority");

    let mut task = Task::new("High Priority Task", &HashMap::new(), None);
    task.priority = 1;
    let tasks = vec![task];

    let ics = LocalStorage::to_ics_string(&tasks);

    assert!(ics.contains("PRIORITY:1"));

    cleanup_test_env();
}

#[test]
#[serial]
fn test_export_completed_task() {
    setup_test_env("export_completed");

    let mut task = Task::new("Completed Task", &HashMap::new(), None);
    task.status = TaskStatus::Completed;
    task.percent_complete = Some(100);
    let tasks = vec![task];

    let ics = LocalStorage::to_ics_string(&tasks);

    assert!(ics.contains("STATUS:COMPLETED"));

    cleanup_test_env();
}

#[test]
#[serial]
fn test_export_task_with_recurrence() {
    setup_test_env("export_recurrence");

    let mut task = Task::new("Weekly Task", &HashMap::new(), None);
    task.rrule = Some("FREQ=WEEKLY;BYDAY=MO,WE,FR".to_string());
    let tasks = vec![task];

    let ics = LocalStorage::to_ics_string(&tasks);

    assert!(ics.contains("RRULE:FREQ=WEEKLY;BYDAY=MO,WE,FR"));

    cleanup_test_env();
}

// ==================== Full Roundtrip Tests ====================

#[test]
#[serial]
fn test_roundtrip_simple_task() {
    setup_test_env("roundtrip_simple");

    let href = "local://roundtrip-calendar-1";

    // 1. Create task
    let original = Task::new("Roundtrip Task", &HashMap::new(), None);
    LocalStorage::save_for_href(href, std::slice::from_ref(&original)).unwrap();

    // 2. Export
    let tasks = LocalStorage::load_for_href(href).unwrap();
    let ics = LocalStorage::to_ics_string(&tasks);

    // 3. Import to new calendar
    let import_href = "local://roundtrip-import-1";
    let result = LocalStorage::import_from_ics(import_href, &ics);
    assert!(result.is_ok(), "Import failed: {:?}", result.err());
    assert_eq!(result.unwrap(), 1);

    // 4. Verify
    let imported_tasks = LocalStorage::load_for_href(import_href).unwrap();
    assert_eq!(imported_tasks.len(), 1);
    assert_eq!(imported_tasks[0].summary, original.summary);
    assert_eq!(imported_tasks[0].calendar_href, import_href);

    cleanup_test_env();
}

#[test]
#[serial]
fn test_roundtrip_multiple_tasks() {
    setup_test_env("roundtrip_multiple");

    let href = "local://roundtrip-calendar-2";

    // 1. Create multiple tasks
    let task1 = Task::new("First Task", &HashMap::new(), None);
    let task2 = Task::new("Second Task", &HashMap::new(), None);
    let task3 = Task::new("Third Task", &HashMap::new(), None);
    LocalStorage::save_for_href(href, &[task1, task2, task3]).unwrap();

    // 2. Export
    let tasks = LocalStorage::load_for_href(href).unwrap();
    assert_eq!(tasks.len(), 3);
    let ics = LocalStorage::to_ics_string(&tasks);

    // 3. Import
    let import_href = "local://roundtrip-import-2";
    let result = LocalStorage::import_from_ics(import_href, &ics);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 3);

    // 4. Verify
    let imported_tasks = LocalStorage::load_for_href(import_href).unwrap();
    assert_eq!(imported_tasks.len(), 3);
    assert_eq!(imported_tasks[0].summary, "First Task");
    assert_eq!(imported_tasks[1].summary, "Second Task");
    assert_eq!(imported_tasks[2].summary, "Third Task");

    cleanup_test_env();
}

#[test]
#[serial]
fn test_priority_clamp_export_and_import() {
    setup_test_env("prio_roundtrip");

    // Export side: a task with a priority > 9 should be clamped in the generated ICS
    let mut task = Task::new("Prio Export Task", &HashMap::new(), None);
    task.priority = 15;
    let tasks = vec![task];
    let ics = LocalStorage::to_ics_string(&tasks);
    // Export should clamp to 9
    assert!(
        ics.contains("PRIORITY:9"),
        "Exported ICS did not contain clamped PRIORITY:9: {}",
        ics
    );

    // Import side: an incoming ICS with PRIORITY:15 should be clamped to 9 when imported
    let ics_in = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Cfait//Export//EN\r\nBEGIN:VTODO\r\nUID:uid-prio\r\nSUMMARY:Prio Import\r\nPRIORITY:15\r\nEND:VTODO\r\nEND:VCALENDAR";
    let import_href = "local://prio-import";
    let res = LocalStorage::import_from_ics(import_href, ics_in);
    assert!(res.is_ok());
    assert_eq!(res.unwrap(), 1);
    let imported = LocalStorage::load_for_href(import_href).unwrap();
    assert_eq!(imported.len(), 1);
    assert_eq!(
        imported[0].priority, 9,
        "Imported task priority was not clamped to 9"
    );
    cleanup_test_env();
}

#[test]
#[serial]
fn test_alias_priority_clamp() {
    setup_test_env("alias_prio");

    // Use TaskStore.apply_alias_retroactively to apply an alias that contains a priority >9
    let mut store = TaskStore::new();
    let mut task = Task::new("Alias Prio", &HashMap::new(), None);
    task.uid = "t-alias".to_string();
    task.location = Some("Home".to_string());
    // Ensure the task is associated with a calendar so apply_alias_retroactively can find it
    task.calendar_href = "local://alias-test".to_string();
    store.add_task(task);

    // Apply location alias that includes a priority of 15 (should clamp to 9)
    let modified = store.apply_alias_retroactively("@@Home", &["!15".to_string()]);
    assert_eq!(
        modified.len(),
        1,
        "Expected one modified task from alias application"
    );
    assert_eq!(
        modified[0].priority, 9,
        "Alias-applied priority was not clamped to 9"
    );

    cleanup_test_env();
}

#[test]
#[serial]
fn test_roundtrip_task_with_all_fields() {
    setup_test_env("roundtrip_all_fields");

    let href = "local://roundtrip-calendar-3";

    // 1. Create task with many fields
    let mut original = Task::new("Complex Task", &HashMap::new(), None);
    original.description = "This is a detailed description".to_string();
    original.priority = 2;
    original.due = Some(DateType::Specific(
        Utc.with_ymd_and_hms(2026, 6, 1, 10, 0, 0).unwrap(),
    ));
    original.alarms.push(Alarm::new_relative(30));
    original.rrule = Some("FREQ=DAILY;COUNT=10".to_string());

    LocalStorage::save_for_href(href, &[original.clone()]).unwrap();

    // 2. Export
    let tasks = LocalStorage::load_for_href(href).unwrap();
    let ics = LocalStorage::to_ics_string(&tasks);

    // 3. Import
    let import_href = "local://roundtrip-import-3";
    let result = LocalStorage::import_from_ics(import_href, &ics);
    assert!(result.is_ok());

    // 4. Verify all fields preserved
    let imported = &LocalStorage::load_for_href(import_href).unwrap()[0];
    assert_eq!(imported.summary, original.summary);
    assert_eq!(imported.description, original.description);
    assert_eq!(imported.priority, original.priority);
    assert!(imported.due.is_some());
    assert_eq!(imported.rrule, original.rrule);
    assert!(!imported.alarms.is_empty());

    cleanup_test_env();
}

#[test]
#[serial]
fn test_roundtrip_preserves_completed_status() {
    setup_test_env("roundtrip_completed");

    let href = "local://roundtrip-calendar-4";

    // 1. Create completed task
    let mut original = Task::new("Done Task", &HashMap::new(), None);
    original.status = TaskStatus::Completed;
    original.percent_complete = Some(100);

    LocalStorage::save_for_href(href, &[original.clone()]).unwrap();

    // 2. Export
    let tasks = LocalStorage::load_for_href(href).unwrap();
    let ics = LocalStorage::to_ics_string(&tasks);

    // 3. Import
    let import_href = "local://roundtrip-import-4";
    LocalStorage::import_from_ics(import_href, &ics).unwrap();

    // 4. Verify completion preserved
    let imported = &LocalStorage::load_for_href(import_href).unwrap()[0];
    assert!(imported.status.is_done());
    assert_eq!(imported.percent_complete, Some(100));

    cleanup_test_env();
}

#[test]
#[serial]
fn test_roundtrip_allday_date() {
    setup_test_env("roundtrip_allday");

    let href = "local://roundtrip-calendar-5";

    // 1. Create task with all-day date
    let mut original = Task::new("All Day Task", &HashMap::new(), None);
    original.due = Some(DateType::AllDay(
        NaiveDate::from_ymd_opt(2026, 7, 4).unwrap(),
    ));

    LocalStorage::save_for_href(href, &[original.clone()]).unwrap();

    // 2. Export
    let tasks = LocalStorage::load_for_href(href).unwrap();
    let ics = LocalStorage::to_ics_string(&tasks);

    // Verify all-day format in ICS
    assert!(ics.contains("DUE;VALUE=DATE:20260704"));

    // 3. Import
    let import_href = "local://roundtrip-import-5";
    LocalStorage::import_from_ics(import_href, &ics).unwrap();

    // 4. Verify all-day date preserved
    let imported = &LocalStorage::load_for_href(import_href).unwrap()[0];
    match imported.due {
        Some(DateType::AllDay(date)) => {
            assert_eq!(date.year(), 2026);
            assert_eq!(date.month(), 7);
            assert_eq!(date.day(), 4);
        }
        _ => panic!("Expected AllDay date type"),
    }

    cleanup_test_env();
}

#[test]
#[serial]
fn test_full_chain_create_export_import() {
    setup_test_env("full_chain");

    // Simulate real user workflow: create tasks → export → import to different calendar

    let original_href = "local://my-personal-tasks";
    let backup_href = "local://backup-calendar";

    // 1. CREATE: User creates several tasks over time
    let mut shopping = Task::new("Buy groceries", &HashMap::new(), None);
    shopping.priority = 3;

    let mut meeting = Task::new("Team meeting", &HashMap::new(), None);
    meeting.due = Some(DateType::Specific(
        Utc.with_ymd_and_hms(2026, 2, 20, 15, 0, 0).unwrap(),
    ));
    meeting.alarms.push(Alarm::new_relative(15));

    let mut workout = Task::new("Gym session", &HashMap::new(), None);
    workout.rrule = Some("FREQ=WEEKLY;BYDAY=MO,WE,FR".to_string());

    let all_tasks = vec![shopping, meeting, workout];
    LocalStorage::save_for_href(original_href, &all_tasks).unwrap();

    // 2. EXPORT: User exports their calendar
    let saved_tasks = LocalStorage::load_for_href(original_href).unwrap();
    assert_eq!(saved_tasks.len(), 3);
    let exported_ics = LocalStorage::to_ics_string(&saved_tasks);

    // Verify export contains all tasks
    assert!(exported_ics.contains("Buy groceries"));
    assert!(exported_ics.contains("Team meeting"));
    assert!(exported_ics.contains("Gym session"));

    // 3. IMPORT: User imports to new device/calendar
    let import_result = LocalStorage::import_from_ics(backup_href, &exported_ics);
    assert!(import_result.is_ok());
    assert_eq!(import_result.unwrap(), 3);

    // 4. VERIFY: All data preserved
    let restored_tasks = LocalStorage::load_for_href(backup_href).unwrap();
    assert_eq!(restored_tasks.len(), 3);

    // Find each task and verify
    let shopping_restored = restored_tasks
        .iter()
        .find(|t| t.summary == "Buy groceries")
        .unwrap();
    assert_eq!(shopping_restored.priority, 3);

    let meeting_restored = restored_tasks
        .iter()
        .find(|t| t.summary == "Team meeting")
        .unwrap();
    assert!(meeting_restored.due.is_some());
    assert!(!meeting_restored.alarms.is_empty());

    let workout_restored = restored_tasks
        .iter()
        .find(|t| t.summary == "Gym session")
        .unwrap();
    assert_eq!(
        workout_restored.rrule,
        Some("FREQ=WEEKLY;BYDAY=MO,WE,FR".to_string())
    );

    cleanup_test_env();
}

#[test]
#[serial]
fn test_export_then_merge_import() {
    setup_test_env("export_merge");

    // Test exporting from one calendar and importing into another that already has tasks

    let calendar_a = "local://calendar-a";
    let calendar_b = "local://calendar-b";

    // Calendar A has 2 tasks
    let task_a1 = Task::new("Task A1", &HashMap::new(), None);
    let task_a2 = Task::new("Task A2", &HashMap::new(), None);
    LocalStorage::save_for_href(calendar_a, &[task_a1, task_a2]).unwrap();

    // Calendar B already has 1 task
    let task_b1 = Task::new("Task B1", &HashMap::new(), None);
    LocalStorage::save_for_href(calendar_b, &[task_b1]).unwrap();

    // Export from A
    let tasks_a = LocalStorage::load_for_href(calendar_a).unwrap();
    let ics_a = LocalStorage::to_ics_string(&tasks_a);

    // Import A's tasks into B (merge)
    LocalStorage::import_from_ics(calendar_b, &ics_a).unwrap();

    // Verify B now has 3 tasks
    let tasks_b = LocalStorage::load_for_href(calendar_b).unwrap();
    assert_eq!(tasks_b.len(), 3);

    let summaries: Vec<&str> = tasks_b.iter().map(|t| t.summary.as_str()).collect();
    assert!(summaries.contains(&"Task A1"));
    assert!(summaries.contains(&"Task A2"));
    assert!(summaries.contains(&"Task B1"));

    cleanup_test_env();
}
