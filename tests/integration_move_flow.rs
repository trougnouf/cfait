use std::collections::HashMap;
use std::sync::Mutex;
use std::env;
use std::fs;

use cfait::journal::{Action, Journal};
use cfait::model::Task;

/// Add global lock for env var manipulation
static TEST_MUTEX: Mutex<()> = Mutex::new(());

fn setup_env(suffix: &str) -> std::path::PathBuf {
    let temp_dir = env::temp_dir().join(format!("cfait_test_move_{}_{}", suffix, std::process::id()));
    let _ = fs::create_dir_all(&temp_dir);

    // UNSAFE: modifying process environment
    unsafe {
        env::set_var("CFAIT_TEST_DIR", &temp_dir);
    }

    // Clear potential previous run
    if let Some(p) = Journal::get_path() && p.exists() {
        let _ = fs::remove_file(p);
    }
    temp_dir
}

fn teardown(path: std::path::PathBuf) {
    unsafe {
        env::remove_var("CFAIT_TEST_DIR");
    }
    let _ = fs::remove_dir_all(path);
}

/// A small test-double that simulates the server-side behavior of `RustyClient`
/// for the narrow purpose of testing move semantics. The fake client keeps an
/// in-memory map of calendar href -> Vec<Task> and implements `move_task` and
/// `get_tasks` used in the network flow.
struct FakeClient {
    calendars: Mutex<HashMap<String, Vec<Task>>>,
}

impl FakeClient {
    fn new() -> Self {
        Self {
            calendars: Mutex::new(HashMap::new()),
        }
    }

    fn with_calendar(self, href: &str, tasks: Vec<Task>) -> Self {
        let mut map = self.calendars.lock().unwrap();
        map.insert(href.to_string(), tasks);
        drop(map);
        self
    }

    /// Simulates the server `move_task` behavior: uses the provided task's
    /// `calendar_href` to identify the source calendar to remove from, then
    /// inserts a copy (with updated calendar_href) into the target calendar.
    /// Returns the updated task as the real client would.
    fn move_task(
        &self,
        task: &Task,
        new_calendar_href: &str,
    ) -> Result<(Task, Vec<String>), String> {
        let mut map = self.calendars.lock().unwrap();

        let source_href = task.calendar_href.clone();
        // Attempt to remove from source calendar based on the uid.
        if let Some(vec) = map.get_mut(&source_href)
            && let Some(pos) = vec.iter().position(|t| t.uid == task.uid)
        {
            vec.remove(pos);
        }

        // Produce updated task and insert into target
        let mut updated = task.clone();
        updated.calendar_href = new_calendar_href.to_string();

        map.entry(new_calendar_href.to_string())
            .or_default()
            .push(updated.clone());

        Ok((updated, vec![]))
    }

    fn get_tasks(&self, href: &str) -> Result<Vec<Task>, String> {
        let map = self.calendars.lock().unwrap();
        Ok(map.get(href).cloned().unwrap_or_else(Vec::new))
    }
}

#[test]
fn fake_client_move_with_mutated_task_creates_duplicate() {
    // Setup: original task lives in old calendar
    let old_href = "cal://old";
    let new_href = "cal://new";
    let mut original = Task::new("Test Task", &HashMap::new(), None);
    original.uid = "task-1".to_string();
    original.calendar_href = old_href.to_string();

    // Fake client seeded with the task in the old calendar
    let client = FakeClient::new().with_calendar(old_href, vec![original.clone()]);

    // Simulate the UI/store bug: pass a mutated task (calendar_href already set to target)
    let mut mutated = original.clone();
    mutated.calendar_href = new_href.to_string();

    // Perform move (server looks at task.calendar_href to determine source => mutated points to new)
    let _res = client
        .move_task(&mutated, new_href)
        .expect("move should succeed");

    // After such a move, the fake server will NOT have removed from the old calendar,
    // because it used `mutated.calendar_href` (which equals new_href) to find the source.
    let old_tasks = client.get_tasks(old_href).expect("fetch old");
    let new_tasks = client.get_tasks(new_href).expect("fetch new");

    // Old still contains the task (duplicate)
    assert!(
        old_tasks.iter().any(|t| t.uid == original.uid),
        "Old calendar should still contain the task (duplicate case)"
    );

    // New also contains the moved task
    assert!(
        new_tasks.iter().any(|t| t.uid == original.uid),
        "New calendar should contain the task after move"
    );
}

#[test]
fn fake_client_move_with_original_task_moves_cleanly() {
    let old_href = "cal://old";
    let new_href = "cal://new";
    let mut original = Task::new("Test Task", &HashMap::new(), None);
    original.uid = "task-2".to_string();
    original.calendar_href = old_href.to_string();

    let client = FakeClient::new().with_calendar(old_href, vec![original.clone()]);

    // Call move using the original (pre-mutation) task
    let (_updated, _msgs) = client
        .move_task(&original, new_href)
        .expect("move should succeed");

    // After a correct call, the fake server should have removed from the old calendar
    // and added to the new calendar.
    let old_tasks = client.get_tasks(old_href).expect("fetch old");
    let new_tasks = client.get_tasks(new_href).expect("fetch new");

    assert!(
        !old_tasks.iter().any(|t| t.uid == original.uid),
        "Old calendar should not contain the task after proper move"
    );
    assert!(
        new_tasks.iter().any(|t| t.uid == original.uid),
        "New calendar should contain the task after proper move"
    );
}

#[test]
fn journal_apply_move_removes_from_source_and_adds_to_target() {
    // Acquire lock to modify environment safely
    let _guard = TEST_MUTEX.lock().unwrap();
    let temp_dir = setup_env("journal_apply");

    // This test ensures the Journal's `apply_to_tasks` behaves correctly when
    // a Move action with the original (pre-mutation) task is present.
    let old_href = "cal://old_journal";
    let new_href = "cal://new_journal";

    // Create a Task in old calendar
    let mut task = Task::new("Journal Move Task", &HashMap::new(), None);
    task.uid = "journal-task-1".to_string();
    task.calendar_href = old_href.to_string();

    // Simulate fetched tasks for old calendar (what the client would return)
    let mut old_tasks = vec![task.clone()];
    let mut new_tasks: Vec<Task> = Vec::new();

    // Clear any existing journal actions for test isolation
    let _ = Journal::modify(|q| q.clear());

    // Push a Move action with the ORIGINAL task (pre-mutation)
    let _ = Journal::push(Action::Move(task.clone(), new_href.to_string()));

    // Apply journal to old calendar tasks: should remove the moved task
    Journal::apply_to_tasks(&mut old_tasks, old_href);
    assert!(
        !old_tasks.iter().any(|t| t.uid == task.uid),
        "Old calendar tasks should have had the moved task removed by Journal::apply_to_tasks"
    );

    // Apply journal to new calendar tasks: should insert the moved task with updated calendar_href
    Journal::apply_to_tasks(&mut new_tasks, new_href);
    assert!(
        new_tasks.iter().any(|t| t.uid == task.uid),
        "New calendar tasks should contain the moved task after Journal::apply_to_tasks"
    );
    // Confirm inserted task has calendar_href updated
    let inserted = new_tasks.iter().find(|t| t.uid == task.uid).unwrap();
    assert_eq!(
        inserted.calendar_href, new_href,
        "Inserted task should have calendar_href set to target"
    );

    // Cleanup journal so other tests aren't affected
    let _ = Journal::modify(|q| q.clear());

    teardown(temp_dir);
}
