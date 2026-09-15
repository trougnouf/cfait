// SPDX-License-Identifier: GPL-3.0-or-later
//! Tests for inline data: URI stripping at the model boundary.
//! data: URIs are stripped from task.description during from_ics (replaced
//! with cfait-media://UUID placeholders) and from user input, so multi-MB
//! blobs never enter UI state or editor buffers. The original payloads are
//! stored in inline_media (a HashMap keyed by UUID) and re-injected by to_ics
//! for any placeholders that survive in the edited description.
use cfait::model::IcsAdapter;
use cfait::model::extractor::extract_markdown_tasks;

fn make_ics(description: &str) -> String {
    format!(
        "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Test//Test//EN\r\n\
         BEGIN:VTODO\r\nUID:test-uid@example.com\r\nSUMMARY:water plants\r\n\
         DESCRIPTION:{description}\r\n\
         END:VTODO\r\nEND:VCALENDAR\r\n"
    )
}

fn count_placeholders(text: &str) -> usize {
    text.matches("cfait-media://").count()
}

#[test]
fn from_ics_strips_data_uri_to_placeholder() {
    let blob = "A".repeat(100_000);
    let ics = make_ics(&format!("![garden](data:image/png;base64,{blob})"));
    let task = IcsAdapter::from_ics(
        &ics,
        "etag".to_string(),
        "href".to_string(),
        "cal".to_string(),
    )
    .unwrap();
    assert!(!task.description.contains("data:"));
    assert!(!task.description.contains(&blob));
    assert_eq!(count_placeholders(&task.description), 1);
    assert_eq!(task.inline_media.len(), 1);
    let payload = task.inline_media.values().next().unwrap();
    assert!(payload.contains("data:image/png;base64,"));
    assert!(payload.contains(&blob));
}

#[test]
fn from_ics_without_data_uri_has_no_inline_media() {
    let ics = make_ics("water the tomatoes and note the weather");
    let task = IcsAdapter::from_ics(
        &ics,
        "etag".to_string(),
        "href".to_string(),
        "cal".to_string(),
    )
    .unwrap();
    assert_eq!(task.description, "water the tomatoes and note the weather");
    assert!(task.inline_media.is_empty());
}

#[test]
fn to_ics_re_injects_placeholder_on_unchanged_description() {
    let blob = "A".repeat(100_000);
    let ics = make_ics(&format!("see ![photo](data:image/png;base64,{blob})"));
    let task = IcsAdapter::from_ics(
        &ics,
        "etag".to_string(),
        "href".to_string(),
        "cal".to_string(),
    )
    .unwrap();
    let roundtrip = IcsAdapter::to_ics(&task);
    assert!(roundtrip.contains("data:image/png"));
    assert_eq!(count_placeholders(&roundtrip), 0);
}

#[test]
fn to_ics_re_injects_after_typo_fix_around_placeholder() {
    let blob = "A".repeat(100_000);
    let ics = make_ics(&format!(
        "line one has a typo\n\n![photo](data:image/png;base64,{blob})"
    ));
    let mut task = IcsAdapter::from_ics(
        &ics,
        "etag".to_string(),
        "href".to_string(),
        "cal".to_string(),
    )
    .unwrap();
    task.description = task
        .description
        .replace("line one has a typo", "line one has a fix");
    let roundtrip = IcsAdapter::to_ics(&task);
    assert!(roundtrip.contains("data:image/png"));
    assert!(roundtrip.contains("line one has a fix"));
    assert_eq!(count_placeholders(&roundtrip), 0);
}

#[test]
fn to_ics_drops_placeholder_when_user_deletes_it() {
    let blob = "A".repeat(100_000);
    let ics = make_ics(&format!("see ![photo](data:image/png;base64,{blob})"));
    let mut task = IcsAdapter::from_ics(
        &ics,
        "etag".to_string(),
        "href".to_string(),
        "cal".to_string(),
    )
    .unwrap();
    task.description = "just text now".to_string();
    let roundtrip = IcsAdapter::to_ics(&task);
    assert!(roundtrip.contains("just text now"));
    assert!(!roundtrip.contains("data:image/png"));
}

#[test]
fn from_ics_strips_multiple_data_uris() {
    let ics = make_ics("a(data:image/png;base64,QUJD) b(data:image/jpeg;base64,DEF)");
    let task = IcsAdapter::from_ics(
        &ics,
        "etag".to_string(),
        "href".to_string(),
        "cal".to_string(),
    )
    .unwrap();
    assert!(!task.description.contains("data:"));
    assert_eq!(count_placeholders(&task.description), 2);
    assert_eq!(task.inline_media.len(), 2);
}

#[test]
fn data_in_http_url_not_stripped() {
    let ics = make_ics("see https://example.com/data:foo");
    let task = IcsAdapter::from_ics(
        &ics,
        "etag".to_string(),
        "href".to_string(),
        "cal".to_string(),
    )
    .unwrap();
    assert!(task.description.contains("https://example.com/data:foo"));
    assert!(task.inline_media.is_empty());
}

#[test]
fn journal_strips_and_round_trips_data_uri() {
    let blob = "B".repeat(50_000);
    let ics = format!(
        "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Test//Test//EN\r\n\
         BEGIN:VJOURNAL\r\nUID:journal-uid@example.com\r\nSUMMARY:garden notes\r\n\
         DESCRIPTION:![seedling](data:image/png;base64,{blob})\r\n\
         END:VJOURNAL\r\nEND:VCALENDAR\r\n"
    );
    let task = IcsAdapter::from_ics(
        &ics,
        "etag".to_string(),
        "href".to_string(),
        "cal".to_string(),
    )
    .unwrap();
    assert!(!task.description.contains("data:"));
    assert_eq!(count_placeholders(&task.description), 1);
    assert_eq!(task.inline_media.len(), 1);

    let roundtrip = IcsAdapter::to_ics(&task);
    assert!(roundtrip.contains("data:image/png"));
    assert_eq!(count_placeholders(&roundtrip), 0);
}

#[test]
fn extract_markdown_tasks_strips_pasted_data_uris() {
    let blob = "C".repeat(50_000);
    let input = format!("water plants\n\n![garden](data:image/png;base64,{blob})");
    let mut media = std::collections::HashMap::new();
    let (cleaned, extracted) = extract_markdown_tasks(&input, false, &mut media);
    assert!(!cleaned.contains("data:"));
    assert!(!cleaned.contains(&blob));
    assert_eq!(count_placeholders(&cleaned), 1);
    assert_eq!(media.len(), 1);
    let payload = media.values().next().unwrap();
    assert!(payload.contains("data:image/png;base64,"));
    assert!(extracted.is_empty() || extracted.iter().all(|t| !t.description.contains("data:")));
}

#[test]
fn extract_markdown_tasks_distributes_media_to_subtasks() {
    let blob = "D".repeat(50_000);
    let input = format!(
        "root text\n\n\
         - [ ] subtask\n  ![photo](data:image/png;base64,{blob})"
    );
    let mut media = std::collections::HashMap::new();
    let (cleaned, extracted) = extract_markdown_tasks(&input, false, &mut media);
    assert!(!cleaned.contains("data:"));
    assert_eq!(extracted.len(), 1);
    assert!(!extracted[0].description.contains("data:"));
    assert!(!extracted[0].description.contains(&blob));
    assert_eq!(extracted[0].inline_media.len(), 1);
    let payload = extracted[0].inline_media.values().next().unwrap();
    assert!(payload.contains("data:image/png;base64,"));
    assert!(media.is_empty());
}
