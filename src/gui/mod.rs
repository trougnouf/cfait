// Entry point and setup for the GUI application.
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
    async_ops::init_runtime();

    iced::application(
        move || GuiApp::new_with_ics(ics_file_path.clone()),
        GuiApp::update,
        GuiApp::view,
    )
    .title(GuiApp::title)
    .subscription(GuiApp::subscription)
    .theme(GuiApp::theme)
    .window(window::Settings {
        decorations: false,
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
    fn new_with_ics(ics_file_path: Option<String>) -> (Self, Task<Message>) {
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

        (Self::default(), Task::batch(tasks))
    }

    fn view(&self) -> Element<'_, Message> {
        view::root_view(self)
    }

    fn title(&self) -> String {
        "Cfait | 🗹 Take control of your TODO list".to_string()
    }

    fn theme(&self) -> Theme {
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
            AppTheme::RustyDark => {
                let mut palette = iced::Theme::Dark.palette();
                palette.background = iced::Color::from_rgb8(0x21, 0x1e, 0x1e);
                palette.text = iced::Color::WHITE;
                palette.primary = iced::Color::from_rgb8(0xFF, 0xA5, 0x00); // Orange
                palette.success = iced::Color::from_rgb8(0xA3, 0xBE, 0x8C); // Muted Green
                palette.danger = iced::Color::from_rgb8(0xBF, 0x61, 0x6A); // Muted Red;
                Theme::custom("Rusty Dark", palette)
            }
            // Fallback: If for some reason resolved_random_theme was Random (shouldn't happen), default to Dark
            AppTheme::Random => Theme::Dark,
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
