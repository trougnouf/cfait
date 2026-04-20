// SPDX-License-Identifier: GPL-3.0-or-later
use cfait::context::TestContext;
use cfait::model::CalendarListEntry;
use cfait::tui::action::Action;
use cfait::tui::handlers::handle_key_event;
use cfait::tui::state::{AppState, InputMode};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::sync::Arc;
use tokio::sync::mpsc;

#[tokio::test]
async fn test_tui_creating_tag_only_input_creates_task_instead_of_filtering() {
    let ctx = Arc::new(TestContext::new());
    let mut state = AppState::new_with_ctx(ctx);
    let cal_href = "https://example.test/cal/".to_string();

    state.calendars.push(CalendarListEntry {
        name: "Remote".to_string(),
        href: cal_href.clone(),
        color: None,
    });
    state.active_cal_href = Some(cal_href);
    state.mode = InputMode::Creating;
    state.input_buffer = "#work".to_string();
    state.cursor_position = state.input_buffer.chars().count();

    let (action_tx, _action_rx) = mpsc::channel(1);
    let action = handle_key_event(
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        &mut state,
        &action_tx,
    )
    .await;

    match action {
        Some(Action::CreateTask(task)) => {
            assert!(
                task.categories.contains(&"work".to_string()),
                "Expected the smart input to become a task tag"
            );
        }
        other => panic!("Expected CreateTask, got {:?}", other),
    }

    assert!(
        state.selected_categories.is_empty(),
        "Creating mode should no longer convert #tag input into a sidebar filter"
    );
    assert!(matches!(state.mode, InputMode::Normal));
}
