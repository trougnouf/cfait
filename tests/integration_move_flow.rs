use cfait::context::TestContext;
use cfait::journal::{Action, Journal};
use cfait::model::Task;
use std::collections::HashMap;
use std::sync::Mutex; // Fix for E0425/E0433

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

    fn move_task(
        &self,
        task: &Task,
        new_calendar_href: &str,
    ) -> Result<(Task, Vec<String>), String> {
        let mut map = self.calendars.lock().unwrap();

        let source_href = task.calendar_href.clone();
        if let Some(vec) = map.get_mut(&source_href) {
            // Fix for E0282: Added explicit type &Task
            if let Some(pos) = vec.iter().position(|t: &Task| t.uid == task.uid) {
                vec.remove(pos);
            }
        }

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
    let old_href = "cal://old";
    let new_href = "cal://new";
    let mut original = Task::new("Test Task", &HashMap::new(), None);
    original.uid = "task-1".to_string();
    original.calendar_href = old_href.to_string();

    let client = FakeClient::new().with_calendar(old_href, vec![original.clone()]);

    let mut mutated = original.clone();
    mutated.calendar_href = new_href.to_string();

    let _res = client
        .move_task(&mutated, new_href)
        .expect("move should succeed");

    let old_tasks = client.get_tasks(old_href).expect("fetch old");
    let new_tasks = client.get_tasks(new_href).expect("fetch new");

    assert!(old_tasks.iter().any(|t| t.uid == original.uid));
    assert!(new_tasks.iter().any(|t| t.uid == original.uid));
}

#[test]
fn fake_client_move_with_original_task_moves_cleanly() {
    let old_href = "cal://old";
    let new_href = "cal://new";
    let mut original = Task::new("Test Task", &HashMap::new(), None);
    original.uid = "task-2".to_string();
    original.calendar_href = old_href.to_string();

    let client = FakeClient::new().with_calendar(old_href, vec![original.clone()]);

    let (_updated, _msgs) = client
        .move_task(&original, new_href)
        .expect("move should succeed");

    let old_tasks = client.get_tasks(old_href).expect("fetch old");
    let new_tasks = client.get_tasks(new_href).expect("fetch new");

    assert!(!old_tasks.iter().any(|t| t.uid == original.uid));
    assert!(new_tasks.iter().any(|t| t.uid == original.uid));
}

#[test]
fn journal_apply_move_removes_from_source_and_adds_to_target() {
    let ctx = TestContext::new();
    let old_href = "cal://old_journal";
    let new_href = "cal://new_journal";

    let mut task = Task::new("Journal Move Task", &HashMap::new(), None);
    task.uid = "journal-task-1".to_string();
    task.calendar_href = old_href.to_string();

    let mut old_tasks = vec![task.clone()];
    let mut new_tasks: Vec<Task> = Vec::new();

    let _ = Journal::modify(&ctx, |q: &mut Vec<Action>| q.clear());
    let _ = Journal::push(&ctx, Action::Move(task.clone(), new_href.to_string()));

    Journal::apply_to_tasks(&ctx, &mut old_tasks, old_href);
    assert!(!old_tasks.iter().any(|t| t.uid == task.uid));

    Journal::apply_to_tasks(&ctx, &mut new_tasks, new_href);
    assert!(new_tasks.iter().any(|t| t.uid == task.uid));

    let inserted = new_tasks.iter().find(|t| t.uid == task.uid).unwrap();
    assert_eq!(inserted.calendar_href, new_href);
}
