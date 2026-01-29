use cfait::context::TestContext;
use cfait::model::{Alarm, DateType, Task, TaskStatus};
use cfait::storage::LocalStorage;
use chrono::{Datelike, TimeZone, Utc};
use serial_test::serial;
use std::collections::HashMap;

// ==================== Export Tests ====================

#[test]
#[serial]
fn test_export_single_task() {
    let task = Task::new("Test Task", &HashMap::new(), None);
    let tasks = vec![task];

    let ics = LocalStorage::to_ics_string(&tasks);

    assert!(ics.contains("BEGIN:VCALENDAR"));
    assert!(ics.contains("SUMMARY:Test Task"));
    assert_eq!(ics.matches("BEGIN:VTODO").count(), 1);
}

#[test]
#[serial]
fn test_export_multiple_tasks() {
    let task1 = Task::new("Task One", &HashMap::new(), None);
    let task2 = Task::new("Task Two", &HashMap::new(), None);
    let task3 = Task::new("Task Three", &HashMap::new(), None);
    let tasks = vec![task1, task2, task3];

    let ics = LocalStorage::to_ics_string(&tasks);

    assert_eq!(ics.matches("BEGIN:VTODO").count(), 3);
    assert!(ics.contains("SUMMARY:Task One"));
}

#[test]
#[serial]
fn test_export_empty_task_list() {
    let tasks: Vec<Task> = vec![];
    let ics = LocalStorage::to_ics_string(&tasks);
    assert!(ics.contains("BEGIN:VCALENDAR"));
    assert_eq!(ics.matches("BEGIN:VTODO").count(), 0);
}

#[test]
#[serial]
fn test_export_task_with_due_date() {
    let mut task = Task::new("Task with Due", &HashMap::new(), None);
    task.due = Some(DateType::Specific(
        Utc.with_ymd_and_hms(2026, 3, 15, 14, 30, 0).unwrap(),
    ));
    let tasks = vec![task];
    let ics = LocalStorage::to_ics_string(&tasks);
    assert!(ics.contains("DUE:20260315T143000Z"));
}

#[test]
#[serial]
fn test_export_task_with_alarm() {
    let mut task = Task::new("Task with Alarm", &HashMap::new(), None);
    task.due = Some(DateType::Specific(
        Utc.with_ymd_and_hms(2026, 3, 15, 14, 0, 0).unwrap(),
    ));
    task.alarms.push(Alarm::new_relative(15));
    let tasks = vec![task];

    let ics = LocalStorage::to_ics_string(&tasks);
    assert!(ics.contains("BEGIN:VALARM"));
    assert!(ics.contains("TRIGGER:-PT15M"));
}

#[test]
#[serial]
fn test_export_task_with_priority() {
    let mut task = Task::new("High Priority Task", &HashMap::new(), None);
    task.priority = 1;
    let tasks = vec![task];
    let ics = LocalStorage::to_ics_string(&tasks);
    assert!(ics.contains("PRIORITY:1"));
}

#[test]
#[serial]
fn test_export_completed_task() {
    let mut task = Task::new("Completed Task", &HashMap::new(), None);
    task.status = TaskStatus::Completed;
    task.percent_complete = Some(100);
    let tasks = vec![task];
    let ics = LocalStorage::to_ics_string(&tasks);
    assert!(ics.contains("STATUS:COMPLETED"));
}

#[test]
#[serial]
fn test_export_task_with_recurrence() {
    let mut task = Task::new("Weekly Task", &HashMap::new(), None);
    task.rrule = Some("FREQ=WEEKLY;BYDAY=MO,WE,FR".to_string());
    let tasks = vec![task];
    let ics = LocalStorage::to_ics_string(&tasks);
    assert!(ics.contains("RRULE:FREQ=WEEKLY;BYDAY=MO,WE,FR"));
}

// ==================== Full Roundtrip Tests ====================

#[test]
#[serial]
fn test_roundtrip_simple_task() {
    let ctx = TestContext::new();
    let href = "local://roundtrip-calendar-1";

    // 1. Create task
    let original = Task::new("Roundtrip Task", &HashMap::new(), None);
    LocalStorage::save_for_href(&ctx, href, std::slice::from_ref(&original)).unwrap();

    // 2. Export
    let tasks = LocalStorage::load_for_href(&ctx, href).unwrap();
    let ics = LocalStorage::to_ics_string(&tasks);

    // 3. Import to new calendar
    let import_href = "local://roundtrip-import-1";
    let result = LocalStorage::import_from_ics(&ctx, import_href, &ics);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 1);

    // 4. Verify
    let imported_tasks = LocalStorage::load_for_href(&ctx, import_href).unwrap();
    assert_eq!(imported_tasks.len(), 1);
    assert_eq!(imported_tasks[0].summary, original.summary);
    assert_eq!(imported_tasks[0].calendar_href, import_href);
}

#[test]
#[serial]
fn test_roundtrip_multiple_tasks() {
    let ctx = TestContext::new();
    let href = "local://roundtrip-calendar-2";

    let task1 = Task::new("First Task", &HashMap::new(), None);
    let task2 = Task::new("Second Task", &HashMap::new(), None);
    let task3 = Task::new("Third Task", &HashMap::new(), None);
    LocalStorage::save_for_href(&ctx, href, &[task1, task2, task3]).unwrap();

    let tasks = LocalStorage::load_for_href(&ctx, href).unwrap();
    assert_eq!(tasks.len(), 3);
    let ics = LocalStorage::to_ics_string(&tasks);

    let import_href = "local://roundtrip-import-2";
    let result = LocalStorage::import_from_ics(&ctx, import_href, &ics);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 3);

    let imported_tasks = LocalStorage::load_for_href(&ctx, import_href).unwrap();
    assert_eq!(imported_tasks.len(), 3);
}

#[test]
#[serial]
fn test_priority_clamp_export_and_import() {
    let ctx = TestContext::new();

    let mut task = Task::new("Prio Export Task", &HashMap::new(), None);
    task.priority = 15;
    let tasks = vec![task];
    let ics = LocalStorage::to_ics_string(&tasks);
    assert!(ics.contains("PRIORITY:9"));

    let ics_in = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Cfait//Export//EN\r\nBEGIN:VTODO\r\nUID:uid-prio\r\nSUMMARY:Prio Import\r\nPRIORITY:15\r\nEND:VTODO\r\nEND:VCALENDAR";
    let import_href = "local://prio-import";
    let res = LocalStorage::import_from_ics(&ctx, import_href, ics_in);
    assert!(res.is_ok());

    let imported = LocalStorage::load_for_href(&ctx, import_href).unwrap();
    assert_eq!(imported[0].priority, 9);
}

#[test]
#[serial]
fn test_roundtrip_task_with_all_fields() {
    let ctx = TestContext::new();
    let href = "local://roundtrip-calendar-3";

    let mut original = Task::new("Complex Task", &HashMap::new(), None);
    original.description = "This is a detailed description".to_string();
    original.priority = 2;
    original.due = Some(DateType::Specific(
        Utc.with_ymd_and_hms(2026, 6, 1, 10, 0, 0).unwrap(),
    ));
    original.alarms.push(Alarm::new_relative(30));
    original.rrule = Some("FREQ=DAILY;COUNT=10".to_string());

    LocalStorage::save_for_href(&ctx, href, &[original.clone()]).unwrap();

    let tasks = LocalStorage::load_for_href(&ctx, href).unwrap();
    let ics = LocalStorage::to_ics_string(&tasks);

    let import_href = "local://roundtrip-import-3";
    LocalStorage::import_from_ics(&ctx, import_href, &ics).unwrap();

    let imported = &LocalStorage::load_for_href(&ctx, import_href).unwrap()[0];
    assert_eq!(imported.summary, original.summary);
    assert_eq!(imported.description, original.description);
    assert_eq!(imported.priority, original.priority);
    assert!(imported.due.is_some());
    assert_eq!(imported.rrule, original.rrule);
    assert!(!imported.alarms.is_empty());
}

#[test]
#[serial]
fn test_roundtrip_preserves_completed_status() {
    let ctx = TestContext::new();
    let href = "local://roundtrip-calendar-4";

    let mut original = Task::new("Done Task", &HashMap::new(), None);
    original.status = TaskStatus::Completed;
    original.percent_complete = Some(100);

    LocalStorage::save_for_href(&ctx, href, &[original.clone()]).unwrap();

    let tasks = LocalStorage::load_for_href(&ctx, href).unwrap();
    let ics = LocalStorage::to_ics_string(&tasks);

    let import_href = "local://roundtrip-import-4";
    LocalStorage::import_from_ics(&ctx, import_href, &ics).unwrap();

    let imported = &LocalStorage::load_for_href(&ctx, import_href).unwrap()[0];
    assert!(imported.status.is_done());
    assert_eq!(imported.percent_complete, Some(100));
}

#[test]
#[serial]
fn test_roundtrip_allday_date() {
    let ctx = TestContext::new();
    let href = "local://roundtrip-calendar-5";

    let mut original = Task::new("All Day Task", &HashMap::new(), None);
    original.due = Some(DateType::AllDay(
        chrono::NaiveDate::from_ymd_opt(2026, 7, 4).unwrap(),
    ));

    LocalStorage::save_for_href(&ctx, href, &[original.clone()]).unwrap();

    let tasks = LocalStorage::load_for_href(&ctx, href).unwrap();
    let ics = LocalStorage::to_ics_string(&tasks);

    assert!(ics.contains("DUE;VALUE=DATE:20260704"));

    let import_href = "local://roundtrip-import-5";
    LocalStorage::import_from_ics(&ctx, import_href, &ics).unwrap();

    let imported = &LocalStorage::load_for_href(&ctx, import_href).unwrap()[0];
    match imported.due {
        Some(DateType::AllDay(date)) => {
            assert_eq!(date.year(), 2026);
            assert_eq!(date.month(), 7);
            assert_eq!(date.day(), 4);
        }
        _ => panic!("Expected AllDay date type"),
    }
}

#[test]
#[serial]
fn test_full_chain_create_export_import() {
    let ctx = TestContext::new();
    let original_href = "local://my-personal-tasks";
    let backup_href = "local://backup-calendar";

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
    LocalStorage::save_for_href(&ctx, original_href, &all_tasks).unwrap();

    let saved_tasks = LocalStorage::load_for_href(&ctx, original_href).unwrap();
    let exported_ics = LocalStorage::to_ics_string(&saved_tasks);

    let import_result = LocalStorage::import_from_ics(&ctx, backup_href, &exported_ics);
    assert!(import_result.is_ok());

    let restored_tasks = LocalStorage::load_for_href(&ctx, backup_href).unwrap();
    assert_eq!(restored_tasks.len(), 3);
}

#[test]
#[serial]
fn test_export_then_merge_import() {
    let ctx = TestContext::new();
    let calendar_a = "local://calendar-a";
    let calendar_b = "local://calendar-b";

    let task_a1 = Task::new("Task A1", &HashMap::new(), None);
    let task_a2 = Task::new("Task A2", &HashMap::new(), None);
    LocalStorage::save_for_href(&ctx, calendar_a, &[task_a1, task_a2]).unwrap();

    let task_b1 = Task::new("Task B1", &HashMap::new(), None);
    LocalStorage::save_for_href(&ctx, calendar_b, &[task_b1]).unwrap();

    let tasks_a = LocalStorage::load_for_href(&ctx, calendar_a).unwrap();
    let ics_a = LocalStorage::to_ics_string(&tasks_a);

    LocalStorage::import_from_ics(&ctx, calendar_b, &ics_a).unwrap();

    let tasks_b = LocalStorage::load_for_href(&ctx, calendar_b).unwrap();
    assert_eq!(tasks_b.len(), 3);
}
