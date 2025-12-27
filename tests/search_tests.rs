use cfait::model::Task;
use std::collections::HashMap;

fn create_task_with_loc(summary: &str, location: &str) -> Task {
    let mut t = Task::new(summary, &HashMap::new(), None);
    t.location = Some(location.to_string());
    t
}

#[test]
fn test_implicit_location_search() {
    let t = create_task_with_loc("Buy Milk", "Supermarket");

    // 1. Explicit @@ search
    assert!(t.matches_search_term("@@Supermarket"));
    assert!(t.matches_search_term("loc:Supermarket"));

    // 2. Implicit search (New Feature)
    // Should match because "Supermarket" is in the location field
    assert!(t.matches_search_term("Supermarket"));

    // 3. Partial implicit match
    assert!(t.matches_search_term("market"));

    // 4. Case insensitive
    assert!(t.matches_search_term("supermarket"));

    // 5. Negative case
    assert!(!t.matches_search_term("Office"));
}

#[test]
fn test_mixed_filters() {
    let mut t = create_task_with_loc("Meeting", "Office");
    t.priority = 1;

    // Matches Priority AND Location implicitly
    assert!(t.matches_search_term("!1 Office"));

    // Matches Priority but NOT Location
    assert!(!t.matches_search_term("!1 Home"));
}
