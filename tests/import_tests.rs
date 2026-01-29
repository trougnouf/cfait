use cfait::context::TestContext;
use cfait::model::Task;
use cfait::storage::LocalStorage;
use serial_test::serial;
use std::fs;

fn create_simple_ics() -> String {
    r#"BEGIN:VCALENDAR
VERSION:2.0
PRODID:-//Test//Test//EN
BEGIN:VTODO
UID:test-task-1
SUMMARY:Test Task 1
STATUS:NEEDS-ACTION
PRIORITY:5
END:VTODO
END:VCALENDAR"#
        .to_string()
}

fn create_multi_task_ics() -> String {
    r#"BEGIN:VCALENDAR
VERSION:2.0
PRODID:-//Test//Test//EN
BEGIN:VTODO
UID:test-task-1
SUMMARY:First Task
STATUS:NEEDS-ACTION
PRIORITY:1
END:VTODO
BEGIN:VTODO
UID:test-task-2
SUMMARY:Second Task
STATUS:NEEDS-ACTION
PRIORITY:5
DUE:20260215T140000Z
END:VTODO
BEGIN:VTODO
UID:test-task-3
SUMMARY:Third Task
STATUS:COMPLETED
COMPLETED:20260101T120000Z
END:VTODO
END:VCALENDAR"#
        .to_string()
}

fn create_ics_with_alarms() -> String {
    r#"BEGIN:VCALENDAR
VERSION:2.0
PRODID:-//cfait//cfait//EN
BEGIN:VTODO
UID:alarm-task-1
SUMMARY:Task with Alarm
DUE:20260215T140000Z
STATUS:NEEDS-ACTION
BEGIN:VALARM
ACTION:DISPLAY
TRIGGER:-PT15M
DESCRIPTION:Reminder
END:VALARM
END:VTODO
END:VCALENDAR"#
        .to_string()
}

fn create_ics_with_recurrence() -> String {
    r#"BEGIN:VCALENDAR
VERSION:2.0
PRODID:-//cfait//cfait//EN
BEGIN:VTODO
UID:recurring-task-1
SUMMARY:Weekly Meeting
STATUS:NEEDS-ACTION
DTSTART:20260101T100000Z
RRULE:FREQ=WEEKLY;BYDAY=MO
END:VTODO
END:VCALENDAR"#
        .to_string()
}

fn create_ics_without_vcalendar_wrapper() -> String {
    r#"BEGIN:VTODO
UID:unwrapped-task-1
SUMMARY:Unwrapped Task
STATUS:NEEDS-ACTION
END:VTODO"#
        .to_string()
}

#[test]
#[serial]
fn test_import_single_task() {
    let ctx = TestContext::new();
    let href = "local://test-calendar-1";
    let ics_content = create_simple_ics();

    let result = LocalStorage::import_from_ics(&ctx, href, &ics_content);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 1);

    let tasks = LocalStorage::load_for_href(&ctx, href).unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].summary, "Test Task 1");
    assert_eq!(tasks[0].priority, 5);
}

#[test]
#[serial]
fn test_import_multiple_tasks() {
    let ctx = TestContext::new();
    let href = "local://test-calendar-2";
    let ics_content = create_multi_task_ics();

    let result = LocalStorage::import_from_ics(&ctx, href, &ics_content);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 3);

    let tasks = LocalStorage::load_for_href(&ctx, href).unwrap();
    assert_eq!(tasks.len(), 3);
    assert_eq!(tasks[0].summary, "First Task");
    assert_eq!(tasks[0].priority, 1);
}

#[test]
#[serial]
fn test_import_with_existing_tasks() {
    let ctx = TestContext::new();
    let href = "local://test-calendar-3";

    let existing_task = Task::new("Existing Task", &Default::default(), None);
    LocalStorage::save_for_href(&ctx, href, &[existing_task]).unwrap();

    let ics_content = create_multi_task_ics();
    let result = LocalStorage::import_from_ics(&ctx, href, &ics_content);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 3);

    let tasks = LocalStorage::load_for_href(&ctx, href).unwrap();
    assert_eq!(tasks.len(), 4);
}

#[test]
#[serial]
fn test_import_preserves_alarms() {
    let ctx = TestContext::new();
    let href = "local://test-calendar-4";
    let ics_content = create_ics_with_alarms();

    let result = LocalStorage::import_from_ics(&ctx, href, &ics_content);
    assert!(result.is_ok());

    let tasks = LocalStorage::load_for_href(&ctx, href).unwrap();
    assert_eq!(tasks.len(), 1);
    assert!(!tasks[0].alarms.is_empty());
}

#[test]
#[serial]
fn test_import_preserves_recurrence() {
    let ctx = TestContext::new();
    let href = "local://test-calendar-5";
    let ics_content = create_ics_with_recurrence();

    let result = LocalStorage::import_from_ics(&ctx, href, &ics_content);
    assert!(result.is_ok());

    let tasks = LocalStorage::load_for_href(&ctx, href).unwrap();
    assert_eq!(tasks.len(), 1);
    assert!(tasks[0].rrule.is_some());
}

#[test]
#[serial]
fn test_import_unwrapped_vtodo() {
    let ctx = TestContext::new();
    let href = "local://test-calendar-6";
    let ics_content = create_ics_without_vcalendar_wrapper();

    let result = LocalStorage::import_from_ics(&ctx, href, &ics_content);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 1);

    let tasks = LocalStorage::load_for_href(&ctx, href).unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].summary, "Unwrapped Task");
}

#[test]
#[serial]
fn test_import_empty_ics() {
    let ctx = TestContext::new();
    let href = "local://test-calendar-7";
    let ics_content = "BEGIN:VCALENDAR\nVERSION:2.0\nEND:VCALENDAR".to_string();

    let result = LocalStorage::import_from_ics(&ctx, href, &ics_content);
    assert!(result.is_err());
}

#[test]
#[serial]
fn test_import_invalid_ics() {
    let ctx = TestContext::new();
    let href = "local://test-calendar-8";
    let ics_content = "This is not valid ICS content".to_string();

    let result = LocalStorage::import_from_ics(&ctx, href, &ics_content);
    assert!(result.is_err());
}

#[test]
#[serial]
fn test_import_assigns_new_calendar_href() {
    let ctx = TestContext::new();
    let target_href = "local://target-calendar";
    let ics_content = create_simple_ics();

    let result = LocalStorage::import_from_ics(&ctx, target_href, &ics_content);
    assert!(result.is_ok());

    let tasks = LocalStorage::load_for_href(&ctx, target_href).unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].calendar_href, target_href);
}

#[test]
#[serial]
fn test_cli_import_from_file() {
    let ctx = TestContext::new();
    let temp_dir = std::env::temp_dir();
    let test_file = temp_dir.join("test_cli_import.ics");
    let ics_content = create_multi_task_ics();
    fs::write(&test_file, &ics_content).unwrap();

    let default_href = "local://default";
    let result = LocalStorage::import_from_ics(&ctx, default_href, &ics_content);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 3);

    let tasks = LocalStorage::load_for_href(&ctx, default_href).unwrap();
    assert_eq!(tasks.len(), 3);

    fs::remove_file(&test_file).ok();
}
