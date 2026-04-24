// File: ./src/gui/view/help.rs
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::gui::message::Message;
use crate::gui::state::{AppState, GuiApp};
use crate::gui::view::focusable::focusable;
use crate::help::HelpTab;
use iced::widget::{MouseArea, Space, button, column, container, row, scrollable, svg, text};
use iced::{Color, Element, Font, Length, Theme};

const COL_ACCENT: Color = Color::from_rgb(0.4, 0.7, 1.0);
const COL_SYNTAX: Color = Color::from_rgb(1.0, 0.85, 0.4);
const COL_MUTED: Color = Color::from_rgb(0.6, 0.6, 0.6);

// Helper function to create a tab button, solving the lifetime issue
fn tab_btn(label: &'static str, target_id: &'static str) -> Element<'static, Message> {
    button(text(label).size(16))
        .width(Length::Fill)
        .style(iced::widget::button::secondary)
        .padding(10)
        .on_press(Message::JumpToHelpSection(target_id))
        .into()
}

pub fn view_help<'a>(_tab: HelpTab, app: &'a GuiApp) -> Element<'a, Message> {
    let icon_choice = match app.state {
        AppState::Help(_, choice) => choice,
        _ => 0,
    };

    let help_icon = match icon_choice {
        0 => crate::gui::icon::HELP_ICON_QUESTION,
        1 => crate::gui::icon::HELP_ICON_ROBOT,
        _ => crate::gui::icon::HELP_ICON_ROBOT_HELP,
    };

    let title_row = row![
        button(crate::gui::icon::icon(crate::gui::icon::ARROW_LEFT).size(24))
            .style(iced::widget::button::text)
            .on_press(Message::CloseHelp),
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

    // --- 1. Sticky Top Tabs ---
    let tabs_row = row![
        tab_btn("Syntax", "help_syntax"),
        tab_btn("Shortcuts", "help_shortcuts"),
        tab_btn("Support & links", "help_about")
    ]
    .spacing(10)
    .padding(iced::Padding {
        left: 20.0,
        right: 20.0,
        bottom: 15.0,
        top: 0.0,
    });

    // --- 2. Continuous Content Generation ---
    let mut content_col = column![].spacing(30).padding(20).max_width(800);

    // A. Syntax Section
    let syntax_anchor = focusable(
        text("Syntax")
            .size(28)
            .font(Font {
                weight: iced::font::Weight::Bold,
                ..Default::default()
            })
            .style(|theme: &Theme| text::Style {
                color: Some(if theme.extended_palette().is_dark {
                    COL_ACCENT
                } else {
                    theme.extended_palette().primary.base.color
                }),
            }),
    )
    .id(iced::widget::Id::new("help_syntax"));

    content_col = content_col.push(syntax_anchor);
    for section in crate::help::get_syntax_help() {
        content_col = content_col.push(help_card(&section.title, &section.items));
    }

    // B. Shortcuts Section
    let shortcuts_anchor = focusable(
        text("Shortcuts")
            .size(28)
            .font(Font {
                weight: iced::font::Weight::Bold,
                ..Default::default()
            })
            .style(|theme: &Theme| text::Style {
                color: Some(if theme.extended_palette().is_dark {
                    COL_ACCENT
                } else {
                    theme.extended_palette().primary.base.color
                }),
            }),
    )
    .id(iced::widget::Id::new("help_shortcuts"));

    content_col = content_col.push(shortcuts_anchor);
    for section in crate::help::get_shortcuts_help() {
        content_col = content_col.push(help_card(&section.title, &section.items));
    }

    // C. About / Support Section
    let about_anchor = focusable(
        text("Support development")
            .size(28)
            .font(Font {
                weight: iced::font::Weight::Bold,
                ..Default::default()
            })
            .style(|theme: &Theme| text::Style {
                color: Some(if theme.extended_palette().is_dark {
                    COL_ACCENT
                } else {
                    theme.extended_palette().primary.base.color
                }),
            }),
    )
    .id(iced::widget::Id::new("help_about"));

    let footer_links = column![
        text(format!("Cfait v{}  •  GPL-3.0", env!("CARGO_PKG_VERSION")))
            .size(14)
            .style(|_: &Theme| text::Style {
                color: Some(COL_MUTED)
            }),
        text("Trougnouf (Benoit Brummer)")
            .size(14)
            .style(|_: &Theme| text::Style {
                color: Some(COL_MUTED)
            }),
        button(
            text("https://codeberg.org/trougnouf/cfait")
                .size(14)
                .style(|theme: &Theme| text::Style {
                    color: Some(if theme.extended_palette().is_dark {
                        COL_ACCENT
                    } else {
                        theme.extended_palette().primary.base.color
                    })
                })
        )
        .padding(0)
        .style(iced::widget::button::text)
        .on_press(Message::OpenUrl(
            "https://codeberg.org/trougnouf/cfait".to_string()
        )),
        button(
            text("Chat: #Cfait:matrix.org")
                .size(14)
                .style(|theme: &Theme| text::Style {
                    color: Some(if theme.extended_palette().is_dark {
                        COL_ACCENT
                    } else {
                        theme.extended_palette().primary.base.color
                    })
                })
        )
        .padding(0)
        .style(iced::widget::button::text)
        .on_press(Message::OpenUrl(
            "https://matrix.to/#/#Cfait:matrix.org".to_string()
        ))
    ]
    .spacing(8)
    .align_x(iced::Alignment::Center);

    content_col = content_col.push(
        column![about_anchor, support_card(), footer_links]
            .spacing(20)
            .align_x(iced::Alignment::Center),
    );

    // --- 3. Final Assembly ---
    let scrollable_content = scrollable(
        container(content_col)
            .width(Length::Fill)
            .center_x(Length::Fill),
    );

    column![title, tabs_row, scrollable_content].into()
}

fn help_card<'a>(title: &str, items: &[crate::help::HelpItem]) -> Element<'a, Message> {
    let header = text(title.to_string())
        .size(18)
        .font(Font {
            weight: iced::font::Weight::Bold,
            ..Default::default()
        })
        .style(|theme: &Theme| text::Style {
            color: Some(theme.extended_palette().background.base.text),
        });

    let mut rows = column![header, iced::widget::rule::horizontal(1)].spacing(12);

    for item in items {
        let syntax_pill = container(
            text::<Theme, iced::Renderer>(item.keys.clone())
                .size(14)
                .font(Font::MONOSPACE)
                .style(|theme: &Theme| text::Style {
                    color: Some(if theme.extended_palette().is_dark {
                        COL_SYNTAX
                    } else {
                        theme.extended_palette().primary.strong.color
                    }),
                }),
        )
        .padding([4, 8])
        .style(|theme: &Theme| container::Style {
            background: Some(
                Color {
                    a: 0.15,
                    ..theme.extended_palette().primary.base.color
                }
                .into(),
            ),
            border: iced::Border {
                radius: 6.0.into(),
                ..Default::default()
            },
            ..Default::default()
        });

        let content = column![
            row![
                syntax_pill.width(Length::Fixed(170.0)),
                text::<Theme, iced::Renderer>(item.desc.clone())
                    .size(15)
                    .width(Length::Fill)
            ]
            .spacing(15)
            .align_y(iced::Alignment::Center),
            if !item.example.is_empty() {
                Element::from(row![
                    Space::new().width(Length::Fixed(185.0)),
                    text::<Theme, iced::Renderer>(format!("e.g. {}", item.example))
                        .size(13)
                        .font(Font::MONOSPACE)
                        .style(|_: &Theme| text::Style {
                            color: Some(COL_MUTED)
                        })
                ])
            } else {
                Element::from(Space::new().height(0))
            }
        ]
        .spacing(4);

        rows = rows.push(content);
    }

    container(rows)
        .padding(20)
        .style(|theme: &Theme| container::Style {
            background: Some(
                Color {
                    a: 0.5,
                    ..theme.extended_palette().background.weak.color
                }
                .into(),
            ),
            border: iced::Border {
                radius: 8.0.into(),
                width: 1.0,
                color: Color {
                    a: 0.2,
                    ..theme.extended_palette().background.base.text
                },
            },
            ..Default::default()
        })
        .width(Length::Fill)
        .into()
}

fn support_card<'a>() -> Element<'a, Message> {
    let mut content = column![].spacing(8);

    content = content.push(donation_row(
        crate::gui::icon::CREDIT_CARD,
        "Liberapay",
        "https://liberapay.com/trougnouf",
        false,
    ));
    content = content.push(donation_row(
        crate::gui::icon::CREDIT_CARD,
        "Ko-fi",
        "https://ko-fi.com/trougnouf",
        false,
    ));
    content = content.push(donation_row(
        crate::gui::icon::BANK,
        "Bank (SEPA)",
        "BE77 9731 6116 6342",
        true,
    ));
    content = content.push(donation_row(
        crate::gui::icon::BITCOIN,
        "Bitcoin",
        "bc1qc3z9ctv34v0ufxwpmq875r89umnt6ggeclp979",
        true,
    ));
    content = content.push(donation_row(
        crate::gui::icon::LITECOIN,
        "Litecoin",
        "ltc1qv0xcmeuve080j7ad2cj2sd9d22kgqmlxfxvhmg",
        true,
    ));
    content = content.push(donation_row(
        crate::gui::icon::ETHEREUM,
        "Ethereum",
        "0x0A5281F3B6f609aeb9D71D7ED7acbEc5d00687CB",
        true,
    ));

    container(content)
        .padding(20)
        .style(|theme: &Theme| container::Style {
            background: Some(
                Color {
                    a: 0.5,
                    ..theme.extended_palette().background.weak.color
                }
                .into(),
            ),
            border: iced::Border {
                radius: 8.0.into(),
                width: 1.0,
                color: Color {
                    a: 0.2,
                    ..theme.extended_palette().background.base.text
                },
            },
            ..Default::default()
        })
        .width(Length::Fill)
        .into()
}

fn donation_row<'a>(
    icon_char: char,
    name: &'a str,
    value: &'a str,
    is_copy: bool,
) -> Element<'a, Message> {
    let row_content = row![
        crate::gui::icon::icon(icon_char)
            .size(18)
            .style(|_: &Theme| text::Style {
                color: Some(COL_MUTED),
            }),
        Space::new().width(12),
        column![
            text(name).size(15),
            text(value).size(12).style(|_: &Theme| text::Style {
                color: Some(COL_MUTED),
            })
        ]
        .spacing(4),
        Space::new().width(Length::Fill),
        crate::gui::icon::icon(if is_copy {
            crate::gui::icon::COPY
        } else {
            crate::gui::icon::EXTERNAL_LINK
        })
        .size(16)
        .style(|_: &Theme| text::Style {
            color: Some(COL_MUTED),
        })
    ]
    .align_y(iced::Alignment::Center);

    let mut btn = button(row_content)
        .width(Length::Fill)
        .padding(8)
        .style(iced::widget::button::text);

    if is_copy {
        btn = btn.on_press(Message::YankSelected);
    } else {
        btn = btn.on_press(Message::OpenUrl(value.to_string()));
    }

    btn.into()
}
