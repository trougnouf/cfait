// File: src/tui/view.rs
use crate::color_utils;
use crate::model::parser::{SyntaxType, tokenize_smart_input};
use crate::store::UNCATEGORIZED_ID;
use crate::tui::action::SidebarMode;
use crate::tui::state::{AppState, Focus, InputMode};

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};

pub fn draw(f: &mut Frame, state: &mut AppState) {
    let full_help_text = vec![
        Line::from(vec![
            Span::styled(
                " GLOBAL ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Tab:Switch Focus  ?:Toggle Help  q:Quit"),
        ]),
        Line::from(vec![
            Span::styled(
                " NAVIGATION ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" j/k:Up/Down  PgUp/PgDn:Scroll"),
        ]),
        Line::from(vec![
            Span::styled(
                " TASKS ",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" a:Add  e:Edit Title  E:Edit Desc  d:Delete  Space:Toggle Done"),
        ]),
        Line::from(vec![
            Span::styled("       ", Style::default()), // Indent alignment
            Span::raw("s:Start/Pause  S:Stop  x:Cancel  M:Move  r:Sync  X:Export"),
        ]),
        Line::from(vec![
            Span::styled(
                " ORGANIZATION ",
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" +/-:Priority  </>:Indent  y:Yank { b:Block  c:Child }  C:NewChild"),
        ]),
        Line::from(vec![
            Span::styled("              ", Style::default()), // Indent alignment
            Span::raw("#alias:=#tag,@@loc (Define alias inline, retroactive)"),
        ]),
        Line::from(vec![
            Span::styled(
                " VIEW & FILTER ",
                Style::default()
                    .fg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" /:Search  H:Hide Completed  1:Cal  2:Tag  3:Loc"),
        ]),
        Line::from(vec![
            Span::styled(
                " SIDEBAR ",
                Style::default()
                    .fg(Color::LightCyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(
                " Enter:Select/Toggle  Space:Toggle Visibility  *:Show/Clear All  Right:Focus",
            ),
        ]),
    ];

    let footer_height = if state.mode == InputMode::EditingDescription {
        Constraint::Length(10)
    } else if state.show_full_help {
        Constraint::Length(full_help_text.len() as u16 + 2)
    } else {
        Constraint::Length(3)
    };

    let v_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), footer_height])
        .split(f.area());

    let h_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(20), Constraint::Percentage(80)])
        .split(v_chunks[0]);

    // --- 1. Prepare Details Text ---
    let mut full_details = String::new();
    if let Some(task) = state.get_selected_task() {
        if !task.description.is_empty() {
            full_details.push_str(&task.description);
            full_details.push_str("\n\n");
        }

        // Render extra fields
        if let Some(url) = &task.url {
            full_details.push_str(&format!("URL: {}\n", url));
        }
        if let Some(geo) = &task.geo {
            full_details.push_str(&format!("Geo: {}\n", geo));
        }
        if let Some(loc) = &task.location {
            full_details.push_str(&format!("Location: {}\n", loc));
        }
        if !full_details.is_empty() {
            full_details.push('\n');
        }

        if !task.dependencies.is_empty() {
            full_details.push_str("[Blocked By]:\n");
            for dep_uid in &task.dependencies {
                let name = state
                    .store
                    .get_summary(dep_uid)
                    .unwrap_or_else(|| "Unknown task".to_string());
                let is_done = state.store.get_task_status(dep_uid).unwrap_or(false);
                let check = if is_done { "[x]" } else { "[ ]" };
                full_details.push_str(&format!(" {} {}\n", check, name));
            }
        }
    }
    if full_details.is_empty() {
        full_details = "No details.".to_string();
    }

    // --- 2. Calculate Dynamic Height ---
    let details_width = h_chunks[1].width.saturating_sub(2); // subtract borders
    let mut required_lines: u16 = 0;

    if details_width > 0 {
        for line in full_details.lines() {
            let line_len = line.chars().count() as u16;
            if line_len == 0 {
                required_lines += 1;
            } else {
                required_lines += line_len.div_ceil(details_width);
            }
        }
    }

    let calculated_height = required_lines + 2;
    let available_height = v_chunks[0].height;
    let max_details_height = available_height / 2;
    let final_details_height = calculated_height.clamp(3, max_details_height);

    // --- 3. Dynamic Layout ---
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),                       // Task list takes remaining space
            Constraint::Length(final_details_height), // Details takes only what it needs
        ])
        .split(h_chunks[1]);

    // --- Sidebar ---
    let sidebar_style = if state.active_focus == Focus::Sidebar {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };

    let (sidebar_title, sidebar_items) = match state.sidebar_mode {
        SidebarMode::Calendars => {
            let items: Vec<ListItem> = state
                .calendars
                .iter()
                .filter(|c| !state.disabled_calendars.contains(&c.href))
                .map(|c| {
                    let is_target = Some(&c.href) == state.active_cal_href.as_ref();
                    let is_visible = !state.hidden_calendars.contains(&c.href);

                    let cal_color_style = if is_visible {
                        if let Some(hex) = &c.color
                            && let Some((r, g, b)) = color_utils::parse_hex_to_u8(hex)
                        {
                            Style::default().fg(Color::Rgb(r, g, b))
                        } else {
                            Style::default()
                        }
                    } else {
                        Style::default().fg(Color::DarkGray)
                    };

                    let prefix = if is_target { ">" } else { " " };
                    let check_mark = if is_visible { "x" } else { " " };

                    let mut spans = vec![
                        Span::raw(format!("{} ", prefix)),
                        Span::styled("[", cal_color_style),
                        Span::raw(check_mark),
                        Span::styled("]", cal_color_style),
                    ];

                    let text_style = if is_target {
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD)
                    } else if !is_visible {
                        Style::default().fg(Color::DarkGray)
                    } else {
                        Style::default()
                    };

                    spans.push(Span::styled(format!(" {}", c.name), text_style));

                    ListItem::new(Line::from(spans))
                })
                .collect();
            ("  Calendars ".to_string(), items)
        }
        SidebarMode::Categories => {
            let all_cats = state.store.get_all_categories(
                state.hide_completed,
                state.hide_fully_completed_tags,
                &state.selected_categories,
                &state.hidden_calendars,
            );
            let items: Vec<ListItem> = all_cats
                .iter()
                .map(|(c, count)| {
                    let selected = if state.selected_categories.contains(c) {
                        "[x]"
                    } else {
                        "[ ]"
                    };
                    if c == UNCATEGORIZED_ID {
                        ListItem::new(Line::from(format!(
                            "{} Uncategorized ({})",
                            selected, count
                        )))
                    } else {
                        let (r, g, b) = color_utils::generate_color(c);
                        let color =
                            Color::Rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8);
                        let spans = vec![
                            Span::raw(format!("{} ", selected)),
                            Span::styled("#", Style::default().fg(color)),
                            Span::raw(format!("{} ({})", c, count)),
                        ];
                        ListItem::new(Line::from(spans))
                    }
                })
                .collect();
            let logic = if state.match_all_categories {
                "AND"
            } else {
                "OR"
            };
            (format!("  Tags ({}) ", logic), items)
        }
        SidebarMode::Locations => {
            let all_locs = state
                .store
                .get_all_locations(state.hide_completed, &state.hidden_calendars);
            let items: Vec<ListItem> = all_locs
                .iter()
                .map(|(loc, count)| {
                    let selected = if state.selected_locations.contains(loc) {
                        "[x]"
                    } else {
                        "[ ]"
                    };
                    let spans = vec![
                        Span::raw(format!("{} ", selected)),
                        Span::styled("@@", Style::default().fg(Color::LightCyan)),
                        Span::raw(format!("{} ({})", loc, count)),
                    ];
                    ListItem::new(Line::from(spans))
                })
                .collect();
            ("  Locations ".to_string(), items)
        }
    };

    let sidebar = List::new(sidebar_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(sidebar_title)
                .border_style(sidebar_style),
        )
        .highlight_style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .bg(Color::Blue),
        );
    f.render_stateful_widget(sidebar, h_chunks[0], &mut state.cal_state);

    // --- Task List Rendering ---
    let _list_inner_width = main_chunks[0].width.saturating_sub(2) as usize;

    let task_items: Vec<ListItem> = state
        .tasks
        .iter()
        .map(|t| {
            let is_blocked = state.store.is_blocked(t);
            let base_style = if is_blocked {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default()
            };
            let bracket_style = Style::default();

            let full_symbol = t.checkbox_symbol();
            let inner_char = full_symbol.trim_start_matches('[').trim_end_matches(']');

            let due_str = t
                .due
                .as_ref()
                .map(|d| format!(" @{}", d.format_smart()))
                .unwrap_or_default();
            let dur_str = t.format_duration_short();
            let recur_str = if t.rrule.is_some() { " (R)" } else { "" };

            let prefix_indent = Span::raw(if state.active_cal_href.is_some() {
                "  ".repeat(t.depth)
            } else {
                "".to_string()
            });
            let prefix_bracket_l = Span::styled("[", bracket_style);
            let prefix_inner = Span::styled(inner_char, base_style);
            let prefix_bracket_r = Span::styled("]", bracket_style);
            let prefix_blocked = Span::raw(if is_blocked { " [B] " } else { " " });

            // Build Title
            let mut spans = vec![
                prefix_indent,
                prefix_bracket_l,
                prefix_inner,
                prefix_bracket_r,
                prefix_blocked,
                Span::styled(t.summary.clone(), base_style),
            ];

            // 1. Metadata: Duration, Recurrence
            if !dur_str.is_empty() {
                spans.push(Span::styled(
                    format!(" {}", dur_str),
                    Style::default().fg(Color::DarkGray),
                ));
            }
            if !recur_str.is_empty() {
                spans.push(Span::styled(
                    recur_str.to_string(),
                    Style::default().fg(Color::Magenta),
                ));
            }

            // Alarm Indicator
            if t.alarms
                .iter()
                .any(|a| a.acknowledged.is_none() && !a.is_snooze())
            {
                spans.push(Span::raw(" "));
                spans.push(Span::styled(
                    "🔔",
                    Style::default()
                        .fg(Color::LightRed)
                        .add_modifier(Modifier::BOLD),
                ));
            }

            // Due Date
            if !due_str.is_empty() {
                if !spans
                    .last()
                    .map(|s| s.content.ends_with(' '))
                    .unwrap_or(true)
                {
                    spans.push(Span::raw(" "));
                }
                spans.push(Span::styled(due_str, Style::default().fg(Color::Blue)));
            }

            // 2. NEW: URL & Geo Indicators
            if t.geo.is_some() {
                spans.push(Span::raw(" "));
                spans.push(Span::styled(
                    "\u{ee69}",
                    Style::default().fg(Color::LightBlue),
                )); // Map Dot
            }
            if t.url.is_some() {
                spans.push(Span::raw(" "));
                spans.push(Span::styled(
                    "\u{f0789}",
                    Style::default().fg(Color::LightBlue),
                )); // Web Check
            }

            // 3. Location
            if let Some(loc) = &t.location {
                spans.push(Span::raw(" "));
                spans.push(Span::styled("@@", Style::default().fg(Color::Yellow)));
                spans.push(Span::styled(
                    loc.clone(),
                    Style::default().fg(Color::Yellow),
                ));
            }

            // 4. Tags
            for cat in &t.categories {
                let (r, g, b) = color_utils::generate_color(cat);
                spans.push(Span::raw(" "));
                spans.push(Span::styled(
                    format!("#{}", cat),
                    Style::default().fg(Color::Rgb(
                        (r * 255.0) as u8,
                        (g * 255.0) as u8,
                        (b * 255.0) as u8,
                    )),
                ));
            }

            ListItem::new(Line::from(spans))
        })
        .collect();

    let active_count = state.tasks.iter().filter(|t| !t.status.is_done()).count();

    let mut title = if state.loading {
        " Tasks (Loading...) ".to_string()
    } else {
        format!(" Tasks ({}) ", active_count)
    };
    if state.unsynced_changes {
        title.push_str(" [UNSYNCED] ");
    }
    if !state.active_search_query.is_empty() {
        title.push_str(&format!("[Search: '{}']", state.active_search_query));
    }

    let main_style = if state.active_focus == Focus::Main {
        Style::default().fg(Color::Yellow)
    } else if state.unsynced_changes {
        Style::default().fg(Color::LightRed)
    } else {
        Style::default()
    };

    let task_list = List::new(task_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(main_style),
        )
        .highlight_style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .bg(Color::Green)
                .fg(Color::Black),
        );
    f.render_stateful_widget(task_list, main_chunks[0], &mut state.list_state);

    // Details
    let details = Paragraph::new(full_details)
        .wrap(Wrap { trim: true })
        .block(Block::default().borders(Borders::ALL).title(" Details "));
    f.render_widget(details, main_chunks[1]);

    // Footer
    let footer_area = v_chunks[1];
    f.render_widget(Clear, footer_area);

    match state.mode {
        InputMode::Creating
        | InputMode::Editing
        | InputMode::Searching
        | InputMode::EditingDescription => {
            let (mut title_str, prefix, color) = match state.mode {
                InputMode::Searching => (" Search ".to_string(), "/ ", Color::Green),
                InputMode::Editing => (" Edit Title ".to_string(), "> ", Color::Magenta),
                InputMode::EditingDescription => {
                    (" Edit Description ".to_string(), "📝 ", Color::Blue)
                }
                InputMode::Creating => {
                    if state.creating_child_of.is_some() {
                        (" Create Child Task ".to_string(), "> ", Color::LightYellow)
                    } else {
                        (" Create Task ".to_string(), "> ", Color::Yellow)
                    }
                }
                _ => (" Create Task ".to_string(), "> ", Color::Yellow),
            };

            let show_tag_hint = (state.mode == InputMode::Searching
                && state.input_buffer.starts_with('#'))
                || (state.mode == InputMode::Creating
                    && state.input_buffer.starts_with('#')
                    && state.creating_child_of.is_none());

            if show_tag_hint {
                title_str.push_str(" [Enter to jump to tag] ");
            }

            let prefix_span = Span::styled(prefix, Style::default().fg(color));

            let mut input_spans = vec![prefix_span];

            if state.mode == InputMode::EditingDescription {
                input_spans.push(Span::raw(&state.input_buffer));
            } else {
                let tokens = tokenize_smart_input(&state.input_buffer);

                for token in tokens {
                    let text = &state.input_buffer[token.start..token.end];
                    let style = match token.kind {
                        SyntaxType::Priority => {
                            let p = text.trim_start_matches('!').parse::<u8>().unwrap_or(0);
                            match p {
                                1 => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                                2..=4 => Style::default().fg(Color::LightRed),
                                5 => Style::default().fg(Color::Yellow),
                                6..=8 => Style::default().fg(Color::LightBlue),
                                9 => Style::default().fg(Color::DarkGray),
                                _ => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                            }
                        }
                        SyntaxType::DueDate => Style::default().fg(Color::Blue),
                        SyntaxType::StartDate => Style::default().fg(Color::Green),
                        SyntaxType::Recurrence => Style::default().fg(Color::Magenta),
                        SyntaxType::Duration => Style::default().fg(Color::DarkGray),
                        SyntaxType::Tag => {
                            let tag_name = text.trim_start_matches('#');
                            let (r, g, b) = color_utils::generate_color(tag_name);
                            Style::default().fg(Color::Rgb(
                                (r * 255.0) as u8,
                                (g * 255.0) as u8,
                                (b * 255.0) as u8,
                            ))
                        }
                        SyntaxType::Text => Style::default().fg(color),
                        // New fields syntax highlight
                        SyntaxType::Location => Style::default().fg(Color::LightCyan),
                        SyntaxType::Url => Style::default().fg(Color::Blue),
                        SyntaxType::Geo => Style::default().fg(Color::DarkGray),
                        SyntaxType::Description => Style::default().fg(Color::Gray),
                        SyntaxType::Reminder => Style::default().fg(Color::LightRed),
                    };
                    input_spans.push(Span::styled(text, style));
                }
            }

            let input_text = Line::from(input_spans);

            let input = Paragraph::new(input_text)
                .style(Style::default())
                .block(Block::default().borders(Borders::ALL).title(title_str))
                .wrap(Wrap { trim: false });

            f.render_widget(input, footer_area);

            // Cursor rendering
            let cursor_x =
                footer_area.x + 1 + prefix.chars().count() as u16 + state.cursor_position as u16;
            f.set_cursor_position((
                cursor_x.min(footer_area.x + footer_area.width - 2),
                footer_area.y + 1,
            ));
        }
        _ => {
            if state.show_full_help {
                let p = Paragraph::new(full_help_text)
                    .block(Block::default().borders(Borders::ALL).title(" Help "))
                    .wrap(Wrap { trim: false });
                f.render_widget(p, footer_area);
            } else {
                let status = Paragraph::new(state.message.clone())
                    .style(Style::default().fg(Color::Cyan))
                    .block(
                        Block::default()
                            .borders(Borders::LEFT | Borders::TOP | Borders::BOTTOM)
                            .title(" Status "),
                    );
                let help_str = match state.active_focus {
                    Focus::Sidebar => "?:Help q:Quit Tab:Tasks ↵:Select Spc:Show/Hide *:All →:Iso",
                    Focus::Main => {
                        "?:Help q:Quit Tab:Side a:Add e:Edit E:Details Spc:Done d:Del /:Find"
                    }
                };
                let help = Paragraph::new(help_str).alignment(Alignment::Right).block(
                    Block::default()
                        .borders(Borders::RIGHT | Borders::TOP | Borders::BOTTOM)
                        .title(" Actions "),
                );

                let chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
                    .split(footer_area);
                f.render_widget(status, chunks[0]);
                f.render_widget(help, chunks[1]);
            }
        }
    }

    if state.mode == InputMode::Moving {
        let area = centered_rect(60, 50, f.area());
        let items: Vec<ListItem> = state
            .move_targets
            .iter()
            .map(|c| ListItem::new(c.name.as_str()))
            .collect();
        let popup = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(" Move Task "))
            .highlight_style(Style::default().bg(Color::Blue));
        f.render_widget(Clear, area);
        f.render_stateful_widget(popup, area, &mut state.move_selection_state);
    }

    // --- ALARM POPUP ---
    if let Some((task, _alarm_uid)) = &state.active_alarm {
        let area = centered_rect(60, 40, f.area());

        let block = Block::default()
            .title(" 🔔 REMINDER ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::LightRed))
            .style(Style::default().bg(Color::DarkGray));

        let mut lines = vec![
            Line::from(""),
            Line::from(vec![
                Span::raw("Task: "),
                Span::styled(
                    task.summary.clone(),
                    Style::default()
                        .add_modifier(Modifier::BOLD)
                        .fg(Color::White),
                ),
            ]),
            Line::from(""),
        ];

        if !task.description.is_empty() {
            lines.push(Line::from(Span::styled(
                task.description.clone(),
                Style::default().fg(Color::Gray),
            )));
            lines.push(Line::from(""));
        }

        lines.push(Line::from(vec![
            Span::styled(
                " [D] ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Dismiss    "),
            Span::styled(
                " [S] ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Snooze (10m)"),
        ]));

        let p = Paragraph::new(lines)
            .block(block)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });

        f.render_widget(Clear, area);
        f.render_widget(p, area);
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
