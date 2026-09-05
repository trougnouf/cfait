use cfait::model::parser::{InlineElement, parse_inline_markdown};

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
