// SPDX-License-Identifier: GPL-3.0-or-later
//! Regression tests: Enter in the TUI journal day pane must create the day's
//! journal entry from the editor text — both when an external editor produced
//! new text (it was silently discarded) and when typing in the builtin editor.
use cfait::context::{AppContext, TestContext};
use cfait::tui::action::{Action, SidebarMode};
use cfait::tui::handlers::handle_key_event;
use cfait::tui::state::{AppState, Focus, InputMode};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::sync::Arc;
use tokio::sync::mpsc;

fn journal_state(ctx: Arc<TestContext>, day: chrono::NaiveDate) -> AppState {
    let mut state = AppState::new_with_ctx(ctx);
    state.sidebar_mode = SidebarMode::Journal;
    state.active_focus = Focus::Main;
    state.active_cal_href = Some("local://default".to_string());
    state.journal_date = day;
    state
}

fn find_create(actions: &[cfait::journal::Action]) -> Option<&cfait::model::Task> {
    actions.iter().find_map(|a| match a {
        cfait::journal::Action::Create(t) => Some(t),
        _ => None,
    })
}

#[tokio::test]
async fn test_journal_enter_applies_external_editor_text() {
    let ctx = Arc::new(TestContext::new());
    // A fake external editor: writes fixed text into the temp file it is
    // handed, the way micro/vim leave the buffer after the user saves.
    let config_dir = ctx.get_config_dir().unwrap();
    std::fs::write(
        config_dir.join("config.toml"),
        "description_editor = \"printf 'Sanded the benches' >\"\n",
    )
    .unwrap();

    let day = chrono::NaiveDate::from_ymd_opt(2026, 10, 19).unwrap();
    let mut state = journal_state(ctx, day);

    let (action_tx, mut action_rx) = mpsc::channel(4);
    let result = handle_key_event(
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        &mut state,
        &action_tx,
    )
    .await;
    assert!(result.is_none(), "journal Enter handles the edit itself");

    // The batch is sent synchronously while handling the key, so it is
    // already queued; try_recv keeps a regression from hanging the test.
    let action = action_rx
        .try_recv()
        .expect("expected a PersistBatch with the created entry");
    let actions = match action {
        Action::PersistBatch(actions) => actions,
        other => panic!("expected PersistBatch, got {other:?}"),
    };
    let entry = find_create(&actions).expect("expected a Create action for the new entry");
    assert!(entry.is_journal, "expected a VJOURNAL component");
    assert_eq!(entry.summary, "2026-10-19");
    assert_eq!(entry.calendar_href, "local://default");
    assert_eq!(entry.dtstart, Some(cfait::model::DateType::AllDay(day)));

    // The entry resolves as the day's journal entry and holds the text that
    // was typed in the external editor.
    let stored = state
        .store
        .get_journal_entry("local://default", day)
        .expect("journal entry should exist after save");
    assert_eq!(stored.uid, entry.uid);
    assert!(
        stored.description.contains("Sanded the benches"),
        "external editor text was discarded: {stored:?}"
    );
    assert!(matches!(state.mode, InputMode::Normal));
}

#[tokio::test]
async fn test_journal_enter_builtin_editor_creates_entry_on_save() {
    let ctx = Arc::new(TestContext::new());
    let config_dir = ctx.get_config_dir().unwrap();
    std::fs::write(
        config_dir.join("config.toml"),
        "description_editor = \"builtin\"\n",
    )
    .unwrap();

    let day = chrono::NaiveDate::from_ymd_opt(2026, 10, 20).unwrap();
    let mut state = journal_state(ctx, day);

    let (action_tx, mut action_rx) = mpsc::channel(4);
    handle_key_event(
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        &mut state,
        &action_tx,
    )
    .await;
    assert!(
        matches!(state.mode, InputMode::EditingDescription),
        "builtin editor should open the in-TUI editor"
    );
    assert!(action_rx.try_recv().is_err(), "nothing to persist yet");

    // Type the note text, then save with Ctrl+S.
    for c in "Watered the tomatoes".chars() {
        handle_key_event(
            KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE),
            &mut state,
            &action_tx,
        )
        .await;
    }
    handle_key_event(
        KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
        &mut state,
        &action_tx,
    )
    .await;

    let action = action_rx
        .try_recv()
        .expect("expected a PersistBatch with the created entry");
    let actions = match action {
        Action::PersistBatch(actions) => actions,
        other => panic!("expected PersistBatch, got {other:?}"),
    };
    let entry = find_create(&actions).expect("expected a Create action for the new entry");
    assert!(entry.is_journal, "expected a VJOURNAL component");
    assert_eq!(entry.summary, "2026-10-20");
    assert_eq!(entry.dtstart, Some(cfait::model::DateType::AllDay(day)));

    let stored = state
        .store
        .get_journal_entry("local://default", day)
        .expect("journal entry should exist after save");
    assert!(
        stored.description.contains("Watered the tomatoes"),
        "typed text missing: {stored:?}"
    );
    assert!(matches!(state.mode, InputMode::Normal));
}
