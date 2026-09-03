// SPDX-License-Identifier: GPL-3.0-or-later
//! General tests for search functionality.
use cfait::context::TestContext;
use cfait::model::Task;
use cfait::store::TaskStore;
use std::collections::HashMap;
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
