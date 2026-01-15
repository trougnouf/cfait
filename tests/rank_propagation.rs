use std::collections::{HashMap, HashSet};

use cfait::model::Task;
use cfait::store::{FilterOptions, TaskStore};

fn make_task(summary: &str, _calendar_href: &str) -> Task {
    // Create a baseline task using the smart parser to ensure defaults are set.
    // We'll mutate fields we care about afterwards.
    let aliases: HashMap<String, Vec<String>> = HashMap::new();

    Task::new(summary, &aliases, None)
}

#[test]
fn parent_inherits_child_priority_and_sorts_before_sibling() {
    let mut store = TaskStore::new();
    let cal = "local://default";

    // Parent (no explicit priority)
    let mut parent = make_task("Parent", cal);
    parent.uid = "p1".to_string();
    parent.calendar_href = cal.to_string();
    parent.priority = 0;

    // Child is urgent (priority 1) and parent should inherit this urgency
    let mut child = make_task("Child", cal);
    child.uid = "c1".to_string();
    child.calendar_href = cal.to_string();
    child.priority = 1;
    child.parent_uid = Some(parent.uid.clone());

    // A standalone sibling with lower urgency (priority 5)
    let mut sibling = make_task("Sibling", cal);
    sibling.uid = "s1".to_string();
    sibling.calendar_href = cal.to_string();
    sibling.priority = 5;

    // Insert into the same calendar
    store.insert(
        cal.to_string(),
        vec![parent.clone(), child.clone(), sibling.clone()],
    );

    // Prepare filter options (use empty sets / defaults)
    let hidden_calendars: HashSet<String> = HashSet::new();
    let selected_categories: HashSet<String> = HashSet::new();
    let selected_locations: HashSet<String> = HashSet::new();
    let search_term = "";

    let opts = FilterOptions {
        active_cal_href: None,
        hidden_calendars: &hidden_calendars,
        selected_categories: &selected_categories,
        selected_locations: &selected_locations,
        match_all_categories: false,
        search_term,
        hide_completed_global: false,
        cutoff_date: None,
        min_duration: None,
        max_duration: None,
        include_unset_duration: true,
        urgent_days: 1,
        urgent_prio: 1,
        default_priority: 5,
        start_grace_period_days: 1,
    };

    let result = store.filter(opts);

    // Expect parent (with inherited urgency) to appear before sibling.
    // Also the child should be directly after the parent in the flattened hierarchy.
    // Find indices.
    let p_idx = result
        .iter()
        .position(|t| t.uid == "p1")
        .expect("parent not present");
    let c_idx = result
        .iter()
        .position(|t| t.uid == "c1")
        .expect("child not present");
    let s_idx = result
        .iter()
        .position(|t| t.uid == "s1")
        .expect("sibling not present");

    // Parent should appear before sibling
    assert!(
        p_idx < s_idx,
        "parent should be ordered before sibling due to child's urgency"
    );

    // Child should be immediately after parent in the flattened result (depth-first)
    assert_eq!(
        c_idx,
        p_idx + 1,
        "child should directly follow parent in hierarchy flattening"
    );
}

#[test]
fn compare_two_parents_inherited_priorities_determine_order() {
    let mut store = TaskStore::new();
    let cal = "local://default";

    // Parent A has a child with priority 1 (more urgent)
    let mut pa = make_task("Parent A", cal);
    pa.uid = "pa".to_string();
    pa.calendar_href = cal.to_string();
    pa.priority = 0;

    let mut ca = make_task("Child A", cal);
    ca.uid = "ca".to_string();
    ca.calendar_href = cal.to_string();
    ca.priority = 1;
    ca.parent_uid = Some(pa.uid.clone());

    // Parent B has a child with priority 3 (less urgent than Child A)
    let mut pb = make_task("Parent B", cal);
    pb.uid = "pb".to_string();
    pb.calendar_href = cal.to_string();
    pb.priority = 0;

    let mut cb = make_task("Child B", cal);
    cb.uid = "cb".to_string();
    cb.calendar_href = cal.to_string();
    cb.priority = 3;
    cb.parent_uid = Some(pb.uid.clone());

    // Insert all tasks
    store.insert(
        cal.to_string(),
        vec![pa.clone(), ca.clone(), pb.clone(), cb.clone()],
    );

    // Prepare filter options
    let hidden_calendars: HashSet<String> = HashSet::new();
    let selected_categories: HashSet<String> = HashSet::new();
    let selected_locations: HashSet<String> = HashSet::new();
    let search_term = "";

    let opts = FilterOptions {
        active_cal_href: None,
        hidden_calendars: &hidden_calendars,
        selected_categories: &selected_categories,
        selected_locations: &selected_locations,
        match_all_categories: false,
        search_term,
        hide_completed_global: false,
        cutoff_date: None,
        min_duration: None,
        max_duration: None,
        include_unset_duration: true,
        urgent_days: 1,
        urgent_prio: 1,
        default_priority: 5,
        start_grace_period_days: 1,
    };

    let result = store.filter(opts);

    // Identify indices for each task in the flattened result
    let pa_idx = result
        .iter()
        .position(|t| t.uid == "pa")
        .expect("pa missing");
    let ca_idx = result
        .iter()
        .position(|t| t.uid == "ca")
        .expect("ca missing");
    let pb_idx = result
        .iter()
        .position(|t| t.uid == "pb")
        .expect("pb missing");
    let cb_idx = result
        .iter()
        .position(|t| t.uid == "cb")
        .expect("cb missing");

    // Parent A subtree should come before Parent B subtree because Child A is more urgent
    assert!(
        pa_idx < pb_idx,
        "Parent A should be ordered before Parent B due to child's higher urgency"
    );

    // Children should be placed directly after their parents (flattened hierarchy)
    assert_eq!(ca_idx, pa_idx + 1, "Child A should follow Parent A");
    assert_eq!(cb_idx, pb_idx + 1, "Child B should follow Parent B");
}

#[test]
fn parent_inherits_started_child_over_unset_sibling() {
    let mut store = TaskStore::new();
    let cal = "local://default";

    // Parent without explicit priority
    let mut parent = make_task("Parent Started", cal);
    parent.uid = "p_started".to_string();
    parent.calendar_href = cal.to_string();
    parent.priority = 0;

    // Child is started (InProcess) - this should make the parent behave as started
    let mut child = make_task("Child Started", cal);
    child.uid = "c_started".to_string();
    child.calendar_href = cal.to_string();
    child.priority = 0;
    child.parent_uid = Some(parent.uid.clone());
    child.status = cfait::model::TaskStatus::InProcess;

    // Sibling with unset priority (0)
    let mut sibling = make_task("Sibling Unset", cal);
    sibling.uid = "s_unset".to_string();
    sibling.calendar_href = cal.to_string();
    sibling.priority = 0;

    store.insert(
        cal.to_string(),
        vec![parent.clone(), child.clone(), sibling.clone()],
    );

    // Prepare filter options (use same defaults as other tests)
    let hidden_calendars: HashSet<String> = HashSet::new();
    let selected_categories: HashSet<String> = HashSet::new();
    let selected_locations: HashSet<String> = HashSet::new();
    let search_term = "";

    let opts = FilterOptions {
        active_cal_href: None,
        hidden_calendars: &hidden_calendars,
        selected_categories: &selected_categories,
        selected_locations: &selected_locations,
        match_all_categories: false,
        search_term,
        hide_completed_global: false,
        cutoff_date: None,
        min_duration: None,
        max_duration: None,
        include_unset_duration: true,
        urgent_days: 1,
        urgent_prio: 1,
        default_priority: 5,
        start_grace_period_days: 1,
    };

    let result = store.filter(opts);

    let p_idx = result
        .iter()
        .position(|t| t.uid == "p_started")
        .expect("parent not present");
    let c_idx = result
        .iter()
        .position(|t| t.uid == "c_started")
        .expect("child not present");
    let s_idx = result
        .iter()
        .position(|t| t.uid == "s_unset")
        .expect("sibling not present");

    // Parent (inheriting started from child) should appear before sibling
    assert!(
        p_idx < s_idx,
        "parent should be ordered before sibling due to child's started status"
    );
    // Child should directly follow parent
    assert_eq!(
        c_idx,
        p_idx + 1,
        "child should directly follow parent in hierarchy flattening"
    );
}
