use cfait::context::TestContext;
use cfait::model::Task;
use cfait::store::{FilterOptions, TaskStore};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

#[test]
fn test_search_includes_non_matching_children() {
    let ctx = Arc::new(TestContext::new());
    let mut store = TaskStore::new(ctx);
    let aliases = HashMap::new();

    let mut parent = Task::new("Project Alpha", &aliases, None);
    parent.uid = "parent".to_string();
    parent.calendar_href = "cal1".to_string();

    let mut child = Task::new("Implementation details", &aliases, None);
    child.uid = "child".to_string();
    child.parent_uid = Some("parent".to_string());
    child.calendar_href = "cal1".to_string();

    store.add_task(parent);
    store.add_task(child);

    let results = store.filter(FilterOptions {
        active_cal_href: None,
        hidden_calendars: &HashSet::new(),
        selected_categories: &HashSet::new(),
        selected_locations: &HashSet::new(),
        match_all_categories: false,
        search_term: "Project",
        hide_completed_global: false,
        cutoff_date: None,
        min_duration: None,
        max_duration: None,
        include_unset_duration: true,
        urgent_days: 1,
        urgent_prio: 1,
        default_priority: 5,
        start_grace_period_days: 1,
        expanded_done_groups: &HashSet::new(),
        max_done_roots: usize::MAX,
        max_done_subtasks: usize::MAX,
    });

    assert_eq!(results.len(), 2);
    assert!(results.iter().any(|t| t.uid == "parent"));
    assert!(results.iter().any(|t| t.uid == "child"));
}

#[test]
fn test_search_includes_deep_hierarchy() {
    let ctx = Arc::new(TestContext::new());
    let mut store = TaskStore::new(ctx);
    let aliases = HashMap::new();

    let mut gp = Task::new("Grand Parent", &aliases, None);
    gp.uid = "gp".to_string();
    gp.calendar_href = "cal1".to_string();

    let mut p = Task::new("Middle", &aliases, None);
    p.uid = "p".to_string();
    p.parent_uid = Some("gp".to_string());
    p.calendar_href = "cal1".to_string();

    let mut c = Task::new("Leaf", &aliases, None);
    c.uid = "c".to_string();
    c.parent_uid = Some("p".to_string());
    c.calendar_href = "cal1".to_string();

    store.add_task(gp);
    store.add_task(p);
    store.add_task(c);

    let results = store.filter(FilterOptions {
        active_cal_href: None,
        hidden_calendars: &HashSet::new(),
        selected_categories: &HashSet::new(),
        selected_locations: &HashSet::new(),
        match_all_categories: false,
        search_term: "Grand",
        hide_completed_global: false,
        cutoff_date: None,
        min_duration: None,
        max_duration: None,
        include_unset_duration: true,
        urgent_days: 1,
        urgent_prio: 1,
        default_priority: 5,
        start_grace_period_days: 1,
        expanded_done_groups: &HashSet::new(),
        max_done_roots: usize::MAX,
        max_done_subtasks: usize::MAX,
    });

    assert_eq!(results.len(), 3);
}

#[test]
fn test_child_match_does_not_force_parent_if_parent_does_not_match() {
    let ctx = Arc::new(TestContext::new());
    let mut store = TaskStore::new(ctx);
    let aliases = HashMap::new();

    let mut parent = Task::new("Parent", &aliases, None);
    parent.uid = "parent".to_string();
    parent.calendar_href = "cal1".to_string();

    let mut child = Task::new("Child Match", &aliases, None);
    child.uid = "child".to_string();
    child.parent_uid = Some("parent".to_string());
    child.calendar_href = "cal1".to_string();

    store.add_task(parent);
    store.add_task(child);

    let results = store.filter(FilterOptions {
        active_cal_href: None,
        hidden_calendars: &HashSet::new(),
        selected_categories: &HashSet::new(),
        selected_locations: &HashSet::new(),
        match_all_categories: false,
        search_term: "Match",
        hide_completed_global: false,
        cutoff_date: None,
        min_duration: None,
        max_duration: None,
        include_unset_duration: true,
        urgent_days: 1,
        urgent_prio: 1,
        default_priority: 5,
        start_grace_period_days: 1,
        expanded_done_groups: &HashSet::new(),
        max_done_roots: usize::MAX,
        max_done_subtasks: usize::MAX,
    });

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].uid, "child");
}

#[test]
fn test_multiple_parents_with_children() {
    let ctx = Arc::new(TestContext::new());
    let mut store = TaskStore::new(ctx);
    let aliases = HashMap::new();

    let mut parent1 = Task::new("Project Alpha", &aliases, None);
    parent1.uid = "p1".to_string();
    parent1.calendar_href = "cal1".to_string();

    let mut child1 = Task::new("Child 1", &aliases, None);
    child1.uid = "c1".to_string();
    child1.parent_uid = Some("p1".to_string());
    child1.calendar_href = "cal1".to_string();

    let mut parent2 = Task::new("Project Beta", &aliases, None);
    parent2.uid = "p2".to_string();
    parent2.calendar_href = "cal1".to_string();

    let mut child2 = Task::new("Child 2", &aliases, None);
    child2.uid = "c2".to_string();
    child2.parent_uid = Some("p2".to_string());
    child2.calendar_href = "cal1".to_string();

    store.add_task(parent1);
    store.add_task(child1);
    store.add_task(parent2);
    store.add_task(child2);

    let results = store.filter(FilterOptions {
        active_cal_href: None,
        hidden_calendars: &HashSet::new(),
        selected_categories: &HashSet::new(),
        selected_locations: &HashSet::new(),
        match_all_categories: false,
        search_term: "Alpha",
        hide_completed_global: false,
        cutoff_date: None,
        min_duration: None,
        max_duration: None,
        include_unset_duration: true,
        urgent_days: 1,
        urgent_prio: 1,
        default_priority: 5,
        start_grace_period_days: 1,
        expanded_done_groups: &HashSet::new(),
        max_done_roots: usize::MAX,
        max_done_subtasks: usize::MAX,
    });

    assert_eq!(results.len(), 2);
    assert!(results.iter().any(|t| t.uid == "p1"));
    assert!(results.iter().any(|t| t.uid == "c1"));
}

#[test]
fn test_sibling_match_only_includes_matching_sibling() {
    let ctx = Arc::new(TestContext::new());
    let mut store = TaskStore::new(ctx);
    let aliases = HashMap::new();

    let mut parent = Task::new("Parent Task", &aliases, None);
    parent.uid = "parent".to_string();
    parent.calendar_href = "cal1".to_string();

    let mut child1 = Task::new("Special Child", &aliases, None);
    child1.uid = "c1".to_string();
    child1.parent_uid = Some("parent".to_string());
    child1.calendar_href = "cal1".to_string();

    let mut child2 = Task::new("Regular Child", &aliases, None);
    child2.uid = "c2".to_string();
    child2.parent_uid = Some("parent".to_string());
    child2.calendar_href = "cal1".to_string();

    store.add_task(parent);
    store.add_task(child1);
    store.add_task(child2);

    let results = store.filter(FilterOptions {
        active_cal_href: None,
        hidden_calendars: &HashSet::new(),
        selected_categories: &HashSet::new(),
        selected_locations: &HashSet::new(),
        match_all_categories: false,
        search_term: "Special",
        hide_completed_global: false,
        cutoff_date: None,
        min_duration: None,
        max_duration: None,
        include_unset_duration: true,
        urgent_days: 1,
        urgent_prio: 1,
        default_priority: 5,
        start_grace_period_days: 1,
        expanded_done_groups: &HashSet::new(),
        max_done_roots: usize::MAX,
        max_done_subtasks: usize::MAX,
    });

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].uid, "c1");
}

#[test]
fn test_empty_search_shows_all_tasks() {
    let ctx = Arc::new(TestContext::new());
    let mut store = TaskStore::new(ctx);
    let aliases = HashMap::new();

    let mut parent = Task::new("Parent", &aliases, None);
    parent.uid = "parent".to_string();
    parent.calendar_href = "cal1".to_string();

    let mut child = Task::new("Child", &aliases, None);
    child.uid = "child".to_string();
    child.parent_uid = Some("parent".to_string());
    child.calendar_href = "cal1".to_string();

    store.add_task(parent);
    store.add_task(child);

    let results = store.filter(FilterOptions {
        active_cal_href: None,
        hidden_calendars: &HashSet::new(),
        selected_categories: &HashSet::new(),
        selected_locations: &HashSet::new(),
        match_all_categories: false,
        search_term: "",
        hide_completed_global: false,
        cutoff_date: None,
        min_duration: None,
        max_duration: None,
        include_unset_duration: true,
        urgent_days: 1,
        urgent_prio: 1,
        default_priority: 5,
        start_grace_period_days: 1,
        expanded_done_groups: &HashSet::new(),
        max_done_roots: usize::MAX,
        max_done_subtasks: usize::MAX,
    });

    assert_eq!(results.len(), 2);
}

#[test]
fn test_hierarchy_expansion_with_completed_tasks() {
    let ctx = Arc::new(TestContext::new());
    let mut store = TaskStore::new(ctx);
    let aliases = HashMap::new();

    let mut parent = Task::new("Active Project", &aliases, None);
    parent.uid = "parent".to_string();
    parent.calendar_href = "cal1".to_string();

    let mut child = Task::new("Completed subtask", &aliases, None);
    child.uid = "child".to_string();
    child.parent_uid = Some("parent".to_string());
    child.calendar_href = "cal1".to_string();
    child.status = cfait::model::TaskStatus::Completed;

    store.add_task(parent);
    store.add_task(child);

    let results = store.filter(FilterOptions {
        active_cal_href: None,
        hidden_calendars: &HashSet::new(),
        selected_categories: &HashSet::new(),
        selected_locations: &HashSet::new(),
        match_all_categories: false,
        search_term: "Project",
        hide_completed_global: false,
        cutoff_date: None,
        min_duration: None,
        max_duration: None,
        include_unset_duration: true,
        urgent_days: 1,
        urgent_prio: 1,
        default_priority: 5,
        start_grace_period_days: 1,
        expanded_done_groups: &HashSet::new(),
        max_done_roots: usize::MAX,
        max_done_subtasks: usize::MAX,
    });

    assert_eq!(results.len(), 2);

    let results_hidden = store.filter(FilterOptions {
        active_cal_href: None,
        hidden_calendars: &HashSet::new(),
        selected_categories: &HashSet::new(),
        selected_locations: &HashSet::new(),
        match_all_categories: false,
        search_term: "Project",
        hide_completed_global: true,
        cutoff_date: None,
        min_duration: None,
        max_duration: None,
        include_unset_duration: true,
        urgent_days: 1,
        urgent_prio: 1,
        default_priority: 5,
        start_grace_period_days: 1,
        expanded_done_groups: &HashSet::new(),
        max_done_roots: usize::MAX,
        max_done_subtasks: usize::MAX,
    });

    assert_eq!(results_hidden.len(), 1);
    assert_eq!(results_hidden[0].uid, "parent");
}
