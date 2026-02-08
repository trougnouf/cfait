use cfait::context::TestContext;
use cfait::model::Task;
use cfait::store::{FilterOptions, TaskStore};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

fn make_task(summary: &str, _calendar_href: &str) -> Task {
    let aliases: HashMap<String, Vec<String>> = HashMap::new();
    Task::new(summary, &aliases, None)
}

#[test]
fn parent_inherits_child_priority_and_sorts_before_sibling() {
    let ctx = Arc::new(TestContext::new());
    let mut store = TaskStore::new(ctx);
    let cal = "local://default";

    let mut parent = make_task("Parent", cal);
    parent.uid = "p1".to_string();
    parent.calendar_href = cal.to_string();
    parent.priority = 0;

    let mut child = make_task("Child", cal);
    child.uid = "c1".to_string();
    child.calendar_href = cal.to_string();
    child.priority = 1;
    child.parent_uid = Some(parent.uid.clone());

    let mut sibling = make_task("Sibling", cal);
    sibling.uid = "s1".to_string();
    sibling.calendar_href = cal.to_string();
    sibling.priority = 5;

    store.insert(
        cal.to_string(),
        vec![parent.clone(), child.clone(), sibling.clone()],
    );

    let hidden_calendars: HashSet<String> = HashSet::new();
    let selected_categories: HashSet<String> = HashSet::new();
    let selected_locations: HashSet<String> = HashSet::new();

    let opts = FilterOptions {
        active_cal_href: None,
        hidden_calendars: &hidden_calendars,
        selected_categories: &selected_categories,
        selected_locations: &selected_locations,
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
    };

    let result = store.filter(opts);

    let p_idx = result.iter().position(|t| t.uid == "p1").unwrap();
    let s_idx = result.iter().position(|t| t.uid == "s1").unwrap();

    assert!(p_idx < s_idx);
}

#[test]
fn compare_two_parents_inherited_priorities_determine_order() {
    let ctx = Arc::new(TestContext::new());
    let mut store = TaskStore::new(ctx);
    let cal = "local://default";

    let mut pa = make_task("Parent A", cal);
    pa.uid = "pa".to_string();
    pa.calendar_href = cal.to_string();
    pa.priority = 0;

    let mut ca = make_task("Child A", cal);
    ca.uid = "ca".to_string();
    ca.calendar_href = cal.to_string();
    ca.priority = 1;
    ca.parent_uid = Some(pa.uid.clone());

    let mut pb = make_task("Parent B", cal);
    pb.uid = "pb".to_string();
    pb.calendar_href = cal.to_string();
    pb.priority = 0;

    let mut cb = make_task("Child B", cal);
    cb.uid = "cb".to_string();
    cb.calendar_href = cal.to_string();
    cb.priority = 3;
    cb.parent_uid = Some(pb.uid.clone());

    store.insert(
        cal.to_string(),
        vec![pa.clone(), ca.clone(), pb.clone(), cb.clone()],
    );

    let hidden_calendars: HashSet<String> = HashSet::new();
    let selected_categories: HashSet<String> = HashSet::new();
    let selected_locations: HashSet<String> = HashSet::new();

    let opts = FilterOptions {
        active_cal_href: None,
        hidden_calendars: &hidden_calendars,
        selected_categories: &selected_categories,
        selected_locations: &selected_locations,
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
    };

    let result = store.filter(opts);

    let pa_idx = result.iter().position(|t| t.uid == "pa").unwrap();
    let pb_idx = result.iter().position(|t| t.uid == "pb").unwrap();

    assert!(pa_idx < pb_idx);
}

#[test]
fn parent_inherits_started_child_over_unset_sibling() {
    let ctx = Arc::new(TestContext::new());
    let mut store = TaskStore::new(ctx);
    let cal = "local://default";

    let mut parent = make_task("Parent Started", cal);
    parent.uid = "p_started".to_string();
    parent.calendar_href = cal.to_string();
    parent.priority = 0;

    let mut child = make_task("Child Started", cal);
    child.uid = "c_started".to_string();
    child.calendar_href = cal.to_string();
    child.priority = 0;
    child.parent_uid = Some(parent.uid.clone());
    child.status = cfait::model::TaskStatus::InProcess;

    let mut sibling = make_task("Sibling Unset", cal);
    sibling.uid = "s_unset".to_string();
    sibling.calendar_href = cal.to_string();
    sibling.priority = 0;

    store.insert(
        cal.to_string(),
        vec![parent.clone(), child.clone(), sibling.clone()],
    );

    let hidden_calendars: HashSet<String> = HashSet::new();
    let selected_categories: HashSet<String> = HashSet::new();
    let selected_locations: HashSet<String> = HashSet::new();

    let opts = FilterOptions {
        active_cal_href: None,
        hidden_calendars: &hidden_calendars,
        selected_categories: &selected_categories,
        selected_locations: &selected_locations,
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
    };

    let result = store.filter(opts);

    let p_idx = result.iter().position(|t| t.uid == "p_started").unwrap();
    let s_idx = result.iter().position(|t| t.uid == "s_unset").unwrap();

    assert!(p_idx < s_idx);
}
