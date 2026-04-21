// SPDX-License-Identifier: GPL-3.0-or-later
use cfait::context::TestContext;
use cfait::model::CalendarListEntry;
use cfait::tui::action::AppEvent;
use cfait::tui::handlers::handle_app_event;
use cfait::tui::state::AppState;
use std::sync::Arc;

#[test]
fn test_tui_local_mode_disabled_filters_local_calendars() {
    let mut state = AppState::new_with_ctx(Arc::new(TestContext::new()));
    state.local_mode_enabled = false;

    handle_app_event(
        &mut state,
        AppEvent::CalendarsLoaded(vec![
            CalendarListEntry {
                name: "Local".to_string(),
                href: "local://default".to_string(),
                color: None,
            },
            CalendarListEntry {
                name: "Remote".to_string(),
                href: "https://example.test/cal/".to_string(),
                color: None,
            },
        ]),
        &Some("local://default".to_string()),
    );

    assert_eq!(state.calendars.len(), 1);
    assert_eq!(state.calendars[0].href, "https://example.test/cal/");
    assert_eq!(
        state.active_cal_href.as_deref(),
        Some("https://example.test/cal/")
    );
}
