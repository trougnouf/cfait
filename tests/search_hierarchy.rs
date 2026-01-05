// Tests for search filtering with task hierarchy.
use cfait::model::Task;
use cfait::store::{FilterOptions, TaskStore};
use std::collections::{HashMap, HashSet};

#[test]
fn test_search_includes_non_matching_children() {
    let mut store = TaskStore::new();
    let aliases = HashMap::new();

    // 1. Parent task that matches "Project"
    let mut parent = Task::new("Project Alpha", &aliases, None);
    parent.uid = "parent".to_string();
    parent.calendar_href = "cal1".to_string();

    // 2. Child task that DOES NOT match "Project"
    let mut child = Task::new("Implementation details", &aliases, None);
    child.uid = "child".to_string();
    child.parent_uid = Some("parent".to_string());
    child.calendar_href = "cal1".to_string();

    store.add_task(parent);
    store.add_task(child);

    // 3. Search for "Project"
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
    });

    // 4. Assert
    assert_eq!(
        results.len(),
        2,
        "Should return both parent and child, even though child doesn't match"
    );

    let p_res = results.iter().find(|t| t.uid == "parent");
    let c_res = results.iter().find(|t| t.uid == "child");

    assert!(p_res.is_some(), "Parent must be present");
    assert!(
        c_res.is_some(),
        "Child must be present via hierarchy expansion"
    );
}

#[test]
fn test_search_includes_deep_hierarchy() {
    let mut store = TaskStore::new();
    let aliases = HashMap::new();

    // Grandparent matches "Grand"
    let mut gp = Task::new("Grand Parent", &aliases, None);
    gp.uid = "gp".to_string();
    gp.calendar_href = "cal1".to_string();

    // Parent doesn't match
    let mut p = Task::new("Middle", &aliases, None);
    p.uid = "p".to_string();
    p.parent_uid = Some("gp".to_string());
    p.calendar_href = "cal1".to_string();

    // Child doesn't match
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
    });

    assert_eq!(results.len(), 3, "Whole subtree should be included");
    assert!(results.iter().any(|t| t.uid == "gp"));
    assert!(results.iter().any(|t| t.uid == "p"));
    assert!(results.iter().any(|t| t.uid == "c"));
}

#[test]
fn test_child_match_does_not_force_parent_if_parent_does_not_match() {
    // Standard behavior: orphans are shown at root level if parents are filtered out.
    // This test confirms we didn't accidentally reverse logic to include non-matching parents.
    // (Note: `organize_hierarchy` handles the visual display, but `filter` determines existence).

    let mut store = TaskStore::new();
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
    });

    // Should only contain the child
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].uid, "child");
    // Ensure it was organized as a root (depth 0) since parent is missing
    assert_eq!(results[0].depth, 0);
}

#[test]
fn test_multiple_parents_with_children() {
    let mut store = TaskStore::new();
    let aliases = HashMap::new();

    // Parent 1 matches "Alpha"
    let mut parent1 = Task::new("Project Alpha", &aliases, None);
    parent1.uid = "p1".to_string();
    parent1.calendar_href = "cal1".to_string();

    let mut child1 = Task::new("Child 1", &aliases, None);
    child1.uid = "c1".to_string();
    child1.parent_uid = Some("p1".to_string());
    child1.calendar_href = "cal1".to_string();

    // Parent 2 doesn't match "Alpha"
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
    });

    // Should only contain parent1 and its child
    assert_eq!(results.len(), 2);
    assert!(results.iter().any(|t| t.uid == "p1"));
    assert!(results.iter().any(|t| t.uid == "c1"));
    assert!(!results.iter().any(|t| t.uid == "p2"));
    assert!(!results.iter().any(|t| t.uid == "c2"));
}

#[test]
fn test_sibling_match_only_includes_matching_sibling() {
    let mut store = TaskStore::new();
    let aliases = HashMap::new();

    // Parent doesn't match
    let mut parent = Task::new("Parent Task", &aliases, None);
    parent.uid = "parent".to_string();
    parent.calendar_href = "cal1".to_string();

    // Child 1 matches "Special"
    let mut child1 = Task::new("Special Child", &aliases, None);
    child1.uid = "c1".to_string();
    child1.parent_uid = Some("parent".to_string());
    child1.calendar_href = "cal1".to_string();

    // Child 2 doesn't match "Special"
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
    });

    // Should only contain child1 (the one that matches)
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].uid, "c1");
}

#[test]
fn test_empty_search_shows_all_tasks() {
    let mut store = TaskStore::new();
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
    });

    // Empty search should show all tasks
    assert_eq!(results.len(), 2);
}

#[test]
fn test_hierarchy_expansion_with_completed_tasks() {
    let mut store = TaskStore::new();
    let aliases = HashMap::new();

    // Parent matches and is active
    let mut parent = Task::new("Active Project", &aliases, None);
    parent.uid = "parent".to_string();
    parent.calendar_href = "cal1".to_string();

    // Child is completed
    let mut child = Task::new("Completed subtask", &aliases, None);
    child.uid = "child".to_string();
    child.parent_uid = Some("parent".to_string());
    child.calendar_href = "cal1".to_string();
    child.status = cfait::model::TaskStatus::Completed;

    store.add_task(parent);
    store.add_task(child);

    // Test with hide_completed_global = false
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
    });

    // Both should be included when hide_completed is false
    assert_eq!(results.len(), 2);

    // Test with hide_completed_global = true
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
    });

    // Only parent should be included when hide_completed is true
    // (completed child gets filtered in Pass 1)
    assert_eq!(results_hidden.len(), 1);
    assert_eq!(results_hidden[0].uid, "parent");
}
