// File: ./src/gui/view/help.rs
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::gui::message::Message;
use crate::gui::state::{AppState, GuiApp};
use crate::help::HelpTab;
use iced::widget::{MouseArea, Space, button, column, container, row, scrollable, svg, text};
use iced::{Color, Element, Font, Length, Theme};

const COL_ACCENT: Color = Color::from_rgb(0.4, 0.7, 1.0);
const COL_SYNTAX: Color = Color::from_rgb(1.0, 0.85, 0.4);
const COL_MUTED: Color = Color::from_rgb(0.6, 0.6, 0.6);

// Helper function to create a pretty, colored, rounded tab button
fn tab_btn(
    icon_char: char,
    label: String,
    is_active: bool,
    msg: Message,
    base_color: Color,
) -> Element<'static, Message> {
    let content = row![
        text(icon_char).font(crate::gui::icon::FONT).size(18),
        text(label).size(16)
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    button(container(content).center_x(Length::Fill))
        .width(Length::Fill)
        .padding([12, 20])
        .style(
            move |_theme: &Theme, status: iced::widget::button::Status| {
                let is_hovered = status == iced::widget::button::Status::Hovered;
                let bg_alpha = if is_active {
                    0.4
                } else if is_hovered {
                    0.25
                } else {
                    0.15
                };

                let border_alpha = if is_active { 0.9 } else { 0.5 };

                iced::widget::button::Style {
                    background: Some(
                        Color {
                            a: bg_alpha,
                            ..base_color
                        }
                        .into(),
                    ),
                    text_color: base_color,
                    border: iced::Border {
                        radius: 20.0.into(),
                        width: if is_active { 2.0 } else { 1.0 },
                        color: Color {
                            a: border_alpha,
                            ..base_color
                        },
                    },
                    ..Default::default()
                }
            },
        )
        .on_press(msg)
        .into()
}

pub fn view_help<'a>(tab: HelpTab, app: &'a GuiApp) -> Element<'a, Message> {
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
    let support_icons = [
        '\u{f0a52}',
        '\u{f185c}',
        '\u{f188f}',
        '\u{f118}',
        '\u{eda9}',
        '\u{eeed}',
        '\u{f0b79}',
    ];
    let support_icon = support_icons[(icon_choice as usize) % support_icons.len()];

    let tabs_row = row![
        tab_btn(
            '\u{f1fa}',
            rust_i18n::t!("syntax_help").to_string(),
            tab == HelpTab::Syntax,
            Message::OpenHelp(HelpTab::Syntax),
            Color::from_rgb(0.9, 0.6, 0.2)
        ),
        tab_btn(
            '\u{f11c}',
            rust_i18n::t!("keyboard_shortcuts").to_string(),
            tab == HelpTab::Shortcuts,
            Message::OpenHelp(HelpTab::Shortcuts),
            Color::from_rgb(0.2, 0.8, 0.4)
        ),
        tab_btn(
            support_icon,
            rust_i18n::t!("support_card_title").to_string(),
            tab == HelpTab::About,
            Message::OpenHelp(HelpTab::About),
            Color::from_rgb(0.8, 0.3, 0.7)
        ),
    ]
    .spacing(15)
    .padding(iced::Padding {
        left: 20.0,
        right: 20.0,
        bottom: 15.0,
        top: 0.0,
    });

    // --- 2. Dynamic Content Generation ---
    let mut content_col = column![].spacing(30).padding(20).max_width(800);

    match tab {
        HelpTab::Syntax => {
            let syntax_header = row![
                text('\u{f1fa}')
                    .font(crate::gui::icon::FONT)
                    .size(28)
                    .style(|_theme: &Theme| text::Style {
                        color: Some(COL_ACCENT)
                    }),
                text("Syntax")
                    .size(28)
                    .font(Font {
                        weight: iced::font::Weight::Bold,
                        ..Default::default()
                    })
                    .style(|_theme: &Theme| text::Style {
                        color: Some(COL_ACCENT)
                    })
            ]
            .spacing(12)
            .align_y(iced::Alignment::Center);

            content_col = content_col.push(syntax_header);

            for section in crate::help::get_syntax_help() {
                content_col = content_col.push(help_card(&section.title, &section.items));
            }
        }
        HelpTab::Shortcuts => {
            let shortcuts_header = row![
                text('\u{f11c}')
                    .font(crate::gui::icon::FONT)
                    .size(28)
                    .style(|_theme: &Theme| text::Style {
                        color: Some(COL_ACCENT)
                    }),
                text("Shortcuts")
                    .size(28)
                    .font(Font {
                        weight: iced::font::Weight::Bold,
                        ..Default::default()
                    })
                    .style(|_theme: &Theme| text::Style {
                        color: Some(COL_ACCENT)
                    })
            ]
            .spacing(12)
            .align_y(iced::Alignment::Center);

            content_col = content_col.push(shortcuts_header);

            for section in crate::help::get_shortcuts_help() {
                content_col = content_col.push(help_card(&section.title, &section.items));
            }
        }
        HelpTab::About => {
            let about_header = row![
                text(support_icon)
                    .font(crate::gui::icon::FONT)
                    .size(28)
                    .style(|_theme: &Theme| text::Style {
                        color: Some(COL_ACCENT)
                    }),
                text(rust_i18n::t!("support_card_title").to_string())
                    .size(28)
                    .font(Font {
                        weight: iced::font::Weight::Bold,
                        ..Default::default()
                    })
                    .style(|_theme: &Theme| text::Style {
                        color: Some(COL_ACCENT)
                    })
            ]
            .spacing(12)
            .align_y(iced::Alignment::Center);

            let footer_links = column![
                text(rust_i18n::t!("about_title").to_string())
                    .size(14)
                    .style(|_: &Theme| text::Style {
                        color: Some(COL_MUTED)
                    }),
                text(rust_i18n::t!("about_version", version = env!("CARGO_PKG_VERSION")).to_string())
                    .size(14)
                    .style(|_: &Theme| text::Style {
                        color: Some(COL_MUTED)
                    }),
                text(rust_i18n::t!("about_license").to_string())
                    .size(14)
                    .style(|_: &Theme| text::Style {
                        color: Some(COL_MUTED)
                    }),
                button(text(rust_i18n::t!("about_repository", url = "https://codeberg.org/trougnouf/cfait").to_string()).size(14).style(
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
                )),
                button(
                    text(rust_i18n::t!("about_chat", url = "#Cfait:matrix.org").to_string())
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
                column![about_header, support_card(), footer_links]
                    .spacing(20)
                    .align_x(iced::Alignment::Center),
            );
        }
    }

    // --- 3. Final Assembly ---
    // Assign a unique ID per tab so the scroll position resets to the top when navigating
    let scrollable_id = match tab {
        HelpTab::Syntax => iced::widget::Id::new("scroll_help_syntax"),
        HelpTab::Shortcuts => iced::widget::Id::new("scroll_help_shortcuts"),
        HelpTab::About => iced::widget::Id::new("scroll_help_about"),
    };

    let scrollable_content = scrollable(
        container(content_col)
            .width(Length::Fill)
            .center_x(Length::Fill),
    )
    .id(scrollable_id);

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
                syntax_pill.width(Length::Fixed(180.0)),
                text::<Theme, iced::Renderer>(item.desc.clone())
                    .size(15)
                    .width(Length::Fill)
            ]
            .spacing(15)
            .align_y(iced::Alignment::Center),
            if !item.example.is_empty() {
                Element::from(row![
                    Space::new().width(Length::Fixed(195.0)),
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
