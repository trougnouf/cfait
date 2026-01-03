// Renders the help screen overlay.
use crate::gui::message::Message;
use iced::widget::{Space, button, column, container, row, scrollable, svg, text, text_input};
use iced::{Color, Element, Length, Theme};
use std::time::{SystemTime, UNIX_EPOCH};

// --- STYLE CONSTANTS ---
const COL_ACCENT: Color = Color::from_rgb(0.4, 0.7, 1.0); // Soft Blue
const COL_SYNTAX: Color = Color::from_rgb(1.0, 0.85, 0.4); // Gold/Yellow
const COL_MUTED: Color = Color::from_rgb(0.6, 0.6, 0.6); // Grey
const COL_CARD_BG: Color = Color::from_rgb(0.15, 0.15, 0.17); // Slightly lighter than pure black

pub fn view_help() -> Element<'static, Message> {
    // Randomize icon based on current time (choose between 3 icons)
    let icon_choice = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| (d.as_secs() % 3) as u8)
        .unwrap_or(0);

    let help_icon = match icon_choice {
        0 => crate::gui::icon::HELP_ICON_QUESTION,
        1 => crate::gui::icon::HELP_ICON_ROBOT,
        _ => crate::gui::icon::HELP_ICON_ROBOT_HELP,
    };

    let back_btn = button(crate::gui::icon::icon(crate::gui::icon::ARROW_LEFT).size(24))
        .style(iced::widget::button::text)
        .on_press(Message::CloseHelp);

    let title = row![
        back_btn,
        svg(svg::Handle::from_memory(help_icon))
            .width(Length::Fixed(84.0)) // 168 / 2 for smooth rendering at half native size
            .height(Length::Fixed(32.0)) // 64 / 2 for smooth rendering at half native size
            .content_fit(iced::ContentFit::Contain),
        text("Help & About")
            .size(28)
            .style(|_: &Theme| text::Style {
                color: Some(Color::WHITE)
            })
    ]
    .spacing(15)
    .align_y(iced::Alignment::Center);

    let content = column![
        // 1. FUNDAMENTALS
        help_card(
            "Organization",
            crate::gui::icon::TAG,
            vec![
                entry("!1", "Priority High (1) to Low (9)", "!1, !5, !9"),
                entry(
                    "#tag",
                    "Add category. Use ':' for sub-tags.",
                    "#work, #dev:backend, #work:project:urgent"
                ),
                entry(
                    "@@loc",
                    "Location. Supports hierarchy with ':'.",
                    "@@home, @@home:office, @@store:aldi:downtown"
                ),
                entry("~30m", "Estimated Duration (m/h/d/w).", "~30m, ~1.5h, ~2d"),
                entry(
                    "#a:=#b,#c,@@d",
                    "Define tag alias inline (Retroactive).",
                    "#tree_planting:=#gardening,@@home"
                ),
                entry(
                    "@@a:=#b,#c",
                    "Define location alias (Retroactive).",
                    "@@aldi:=#groceries,#shopping"
                ),
                entry(
                    "\\#text",
                    "Escape special characters.",
                    "\\#not-a-tag \\@not-a-date"
                ),
            ]
        ),
        // 2. TIMELINE
        help_card(
            "Timeline & Scheduling",
            crate::gui::icon::CALENDAR,
            vec![
                entry(
                    "@date",
                    "Due Date. Deadline for the task.",
                    "@tomorrow, @2025-12-31"
                ),
                entry("^date", "Start Date (Defer until).", "^next week, ^2025-01-01"),
                entry(
                    "Offsets",
                    "Add time from today.",
                    "1d, 2w, 3mo (optional: @2 weeks = @in 2 weeks)"
                ),
                entry(
                    "Weekdays",
                    "Next occurrence (\"next\" is optional).",
                    "@friday = @next friday, @monday"
                ),
                entry(
                    "Next period",
                    "Next week/month/year.",
                    "@next week, @next month, @next year"
                ),
                entry("Keywords", "Relative dates supported.", "today, tomorrow"),
            ]
        ),
        // 3. RECURRENCE
        help_card(
            "Recurrence",
            crate::gui::icon::REPEAT,
            vec![
                entry(
                    "@daily",
                    "Quick presets.",
                    "@daily, @weekly, @monthly, @yearly"
                ),
                entry(
                    "@every X",
                    "Custom intervals.",
                    "@every 3 days, @every 2 weeks"
                ),
                entry(
                    "@every <weekday>",
                    "Specific weekdays (comma-separated).",
                    "@every monday, @every monday,wednesday,friday"
                ),
                entry(
                    "Auto-dates",
                    "Recurrence without dates auto-sets start & due to first occurrence.",
                    "@daily → today, @every monday → next Monday"
                ),
                entry(
                    "until <date>",
                    "End date for recurrence (inclusive).",
                    "@daily until 2025-12-31, @every 2 weeks until 2026-06-30"
                ),
                entry(
                    "except <date>",
                    "Skip specific dates (comma-separated).",
                    "@weekly except 2025-01-20, @daily except 2025-12-25,2026-01-01"
                ),
                entry(
                    "except <weekday>",
                    "Exclude weekdays (short/long/plural).",
                    "@daily except mo,tue or @daily except saturdays,sundays"
                ),
                entry(
                    "except <month>",
                    "Exclude entire months (short/long).",
                    "@monthly except oct,nov,dec or @weekly except march"
                ),
                entry(
                    "Note",
                    "Recurrence calculates next date based on Start Date if present, else Due Date.",
                    ""
                ),
            ]
        ),
        help_card(
            "Metadata",
            crate::gui::icon::INFO,
            vec![
                entry("url:", "Attach a link.", "url:https://perdu.com"),
                entry(
                    "geo:",
                    "Coordinates (lat,long).",
                    "geo:53.046070, -121.105264"
                ),
                entry("desc:", "Append description text.", "desc:\"Call back later\""),
                entry(
                    "rem:10m",
                    "Relative reminder (before due date).",
                    "Adjusts if due date changes"
                ),
                entry(
                    "rem:in 5m",
                    "Relative from now (becomes absolute).",
                    "rem:in 2h (5 min/2 hours from now)"
                ),
                entry(
                    "rem:next friday",
                    "Next occurrence (becomes absolute).",
                    "rem:next week, rem:next month"
                ),
                entry(
                    "rem:8am",
                    "Absolute reminder (fixed time).",
                    "rem:2025-01-20 9am, rem:2025-12-31 10:00"
                ),
                entry(
                    "+cal",
                    "Force create calendar event.",
                    "Overrides global setting"
                ),
                entry(
                    "-cal",
                    "Prevent calendar event creation.",
                    "Overrides global setting"
                ),
            ]
        ),
        // 4. POWER SEARCH
        help_card(
            "Search & Filtering",
            crate::gui::icon::SHIELD,
            vec![
                entry("text", "Matches summary or description.", "buy cat food"),
                entry("#tag", "Filter by specific tag.", "#gardening"),
                entry(
                    "is:ready",
                    "Work Mode - actionable tasks only.",
                    "Not done, start date passed, not blocked"
                ),
                entry(
                    "is:status",
                    "Filter by state.",
                    "is:done, is:started, is:active, is:blocked, is:ready"
                ),
                entry(
                    "Operators",
                    "Compare values (<, >, <=, >=).",
                    "~<20m (less than 20 minutes), <!4 (urgent tasks)"
                ),
                entry(
                    "  Dates",
                    "Filter by timeframe.",
                    "@<today (Overdue), ^>1w (Start in 1+ weeks)"
                ),
                entry(
                    "  Date!",
                    "Include unset dates with '!' suffix.",
                    "@<today! (Overdue OR no due date)"
                ),
                entry(
                    "  Priority",
                    "Filter by priority range.",
                    "!<3 (High prio), !>=5"
                ),
                entry("  Duration", "Filter by effort.", "~<15m (Quick tasks)"),
                entry(
                    "  Location",
                    "Filter by location (matches sub-locations).",
                    "@@home, @@store:aldi"
                ),
                entry(
                    "Combine",
                    "Mix multiple filters.",
                    "is:ready #work ~<1h (Actionable work tasks under 1 hour)"
                ),
            ]
        ),
        help_card(
            "Tips",
            crate::gui::icon::INFO,
            vec![
                entry(
                    "Escape",
                    "Use \\ to treat special chars as text.",
                    "Buy \\#tag literally"
                ),
                entry(
                    "Quotes",
                    "Use \" \" or { } for values with spaces.",
                    "@@\"my office\""
                ),
                entry("Next dates", "Use natural language.", "@next monday, @next week"),
                entry(
                    "Reminders",
                    "rem:10m (before due) vs rem:next friday.",
                    "rem:in 5m (from now), rem:8am (absolute)"
                ),
            ]
        ),
        // 5. SUPPORT
        support_card(),
        // FOOTER
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
                button(
                    text("https://codeberg.org/trougnouf/cfait")
                        .size(12)
                        .style(|_: &Theme| text::Style {
                            color: Some(COL_ACCENT)
                        })
                )
                .padding(0)
                .style(iced::widget::button::text)
                .on_press(Message::OpenUrl(
                    "https://codeberg.org/trougnouf/cfait".to_string()
                ))
            ]
            .spacing(15)
            .align_x(iced::Alignment::Center)
        )
        .width(Length::Fill)
        .center_x(Length::Fill)
        .padding(20)
    ]
    .spacing(20)
    .padding(20)
    .max_width(800);

    column![
        container(title).padding(20),
        scrollable(
            container(content)
                .width(Length::Fill)
                .center_x(Length::Fill),
        )
        .height(Length::Fill)
    ]
    .into()
}

// --- HELPERS ---

struct HelpEntry {
    syntax: &'static str,
    desc: &'static str,
    example: &'static str,
}

fn entry(syntax: &'static str, desc: &'static str, example: &'static str) -> HelpEntry {
    HelpEntry {
        syntax,
        desc,
        example,
    }
}

fn help_card(
    title: &'static str,
    icon_char: char,
    items: Vec<HelpEntry>,
) -> Element<'static, Message> {
    let header = row![
        crate::gui::icon::icon(icon_char)
            .size(20)
            .style(|_: &Theme| text::Style {
                color: Some(COL_ACCENT)
            }),
        text(title).size(18).style(|_: &Theme| text::Style {
            color: Some(COL_ACCENT)
        })
    ]
    .spacing(10)
    .align_y(iced::Alignment::Center);

    let mut rows = column![
        header,
        iced::widget::rule::horizontal(1).style(|theme: &Theme| {
            let base = iced::widget::rule::default(theme);
            iced::widget::rule::Style {
                color: Color::from_rgb(0.3, 0.3, 0.3),
                ..base
            }
        })
    ]
    .spacing(12);

    for item in items {
        let syntax_pill = container(text::<Theme, iced::Renderer>(item.syntax).size(14).style(
            |_: &Theme| text::Style {
                color: Some(COL_SYNTAX),
            },
        ))
        .padding([2, 6])
        .style(|_: &Theme| container::Style {
            background: Some(Color::from_rgba(1.0, 0.85, 0.4, 0.1).into()),
            border: iced::Border {
                radius: 4.0.into(),
                ..Default::default()
            },
            ..Default::default()
        });

        let content = column![
            row![
                syntax_pill.width(Length::Fixed(120.0)),
                text::<Theme, iced::Renderer>(item.desc)
                    .size(14)
                    .width(Length::Fill)
                    .style(|_: &Theme| text::Style {
                        color: Some(Color::WHITE)
                    }),
            ]
            .spacing(10)
            .align_y(iced::Alignment::Center),
            if !item.example.is_empty() {
                Element::new(row![
                    Space::new().width(Length::Fixed(120.0)),
                    text::<Theme, iced::Renderer>(format!("e.g.: {}", item.example))
                        .size(12)
                        .style(|_: &Theme| text::Style {
                            color: Some(COL_MUTED)
                        })
                ])
            } else {
                Element::new(Space::new().height(0))
            }
        ]
        .spacing(2);

        rows = rows.push(content);
    }

    container(rows)
        .padding(15)
        .style(|_: &Theme| container::Style {
            background: Some(COL_CARD_BG.into()),
            border: iced::Border {
                radius: 8.0.into(),
                width: 1.0,
                color: Color::from_rgb(0.25, 0.25, 0.28),
            },
            ..Default::default()
        })
        .width(Length::Fill)
        .into()
}

fn support_card() -> Element<'static, Message> {
    use crate::gui::icon::*;

    let header = row![
        icon(HEART_HAND).size(20).style(|_: &Theme| text::Style {
            color: Some(Color::from_rgb(1.0, 0.4, 0.4))
        }),
        text("Support Development")
            .size(18)
            .style(|_: &Theme| text::Style {
                color: Some(Color::WHITE)
            })
    ]
    .spacing(10)
    .align_y(iced::Alignment::Center);

    // Explicitly type arguments as &'static str to satisfy Element<'static> requirements
    // and avoid lifetime inference errors when creating the Row.
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
            button(text(url).size(14).style(|_: &Theme| text::Style {
                color: Some(COL_ACCENT)
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
        .style(|_: &Theme| container::Style {
            background: Some(COL_CARD_BG.into()),
            border: iced::Border {
                radius: 8.0.into(),
                width: 1.0,
                color: Color::from_rgb(0.25, 0.25, 0.28),
            },
            ..Default::default()
        })
        .width(Length::Fill)
        .into()
}
