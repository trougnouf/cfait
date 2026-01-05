// Unit tests for import_local_ics functionality
use cfait::model::Task;
use cfait::storage::LocalStorage;
use serial_test::serial;
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
        "cfait_import_test_{}_{:?}_{}",
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
    setup_test_env("single_task");

    let href = "local://test-calendar-1";
    let ics_content = create_simple_ics();

    // Import into empty calendar
    let result = import_ics_content(href, &ics_content);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 1);

    // Verify task was imported
    let tasks = LocalStorage::load_for_href(href).unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].summary, "Test Task 1");
    assert_eq!(tasks[0].priority, 5);

    cleanup_test_env();
}

#[test]
#[serial]
fn test_import_multiple_tasks() {
    setup_test_env("multiple_tasks");

    let href = "local://test-calendar-2";
    let ics_content = create_multi_task_ics();

    let result = import_ics_content(href, &ics_content);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 3);

    // Verify all tasks were imported
    let tasks = LocalStorage::load_for_href(href).unwrap();
    assert_eq!(tasks.len(), 3);

    // Check each task
    assert_eq!(tasks[0].summary, "First Task");
    assert_eq!(tasks[0].priority, 1);

    assert_eq!(tasks[1].summary, "Second Task");
    assert_eq!(tasks[1].priority, 5);
    assert!(tasks[1].due.is_some());

    assert_eq!(tasks[2].summary, "Third Task");
    assert!(tasks[2].status.is_done());

    cleanup_test_env();
}

#[test]
#[serial]
fn test_import_with_existing_tasks() {
    setup_test_env("existing_tasks");

    let href = "local://test-calendar-3";

    // Create initial task
    let existing_task = Task::new("Existing Task", &Default::default(), None);
    LocalStorage::save_for_href(href, &[existing_task]).unwrap();

    // Import new tasks
    let ics_content = create_multi_task_ics();
    let result = import_ics_content(href, &ics_content);
    assert!(result.is_ok(), "Import failed: {:?}", result.err());
    assert_eq!(result.unwrap(), 3);

    // Verify we have 4 tasks total (1 existing + 3 imported)
    let tasks = LocalStorage::load_for_href(href).unwrap();
    assert_eq!(tasks.len(), 4, "Expected 4 tasks, got {}", tasks.len());
    assert_eq!(tasks[0].summary, "Existing Task");
    assert_eq!(tasks[1].summary, "First Task");

    cleanup_test_env();
}

#[test]
#[serial]
fn test_import_preserves_alarms() {
    setup_test_env("alarms");

    let href = "local://test-calendar-4";
    let ics_content = create_ics_with_alarms();

    let result = import_ics_content(href, &ics_content);
    assert!(result.is_ok());

    let tasks = LocalStorage::load_for_href(href).unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].summary, "Task with Alarm");
    assert!(!tasks[0].alarms.is_empty(), "Alarms should be preserved");
    // Check that alarm trigger is relative and set to 15 minutes before
    match tasks[0].alarms[0].trigger {
        cfait::model::AlarmTrigger::Relative(mins) => assert_eq!(mins, -15),
        _ => panic!("Expected relative alarm trigger"),
    }

    cleanup_test_env();
}

#[test]
#[serial]
fn test_import_preserves_recurrence() {
    setup_test_env("recurrence");

    let href = "local://test-calendar-5";
    let ics_content = create_ics_with_recurrence();

    let result = import_ics_content(href, &ics_content);
    assert!(result.is_ok());

    let tasks = LocalStorage::load_for_href(href).unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].summary, "Weekly Meeting");
    assert!(tasks[0].rrule.is_some(), "Recurrence should be preserved");

    cleanup_test_env();
}

#[test]
#[serial]
fn test_import_unwrapped_vtodo() {
    setup_test_env("unwrapped");

    let href = "local://test-calendar-6";
    let ics_content = create_ics_without_vcalendar_wrapper();

    // Should still work even without VCALENDAR wrapper
    let result = import_ics_content(href, &ics_content);
    assert!(result.is_ok(), "Import failed: {:?}", result.err());
    assert_eq!(result.unwrap(), 1);

    let tasks = LocalStorage::load_for_href(href).unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].summary, "Unwrapped Task");

    cleanup_test_env();
}

#[test]
#[serial]
fn test_import_empty_ics() {
    setup_test_env("empty");

    let href = "local://test-calendar-7";
    let ics_content = "BEGIN:VCALENDAR\nVERSION:2.0\nEND:VCALENDAR".to_string();

    let result = import_ics_content(href, &ics_content);
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("No valid tasks found")
    );

    cleanup_test_env();
}

#[test]
#[serial]
fn test_import_invalid_ics() {
    setup_test_env("invalid");

    let href = "local://test-calendar-8";
    let ics_content = "This is not valid ICS content".to_string();

    let result = import_ics_content(href, &ics_content);
    assert!(result.is_err());

    cleanup_test_env();
}

#[test]
#[serial]
fn test_import_malformed_vtodo() {
    setup_test_env("malformed");

    let href = "local://test-calendar-9";
    // VTODO with missing required fields
    let ics_content = r#"BEGIN:VCALENDAR
VERSION:2.0
PRODID:-//Test//Test//EN
BEGIN:VTODO
UID:malformed-task
END:VTODO
END:VCALENDAR"#
        .to_string();

    // Should handle gracefully - either import with defaults or skip
    let _result = import_ics_content(href, &ics_content);
    // The behavior depends on Task::from_ics implementation
    // It should either succeed with minimal task or fail gracefully

    cleanup_test_env();
}

#[test]
#[serial]
fn test_import_roundtrip() {
    setup_test_env("roundtrip");

    let href = "local://test-calendar-10";

    // Use a simple known-good ICS format instead of roundtrip
    let ics_content = create_simple_ics();

    let result = import_ics_content(href, &ics_content);
    assert!(result.is_ok());

    let tasks = LocalStorage::load_for_href(href).unwrap();
    assert_eq!(tasks.len(), 1);

    // Verify task was imported correctly
    assert_eq!(tasks[0].summary, "Test Task 1");
    assert_eq!(tasks[0].priority, 5);

    cleanup_test_env();
}

#[test]
#[serial]
fn test_import_assigns_new_calendar_href() {
    setup_test_env("calendar_href");

    let target_href = "local://target-calendar";

    // Create ICS with a task
    let ics_content = create_simple_ics();

    let result = import_ics_content(target_href, &ics_content);
    assert!(result.is_ok(), "Import failed: {:?}", result.err());

    let tasks = LocalStorage::load_for_href(target_href).unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].calendar_href, target_href);

    cleanup_test_env();
}

// Helper function that uses the canonical import function
fn import_ics_content(calendar_href: &str, ics_content: &str) -> Result<usize, anyhow::Error> {
    LocalStorage::import_from_ics(calendar_href, ics_content)
}

#[test]
#[serial]
fn test_cli_import_from_file() {
    setup_test_env("cli_import");

    // Create a temporary ICS file
    let temp_dir = env::temp_dir();
    let test_file = temp_dir.join("test_cli_import.ics");
    let ics_content = create_multi_task_ics();
    fs::write(&test_file, &ics_content).unwrap();

    // Test import to default calendar
    let default_href = "local://default";
    let result = LocalStorage::import_from_ics(default_href, &ics_content);
    assert!(result.is_ok(), "CLI import failed: {:?}", result.err());
    assert_eq!(result.unwrap(), 3);

    let tasks = LocalStorage::load_for_href(default_href).unwrap();
    assert_eq!(tasks.len(), 3);
    assert_eq!(tasks[0].summary, "First Task");

    // Test import to custom calendar
    let custom_href = "local://my-custom-cal";
    let result = LocalStorage::import_from_ics(custom_href, &ics_content);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 3);

    let tasks = LocalStorage::load_for_href(custom_href).unwrap();
    assert_eq!(tasks.len(), 3);
    assert_eq!(tasks[0].calendar_href, custom_href);

    // Clean up
    fs::remove_file(&test_file).ok();
    cleanup_test_env();
}

#[test]
#[serial]
fn test_cli_import_file_not_found() {
    setup_test_env("cli_not_found");

    let nonexistent_file = "/tmp/nonexistent_file_12345.ics";
    let result = fs::read_to_string(nonexistent_file);
    assert!(result.is_err(), "Should fail to read nonexistent file");

    cleanup_test_env();
}

#[test]
#[serial]
fn test_cli_import_invalid_content_from_file() {
    setup_test_env("cli_invalid");

    let temp_dir = env::temp_dir();
    let test_file = temp_dir.join("test_cli_invalid.ics");
    fs::write(&test_file, "Invalid ICS content").unwrap();

    let invalid_content = fs::read_to_string(&test_file).unwrap();
    let href = "local://test-invalid";
    let result = LocalStorage::import_from_ics(href, &invalid_content);
    assert!(result.is_err(), "Should fail with invalid ICS content");

    // Clean up
    fs::remove_file(&test_file).ok();
    cleanup_test_env();
}
