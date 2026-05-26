// SPDX-License-Identifier: GPL-3.0-or-later
//! Tests for store behavior.
use cfait::context::TestContext;
use cfait::model::{Task, TaskStatus};
use cfait::store::{FilterOptions, TaskStore};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

fn make_store() -> TaskStore {
    let ctx = Arc::new(TestContext::new());
    TaskStore::new(ctx)
}

#[test]
fn test_filter_by_tag() {
    let mut store = make_store();

    let mut t1 = Task::new("Work Task #work", &HashMap::new(), None);
    t1.uid = "1".to_string();
    t1.calendar_href = "cal1".to_string();

    let mut t2 = Task::new("Home Task #home", &HashMap::new(), None);
    t2.uid = "2".to_string();
    t2.calendar_href = "cal1".to_string();

    store.add_task(t1);
    store.add_task(t2);

    let mut cats = HashSet::new();
    cats.insert("work".to_string());
    let empty_set = HashSet::new();

    let filter_res = store.filter(FilterOptions {
        active_cal_href: None,
        hidden_calendars: &empty_set,
        selected_categories: &cats,
        selected_locations: &empty_set,
        match_all_categories: false,
        search_term: "",
        hide_completed_global: false,
        hide_fully_completed_tags: false,
        cutoff_date: None,
        min_duration: None,
        max_duration: None,
        include_unset_duration: true,
        urgent_days: 1,
        urgent_prio: 1,
        default_priority: 5,
        start_grace_period_days: 1,
        sort_standard_by_priority: false,
        expanded_done_groups: &empty_set,

        max_done_roots: usize::MAX,
        max_done_subtasks: usize::MAX,
        tag_aliases: &HashMap::new(),
    });
    let results = filter_res.items;

    assert_eq!(results.len(), 1);
    if let cfait::store::TaskListItem::Task(task) = &results[0] {
        assert_eq!(task.summary, "Work Task");
    } else {
        panic!("Expected Task variant");
    }
}

#[test]
fn test_filter_hierarchical_tags() {
    let mut store = make_store();

    let mut t1 = Task::new("Backend #dev:backend", &HashMap::new(), None);
    t1.uid = "1".to_string();
    t1.calendar_href = "cal1".to_string();
    store.add_task(t1);

    let mut cats = HashSet::new();
    cats.insert("dev".to_string());
    let empty_set = HashSet::new();

    let filter_res = store.filter(FilterOptions {
        active_cal_href: None,
        hidden_calendars: &empty_set,
        selected_categories: &cats,
        selected_locations: &empty_set,
        match_all_categories: false,
        search_term: "",
        hide_completed_global: false,
        hide_fully_completed_tags: false,
        cutoff_date: None,
        min_duration: None,
        max_duration: None,
        include_unset_duration: true,
        urgent_days: 1,
        urgent_prio: 1,
        default_priority: 5,
        start_grace_period_days: 1,
        sort_standard_by_priority: false,
        expanded_done_groups: &empty_set,

        max_done_roots: usize::MAX,
        max_done_subtasks: usize::MAX,
        tag_aliases: &HashMap::new(),
    });
    let results = filter_res.items;

    assert_eq!(results.len(), 1);
    if let cfait::store::TaskListItem::Task(task) = &results[0] {
        assert_eq!(task.summary, "Backend");
    } else {
        panic!("Expected Task variant");
    }
}

#[test]
fn test_hide_hidden_calendars() {
    let mut store = make_store();

    let mut t1 = Task::new("Cal 1 Task", &HashMap::new(), None);
    t1.uid = "1".to_string();
    t1.calendar_href = "cal1".to_string();

    let mut t2 = Task::new("Cal 2 Task", &HashMap::new(), None);
    t2.uid = "2".to_string();
    t2.calendar_href = "cal2".to_string();

    store.add_task(t1);
    store.add_task(t2);

    let mut hidden = HashSet::new();
    hidden.insert("cal2".to_string());
    let empty_set = HashSet::new();

    let filter_res = store.filter(FilterOptions {
        active_cal_href: None,
        hidden_calendars: &hidden,
        selected_categories: &empty_set,
        selected_locations: &empty_set,
        match_all_categories: false,
        search_term: "",
        hide_completed_global: false,
        hide_fully_completed_tags: false,
        cutoff_date: None,
        min_duration: None,
        max_duration: None,
        include_unset_duration: true,
        urgent_days: 1,
        urgent_prio: 1,
        default_priority: 5,
        start_grace_period_days: 1,
        sort_standard_by_priority: false,
        expanded_done_groups: &empty_set,

        max_done_roots: usize::MAX,
        max_done_subtasks: usize::MAX,
        tag_aliases: &HashMap::new(),
    });
    let results = filter_res.items;

    assert_eq!(results.len(), 1);
    if let cfait::store::TaskListItem::Task(task) = &results[0] {
        assert_eq!(task.summary, "Cal 1 Task");
    } else {
        panic!("Expected Task variant");
    }
}

#[test]
fn test_toggle_task_shift_advances_from_today() {
    let mut store = make_store();

    // Create a task that was originally due 10 days ago, recurring weekly.
    let mut t = Task::new("Overdue Weekly", &HashMap::new(), None);
    t.uid = "shift_test".to_string();
    t.calendar_href = "cal1".to_string();

    let now = chrono::Utc::now();
    let ten_days_ago = now - chrono::Duration::days(10);

    t.dtstart = Some(cfait::model::DateType::Specific(ten_days_ago));
    t.due = Some(cfait::model::DateType::Specific(ten_days_ago));
    t.rrule = Some("FREQ=WEEKLY".to_string());
    t.status = TaskStatus::NeedsAction;

    store.add_task(t);

    // Call toggle_task_shift (which simulates pressing Shift+Space in the UI)
    let res = store.toggle_task_shift("shift_test");
    assert!(res.is_some());
    let (history, secondary, _children) = res.unwrap();

    // Primary history should be completed
    assert_eq!(history.status, TaskStatus::Completed);

    // Secondary should be advanced to exactly ONE WEEK from TODAY.
    let advanced = secondary.expect("Expected advanced task");

    let next_due = match advanced.due.unwrap() {
        cfait::model::DateType::Specific(dt) => dt,
        _ => panic!("Expected specific date"),
    };

    let diff = next_due - now;
    assert!(diff.num_days() >= 6 && diff.num_days() <= 7, "Task should be shifted to next week from today");
}

#[test]
fn test_set_status_cancelled_advances_recurring_task() {
    let mut store = make_store();

    let mut t = Task::new("Daily Task", &HashMap::new(), None);
    t.uid = "recurring-1".to_string();
    t.calendar_href = "cal1".to_string();
    let original_due = chrono::Utc::now() - chrono::Duration::days(1);
    t.due = Some(cfait::model::DateType::Specific(original_due));
    t.rrule = Some("FREQ=DAILY".to_string());
    t.status = TaskStatus::NeedsAction;

    store.add_task(t);

    let updated = store.set_status("recurring-1", TaskStatus::Cancelled, false);
    assert!(updated.is_some());

    let (history, secondary, _children) = updated.unwrap();
    let recycled_task = secondary.expect("Recurring task should recycle into a secondary task");

    // The recycled task should be the next instance, ready for action.
    assert_eq!(recycled_task.status, TaskStatus::NeedsAction);
    // It should have accumulated the EXDATE from the cancellation.
    assert_eq!(recycled_task.exdates.len(), 1);
    assert_eq!(
        recycled_task.exdates[0],
        cfait::model::DateType::Specific(original_due)
    );

    // The history item should be a snapshot of the cancelled occurrence.
    assert_eq!(history.status, TaskStatus::Cancelled);
    assert!(history.rrule.is_none());
}

#[test]
fn test_set_status_cancelled_non_recurring_task() {
    let mut store = make_store();

    let mut t = Task::new("One-time Task", &HashMap::new(), None);
    t.uid = "one-time-1".to_string();
    t.calendar_href = "cal1".to_string();
    t.status = TaskStatus::NeedsAction;

    store.add_task(t);

    let updated = store.set_status("one-time-1", TaskStatus::Cancelled, false);
    assert!(updated.is_some());

    let (task, _secondary, _children) = updated.unwrap();
    assert_eq!(task.status, TaskStatus::Cancelled);
    assert!(task.rrule.is_none());
}

#[test]
fn test_toggle_status_cancelled_back_to_needs_action() {
    let mut store = make_store();

    let mut t = Task::new("Task", &HashMap::new(), None);
    t.uid = "toggle-1".to_string();
    t.calendar_href = "cal1".to_string();
    t.status = TaskStatus::NeedsAction;

    store.add_task(t);

    let updated = store.set_status("toggle-1", TaskStatus::Cancelled, false);
    assert!(updated.is_some());
    // FIX: Check status on the primary task from the tuple
    let (primary, _sec, _children) = updated.unwrap();
    assert_eq!(primary.status, TaskStatus::Cancelled);

    let updated = store.set_status("toggle-1", TaskStatus::Cancelled, false);
    assert!(updated.is_some());
    // FIX: Check status on the primary task from the tuple
    let (primary, _sec, _children) = updated.unwrap();
    assert_eq!(primary.status, TaskStatus::NeedsAction);
}

#[test]
fn test_taskstore_clear_and_remove() {
    let ctx = Arc::new(TestContext::new());
    let mut store = TaskStore::new(ctx);

    let mut task = Task::new("Test", &HashMap::new(), None);
    task.uid = "123".to_string();
    task.calendar_href = "cal1".to_string();
    store.add_task(task);

    assert!(store.has_any_tasks());
    store.remove("cal1");
    assert!(!store.has_any_tasks());

    let mut task2 = Task::new("Test2", &HashMap::new(), None);
    task2.uid = "456".to_string();
    task2.calendar_href = "cal1".to_string();
    store.add_task(task2);

    assert!(store.has_any_tasks());
    store.clear();
    assert!(!store.has_any_tasks());
}

#[test]
fn test_get_descendant_uids() {
    let ctx = Arc::new(TestContext::new());
    let mut store = TaskStore::new(ctx);

    let mut parent = Task::new("Parent", &HashMap::new(), None);
    parent.uid = "p1".to_string();
    store.add_task(parent);

    let mut child = Task::new("Child", &HashMap::new(), None);
    child.uid = "c1".to_string();
    child.parent_uid = Some("p1".to_string());
    store.add_task(child);

    let mut grandchild = Task::new("Grandchild", &HashMap::new(), None);
    grandchild.uid = "g1".to_string();
    grandchild.parent_uid = Some("c1".to_string());
    store.add_task(grandchild);

    let descendants = store.get_descendant_uids("p1");
    assert_eq!(descendants.len(), 2);
    assert!(descendants.contains(&"c1".to_string()));
    assert!(descendants.contains(&"g1".to_string()));
}

#[test]
fn test_task_time_tracking_intents() {
    let ctx = Arc::new(TestContext::new());
    let mut store = TaskStore::new(ctx);
    let config = cfait::config::Config::default();

    let mut task = Task::new("Test", &HashMap::new(), None);
    task.uid = "123".to_string();
    store.add_task(task);

    // Start
    let intent_start = cfait::model::AppIntent::StartTask { uid: "123".to_string() };
    store.apply_task_intent(&intent_start, &config);
    let t = store.get_task_ref("123").unwrap();
    assert_eq!(t.status, TaskStatus::InProcess);
    assert!(t.last_started_at.is_some());

    // Pause
    let intent_pause = cfait::model::AppIntent::PauseTask { uid: "123".to_string() };
    store.apply_task_intent(&intent_pause, &config);
    let t2 = store.get_task_ref("123").unwrap();
    assert_eq!(t2.status, TaskStatus::NeedsAction);
    assert!(t2.last_started_at.is_none());

    // Start again
    store.apply_task_intent(&intent_start, &config);

    // Stop
    let intent_stop = cfait::model::AppIntent::StopTask { uid: "123".to_string() };
    store.apply_task_intent(&intent_stop, &config);
    let t3 = store.get_task_ref("123").unwrap();
    assert_eq!(t3.status, TaskStatus::NeedsAction);
    assert!(t3.last_started_at.is_none());
}

#[test]
fn test_apply_task_intent_comprehensive() {
    let ctx = Arc::new(TestContext::new());
    let mut store = TaskStore::new(ctx);
    let config = cfait::config::Config {
        trash_retention_days: 0,
        ..Default::default()
    };

    let mut t1 = Task::new("T1", &HashMap::new(), None);
    t1.uid = "1".to_string();
    t1.calendar_href = "cal1".to_string();
    store.add_task(t1);

    let mut t2 = Task::new("T2", &HashMap::new(), None);
    t2.uid = "2".to_string();
    t2.calendar_href = "cal1".to_string();
    store.add_task(t2);

    // MakeChild
    store.apply_task_intent(&cfait::model::AppIntent::MakeChild { uid: "2".to_string(), parent_uid: "1".to_string() }, &config);
    assert_eq!(store.get_task_ref("2").unwrap().parent_uid, Some("1".to_string()));

    // RemoveParent
    store.apply_task_intent(&cfait::model::AppIntent::RemoveParent { uid: "2".to_string() }, &config);
    assert_eq!(store.get_task_ref("2").unwrap().parent_uid, None);

    // AddDependency
    store.apply_task_intent(&cfait::model::AppIntent::AddDependency { uid: "1".to_string(), blocker_uid: "2".to_string() }, &config);
    assert!(store.get_task_ref("1").unwrap().dependencies.contains(&"2".to_string()));

    // RemoveDependency
    store.apply_task_intent(&cfait::model::AppIntent::RemoveDependency { uid: "1".to_string(), blocker_uid: "2".to_string() }, &config);
    assert!(!store.get_task_ref("1").unwrap().dependencies.contains(&"2".to_string()));

    // AddRelatedTo
    store.apply_task_intent(&cfait::model::AppIntent::AddRelatedTo { uid: "1".to_string(), related_uid: "2".to_string() }, &config);
    assert!(store.get_task_ref("1").unwrap().related_to.contains(&"2".to_string()));

    // RemoveRelatedTo
    store.apply_task_intent(&cfait::model::AppIntent::RemoveRelatedTo { uid: "1".to_string(), related_uid: "2".to_string() }, &config);
    assert!(!store.get_task_ref("1").unwrap().related_to.contains(&"2".to_string()));

    // ChangePriority
    store.apply_task_intent(&cfait::model::AppIntent::ChangePriority { uid: "1".to_string(), delta: 1 }, &config);

    // ToggleTreeCollapse
    store.apply_task_intent(&cfait::model::AppIntent::ToggleTreeCollapse { uid: "1".to_string() }, &config);
    assert!(store.get_task_ref("1").unwrap().collapsed);

    // MoveTask
    store.apply_task_intent(&cfait::model::AppIntent::MoveTask { uid: "1".to_string(), target_href: "cal2".to_string() }, &config);
    assert_eq!(store.get_task_ref("1").unwrap().calendar_href, "cal2");

    // CancelTask
    store.apply_task_intent(&cfait::model::AppIntent::CancelTask { uid: "2".to_string() }, &config);
    assert_eq!(store.get_task_ref("2").unwrap().status, TaskStatus::Cancelled);

    // DuplicateTaskTree
    store.apply_task_intent(&cfait::model::AppIntent::DuplicateTaskTree { uid: "1".to_string() }, &config);

    // DeleteTask
    store.apply_task_intent(&cfait::model::AppIntent::DeleteTask { uid: "1".to_string() }, &config);
    assert!(store.get_task_ref("1").is_none());

    // DeleteTaskTree
    let mut t3 = Task::new("T3", &HashMap::new(), None);
    t3.uid = "3".to_string();
    store.add_task(t3);
    store.apply_task_intent(&cfait::model::AppIntent::DeleteTaskTree { uid: "3".to_string() }, &config);
    assert!(store.get_task_ref("3").is_none());
}

#[test]
fn test_extract_markdown_tasks_full() {
    let input = "Root description.\n\n- [ ] Subtask 1\n  Details for subtask 1\n* [x] Subtask 2\n1. [ ] Numbered 1\n2. [ ] Numbered 2\n";
    let (root_desc, tasks) = cfait::model::extract_markdown_tasks(input);

    assert_eq!(root_desc, "Root description.");
    assert_eq!(tasks.len(), 4);

    assert_eq!(tasks[0].raw_text, "Subtask 1");
    assert!(!tasks[0].is_completed);
    assert_eq!(tasks[0].description, "Details for subtask 1");

    assert_eq!(tasks[1].raw_text, "Subtask 2");
    assert!(tasks[1].is_completed);
    assert_eq!(tasks[1].description, "");

    assert_eq!(tasks[2].raw_text, "Numbered 1");
    assert_eq!(tasks[3].raw_text, "Numbered 2");

    assert_eq!(tasks[3].dependencies.len(), 1);
}

#[test]
fn test_task_display_logic() {
    let mut task = Task::new("Test", &HashMap::new(), None);

    assert_eq!(task.checkbox_symbol(), "[ ]");
    assert!(!task.is_paused());
    assert_eq!(task.format_duration_short(), "");

    task.status = TaskStatus::InProcess;
    assert_eq!(task.checkbox_symbol(), "[▶]");

    task.status = TaskStatus::Completed;
    assert_eq!(task.checkbox_symbol(), "[✔]");

    task.status = TaskStatus::Cancelled;
    assert_eq!(task.checkbox_symbol(), "[✘]");

    task.status = TaskStatus::NeedsAction;
    task.time_spent_seconds = 3600;
    assert!(task.is_paused());
    assert_eq!(task.checkbox_symbol(), "[‖]");
    assert!(task.format_duration_short().contains("1h"));
}

#[test]
fn test_color_utils_hex_parsing() {
    let floats = cfait::color_utils::parse_hex_to_floats("#FF0000").unwrap();
    assert_eq!(floats, (1.0, 0.0, 0.0));

    let u8s = cfait::color_utils::parse_hex_to_u8("00FF00").unwrap();
    assert_eq!(u8s, (0, 255, 0));

    assert!(cfait::color_utils::parse_hex_to_floats("invalid").is_none());

    let color = cfait::color_utils::generate_color("test_tag");
    assert!(color.0 >= 0.0 && color.0 <= 1.0);
    assert!(color.1 >= 0.0 && color.1 <= 1.0);
    assert!(color.2 >= 0.0 && color.2 <= 1.0);

    let _dark = cfait::color_utils::is_dark(color.0, color.1, color.2);
}

#[test]
fn test_session_state_intents() {
    let mut session = cfait::model::SessionState::default();

    session.apply_session_intent(&cfait::model::AppIntent::SetSearchTerm { term: "test".to_string() });
    assert_eq!(session.search_term, "test");

    session.apply_session_intent(&cfait::model::AppIntent::ToggleTagFilter { tag: "work".to_string() });
    assert!(session.selected_categories.contains(&"work".to_string()));

    session.apply_session_intent(&cfait::model::AppIntent::ToggleTagFilter { tag: "work".to_string() });
    assert!(!session.selected_categories.contains(&"work".to_string()));

    session.apply_session_intent(&cfait::model::AppIntent::ToggleLocationFilter { location: "home".to_string() });
    assert!(session.selected_locations.contains(&"home".to_string()));

    session.apply_session_intent(&cfait::model::AppIntent::ToggleMatchAllCategories);
    assert!(session.match_all_categories);

    session.apply_session_intent(&cfait::model::AppIntent::ClearFilters);
    assert!(session.search_term.is_empty());
    assert!(session.selected_categories.is_empty());
    assert!(session.selected_locations.is_empty());

    session.apply_session_intent(&cfait::model::AppIntent::ToggleDoneGroup { key: "group".to_string() });
    assert!(session.expanded_done_groups.contains(&"group".to_string()));
}

#[test]
fn test_alarm_index_empty_and_default() {
    let ctx = Arc::new(TestContext::new());
    let idx = cfait::alarm_index::AlarmIndex::load(ctx.as_ref());
    assert!(idx.is_empty());
    assert_eq!(idx.len(), 0);

    let next = idx.get_next_alarm_timestamp();
    assert!(next.is_none());

    idx.save(ctx.as_ref()).unwrap();

    let idx2 = cfait::alarm_index::AlarmIndex::load(ctx.as_ref());
    assert!(idx2.is_empty());
}

#[test]
fn test_config_errors_and_save() {
    let ctx = Arc::new(TestContext::new());

    let err = cfait::config::Config::load(ctx.as_ref()).unwrap_err();
    assert!(cfait::config::Config::is_missing_config_error(&err));

    let config = cfait::config::Config {
        url: "http://example.com".to_string(),
        ..Default::default()
    };

    config.save_with_credentials(ctx.as_ref()).unwrap();

    let loaded = cfait::config::Config::load(ctx.as_ref()).unwrap();
    assert_eq!(loaded.url, "http://example.com");

    let loaded_cred = cfait::config::Config::load_with_credentials(ctx.as_ref()).unwrap();
    assert_eq!(loaded_cred.url, "http://example.com");

    assert_eq!(cfait::config::LogLevel::Debug.to_level_filter(), log::LevelFilter::Debug);

    assert!(!cfait::config::TaskAction::OpenUrl.label().is_empty());
}

#[test]
fn test_system_logging_and_keyring() {
    let ctx = Arc::new(TestContext::new());

    cfait::system::init_logging(ctx.as_ref(), false, Some(cfait::config::LogLevel::Debug.to_level_filter()));
    cfait::system::set_log_level(log::LevelFilter::Trace);

    cfait::system::init_keyring();
}

#[tokio::test]
async fn test_companion_events_batching() {
    let ctx = Arc::new(TestContext::new());
    let mut server = mockito::Server::new_async().await;
    let url = server.url();

    let client = cfait::client::RustyClient::new(ctx.clone(), &url, "u", "p", true, None).unwrap();

    let mut task1 = Task::new("T1", &HashMap::new(), None);
    task1.uid = "t1".to_string();
    task1.calendar_href = format!("{}/cal/", url);
    task1.href = format!("{}/cal/t1.ics", url);

    let _mock_report = server.mock("REPORT", "/cal/")
        .with_status(207)
        .with_body(r#"<d:multistatus xmlns:d="DAV:"></d:multistatus>"#)
        .create_async().await;

    let count = client.delete_all_companion_events(&task1.calendar_href).await.unwrap();
    assert_eq!(count, 0);

    let count = client.sync_multiple_companion_events(&[task1], true, false).await.unwrap();
    assert_eq!(count, 3); // 3 deletion futures for legacy suffixes
}
