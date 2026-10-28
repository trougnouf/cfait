// SPDX-License-Identifier: GPL-3.0-or-later
//! General tests for search functionality.
use cfait::config::{PausedSortBehavior, SortPreset};
use cfait::context::TestContext;
use cfait::model::{CalendarListEntry, Task};
use cfait::store::{FilterOptions, TaskStore};
use chrono::NaiveTime;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

fn create_store() -> TaskStore {
    TaskStore::new(Arc::new(TestContext::new()))
}

fn create_task_with_loc(summary: &str, location: &str) -> Task {
    let mut t = Task::new(summary, &HashMap::new(), None);
    t.locations = vec![location.to_string()];
    t
}

#[test]
fn test_implicit_location_search() {
    let t = create_task_with_loc("Buy Milk", "Supermarket");

    // 1. Explicit @@ search
    assert!(t.matches_search_term("@@Supermarket", &create_store()));
    assert!(t.matches_search_term("loc:Supermarket", &create_store()));

    // 2. Implicit search (New Feature)
    // Should match because "Supermarket" is in the location field
    assert!(t.matches_search_term("Supermarket", &create_store()));

    // 3. Partial implicit match
    assert!(t.matches_search_term("market", &create_store()));

    // 4. Case insensitive
    assert!(t.matches_search_term("supermarket", &create_store()));

    // 5. Negative case
    assert!(!t.matches_search_term("Office", &create_store()));
}

#[test]
fn test_mixed_filters() {
    let mut t = create_task_with_loc("Meeting", "Office");
    t.priority = 1;

    // Matches Priority AND Location implicitly
    assert!(t.matches_search_term("!1 Office", &create_store()));

    // Matches Priority but NOT Location
    assert!(!t.matches_search_term("!1 Home", &create_store()));
}

fn run_filter(
    store: &TaskStore,
    calendars: &[CalendarListEntry],
    search_term: &str,
) -> Vec<cfait::store::TaskListItem> {
    store
        .filter(FilterOptions {
            active_cal_href: None,
            hidden_calendars: &HashSet::new(),
            calendars,
            selected_categories: &HashSet::new(),
            selected_locations: &HashSet::new(),
            match_all_categories: false,
            search_term,
            hide_completed_global: false,
            hide_fully_completed_tags: false,
            hide_aliases_in_sidebar: false,
            cutoff_date: None,
            min_duration: None,
            max_duration: None,
            include_unset_duration: true,
            urgent_days: 1,
            urgent_prio: 1,
            default_priority: 5,
            start_grace_period_days: 1,
            sort_standard_by_priority: false,
            sort_preset: SortPreset::default(),
            expanded_done_groups: &HashSet::new(),
            expanded_tags: &HashSet::new(),
            expanded_locations: &HashSet::new(),
            max_done_roots: usize::MAX,
            max_done_subtasks: usize::MAX,
            tag_aliases: &HashMap::new(),
            search_collapsed_tasks: &HashSet::new(),
            focused_task_uid: None,
            paused_sort_behavior: PausedSortBehavior::default(),
            sort_tiebreak_recent: false,
            default_reminder_time: NaiveTime::from_hms_opt(8, 0, 0).unwrap(),
        })
        .items
}

fn filter_by_col(store: &TaskStore, search_term: &str) -> Vec<String> {
    let calendars = vec![
        CalendarListEntry {
            name: "Herbary".to_string(),
            href: "local://herbary".to_string(),
            color: None,
            supports_vjournal: None,
            supports_vevent: None,
        },
        CalendarListEntry {
            name: "Kitchen".to_string(),
            href: "local://kitchen".to_string(),
            color: None,
            supports_vjournal: None,
            supports_vevent: None,
        },
    ];
    run_filter(store, &calendars, search_term)
        .into_iter()
        .filter_map(|item| {
            if let cfait::store::TaskListItem::Task(task) = item {
                Some(task.uid)
            } else {
                None
            }
        })
        .collect()
}

#[test]
fn test_collection_filter_search() {
    let mut store = create_store();
    let aliases = HashMap::new();

    let mut tomatoes = Task::new("Water the tomatoes", &aliases, None);
    tomatoes.uid = "tomatoes".to_string();
    tomatoes.calendar_href = "local://herbary".to_string();

    let mut dishes = Task::new("Wash the dishes", &aliases, None);
    dishes.uid = "dishes".to_string();
    dishes.calendar_href = "local://kitchen".to_string();

    store.add_task(tomatoes);
    store.add_task(dishes);

    // Name match (case-insensitive substring)
    assert_eq!(
        filter_by_col(&store, "col:herb"),
        vec!["tomatoes".to_string()]
    );
    assert_eq!(
        filter_by_col(&store, "col:HERBARY"),
        vec!["tomatoes".to_string()]
    );

    // Href match
    assert_eq!(
        filter_by_col(&store, "col:local://herbary"),
        vec!["tomatoes".to_string()]
    );

    // Other collection only
    assert_eq!(
        filter_by_col(&store, "col:kitchen"),
        vec!["dishes".to_string()]
    );

    // No such collection
    assert!(filter_by_col(&store, "col:balcony").is_empty());

    // Negation
    assert_eq!(
        filter_by_col(&store, "-col:kitchen"),
        vec!["tomatoes".to_string()]
    );
}

#[test]
fn test_collection_filter_shared_account_path() {
    let mut store = create_store();
    let aliases = HashMap::new();
    // Remote collections all share the /trougnouf/ account path in their
    // hrefs, so a collection named like the account must not match them all.
    let calendars = vec![
        CalendarListEntry {
            name: "trougnouf".to_string(),
            href: "/trougnouf/18ead562-aaaa-bbbb-cccc-000000000001/".to_string(),
            color: None,
            supports_vjournal: None,
            supports_vevent: None,
        },
        CalendarListEntry {
            name: "Workout".to_string(),
            href: "/trougnouf/07f0b86f-aaaa-bbbb-cccc-000000000002/".to_string(),
            color: None,
            supports_vjournal: None,
            supports_vevent: None,
        },
        CalendarListEntry {
            name: "Kitchen".to_string(),
            href: "local://kitchen".to_string(),
            color: None,
            supports_vjournal: None,
            supports_vevent: None,
        },
    ];

    let mut prep = Task::new("Garden prep", &aliases, None);
    prep.uid = "prep".to_string();
    prep.calendar_href = "/trougnouf/18ead562-aaaa-bbbb-cccc-000000000001/".to_string();

    let mut chop = Task::new("Chop the squash", &aliases, None);
    chop.uid = "chop".to_string();
    chop.parent_uid = Some("prep".to_string());
    chop.calendar_href = "/trougnouf/07f0b86f-aaaa-bbbb-cccc-000000000002/".to_string();

    let mut arugula = Task::new("Water the arugula", &aliases, None);
    arugula.uid = "arugula".to_string();
    arugula.calendar_href = "/trougnouf/18ead562-aaaa-bbbb-cccc-000000000001/".to_string();

    let mut dishes = Task::new("Wash the dishes", &aliases, None);
    dishes.uid = "dishes".to_string();
    dishes.calendar_href = "local://kitchen".to_string();

    for t in [prep, chop, arugula, dishes] {
        store.add_task(t);
    }

    let filter_items = |term: &str| -> Vec<(String, bool)> {
        run_filter(&store, &calendars, term)
            .into_iter()
            .filter_map(|item| {
                if let cfait::store::TaskListItem::Task(task) = item {
                    Some((task.uid, task.is_search_context))
                } else {
                    None
                }
            })
            .collect()
    };

    // col:workout matches only the workout task; its parent (in another
    // collection) is pulled in as dimmed context, and no task from the
    // same-account sibling collections leaks in.
    assert_eq!(
        filter_items("col:workout"),
        vec![("prep".to_string(), true), ("chop".to_string(), false),]
    );

    // col:trougnouf matches the tasks of the collection named "trougnouf"
    // and their descendants — not every task under the /trougnouf/ path.
    assert_eq!(
        filter_items("col:trougnouf"),
        vec![
            ("prep".to_string(), false),
            ("chop".to_string(), false),
            ("arugula".to_string(), false),
        ]
    );
}
