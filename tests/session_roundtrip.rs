// File: tests/session_roundtrip.rs
use cfait::model::{Task, item::WorkSession};
use std::collections::HashMap;

#[test]
fn test_session_roundtrip_persistence() {
    let aliases = HashMap::new();
    let mut original = Task::new("Work Task", &aliases, None);

    // Add two sessions
    original.sessions.push(WorkSession {
        start: 1700000000,
        end: 1700003600, // 1 hour
    });
    original.sessions.push(WorkSession {
        start: 1700007200,
        end: 1700010800, // 1 hour
    });

    // Serialize to ICS
    let ics_string = original.to_ics();

    // Ensure the custom property is present in the output
    assert!(ics_string.contains("X-CFAIT-SESSION:1700000000,1700003600"));
    assert!(ics_string.contains("X-CFAIT-SESSION:1700007200,1700010800"));

    // Deserialize back
    let restored = Task::from_ics(
        &ics_string,
        "etag".to_string(),
        "href".to_string(),
        "cal".to_string(),
    )
    .expect("Failed to parse ICS");

    // Verify consistency
    assert_eq!(restored.sessions.len(), 2);
    assert_eq!(restored.sessions[0].start, 1700000000);
    assert_eq!(restored.sessions[0].end, 1700003600);
    assert_eq!(restored.sessions[1].start, 1700007200);
    assert_eq!(restored.sessions[1].end, 1700010800);
}
