// SPDX-License-Identifier: GPL-3.0-or-later
//! Regression tests for pressing E in the TUI journal tab:
//! - on a wiki page, the smart input line must carry the page's `is:archived`
//!   flag and `#tags`, so saving the edit neither moves an archived page out
//!   of the journal Archive nor drops its categories;
//! - on a daily (dated) entry, the title line must carry the `^date` anchor
//!   and the entry's `#tags`, so saving the edit keeps the entry on its day
//!   and lets the tags be edited.
use cfait::context::TestContext;
use cfait::model::{CalendarListEntry, DateType, Task};
use cfait::tui::action::SidebarMode;
use cfait::tui::handlers::handle_key_event;
use cfait::tui::state::{AppState, Focus, InputMode};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;

fn journal_state(ctx: Arc<TestContext>) -> AppState {
    let mut state = AppState::new_with_ctx(ctx);
    state.calendars.push(CalendarListEntry {
        name: "Local".to_string(),
        href: "local://default".to_string(),
        color: None,
        supports_vjournal: Some(true),
        supports_vevent: Some(true),
    });
    state.active_cal_href = Some("local://default".to_string());
    state.sidebar_mode = SidebarMode::Journal;
    state.active_focus = Focus::Main;
    state.journal_date = chrono::NaiveDate::from_ymd_opt(2026, 10, 31).unwrap();
    state
}

fn add_page(state: &mut AppState, input: &str) -> String {
    let mut page = Task::new(input, &HashMap::new(), None);
    page.calendar_href = "local://default".to_string();
    let uid = page.uid.clone();
    state.store.add_task(page);
    uid
}

#[tokio::test]
async fn test_e_on_archived_page_keeps_is_archived_token() {
    let ctx = Arc::new(TestContext::new());
    let mut state = journal_state(ctx);
    let uid = add_page(&mut state, "Old plans is:page is:archived #garden @@home");
    assert!(state.store.get_task_ref(&uid).unwrap().archived);
    // The page body carries tag-like prose and a tagged child; an E edit of
    // the title line must leave the body byte-identical.
    let body = "Body notes about #garden\n- [ ] Water the tomatoes #plants";
    if let Some((t, _)) = state.store.get_task_mut(&uid) {
        t.description = body.to_string();
    }
    state.journal_editing_uid = Some(uid.clone());

    let (action_tx, _rx) = mpsc::channel(4);
    handle_key_event(
        KeyEvent::new(KeyCode::Char('E'), KeyModifiers::NONE),
        &mut state,
        &action_tx,
    )
    .await;

    assert!(matches!(state.mode, InputMode::Editing));
    assert_eq!(state.editing_uid.as_deref(), Some(uid.as_str()));
    assert_eq!(
        state.input_buffer,
        "is:page Old plans #garden @@home is:archived"
    );

    // Saving the edit must not un-archive the page or drop its tags/locations.
    handle_key_event(
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        &mut state,
        &action_tx,
    )
    .await;

    let saved = state
        .store
        .get_task_ref(&uid)
        .expect("page should still be stored");
    assert!(saved.archived, "saving the E edit un-archived the page");
    assert!(saved.is_journal, "saving the E edit un-paged the page");
    assert_eq!(saved.summary, "Old plans");
    assert!(
        saved.categories.contains(&"garden".to_string()),
        "saving the E edit dropped the page's categories: {saved:?}"
    );
    assert!(
        saved.locations.contains(&"home".to_string()),
        "saving the E edit dropped the page's locations: {saved:?}"
    );
    assert_eq!(
        saved.description, body,
        "saving the E edit modified the page body"
    );
    assert!(matches!(state.mode, InputMode::Normal));
}

#[tokio::test]
async fn test_e_on_active_page_has_no_is_archived_token() {
    let ctx = Arc::new(TestContext::new());
    let mut state = journal_state(ctx);
    let uid = add_page(&mut state, "Garden ideas is:page #home");
    state.journal_editing_uid = Some(uid.clone());

    let (action_tx, _rx) = mpsc::channel(4);
    handle_key_event(
        KeyEvent::new(KeyCode::Char('E'), KeyModifiers::NONE),
        &mut state,
        &action_tx,
    )
    .await;

    assert!(matches!(state.mode, InputMode::Editing));
    assert_eq!(state.input_buffer, "is:page Garden ideas #home");
}

#[tokio::test]
async fn test_e_on_daily_entry_keeps_date_anchor_and_edits_tags() {
    let ctx = Arc::new(TestContext::new());
    let mut state = journal_state(ctx);
    let day = chrono::NaiveDate::from_ymd_opt(2026, 10, 31).unwrap();

    // A daily entry with a tag, anchored to its date via ^date.
    let uid = add_page(&mut state, "2026-10-31 ^2026-10-31 is:page #mood");
    {
        let entry = state.store.get_task_ref(&uid).unwrap();
        assert_eq!(entry.dtstart, Some(DateType::AllDay(day)));
        assert!(entry.categories.contains(&"mood".to_string()));
    }
    state.journal_editing_uid = None; // main pane shows the day

    let (action_tx, _rx) = mpsc::channel(4);
    handle_key_event(
        KeyEvent::new(KeyCode::Char('E'), KeyModifiers::NONE),
        &mut state,
        &action_tx,
    )
    .await;

    // The title line carries the date anchor and the current tags.
    assert!(matches!(state.mode, InputMode::Editing));
    assert_eq!(state.editing_uid.as_deref(), Some(uid.as_str()));
    assert_eq!(state.input_buffer, "is:page 2026-10-31 ^2026-10-31 #mood");

    // Add another tag on the line, then save.
    state.input_buffer.push_str(" #plans");
    state.cursor_position = state.input_buffer.chars().count();
    handle_key_event(
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        &mut state,
        &action_tx,
    )
    .await;

    let saved = state
        .store
        .get_task_ref(&uid)
        .expect("daily entry should still be stored");
    assert_eq!(
        saved.dtstart,
        Some(DateType::AllDay(day)),
        "saving the E edit un-anchored the daily entry from its date"
    );
    assert!(saved.is_journal);
    assert_eq!(saved.summary, "2026-10-31");
    assert!(
        saved.categories.contains(&"mood".to_string())
            && saved.categories.contains(&"plans".to_string()),
        "saving the E edit dropped the daily entry's tags: {saved:?}"
    );
    assert!(
        state
            .store
            .get_journal_entry("local://default", day)
            .is_some_and(|e| e.uid == uid),
        "daily entry no longer resolves for its date"
    );
}
