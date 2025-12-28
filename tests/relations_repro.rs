// File: tests/relations_repro.rs
use cfait::model::Task;
use std::collections::HashMap;

#[test]
fn test_parent_and_dependency_coexistence() {
    let mut task = Task::new("Mixed Relations", &HashMap::new(), None);
    task.parent_uid = Some("parent-123".to_string());
    task.dependencies.push("blocker-456".to_string());

    let ics = task.to_ics();

    // Verify string content contains both (this fails if one overwrites the other)
    let parent_present = ics.contains("RELATED-TO:parent-123");
    let dep_present = ics.contains("RELATED-TO;RELTYPE=DEPENDS-ON:blocker-456");

    if !parent_present || !dep_present {
        println!("Generated ICS:\n{}", ics);
    }

    assert!(parent_present, "ICS missing Parent relation");
    assert!(dep_present, "ICS missing Dependency relation");

    // Roundtrip check
    let parsed =
        Task::from_ics(&ics, "etag".into(), "href".into(), "cal".into()).expect("Parse failed");

    assert_eq!(
        parsed.parent_uid,
        Some("parent-123".to_string()),
        "Parent UID lost in roundtrip"
    );
    assert!(
        parsed.dependencies.contains(&"blocker-456".to_string()),
        "Dependency lost in roundtrip"
    );
}

#[test]
fn test_multiple_dependencies_with_parent() {
    let mut task = Task::new("Complex Relations", &HashMap::new(), None);
    task.parent_uid = Some("parent-abc".to_string());
    task.dependencies.push("dep-1".to_string());
    task.dependencies.push("dep-2".to_string());
    task.dependencies.push("dep-3".to_string());

    let ics = task.to_ics();

    // Verify all relations are present in ICS
    assert!(
        ics.contains("RELATED-TO:parent-abc"),
        "Parent missing in ICS"
    );
    assert!(
        ics.contains("RELATED-TO;RELTYPE=DEPENDS-ON:dep-1"),
        "Dep-1 missing"
    );
    assert!(
        ics.contains("RELATED-TO;RELTYPE=DEPENDS-ON:dep-2"),
        "Dep-2 missing"
    );
    assert!(
        ics.contains("RELATED-TO;RELTYPE=DEPENDS-ON:dep-3"),
        "Dep-3 missing"
    );

    // Roundtrip
    let parsed =
        Task::from_ics(&ics, "etag".into(), "href".into(), "cal".into()).expect("Parse failed");

    assert_eq!(parsed.parent_uid, Some("parent-abc".to_string()));
    assert_eq!(parsed.dependencies.len(), 3);
    assert!(parsed.dependencies.contains(&"dep-1".to_string()));
    assert!(parsed.dependencies.contains(&"dep-2".to_string()));
    assert!(parsed.dependencies.contains(&"dep-3".to_string()));
}

#[test]
fn test_only_parent_no_dependencies() {
    let mut task = Task::new("Parent Only", &HashMap::new(), None);
    task.parent_uid = Some("parent-xyz".to_string());

    let ics = task.to_ics();
    let parsed =
        Task::from_ics(&ics, "etag".into(), "href".into(), "cal".into()).expect("Parse failed");

    assert_eq!(parsed.parent_uid, Some("parent-xyz".to_string()));
    assert!(parsed.dependencies.is_empty());
}

#[test]
fn test_only_dependencies_no_parent() {
    let mut task = Task::new("Deps Only", &HashMap::new(), None);
    task.dependencies.push("blocker-a".to_string());
    task.dependencies.push("blocker-b".to_string());

    let ics = task.to_ics();
    let parsed =
        Task::from_ics(&ics, "etag".into(), "href".into(), "cal".into()).expect("Parse failed");

    assert_eq!(parsed.parent_uid, None);
    assert_eq!(parsed.dependencies.len(), 2);
    assert!(parsed.dependencies.contains(&"blocker-a".to_string()));
    assert!(parsed.dependencies.contains(&"blocker-b".to_string()));
}
