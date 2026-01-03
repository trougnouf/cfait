// Handles event subscriptions (keyboard, window) for the GUI.
use crate::gui::message::Message;
use crate::gui::state::{AppState, GuiApp};
use iced::{Subscription, event, keyboard, window};

pub fn subscription(app: &GuiApp) -> Subscription<Message> {
    use iced::keyboard::key;

    let mut subs = Vec::new();

    if matches!(app.state, AppState::Onboarding | AppState::Settings) {
        subs.push(keyboard::listen().filter_map(|event| {
            if let keyboard::Event::KeyPressed { key, modifiers, .. } = event
                && key == key::Key::Named(key::Named::Tab)
            {
                return Some(Message::TabPressed(modifiers.shift()));
            }
            None
        }));
    }

    if app.state == AppState::Active {
        subs.push(event::listen_with(|evt, status, _window| {
            if status == event::Status::Captured {
                return None;
            }
            if let iced::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) = evt {
                if !modifiers.is_empty() {
                    return None;
                }

                match key.as_ref() {
                    keyboard::Key::Character("a") => Some(Message::FocusInput),
                    keyboard::Key::Character("/") => Some(Message::FocusSearch),
                    _ => None,
                }
            } else {
                None
            }
        }));
    }

    // Track window metrics (Size)
    subs.push(event::listen_with(|evt, _status, _window_id| match evt {
        iced::Event::Window(window::Event::Resized(size)) => Some(Message::WindowResized(size)),
        _ => None,
    }));

    Subscription::batch(subs)
}
