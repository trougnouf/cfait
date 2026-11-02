// SPDX-License-Identifier: GPL-3.0-or-later
//! Regression tests: the title of a newly created wiki page must stay visible
//! in the journal tab after switching to the task list and back (reported by
//! wunter8), and a bare `is:page` token must not leak into the page's summary.
use cfait::context::{AppContext, TestContext};
use cfait::tui::action::SidebarMode;
use cfait::tui::handlers::handle_key_event;
use cfait::tui::state::{AppState, Focus};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::sync::Arc;
use tokio::sync::mpsc;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}

fn journal_state(ctx: Arc<TestContext>) -> AppState {
    let config_dir = ctx.get_config_dir().unwrap();
    std::fs::write(
        config_dir.join("config.toml"),
        "description_editor = \"builtin\"\n",
    )
    .unwrap();
    let mut state = AppState::new_with_ctx(ctx);
    state.local_mode_enabled = true;
    state.sidebar_mode = SidebarMode::Journal;
    state.active_focus = Focus::Main;
    state.active_cal_href = Some("local://default".to_string());
    state.journal_date = chrono::NaiveDate::from_ymd_opt(2026, 11, 2).unwrap();
    state.expanded_tags.insert("j:pages".to_string());
    state
}

macro_rules! type_str {
    ($state:expr, $tx:expr, $s:expr) => {
        for c in $s.chars() {
            handle_key_event(key(KeyCode::Char(c)), $state, $tx).await;
        }
    };
}

/// The reported flow: create a wiki page in the journal tab (Ctrl+N), type a
/// title and contents, save, switch to the task list tab and back to the
/// journal tab — the page must still be listed under its title.
#[tokio::test]
async fn wiki_page_title_survives_tab_switch() {
    let ctx = Arc::new(TestContext::new());
    let mut state = journal_state(ctx);
    let (action_tx, _rx) = mpsc::channel(64);

    handle_key_event(ctrl('n'), &mut state, &action_tx).await;
    assert_eq!(state.input_buffer, "is:page ");
    type_str!(&mut state, &action_tx, "My page");
    handle_key_event(key(KeyCode::Enter), &mut state, &action_tx).await;
    type_str!(&mut state, &action_tx, "Some contents");
    handle_key_event(ctrl('s'), &mut state, &action_tx).await;

    let page = state
        .store
        .calendars
        .values()
        .flat_map(|m| m.values())
        .find(|t| t.is_journal && t.dtstart.is_none())
        .expect("page should exist")
        .clone();
    assert_eq!(page.summary, "My page");
    assert_eq!(page.description, "Some contents");

    handle_key_event(key(KeyCode::Char('1')), &mut state, &action_tx).await;
    assert_eq!(state.sidebar_mode, SidebarMode::Calendars);
    handle_key_event(key(KeyCode::Char('5')), &mut state, &action_tx).await;
    assert_eq!(state.sidebar_mode, SidebarMode::Journal);

    let shown = state
        .cached_journal_pages
        .iter()
        .find(|p| p.key == page.uid)
        .expect("page should still be listed in the journal sidebar");
    assert_eq!(shown.title, "My page", "page title must stay visible");
}

/// Creating a page with a bare `is:page` prefix (no title on the input line)
/// must not title the page "is:page": the summary stays empty so the UI shows
/// the untitled placeholder, while the typed body is kept in the entry.
#[tokio::test]
async fn bare_is_page_does_not_become_the_title() {
    let ctx = Arc::new(TestContext::new());
    let mut state = journal_state(ctx);
    let (action_tx, _rx) = mpsc::channel(64);

    handle_key_event(ctrl('n'), &mut state, &action_tx).await;
    assert_eq!(state.input_buffer, "is:page ");
    handle_key_event(key(KeyCode::Enter), &mut state, &action_tx).await;
    type_str!(&mut state, &action_tx, "My Title\nSome contents");
    handle_key_event(ctrl('s'), &mut state, &action_tx).await;

    let page = state
        .store
        .calendars
        .values()
        .flat_map(|m| m.values())
        .find(|t| t.is_journal && t.dtstart.is_none())
        .expect("page should exist")
        .clone();
    assert_eq!(
        page.summary, "",
        "a bare is:page token must not become the page title"
    );
    assert_eq!(page.description, "My Title\nSome contents");

    handle_key_event(key(KeyCode::Char('1')), &mut state, &action_tx).await;
    handle_key_event(key(KeyCode::Char('5')), &mut state, &action_tx).await;
    let shown = state
        .cached_journal_pages
        .iter()
        .find(|p| p.key == page.uid)
        .expect("page should be listed in the journal sidebar");
    assert_ne!(shown.title, "is:page");
}
