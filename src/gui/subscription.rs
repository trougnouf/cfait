// Handles event subscriptions (keyboard, window) for the GUI.
use crate::gui::message::Message;
use crate::gui::state::{AppState, GuiApp, SidebarMode};
use iced::{Subscription, event, keyboard, window};

pub fn subscription(app: &GuiApp) -> Subscription<Message> {
    use iced::keyboard::key::Named;

    let mut subs = Vec::new();

    if matches!(app.state, AppState::Onboarding | AppState::Settings) {
        subs.push(keyboard::listen().filter_map(|event| {
            if let keyboard::Event::KeyPressed { key, modifiers, .. } = event
                && key == keyboard::Key::Named(Named::Tab)
            {
                return Some(Message::TabPressed(modifiers.shift()));
            }
            None
        }));
    }

    if app.state == AppState::Active {
        // Use a static function to handle hotkeys so we don't capture `app`
        // This avoids the "expected fn pointer, found closure" error
        subs.push(event::listen_with(handle_hotkey));
    }

    // Track window metrics (Size)
    subs.push(event::listen_with(|evt, _status, _window_id| match evt {
        iced::Event::Window(window::Event::Resized(size)) => Some(Message::WindowResized(size)),
        _ => None,
    }));

    // Auto-refresh subscription (configurable)
    // If config load succeeds and interval > 0, subscribe to a periodic timer that maps to Message::Refresh.
    if let Ok(cfg) = crate::config::Config::load(app.ctx.as_ref())
        && cfg.auto_refresh_interval_mins > 0
    {
        subs.push(
            iced::time::every(std::time::Duration::from_secs(
                cfg.auto_refresh_interval_mins as u64 * 60,
            ))
            .map(|_| Message::Refresh),
        );
    }

    // Tick every minute if there is an active task running, so the timer updates visually
    let has_running_tasks = app.tasks.iter().any(|t| t.last_started_at.is_some());
    if has_running_tasks {
        subs.push(iced::time::every(std::time::Duration::from_secs(60)).map(|_| Message::Tick));
    }

    Subscription::batch(subs)
}

fn handle_hotkey(
    evt: iced::Event,
    status: iced::event::Status,
    _id: iced::window::Id,
) -> Option<Message> {
    use iced::keyboard::key::Named;

    // Allow Escape to work even when input is captured — notify the app that Esc was pressed while captured.
    // The tasks update handler will decide whether to CancelEdit immediately (modal) or to SnapToSelected (non-modal).
    if status == iced::event::Status::Captured {
        if let iced::Event::Keyboard(keyboard::Event::KeyPressed { key, .. }) = evt
            && key == keyboard::Key::Named(Named::Escape)
        {
            return Some(Message::EscCaptured);
        }
        return None;
    }

    if let iced::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) = evt {
        // Ignore if Ctrl/Alt/Cmd is held (avoids conflict with system shortcuts)
        if modifiers.command() || modifiers.control() || modifiers.alt() {
            return None;
        }

        match key.as_ref() {
            // 1. Handle character-based keys first
            keyboard::Key::Character(s) => {
                let s_lower = s.to_lowercase();
                // Match on lowercase char + shift state tuple for alphabetic keys
                match (s_lower.as_str(), modifiers.shift()) {
                    ("j", false) => Some(Message::SelectNextTask),
                    ("k", false) => Some(Message::SelectPrevTask),
                    ("d", false) => Some(Message::DeleteSelected),
                    ("e", false) => Some(Message::EditSelected),
                    ("e", true) => Some(Message::EditSelectedDescription),
                    ("s", false) => Some(Message::ToggleActiveSelected),
                    ("s", true) => Some(Message::StopSelected),
                    ("x", false) => Some(Message::CancelSelected),
                    ("y", false) => Some(Message::YankSelected),
                    ("c", false) => Some(Message::KeyboardLinkChild),
                    ("c", true) => Some(Message::KeyboardCreateChild),
                    ("b", false) => Some(Message::KeyboardAddDependency),
                    ("l", false) => Some(Message::KeyboardAddRelation),
                    ("a", false) => Some(Message::FocusInput),
                    ("h", true) => Some(Message::ToggleHideCompletedToggle),
                    ("m", false) => Some(Message::CategoryMatchModeToggle),
                    ("m", true) => Some(Message::EditSelected), // 'M' for Move (parity)
                    ("q", false) => Some(Message::CloseWindow),
                    ("r", false) => Some(Message::Refresh),
                    ("r", true) => Some(Message::JumpToRandomTask),
                    // Fallback to match exact char for symbols and numbers
                    _ => match s {
                        "1" => Some(Message::SidebarModeChanged(SidebarMode::Calendars)),
                        "2" => Some(Message::SidebarModeChanged(SidebarMode::Categories)),
                        "3" => Some(Message::SidebarModeChanged(SidebarMode::Locations)),
                        "/" | "?" => Some(Message::FocusSearch),
                        "*" => Some(Message::ClearAllFilters),
                        "+" | "=" => Some(Message::ChangePrioritySelected(1)),
                        "-" => Some(Message::ChangePrioritySelected(-1)),
                        "." | ">" => Some(Message::DemoteSelected),
                        "," | "<" => Some(Message::PromoteSelected),
                        _ => None,
                    },
                }
            }

            // 2. Handle Named keys
            keyboard::Key::Named(Named::ArrowDown) => Some(Message::SelectNextTask),
            keyboard::Key::Named(Named::ArrowUp) => Some(Message::SelectPrevTask),
            keyboard::Key::Named(Named::PageDown) => Some(Message::SelectNextPage),
            keyboard::Key::Named(Named::PageUp) => Some(Message::SelectPrevPage),
            keyboard::Key::Named(Named::Space) => Some(Message::ToggleSelected),
            keyboard::Key::Named(Named::Escape) => Some(Message::EscapePressed),

            _ => None,
        }
    } else {
        None
    }
}
