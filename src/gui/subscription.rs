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

    Subscription::batch(subs)
}

fn handle_hotkey(
    evt: iced::Event,
    status: iced::event::Status,
    _id: iced::window::Id,
) -> Option<Message> {
    use iced::keyboard::key::Named;

    // Allow Escape to work even when input is captured (to cancel edit)
    if status == iced::event::Status::Captured {
        if let iced::Event::Keyboard(keyboard::Event::KeyPressed { key, .. }) = evt
            && key == keyboard::Key::Named(Named::Escape) {
                return Some(Message::CancelEdit);
            }
        return None;
    }

    if let iced::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) = evt {
        // Ignore if Ctrl/Alt/Cmd is held (avoids conflict with system shortcuts)
        if modifiers.command() || modifiers.control() || modifiers.alt() {
            return None;
        }

        match key.as_ref() {
            // --- NAVIGATION ---
            keyboard::Key::Character("j") | keyboard::Key::Named(Named::ArrowDown) => {
                Some(Message::SelectNextTask)
            }
            keyboard::Key::Character("k") | keyboard::Key::Named(Named::ArrowUp) => {
                Some(Message::SelectPrevTask)
            }
            keyboard::Key::Named(Named::PageDown) => Some(Message::SelectNextPage),
            keyboard::Key::Named(Named::PageUp) => Some(Message::SelectPrevPage),

            // --- ACTIONS ---
            keyboard::Key::Character("d") => Some(Message::DeleteSelected),
            keyboard::Key::Character("e") => Some(Message::EditSelected),
            keyboard::Key::Character("E") => Some(Message::EditSelectedDescription),
            keyboard::Key::Named(Named::Space) => Some(Message::ToggleSelected),

            // --- STATUS ---
            keyboard::Key::Character("s") => Some(Message::ToggleActiveSelected),
            keyboard::Key::Character("S") => Some(Message::StopSelected),
            keyboard::Key::Character("x") => Some(Message::CancelSelected),

            // --- PRIORITY ---
            keyboard::Key::Character("+") => Some(Message::ChangePrioritySelected(1)),
            keyboard::Key::Character("-") => Some(Message::ChangePrioritySelected(-1)),

            // --- MOVEMENT ---
            keyboard::Key::Character("M") => Some(Message::EditSelected), // Reuse edit mode for moving

            // --- HIERARCHY ---
            keyboard::Key::Character(".") | keyboard::Key::Character(">") => {
                Some(Message::DemoteSelected)
            }
            keyboard::Key::Character(",") | keyboard::Key::Character("<") => {
                Some(Message::PromoteSelected)
            }

            // --- YANK / PASTE ---
            keyboard::Key::Character("y") => Some(Message::YankSelected),
            keyboard::Key::Character("c") => Some(Message::KeyboardCreateChild),
            keyboard::Key::Character("b") => Some(Message::KeyboardAddDependency),
            keyboard::Key::Character("l") => Some(Message::KeyboardAddRelation),
            keyboard::Key::Named(Named::Escape) => Some(Message::ClearYank),

            // --- SEARCH / INPUT ---
            keyboard::Key::Character("/") => Some(Message::FocusSearch),
            keyboard::Key::Character("a") => Some(Message::FocusInput),

            // --- VIEW TABS ---
            keyboard::Key::Character("1") => {
                Some(Message::SidebarModeChanged(SidebarMode::Calendars))
            }
            keyboard::Key::Character("2") => {
                Some(Message::SidebarModeChanged(SidebarMode::Categories))
            }
            keyboard::Key::Character("3") => {
                Some(Message::SidebarModeChanged(SidebarMode::Locations))
            }

            // --- TOGGLES (Stateless) ---
            keyboard::Key::Character("H") => Some(Message::ToggleHideCompletedToggle),
            keyboard::Key::Character("m") => Some(Message::CategoryMatchModeToggle),

            // --- GLOBAL ---
            keyboard::Key::Character("?") => Some(Message::OpenHelp),
            keyboard::Key::Character("q") => Some(Message::CloseWindow),
            keyboard::Key::Character("r") => Some(Message::Refresh),

            _ => None,
        }
    } else {
        None
    }
}
