use cfait::model::Task;
use cfait::model::parser::{InlineElement, parse_inline_markdown};
use std::collections::HashMap;

fn links(text: &str) -> Vec<(&str, &str)> {
    parse_inline_markdown(text)
        .into_iter()
        .filter_map(|el| match el {
            InlineElement::Link { text, url, .. } => Some((text, url)),
            _ => None,
        })
        .collect()
}

#[test]
fn bare_url() {
    assert_eq!(
        links("see https://example.com/foo"),
        &[("https://example.com/foo", "https://example.com/foo")]
    );
}

#[test]
fn md_link() {
    assert_eq!(
        links("go [Google](https://google.com) now"),
        &[("Google", "https://google.com")]
    );
}

#[test]
fn wiki_link() {
    assert_eq!(links("see [[Page Name]]"), &[("Page Name", "Page Name")]);
}

#[test]
fn wiki_link_alias() {
    assert_eq!(links("see [[Target|Display]]"), &[("Display", "Target")]);
}

#[test]
fn mailto() {
    assert_eq!(
        links("mail me@example.com mailto:me@example.com"),
        &[("mailto:me@example.com", "mailto:me@example.com")]
    );
}

fn task_with(description: &str, url: Option<&str>) -> Task {
    let mut t = Task::new("Water the herbs", &HashMap::new(), None);
    t.description = description.to_string();
    t.url = url.map(|s| s.to_string());
    t
}

#[test]
fn first_url_bare_in_description() {
    let t = task_with("see https://example.com/beds for the plan", None);
    assert_eq!(t.first_url().as_deref(), Some("https://example.com/beds"));
}

#[test]
fn first_url_markdown_link_in_description() {
    let t = task_with(
        "recipe at [Tomato Soup](https://cook.example.com/soup)",
        None,
    );
    assert_eq!(
        t.first_url().as_deref(),
        Some("https://cook.example.com/soup")
    );
}

#[test]
fn first_url_mailto_in_description() {
    let t = task_with("ask the baker mailto:baker@example.com", None);
    assert_eq!(t.first_url().as_deref(), Some("mailto:baker@example.com"));
}

#[test]
fn first_url_prefers_description_over_url_field() {
    let t = task_with(
        "notes: https://notes.example.com/page",
        Some("https://explicit.example.com"),
    );
    assert_eq!(
        t.first_url().as_deref(),
        Some("https://notes.example.com/page")
    );
}

#[test]
fn first_url_falls_back_to_url_field() {
    let t = task_with(
        "no links here, just prose",
        Some("https://explicit.example.com"),
    );
    assert_eq!(
        t.first_url().as_deref(),
        Some("https://explicit.example.com")
    );
}

#[test]
fn first_url_ignores_wiki_and_media_links() {
    let t = task_with(
        "see [[Compost Log]] and ![bed](cfait-media://abc-123)",
        Some("https://fallback.example.com"),
    );
    assert_eq!(
        t.first_url().as_deref(),
        Some("https://fallback.example.com")
    );
}

#[test]
fn first_url_none_when_no_links() {
    let t = task_with("just a plain note about pruning", None);
    assert_eq!(t.first_url(), None);
}

#[test]
fn first_url_picks_first_link_on_earliest_line() {
    let t = task_with(
        "first line https://a.example.com\nsecond line https://b.example.com",
        None,
    );
    assert_eq!(t.first_url().as_deref(), Some("https://a.example.com"));
}
