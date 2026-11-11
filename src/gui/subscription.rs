// SPDX-License-Identifier: GPL-3.0-or-later
// Handles event subscriptions (keyboard, window) for the GUI.
use crate::gui::message::Message;
use crate::gui::state::{AppState, Focus, GuiApp, SidebarMode};
use iced::{Subscription, event, keyboard, window};
use std::sync::atomic::{AtomicBool, Ordering};

pub static LAST_MOUSE_POS: std::sync::LazyLock<std::sync::RwLock<iced::Point>> =
    std::sync::LazyLock::new(|| std::sync::RwLock::new(iced::Point::new(0.0, 0.0)));

// Tracks the Command/Ctrl modifier state statelessly so Mouse events can check it
static CMD_HELD: AtomicBool = AtomicBool::new(false);

/// Whether Ctrl (or Cmd on macOS) is currently held, for mouse-click shortcuts.
pub fn cmd_held() -> bool {
    CMD_HELD.load(Ordering::Relaxed)
}

// Tracks the current focus statelessly so keyboard event handlers can check it
pub static ACTIVE_FOCUS: std::sync::LazyLock<std::sync::RwLock<Focus>> =
    std::sync::LazyLock::new(|| std::sync::RwLock::new(Focus::default()));

/// Maps a digit character (1-5) to the corresponding sidebar tab message.
/// Uses `modified_key` (the logical character with modifiers like Shift applied,
/// except Ctrl) so it matches the character the user actually typed on their
/// layout — e.g. on programmer Dvorak, "1" is Shift+key, and this still matches.
fn digit_sidebar_message(modified_key: &keyboard::Key) -> Option<Message> {
    let s = match modified_key.as_ref() {
        keyboard::Key::Character(s) => s,
        _ => return None,
    };
    let mode = match s {
        "1" => SidebarMode::Calendars,
        "2" => SidebarMode::Categories,
        "3" => SidebarMode::Locations,
        "4" => SidebarMode::Goals,
        "5" => SidebarMode::Journal,
        _ => return None,
    };
    Some(Message::SidebarModeChanged(mode))
}

/// Ctrl/Cmd + character shortcuts that work both with and without a widget
/// capturing input (save, new with description, settings, undo/redo).
fn shared_cmd_shortcut(s_lower: &str, shift: bool) -> Option<Message> {
    match s_lower {
        "s" => Some(if shift {
            Message::SaveTaskKeepEditing
        } else {
            Message::SubmitTask
        }),
        "n" => Some(Message::StartCreateWithDescription),
        "," => Some(Message::OpenSettings),
        "z" => Some(if shift { Message::Redo } else { Message::Undo }),
        "y" => Some(Message::Redo),
        _ => None,
    }
}

/// Arrow/enter/space navigation messages, shared by the captured and normal
/// hotkey paths.
fn navigation_key_message(key: &keyboard::Key, shift: bool) -> Option<Message> {
    match key.as_ref() {
        keyboard::Key::Named(keyboard::key::Named::ArrowDown) => Some(Message::SelectNext),
        keyboard::Key::Named(keyboard::key::Named::ArrowUp) => Some(Message::SelectPrev),
        keyboard::Key::Named(keyboard::key::Named::ArrowRight) => Some(Message::ArrowRight),
        keyboard::Key::Named(keyboard::key::Named::ArrowLeft) => Some(Message::ArrowLeft),
        keyboard::Key::Named(keyboard::key::Named::Enter) => Some(Message::EnterPressed),
        keyboard::Key::Named(keyboard::key::Named::Space) => Some(if shift {
            Message::ShiftSpaceSelected
        } else {
            Message::ToggleSelected
        }),
        _ => None,
    }
}

/// The character the user actually typed: `modified_key` carries the logical
/// key with Shift applied (except Ctrl), so symbols that need Shift on a given
/// layout (e.g. "?" on Turkish-Q, "*" on US) match regardless of the layout.
fn typed_char(key: &keyboard::Key, modified_key: &keyboard::Key) -> Option<String> {
    match modified_key.as_ref() {
        keyboard::Key::Character(s) => Some(s.to_string()),
        _ => match key.as_ref() {
            keyboard::Key::Character(s) => Some(s.to_string()),
            _ => None,
        },
    }
}

pub fn subscription(app: &GuiApp) -> Subscription<Message> {
    let mut subs = Vec::new();

    // Start background syncing worker
    subs.push(crate::gui::async_ops::worker_subscription(app.ctx.clone()));

    match app.state {
        AppState::Active => {
            // Use a static function to handle hotkeys so we don't capture `app`
            // This avoids the "expected fn pointer, found closure" error
            subs.push(event::listen_with(handle_hotkey));
        }
        AppState::Help(_, _) => {
            subs.push(event::listen_with(handle_help_hotkey));
        }
        AppState::Settings => {
            subs.push(event::listen_with(handle_settings_hotkey));
        }
        AppState::Onboarding => {
            subs.push(event::listen_with(handle_onboarding_hotkey));
        }
        _ => {}
    }

    // Track window metrics
    subs.push(event::listen_with(|evt, _status, _window_id| match evt {
        iced::Event::Window(window::Event::Resized(size)) => Some(Message::WindowResized(size)),
        iced::Event::Window(window::Event::Focused) => Some(Message::WindowFocused(true)),
        iced::Event::Window(window::Event::Unfocused) => Some(Message::WindowFocused(false)),
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

    // While a sync is in flight, tick the top-bar spinner icon.
    if app.syncing {
        subs.push(
            iced::time::every(std::time::Duration::from_millis(150)).map(|_| Message::SpinnerTick),
        );
    }

    // While a completion celebration is on screen, keep animating it.
    if app.confetti.is_some() {
        subs.push(
            iced::time::every(std::time::Duration::from_millis(33)).map(|_| Message::ConfettiTick),
        );
    }

    // Tick every minute if any task is running, so the timer updates visually.
    // Scan the whole store (not the filtered list): a visible parent may
    // aggregate session time from a running child that is hidden by filters.
    let has_running_tasks = app
        .store
        .calendars
        .values()
        .flat_map(|m| m.values())
        .any(|t| t.last_started_at.is_some());
    if has_running_tasks {
        subs.push(iced::time::every(std::time::Duration::from_secs(60)).map(|_| Message::Tick));
    }

    Subscription::batch(subs)
}

fn handle_settings_hotkey(
    evt: iced::Event,
    _status: iced::event::Status,
    _id: iced::window::Id,
) -> Option<Message> {
    if let iced::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) = evt {
        match key.as_ref() {
            keyboard::Key::Named(keyboard::key::Named::Tab) => {
                Some(Message::CycleFocus(!modifiers.shift()))
            }
            keyboard::Key::Named(keyboard::key::Named::Escape) => Some(Message::CancelSettings),
            _ => None,
        }
    } else {
        None
    }
}

fn handle_onboarding_hotkey(
    evt: iced::Event,
    _status: iced::event::Status,
    _id: iced::window::Id,
) -> Option<Message> {
    if let iced::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) = evt {
        match key.as_ref() {
            keyboard::Key::Named(keyboard::key::Named::Tab) => {
                Some(Message::CycleFocus(!modifiers.shift()))
            }
            _ => None,
        }
    } else {
        None
    }
}

fn handle_help_hotkey(
    evt: iced::Event,
    status: iced::event::Status,
    _id: iced::window::Id,
) -> Option<Message> {
    if status == iced::event::Status::Captured {
        return None;
    }
    if let iced::Event::Keyboard(keyboard::Event::KeyPressed {
        key,
        modifiers,
        modified_key,
        ..
    }) = evt
    {
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
            keyboard::Key::Character(s) => {
                let typed = typed_char(&key, &modified_key).unwrap_or_default();
                if s == "q" || s == "/" || typed == "?" {
                    Some(Message::CloseHelp)
                } else if s == "l" {
                    Some(Message::SwitchHelpTab(true))
                } else if s == "h" {
                    Some(Message::SwitchHelpTab(false))
                } else {
                    None
                }
            }
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

    if let iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) = &evt
        && let Ok(mut pos) = LAST_MOUSE_POS.write()
    {
        *pos = *position;
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
        if let iced::Event::Keyboard(keyboard::Event::KeyPressed {
            key,
            modifiers,
            modified_key,
            ..
        }) = &evt
        {
            if *key == keyboard::Key::Named(Named::Escape) {
                return Some(Message::EscCaptured);
            }
            let is_cmd = modifiers.control() || modifiers.command();
            if is_cmd && let keyboard::Key::Character(s) = key.as_ref() {
                let s_lower = s.to_lowercase();
                if s_lower == "e" {
                    return Some(Message::KeyboardEditTree);
                }
                if let Some(msg) = shared_cmd_shortcut(&s_lower, modifiers.shift()) {
                    return Some(msg);
                }
            }

            // Ctrl/Cmd + Enter: show details for the selected task
            if is_cmd && *key == keyboard::Key::Named(Named::Enter) {
                return Some(Message::KeyboardToggleDetails);
            }

            // Ctrl/Cmd + digit: switch sidebar tab by logical character
            // (matches the typed digit on any layout; works even from a text field)
            if is_cmd && let Some(msg) = digit_sidebar_message(modified_key) {
                return Some(msg);
            }

            // If we are definitely not in a text input, steal navigation keys back from Iced's Scrollables/Buttons.
            // Only Named keys belong here: scrollables/buttons capture arrows, enter, and space,
            // but never character keys. Character keys in this block would only ever be stolen
            // from text editors/inputs (which capture them), which is a bug.
            if let Ok(focus) = ACTIVE_FOCUS.read()
                && (*focus == Focus::MainList || *focus == Focus::Sidebar)
                && let Some(msg) = navigation_key_message(key, modifiers.shift())
            {
                return Some(msg);
            }
        }
        return None;
    }

    if let iced::Event::Keyboard(keyboard::Event::KeyPressed {
        key,
        modifiers,
        modified_key,
        ..
    }) = evt
    {
        // Catch zoom shortcuts and other modifiers BEFORE we ignore modifier combinations.
        let is_cmd = modifiers.command() || modifiers.control();

        if is_cmd {
            if let keyboard::Key::Character(s) = key.as_ref() {
                let s_lower = s.to_lowercase();
                match s_lower.as_str() {
                    "+" | "=" => return Some(Message::ZoomIn),
                    "-" => return Some(Message::ZoomOut),
                    "0" => return Some(Message::ZoomReset),
                    "b" => return Some(Message::ToggleSidebar),
                    "d" => return Some(Message::KeyboardDuplicateTask),
                    "e" => {
                        if let Ok(focus) = ACTIVE_FOCUS.read()
                            && *focus == Focus::AddTaskInput
                        {
                            return Some(Message::StartCreateWithDescription);
                        }
                        return Some(Message::KeyboardEditTree);
                    }
                    "m" => return Some(Message::ToggleEditorMaximize),
                    "p" => return Some(Message::ToggleSortStandardByPriorityToggle),
                    _ => {}
                }
                if let Some(msg) = shared_cmd_shortcut(&s_lower, modifiers.shift()) {
                    return Some(msg);
                }
            } else if let keyboard::Key::Named(Named::Delete) = key.as_ref() {
                return Some(Message::KeyboardDeleteTaskTree);
            } else if let keyboard::Key::Named(Named::Enter) = key.as_ref() {
                return Some(Message::KeyboardToggleDetails);
            }

            // Ctrl/Cmd + digit: switch sidebar tab by logical character
            // (matches the typed digit on any layout; works even from a text field)
            if let Some(msg) = digit_sidebar_message(&modified_key) {
                return Some(msg);
            }
        }

        // Ignore if Ctrl/Alt/Cmd is held for everything else
        if modifiers.command() || modifiers.control() || modifiers.alt() {
            return None;
        }

        // A focused text field captures every key it handles, so a key that
        // still arrives here was not consumed by it. On Windows, AltGr is
        // reported without its Ctrl/Alt modifiers (winit filters them out),
        // so the check above cannot catch e.g. AltGr+Space: without this
        // guard it would fall through to the list commands below and toggle
        // the selected task while the user is typing. Escape still leaves the
        // field, and Tab still jumps from the journal title into the note body.
        if let Ok(focus) = ACTIVE_FOCUS.read()
            && matches!(
                *focus,
                Focus::SearchInput | Focus::AddTaskInput | Focus::Journal | Focus::JournalTitle
            )
        {
            let is_escape = key == keyboard::Key::Named(Named::Escape);
            let is_journal_title_tab =
                *focus == Focus::JournalTitle && key == keyboard::Key::Named(Named::Tab);
            if !is_escape && !is_journal_title_tab {
                return None;
            }
        }

        // Bare digit: switch sidebar tab by logical character (matches the typed
        // digit on any layout, e.g. Shift+key on programmer Dvorak)
        if let Some(msg) = digit_sidebar_message(&modified_key) {
            return Some(msg);
        }

        match key.as_ref() {
            // 1. Handle character-based keys first
            keyboard::Key::Character(s) => {
                // Symbol shortcuts match the typed character (Shift applied via
                // modified_key) so they fire on any layout, even when the symbol
                // itself requires Shift (e.g. "?" or "*" on Turkish-Q).
                if let Some(typed) = typed_char(&key, &modified_key) {
                    match typed.as_str() {
                        "?" => return Some(Message::OpenHelp(crate::help::HelpTab::Shortcuts)),
                        "/" => return Some(Message::FocusSearch),
                        "*" => return Some(Message::ClearAllFilters),
                        "+" | "=" => return Some(Message::ChangePrioritySelected(1)),
                        "-" => return Some(Message::ChangePrioritySelected(-1)),
                        "." | ">" => return Some(Message::DemoteSelected),
                        "," | "<" => return Some(Message::PromoteSelected),
                        _ => {}
                    }
                }
                let s_lower = s.to_lowercase();
                // Match on lowercase char + shift state tuple for alphabetic keys
                match (s_lower.as_ref(), modifiers.shift()) {
                    ("j", false) => Some(Message::SelectNext),
                    ("k", false) => Some(Message::SelectPrev),
                    ("e", false) => Some(Message::EditSelected),
                    ("e", true) => Some(Message::EditSelectedDescription),
                    ("s", false) => Some(Message::ToggleActiveSelected),
                    ("s", true) => Some(Message::StopSelected),
                    ("x", false) => Some(Message::CancelSelected),
                    ("y", false) => Some(Message::YankSelected),
                    ("y", true) => Some(Message::ToggleYankLock),
                    ("f", false) => Some(Message::FocusSelected),
                    ("c", false) => Some(Message::KeyboardLinkChild),
                    ("c", true) => Some(Message::KeyboardCreateChild),
                    ("t", false) => Some(Message::KeyboardAddSession),
                    ("t", true) => Some(Message::KeyboardToggleSessions),
                    ("b", false) => Some(Message::KeyboardAddDependency),
                    ("l", false) => Some(Message::KeyboardAddRelation),
                    ("l", true) => Some(Message::KeyboardToggleDetails),
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
                    _ => None,
                }
            }

            // 2. Handle Named keys
            keyboard::Key::Named(Named::F1) => {
                Some(Message::OpenHelp(crate::help::HelpTab::Shortcuts))
            }
            keyboard::Key::Named(Named::PageDown) => Some(Message::SelectNextPage),
            keyboard::Key::Named(Named::PageUp) => Some(Message::SelectPrevPage),
            keyboard::Key::Named(Named::Escape) => Some(Message::EscapePressed),
            keyboard::Key::Named(Named::Delete) => {
                // Handled in is_cmd block for Ctrl+Delete, so here it's just Delete
                Some(Message::DeleteSelected)
            }
            keyboard::Key::Named(Named::Tab) => {
                // From the new-page title, Tab jumps to the note body instead of
                // cycling focus.
                if let Ok(focus) = ACTIVE_FOCUS.read()
                    && *focus == Focus::JournalTitle
                {
                    Some(Message::FocusJournalEditor)
                } else {
                    Some(Message::CycleFocus(!modifiers.shift()))
                }
            }
            keyboard::Key::Named(_) => navigation_key_message(&key, modifiers.shift()),

            _ => None,
        }
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::keyboard::key::{Code, Physical};
    use iced::keyboard::{Key, Location, Modifiers};
    use std::sync::Mutex;

    // ACTIVE_FOCUS is global, so serialize the tests that mutate it.
    static FOCUS_LOCK: Mutex<()> = Mutex::new(());

    fn key_press(key: Key, modified_key: Key, modifiers: Modifiers, text: Option<&str>) -> iced::Event {
        iced::Event::Keyboard(keyboard::Event::KeyPressed {
            key,
            modified_key,
            physical_key: Physical::Code(Code::Space),
            location: Location::Standard,
            modifiers,
            text: text.map(|t| t.into()),
            repeat: false,
        })
    }

    fn space(modifiers: Modifiers) -> iced::Event {
        key_press(
            Key::Named(keyboard::key::Named::Space),
            Key::Named(keyboard::key::Named::Space),
            modifiers,
            None,
        )
    }

    fn set_focus(focus: Focus) {
        if let Ok(mut g) = ACTIVE_FOCUS.write() {
            *g = focus;
        }
    }

    #[test]
    fn altgr_style_space_in_text_input_is_not_a_list_command() {
        let _guard = FOCUS_LOCK.lock().unwrap();
        // On Windows, AltGr is reported without Ctrl/Alt modifiers, so an
        // AltGr+Space arrives as a bare, uncaptured Space. While a text input
        // has focus it must not toggle the selected task.
        set_focus(Focus::AddTaskInput);
        let msg = handle_hotkey(space(Modifiers::empty()), iced::event::Status::Ignored, iced::window::Id::unique());
        assert!(
            !matches!(msg, Some(Message::ToggleSelected) | Some(Message::ShiftSpaceSelected)),
            "AltGr+Space in the input bar must not toggle tasks, got {msg:?}"
        );

        // The same keypress on the task list still toggles the selection.
        set_focus(Focus::MainList);
        let msg = handle_hotkey(space(Modifiers::empty()), iced::event::Status::Ignored, iced::window::Id::unique());
        assert!(matches!(msg, Some(Message::ToggleSelected)));

        // Shift+Space on the list still completes the whole tree.
        let msg = handle_hotkey(space(Modifiers::SHIFT), iced::event::Status::Ignored, iced::window::Id::unique());
        assert!(matches!(msg, Some(Message::ShiftSpaceSelected)));
    }

    #[test]
    fn text_input_keeps_escape_and_journal_title_tab() {
        let _guard = FOCUS_LOCK.lock().unwrap();
        set_focus(Focus::AddTaskInput);
        let esc = key_press(
            Key::Named(keyboard::key::Named::Escape),
            Key::Named(keyboard::key::Named::Escape),
            Modifiers::empty(),
            None,
        );
        let msg = handle_hotkey(esc, iced::event::Status::Ignored, iced::window::Id::unique());
        assert!(matches!(msg, Some(Message::EscapePressed)), "Escape must still leave the text field, got {msg:?}");

        set_focus(Focus::JournalTitle);
        let tab = key_press(
            Key::Named(keyboard::key::Named::Tab),
            Key::Named(keyboard::key::Named::Tab),
            Modifiers::empty(),
            None,
        );
        let msg = handle_hotkey(tab, iced::event::Status::Ignored, iced::window::Id::unique());
        assert!(matches!(msg, Some(Message::FocusJournalEditor)), "Tab must still jump from the journal title to the body, got {msg:?}");
    }

    #[test]
    fn uncaptured_letter_in_text_input_is_not_a_list_command() {
        let _guard = FOCUS_LOCK.lock().unwrap();
        set_focus(Focus::SearchInput);
        let j = key_press(
            Key::Character("j".into()),
            Key::Character("j".into()),
            Modifiers::empty(),
            None,
        );
        let msg = handle_hotkey(j, iced::event::Status::Ignored, iced::window::Id::unique());
        assert!(msg.is_none(), "a key the search box did not capture must not run list commands, got {msg:?}");

        set_focus(Focus::MainList);
        let j = key_press(
            Key::Character("j".into()),
            Key::Character("j".into()),
            Modifiers::empty(),
            Some("j"),
        );
        let msg = handle_hotkey(j, iced::event::Status::Ignored, iced::window::Id::unique());
        assert!(matches!(msg, Some(Message::SelectNext)), "j on the list must still move the selection, got {msg:?}");
    }
}
