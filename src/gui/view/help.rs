use crate::gui::message::Message;
use crate::gui::state::GuiApp;
use crate::help::HelpTab;
use fastrand;
use iced::widget::{
    MouseArea, Space, button, column, container, row, scrollable, svg, text, text_input,
};
use iced::{Color, Element, Length, Theme};

// --- STYLE CONSTANTS ---
const COL_ACCENT: Color = Color::from_rgb(0.4, 0.7, 1.0); // Soft Blue (Dark Mode)
const COL_SYNTAX: Color = Color::from_rgb(1.0, 0.85, 0.4); // Gold/Yellow (Dark Mode)
const COL_MUTED: Color = Color::from_rgb(0.6, 0.6, 0.6); // Grey

pub fn view_help<'a>(tab: HelpTab, _app: &'a GuiApp) -> Element<'a, Message> {
    let icon_choice = fastrand::u8(0..3);

    let help_icon = match icon_choice {
        0 => crate::gui::icon::HELP_ICON_QUESTION,
        1 => crate::gui::icon::HELP_ICON_ROBOT,
        _ => crate::gui::icon::HELP_ICON_ROBOT_HELP,
    };

    let back_btn = button(crate::gui::icon::icon(crate::gui::icon::ARROW_LEFT).size(24))
        .style(iced::widget::button::text)
        .on_press(Message::CloseHelp);

    let title_row = row![
        back_btn,
        svg(svg::Handle::from_memory(help_icon))
            .width(Length::Fixed(84.0))
            .height(Length::Fixed(32.0))
            .content_fit(iced::ContentFit::Contain),
        text("Help & about")
            .size(28)
            .style(|theme: &Theme| text::Style {
                color: Some(theme.extended_palette().background.base.text)
            }),
        Space::new().width(Length::Fill)
    ]
    .spacing(15)
    .align_y(iced::Alignment::Center);

    let title = MouseArea::new(container(title_row).width(Length::Fill).padding(20))
        .on_press(Message::WindowDragged);

    let tab_row = row![
        button(text("Syntax"))
            .style(if tab == HelpTab::Syntax {
                iced::widget::button::primary
            } else {
                iced::widget::button::secondary
            })
            .width(Length::Fill)
            .on_press(Message::OpenHelp(HelpTab::Syntax)),
        button(text("Keyboard"))
            .style(if tab == HelpTab::Keyboard {
                iced::widget::button::primary
            } else {
                iced::widget::button::secondary
            })
            .width(Length::Fill)
            .on_press(Message::OpenHelp(HelpTab::Keyboard))
    ]
    .spacing(10)
    .padding(iced::Padding {
        left: 20.0,
        right: 20.0,
        top: 0.0,
        bottom: 10.0,
    });

    let data = match tab {
        HelpTab::Syntax => crate::help::SYNTAX_HELP,
        HelpTab::Keyboard => crate::help::KEYBOARD_HELP,
    };

    let mut content_col = column![].spacing(20).padding(20).max_width(800);

    for section in data {
        content_col = content_col.push(help_card(
            section.title,
            if tab == HelpTab::Syntax {
                crate::gui::icon::INFO
            } else {
                crate::gui::icon::KEYBOARD
            },
            section.items,
        ));
    }

    content_col = content_col.push(support_card());

    let footer =
        container(
            column![
                button(
                    text("Close")
                        .size(16)
                        .width(Length::Fill)
                        .align_x(iced::alignment::Horizontal::Center)
                )
                .padding(12)
                .width(Length::Fixed(200.0))
                .style(iced::widget::button::primary)
                .on_press(Message::CloseHelp),
                text(format!(
                    "Cfait v{} \u{2022} GPL3 \u{2022} Trougnouf (Benoit Brummer)",
                    env!("CARGO_PKG_VERSION")
                ))
                .size(12)
                .style(|_: &Theme| text::Style {
                    color: Some(COL_MUTED)
                }),
                button(text("https://codeberg.org/trougnouf/cfait").size(12).style(
                    |theme: &Theme| text::Style {
                        color: Some(if theme.extended_palette().is_dark {
                            COL_ACCENT
                        } else {
                            theme.extended_palette().primary.base.color
                        })
                    }
                ))
                .padding(0)
                .style(iced::widget::button::text)
                .on_press(Message::OpenUrl(
                    "https://codeberg.org/trougnouf/cfait".to_string()
                ))
            ]
            .spacing(15)
            .align_x(iced::Alignment::Center),
        )
        .width(Length::Fill)
        .center_x(Length::Fill)
        .padding(20);

    content_col = content_col.push(footer);

    column![
        title,
        tab_row,
        scrollable(
            container(content_col)
                .width(Length::Fill)
                .center_x(Length::Fill),
        )
        .height(Length::Fill)
    ]
    .into()
}

fn help_card<'a>(
    title: &'static str,
    icon_char: char,
    items: &'static [crate::help::HelpItem],
) -> Element<'a, Message> {
    let header = row![
        crate::gui::icon::icon(icon_char)
            .size(20)
            .style(|theme: &Theme| text::Style {
                color: Some(if theme.extended_palette().is_dark {
                    COL_ACCENT
                } else {
                    theme.extended_palette().primary.base.color
                })
            }),
        text(title).size(18).style(|theme: &Theme| text::Style {
            color: Some(if theme.extended_palette().is_dark {
                COL_ACCENT
            } else {
                theme.extended_palette().primary.base.color
            })
        })
    ]
    .spacing(10)
    .align_y(iced::Alignment::Center);

    let mut rows = column![
        header,
        iced::widget::rule::horizontal(1).style(|theme: &Theme| {
            let base = iced::widget::rule::default(theme);
            let palette = theme.extended_palette();
            iced::widget::rule::Style {
                color: Color {
                    a: 0.12,
                    ..palette.background.base.text
                },
                ..base
            }
        })
    ]
    .spacing(12);

    for item in items {
        let syntax_pill = container(text::<Theme, iced::Renderer>(item.keys).size(14).style(
            |theme: &Theme| {
                let palette = theme.extended_palette();
                text::Style {
                    color: Some(if palette.is_dark {
                        COL_SYNTAX
                    } else {
                        palette.primary.strong.color
                    }),
                }
            },
        ))
        .padding([2, 6])
        .style(|theme: &Theme| {
            let palette = theme.extended_palette();
            container::Style {
                background: Some(
                    Color {
                        a: 0.10,
                        ..palette.primary.base.color
                    }
                    .into(),
                ),
                border: iced::Border {
                    radius: 4.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        });

        let content = column![
            row![
                syntax_pill.width(Length::Fixed(120.0)),
                text::<Theme, iced::Renderer>(item.desc)
                    .size(14)
                    .width(Length::Fill)
                    .style(|theme: &Theme| text::Style {
                        color: Some(theme.extended_palette().background.base.text)
                    }),
            ]
            .spacing(10)
            .align_y(iced::Alignment::Center),
            if !item.example.is_empty() {
                Element::from(row![
                    Space::new().width(Length::Fixed(120.0)),
                    text::<Theme, iced::Renderer>(format!("e.g.: {}", item.example))
                        .size(12)
                        .style(|_: &Theme| text::Style {
                            color: Some(COL_MUTED)
                        })
                ])
            } else {
                Element::from(Space::new().height(0))
            }
        ]
        .spacing(2);

        rows = rows.push(content);
    }

    container(rows)
        .padding(15)
        .style(|theme: &Theme| {
            let palette = theme.extended_palette();
            container::Style {
                background: Some(
                    Color {
                        a: 0.98,
                        ..palette.background.weak.color
                    }
                    .into(),
                ),
                border: iced::Border {
                    radius: 8.0.into(),
                    width: 1.0,
                    color: Color {
                        a: 0.35,
                        ..palette.background.base.text
                    },
                },
                ..Default::default()
            }
        })
        .width(Length::Fill)
        .into()
}

// Keep support_card implementation unchanged but make it public within the module if needed,
// but since it's in the same file it doesn't matter.

fn support_card<'a>() -> Element<'a, Message> {
    use crate::gui::icon::*;

    let header = row![
        icon(HEART_HAND).size(20).style(|_: &Theme| text::Style {
            color: Some(Color::from_rgb(1.0, 0.4, 0.4))
        }),
        text("Support development")
            .size(18)
            .style(|theme: &Theme| text::Style {
                color: Some(theme.extended_palette().background.base.text)
            })
    ]
    .spacing(10)
    .align_y(iced::Alignment::Center);

    let copy_row = |icon_char: char, label: &'static str, val: &'static str| {
        row![
            icon(icon_char)
                .size(16)
                .width(Length::Fixed(24.0))
                .style(|_: &Theme| text::Style {
                    color: Some(COL_MUTED)
                }),
            text(label)
                .size(14)
                .width(Length::Fixed(100.0))
                .style(|_: &Theme| text::Style {
                    color: Some(COL_MUTED)
                }),
            text_input(val, val).size(14).padding(5).width(Length::Fill)
        ]
        .spacing(5)
        .align_y(iced::Alignment::Center)
    };

    let link_row = |icon_char: char, label: &'static str, url: &'static str| {
        row![
            icon(icon_char)
                .size(16)
                .width(Length::Fixed(24.0))
                .style(|_: &Theme| text::Style {
                    color: Some(COL_MUTED)
                }),
            text(label)
                .size(14)
                .width(Length::Fixed(100.0))
                .style(|_: &Theme| text::Style {
                    color: Some(COL_MUTED)
                }),
            button(text(url).size(14).style(|theme: &Theme| text::Style {
                color: Some(if theme.extended_palette().is_dark {
                    COL_ACCENT
                } else {
                    theme.extended_palette().primary.base.color
                })
            }))
            .padding(5)
            .width(Length::Fill)
            .style(iced::widget::button::text)
            .on_press(Message::OpenUrl(url.to_string()))
        ]
        .spacing(5)
        .align_y(iced::Alignment::Center)
    };

    let rows = column![
        header,
        iced::widget::rule::horizontal(1).style(|theme: &Theme| {
            let base = iced::widget::rule::default(theme);
            iced::widget::rule::Style {
                color: Color::from_rgb(0.3, 0.3, 0.3),
                ..base
            }
        }),
        link_row(CREDIT_CARD, "Liberapay", "https://liberapay.com/trougnouf"),
        link_row(CREDIT_CARD, "Ko-fi", "https://ko-fi.com/trougnouf"),
        copy_row(BANK, "Bank (SEPA)", "BE77 9731 6116 6342"),
        copy_row(
            BITCOIN,
            "Bitcoin",
            "bc1qc3z9ctv34v0ufxwpmq875r89umnt6ggeclp979"
        ),
        copy_row(
            LITECOIN,
            "Litecoin",
            "ltc1qv0xcmeuve080j7ad2cj2sd9d22kgqmlxfxvhmg"
        ),
        copy_row(
            ETHEREUM,
            "Ethereum",
            "0x0A5281F3B6f609aeb9D71D7ED7acbEc5d00687CB"
        ),
    ]
    .spacing(12);

    container(rows)
        .padding(15)
        .style(|theme: &Theme| {
            let palette = theme.extended_palette();
            container::Style {
                background: Some(
                    Color {
                        a: 0.98,
                        ..palette.background.weak.color
                    }
                    .into(),
                ),
                border: iced::Border {
                    radius: 8.0.into(),
                    width: 1.0,
                    color: Color {
                        a: 0.3,
                        ..palette.background.base.text
                    },
                },
                ..Default::default()
            }
        })
        .width(Length::Fill)
        .into()
}
