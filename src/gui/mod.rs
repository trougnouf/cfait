/*
Entry point and setup for the GUI application.
This file was updated to parse a `--force-ssd` CLI flag and pass it to the GUI state
initialization. When `--force-ssd` is present the app uses server-side/native window
decorations and disables transparency / rounded corners in the GUI.
*/

pub mod async_ops;
pub mod icon;
pub mod message;
pub mod state;
pub mod subscription;
pub mod update;
pub mod view;

use crate::config::{AppTheme, Config};
use crate::gui::message::Message;
use crate::gui::state::GuiApp;
use crate::system::spawn_alarm_actor;
use iced::futures::SinkExt;
use iced::futures::channel::mpsc::Sender;
use iced::stream;
use iced::{Element, Subscription, Task, Theme, font, window};
use std::sync::Arc;

pub fn run() -> iced::Result {
    run_with_ics_file(None)
}

pub fn run_with_ics_file(ics_file_path: Option<String>) -> iced::Result {
    // Parse CLI args for --force-ssd
    let args: Vec<String> = std::env::args().collect();
    let force_ssd = args.iter().any(|a| a == "--force-ssd");

    async_ops::init_runtime();

    iced::application(
        // Pass force_ssd down into app initialization
        move || GuiApp::new_with_ics(ics_file_path.clone(), force_ssd),
        GuiApp::update,
        GuiApp::view,
    )
    .title(GuiApp::title)
    .subscription(GuiApp::subscription)
    .theme(GuiApp::theme)
    // Window-level styling: pick an appropriate background depending on SSD vs CSD
    .style(|state, _appearance| {
        // `state` is &GuiApp here; obtain the Theme into a local binding so its
        // palette does not borrow from a temporary value that is dropped.
        let theme_binding = state.theme();
        let palette = theme_binding.extended_palette();
        iced::theme::Style {
            background_color: if state.force_ssd {
                // If SSD (native decorations) are used, use an opaque background.
                palette.background.base.color
            } else {
                // If we use client-side decorations with custom rounded frame, the
                // global window background must be transparent to allow rounded corners.
                iced::Color::TRANSPARENT
            },
            text_color: palette.background.base.text,
        }
    })
    .window(window::Settings {
        decorations: force_ssd,  // Enable native decorations when forced
        transparent: !force_ssd, // Only allow transparency when not using native decorations
        platform_specific: window::settings::PlatformSpecific {
            #[cfg(target_os = "linux")]
            application_id: String::from("cfait"),
            ..Default::default()
        },
        ..Default::default()
    })
    .run()
}

// Helper function to satisfy Subscription::run fn pointer requirement
fn alarm_stream() -> impl iced::futures::Stream<Item = Message> {
    stream::channel(100, |mut output: Sender<Message>| async move {
        let (gui_tx, mut gui_rx) = tokio::sync::mpsc::channel(10);
        let actor_tx = spawn_alarm_actor(Some(gui_tx));
        let _ = output.send(Message::InitAlarmActor(actor_tx)).await;

        while let Some(msg) = gui_rx.recv().await {
            let _ = output
                .send(Message::AlarmSignalReceived(Arc::new(msg)))
                .await;
        }

        std::future::pending::<()>().await;
    })
}

impl GuiApp {
    // NOTE: new_with_ics signature was updated to accept force_ssd; call sites must match.
    fn new_with_ics(ics_file_path: Option<String>, force_ssd: bool) -> (Self, Task<Message>) {
        let mut tasks = vec![
            Task::perform(
                async { Config::load().map_err(|e| e.to_string()) },
                Message::ConfigLoaded,
            ),
            font::load(icon::FONT_BYTES).map(|_| Message::FontLoaded(Ok(()))),
        ];

        // If an ICS file path was provided, load it
        if let Some(path) = ics_file_path {
            tasks.push(Task::perform(
                async move {
                    std::fs::read_to_string(&path)
                        .map(|content| (path, content))
                        .map_err(|e| e.to_string())
                },
                |result| match result {
                    Ok((path, content)) => Message::IcsFileLoaded(Ok((path, content))),
                    Err(e) => Message::IcsFileLoaded(Err(e)),
                },
            ));
        }

        let app = Self {
            force_ssd,
            ..Self::default()
        };
        (app, Task::batch(tasks))
    }

    fn view(&self) -> Element<'_, Message> {
        view::root_view(self)
    }

    fn title(&self) -> String {
        "Cfait | 🗹 Take control of your TODO list".to_string()
    }

    fn theme(&self) -> Theme {
        // Helper to create the Rusty Dark custom theme (avoids duplicating the palette)
        fn create_rusty_dark_theme() -> Theme {
            let mut palette = iced::Theme::Dark.palette();
            palette.background = iced::Color::from_rgb8(0x21, 0x1e, 0x1e);
            palette.text = iced::Color::WHITE;
            palette.primary = iced::Color::from_rgb8(0xFF, 0xA5, 0x00); // Orange
            palette.success = iced::Color::from_rgb8(0xA3, 0xBE, 0x8C); // Muted Green
            palette.danger = iced::Color::from_rgb8(0xBF, 0x61, 0x6A); // Muted Red
            Theme::custom("Rusty Dark", palette)
        }

        // Determine which theme to actually render
        let effective_theme = if self.current_theme == AppTheme::Random {
            self.resolved_random_theme
        } else {
            self.current_theme
        };

        match effective_theme {
            AppTheme::Light => Theme::Light,
            AppTheme::Dark => Theme::Dark,
            AppTheme::Dracula => Theme::Dracula,
            AppTheme::Nord => Theme::Nord,
            AppTheme::SolarizedLight => Theme::SolarizedLight,
            AppTheme::SolarizedDark => Theme::SolarizedDark,
            AppTheme::GruvboxLight => Theme::GruvboxLight,
            AppTheme::GruvboxDark => Theme::GruvboxDark,
            AppTheme::CatppuccinLatte => Theme::CatppuccinLatte,
            AppTheme::CatppuccinFrappe => Theme::CatppuccinFrappe,
            AppTheme::CatppuccinMacchiato => Theme::CatppuccinMacchiato,
            AppTheme::CatppuccinMocha => Theme::CatppuccinMocha,
            AppTheme::TokyoNight => Theme::TokyoNight,
            AppTheme::TokyoNightStorm => Theme::TokyoNightStorm,
            AppTheme::TokyoNightLight => Theme::TokyoNightLight,
            AppTheme::KanagawaWave => Theme::KanagawaWave,
            AppTheme::KanagawaDragon => Theme::KanagawaDragon,
            AppTheme::KanagawaLotus => Theme::KanagawaLotus,
            AppTheme::Moonfly => Theme::Moonfly,
            AppTheme::Nightfly => Theme::Nightfly,
            AppTheme::Oxocarbon => Theme::Oxocarbon,
            AppTheme::Ferra => Theme::Ferra,
            AppTheme::RustyDark => create_rusty_dark_theme(),
            // Fallback: If for some reason resolved_random_theme was Random (shouldn't happen), default to RustyDark
            AppTheme::Random => create_rusty_dark_theme(),
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        let subs = subscription::subscription(self);
        let alarm_sub = Subscription::run(alarm_stream);
        Subscription::batch(vec![subs, alarm_sub])
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        update::update(self, message)
    }
}
