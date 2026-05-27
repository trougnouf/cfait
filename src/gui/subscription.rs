// SPDX-License-Identifier: GPL-3.0-or-later
// Handles event subscriptions (keyboard, window) for the GUI.
use crate::gui::message::Message;
use crate::gui::state::{AppState, GuiApp, SidebarMode};
use iced::{Subscription, event, keyboard, window};
use std::sync::atomic::{AtomicBool, Ordering};

// Tracks the Command/Ctrl modifier state statelessly so Mouse events can check it
static CMD_HELD: AtomicBool = AtomicBool::new(false);

pub fn subscription(app: &GuiApp) -> Subscription<Message> {
    use iced::keyboard::key::Named;

    let mut subs = Vec::new();

    // Start background syncing worker
    subs.push(crate::gui::async_ops::worker_subscription(app.ctx.clone()));

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

    match app.state {
        AppState::Active => {
            // Use a static function to handle hotkeys so we don't capture `app`
            // This avoids the "expected fn pointer, found closure" error
            subs.push(event::listen_with(handle_hotkey));
        }
        AppState::Help(_, _) => {
            subs.push(event::listen_with(handle_help_hotkey));
        }
        _ => {}
    }

    // Track window metrics (Size and Cursor Position)
    subs.push(event::listen_with(|evt, _status, _window_id| match evt {
        iced::Event::Window(window::Event::Resized(size)) => Some(Message::WindowResized(size)),
        iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
            Some(Message::CursorMoved(position))
        }
        _ => None,
    }));

    // Auto-refresh subscription (configurable)
    if app.auto_refresh_interval_mins > 0 {
        subs.push(
            iced::time::every(std::time::Duration::from_secs(
                app.auto_refresh_interval_mins as u64 * 60,
            ))
            .map(|_| Message::Refresh),
        );
    }

    // Tick every minute if there is an active task running, so the timer updates visually
    let has_running_tasks = app.tasks.iter().any(|item| {
        if let crate::store::TaskListItem::Task(t) = item {
            t.last_started_at.is_some()
        } else {
            false
        }
    });
    if has_running_tasks {
        subs.push(iced::time::every(std::time::Duration::from_secs(60)).map(|_| Message::Tick));
    }

    Subscription::batch(subs)
}

fn handle_help_hotkey(
    evt: iced::Event,
    status: iced::event::Status,
    _id: iced::window::Id,
) -> Option<Message> {
    if status == iced::event::Status::Captured {
        return None;
    }
    if let iced::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) = evt {
        match key.as_ref() {
            keyboard::Key::Named(keyboard::key::Named::Escape) => Some(Message::CloseHelp),
            keyboard::Key::Named(keyboard::key::Named::Tab) => {
                Some(Message::SwitchHelpTab(!modifiers.shift()))
            }
            keyboard::Key::Named(keyboard::key::Named::ArrowRight) => {
                Some(Message::SwitchHelpTab(true))
            }
            keyboard::Key::Named(keyboard::key::Named::ArrowLeft) => {
                Some(Message::SwitchHelpTab(false))
            }
            keyboard::Key::Character(s) if s == "q" || s == "?" || s == "/" => {
                Some(Message::CloseHelp)
            }
            keyboard::Key::Character("l") => Some(Message::SwitchHelpTab(true)),
            keyboard::Key::Character("h") => Some(Message::SwitchHelpTab(false)),
            _ => None,
        }
    } else {
        None
    }
}

fn handle_hotkey(
    evt: iced::Event,
    status: iced::event::Status,
    _id: iced::window::Id,
) -> Option<Message> {
    use iced::keyboard::key::Named;

    // Track modifier state globally for mouse events
    if let iced::Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) = &evt {
        CMD_HELD.store(
            modifiers.control() || modifiers.command(),
            Ordering::Relaxed,
        );
    }

    // Handle Ctrl + Scroll (Zoom In/Out)
    if let iced::Event::Mouse(iced::mouse::Event::WheelScrolled { delta }) = &evt
        && CMD_HELD.load(Ordering::Relaxed)
    {
        match delta {
            iced::mouse::ScrollDelta::Lines { y, .. }
            | iced::mouse::ScrollDelta::Pixels { y, .. } => {
                if *y > 0.0 {
                    return Some(Message::ZoomIn);
                } else if *y < 0.0 {
                    return Some(Message::ZoomOut);
                }
            }
        }
    }

    // Handle Ctrl + Middle Click (Zoom Reset)
    if let iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Middle)) = &evt
        && CMD_HELD.load(Ordering::Relaxed)
    {
        return Some(Message::ZoomReset);
    }

    // Allow certain keys to bypass capture (e.g. Escape to unfocus, Ctrl+S to save)
    if status == iced::event::Status::Captured {
        if let iced::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) = &evt {
            if *key == keyboard::Key::Named(Named::Escape) {
                return Some(Message::EscCaptured);
            }
            if *key == keyboard::Key::Named(Named::Tab) {
                return Some(Message::TabPressed(modifiers.shift()));
            }
            let is_cmd = modifiers.control() || modifiers.command();
            if is_cmd && let keyboard::Key::Character(s) = key.as_ref() {
                match s {
                    "s" => return Some(Message::SubmitTask),
                    "e" => return Some(Message::StartCreateWithDescription),
                    "," => return Some(Message::OpenSettings),
                    _ => {}
                }
            }
        }
        return None;
    }

    if let iced::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) = evt {
        // Catch zoom shortcuts and other modifiers BEFORE we ignore modifier combinations.
        let is_cmd = modifiers.command() || modifiers.control();

        if is_cmd {
            if let keyboard::Key::Character(s) = key.as_ref() {
                match s {
                    "+" | "=" => return Some(Message::ZoomIn),
                    "-" => return Some(Message::ZoomOut),
                    "0" => return Some(Message::ZoomReset),
                    "b" => return Some(Message::ToggleSidebar),
                    "d" => return Some(Message::KeyboardDuplicateTask),
                    "s" => return Some(Message::SubmitTask),
                    "e" => return Some(Message::StartCreateWithDescription), // Fallback if not focused
                    "," => return Some(Message::OpenSettings),
                    "p" => return Some(Message::ToggleSortStandardByPriorityToggle),
                    _ => {}
                }
            } else if let keyboard::Key::Named(Named::Delete) = key.as_ref() {
                return Some(Message::KeyboardDeleteTaskTree);
            }
        }

        // Ignore if Ctrl/Alt/Cmd is held for everything else
        if modifiers.command() || modifiers.control() || modifiers.alt() {
            return None;
        }

        match key.as_ref() {
            // 1. Handle character-based keys first
            keyboard::Key::Character(s) => {
                let s_lower = s.to_lowercase();
                // Match on lowercase char + shift state tuple for alphabetic keys
                match (s_lower.as_ref(), modifiers.shift()) {
                    ("j", false) => Some(Message::SelectNextTask),
                    ("k", false) => Some(Message::SelectPrevTask),
                    ("e", false) => Some(Message::EditSelected),
                    ("e", true) => Some(Message::EditSelectedDescription),
                    ("s", false) => Some(Message::ToggleActiveSelected),
                    ("s", true) => Some(Message::StopSelected),
                    ("x", false) => Some(Message::CancelSelected),
                    ("y", false) => Some(Message::YankSelected),
                    ("c", false) => Some(Message::KeyboardLinkChild),
                    ("c", true) => Some(Message::KeyboardCreateChild),
                    ("t", false) => Some(Message::KeyboardAddSession),
                    ("t", true) => Some(Message::KeyboardToggleSessions),
                    ("b", false) => Some(Message::KeyboardAddDependency),
                    ("l", false) => Some(Message::KeyboardAddRelation),
                    ("l", true) => Some(Message::KeyboardOpenContextMenu),
                    ("g", false) => Some(Message::KeyboardOpenLocations),
                    ("o", false) => Some(Message::KeyboardOpenUrl),
                    ("a", false) => Some(Message::FocusInput),
                    ("h", true) => Some(Message::ToggleHideCompletedToggle),
                    ("m", false) => Some(Message::CategoryMatchModeToggle),
                    ("m", true) => Some(Message::MoveSelected),
                    ("z", false) => Some(Message::KeyboardToggleTreeCollapse),
                    ("q", false) => Some(Message::CloseWindow),
                    ("w", false) => Some(Message::ToggleQuickFilter),
                    ("r", false) => Some(Message::Refresh),
                    ("r", true) => Some(Message::JumpToRandomTask),

                    ("/", false) => Some(Message::FocusSearch),
                    ("/", true) => Some(Message::OpenHelp(crate::help::HelpTab::Shortcuts)),
                    ("?", _) => Some(Message::OpenHelp(crate::help::HelpTab::Shortcuts)),
                    // Fallback to match exact char for symbols and numbers
                    _ => match s {
                        "1" => Some(Message::SidebarModeChanged(SidebarMode::Calendars)),
                        "2" => Some(Message::SidebarModeChanged(SidebarMode::Categories)),
                        "3" => Some(Message::SidebarModeChanged(SidebarMode::Locations)),
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
            keyboard::Key::Named(Named::Enter) => Some(Message::EnterPressed),
            keyboard::Key::Named(Named::Space) => {
                if modifiers.shift() {
                    Some(Message::ToggleTaskShiftSelected)
                } else {
                    Some(Message::ToggleSelected)
                }
            }
            keyboard::Key::Named(Named::Escape) => Some(Message::EscapePressed),
            keyboard::Key::Named(Named::Delete) => {
                // Handled in is_cmd block for Ctrl+Delete, so here it's just Delete
                Some(Message::DeleteSelected)
            }
            keyboard::Key::Named(Named::Tab) => Some(Message::TabPressed(modifiers.shift())),

            _ => None,
        }
    } else {
        None
    }
}
