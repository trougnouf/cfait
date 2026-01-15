// Renders the Terminal User Interface (TUI) layout and widgets.
use crate::color_utils;
use crate::model::parser::{SyntaxType, strip_quotes, tokenize_smart_input};
use crate::store::UNCATEGORIZED_ID;
use crate::tui::action::SidebarMode;
use crate::tui::state::{AppState, Focus, InputMode};

use chrono::Utc;

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};
use std::collections::HashSet;

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
            Span::raw(
                " a:Add  e:Edit Title  E:Edit Desc  d:Delete  Space:Toggle Done  L:Jump to Related",
            ),
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
            Span::raw(" +/-:Priority  </>:Indent  y:Yank { b:Block  c:Child  l:Link }  C:NewChild"),
        ]),
        Line::from(vec![
            Span::styled("              ", Style::default()), // Indent alignment
            Span::raw(
                "#alias:=#tag,@@loc (Define alias inline, retroactive)  +cal/-cal (Force/prevent calendar event)",
            ),
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
            Span::styled("               ", Style::default()), // Indent alignment
            Span::raw(
                "is:ready (Work Mode: actionable tasks)  @<today (Overdue)  ^>1w (Start 1+ weeks)",
            ),
        ]),
        Line::from(vec![
            Span::styled("               ", Style::default()), // Indent alignment
            Span::raw("@<today! (Overdue OR no due date)  is:ready #work ~<1h (Combine filters)"),
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
    // We'll build this after rendering the task list so we know if the title was truncated
    let mut full_details = String::new();
    let mut selected_task_was_truncated = false;

    // --- 2. Dynamic Layout (before rendering tasks so we know the width) ---
    // Use a default height for now, will be updated after task rendering
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),     // Task list takes remaining space
            Constraint::Length(10), // Initial placeholder height
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
    let list_inner_width = main_chunks[0].width.saturating_sub(2) as usize;

    let task_items: Vec<ListItem> = state
        .tasks
        .iter()
        .enumerate()
        .map(|(idx, t)| {
            // Determine parent attributes to hide redundancy (tags and location)
            let (parent_tags, parent_location) = if state.active_cal_href.is_some()
                && let Some(p_uid) = &t.parent_uid
            {
                let mut p_tags = HashSet::new();
                let mut p_loc = None;
                if let Some(href) = state.store.index.get(p_uid)
                    && let Some(list) = state.store.calendars.get(href)
                    && let Some(p) = list.iter().find(|pt| pt.uid == *p_uid)
                {
                    p_tags = p.categories.iter().cloned().collect();
                    p_loc = p.location.clone();
                }
                (p_tags, p_loc)
            } else {
                (HashSet::new(), None)
            };

            // --- ALIAS SHADOWING LOGIC START ---
            let mut tags_to_hide = parent_tags.clone();
            let mut loc_to_hide = parent_location.clone();

            let mut process_expansions = |targets: &Vec<String>| {
                for target in targets {
                    if let Some(val) = target.strip_prefix('#') {
                        tags_to_hide.insert(strip_quotes(val));
                    } else if let Some(val) = target.strip_prefix("@@") {
                        loc_to_hide = Some(strip_quotes(val));
                    } else if target.to_lowercase().starts_with("loc:") {
                        loc_to_hide = Some(strip_quotes(&target[4..]));
                    }
                }
            };

            // Check Categories
            for cat in &t.categories {
                if let Some(targets) = state.tag_aliases.get(cat) {
                    process_expansions(targets);
                }
                // Check hierarchy
                let mut search = cat.as_str();
                while let Some(idx) = search.rfind(':') {
                    search = &search[..idx];
                    if let Some(targets) = state.tag_aliases.get(search) {
                        process_expansions(targets);
                    }
                }
            }

            // Check Location
            if let Some(loc) = &t.location {
                let key = format!("@@{}", loc);
                if let Some(targets) = state.tag_aliases.get(&key) {
                    process_expansions(targets);
                }
                // Check hierarchy
                let mut search = key.as_str();
                while let Some(idx) = search.rfind(':') {
                    if idx < 2 {
                        break;
                    } // Don't split @@
                    search = &search[..idx];
                    if let Some(targets) = state.tag_aliases.get(search) {
                        process_expansions(targets);
                    }
                }
            }
            // --- ALIAS SHADOWING LOGIC END ---

            let is_blocked = state.store.is_blocked(t);
            let base_style = if is_blocked {
                Style::default().fg(Color::DarkGray)
            } else {
                // Priority Gradient: Red (Hot) -> Yellow (Normal) -> Purple/Slate (Cold)
                match t.priority {
                    // 1: Critical -> Red
                    1 => Style::default().fg(Color::Red),
                    // 2: Urgent -> Orange-Red
                    2 => Style::default().fg(Color::Rgb(255, 69, 0)),
                    // 3: High -> Dark Orange
                    3 => Style::default().fg(Color::Rgb(255, 140, 0)),
                    // 4: Med-High -> Amber/Gold
                    4 => Style::default().fg(Color::Rgb(255, 190, 0)),
                    // 5: Normal -> Yellow
                    5 => Style::default().fg(Color::Yellow),
                    // 6: Med-Low -> Pale Goldenrod / Khaki (Desaturating)
                    6 => Style::default().fg(Color::Rgb(238, 232, 170)),
                    // 7: Low -> Light Slate Gray
                    7 => Style::default().fg(Color::Rgb(176, 196, 222)),
                    // 8: Very Low -> Slate Gray
                    8 => Style::default().fg(Color::Rgb(112, 128, 144)),
                    // 9: Minimal -> Dark Slate Gray
                    9 => Style::default().fg(Color::Rgb(47, 79, 79)),
                    // 0 or unset: Default (no color modification)
                    _ => Style::default(),
                }
            };
            let bracket_style = Style::default();

            let full_symbol = t.checkbox_symbol();
            let inner_char = full_symbol.trim_start_matches('[').trim_end_matches(']');

            // Check for Future Start Date
            let now = Utc::now();
            let is_future_start = t
                .dtstart
                .as_ref()
                .map(|start| start.to_start_comparison_time() > now)
                .unwrap_or(false);

            // Construct Date String
            let (date_display_str, date_style) = if is_future_start {
                let start_str = t.dtstart.as_ref().unwrap().format_smart();

                if let Some(due) = &t.due {
                    let due_str = due.format_smart();
                    if start_str == due_str {
                        // Case 2: Start == Due (Future)
                        (
                            format!(" ►{}⌛", start_str),
                            Style::default().fg(Color::DarkGray),
                        )
                    } else {
                        // Case 1: Start != Due (Future)
                        (
                            format!(" ►{}-{}⌛", start_str, due_str),
                            Style::default().fg(Color::DarkGray),
                        )
                    }
                } else {
                    // Case 4: Start Only (Future)
                    (
                        format!(" ►{}", start_str),
                        Style::default().fg(Color::DarkGray),
                    )
                }
            } else if let Some(d) = &t.due {
                // Case 3: Due Only (or started)
                (
                    format!(" @{}⌛", d.format_smart()),
                    Style::default().fg(Color::Blue),
                )
            } else {
                (String::new(), Style::default())
            };
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

            // Calculate prefix width
            let prefix_width = if state.active_cal_href.is_some() {
                t.depth * 2 + 6 // indent + "[ ] " + " " or " [B] "
            } else {
                6 // "[ ] " + " " or " [B] "
            };

            // Build metadata spans (without title)
            let mut metadata_spans = Vec::new();

            // 1. Metadata: Duration, Recurrence
            if !dur_str.is_empty() {
                metadata_spans.push(Span::styled(
                    format!(" {}", dur_str),
                    Style::default().fg(Color::DarkGray),
                ));
            }
            if !recur_str.is_empty() {
                metadata_spans.push(Span::styled(
                    recur_str.to_string(),
                    Style::default().fg(Color::Magenta),
                ));
            }

            // Alarm Indicator
            if t.alarms
                .iter()
                .any(|a| a.acknowledged.is_none() && !a.is_snooze())
            {
                metadata_spans.push(Span::raw(" "));
                metadata_spans.push(Span::styled(
                    "🔔",
                    Style::default()
                        .fg(Color::LightRed)
                        .add_modifier(Modifier::BOLD),
                ));
            }

            // Date Display (Start or Due)
            if !date_display_str.is_empty() {
                if !metadata_spans
                    .last()
                    .map(|s| s.content.ends_with(' '))
                    .unwrap_or(true)
                {
                    metadata_spans.push(Span::raw(" "));
                }
                metadata_spans.push(Span::styled(date_display_str, date_style));
            }

            // 2. URL & Geo Indicators
            if t.geo.is_some() {
                metadata_spans.push(Span::raw(" "));
                metadata_spans.push(Span::styled(
                    "\u{ee69}",
                    Style::default().fg(Color::LightBlue),
                )); // Map Dot
            }
            if t.url.is_some() {
                metadata_spans.push(Span::raw(" "));
                metadata_spans.push(Span::styled(
                    "\u{f0789}",
                    Style::default().fg(Color::LightBlue),
                )); // Web Check
            }

            // Build right side spans (tags and location)
            let mut right_spans = Vec::new();

            // 3. Location (Hide if shadowed by alias)
            if let Some(loc) = &t.location
                && loc_to_hide.as_ref() != Some(loc)
            {
                right_spans.push(Span::styled("@@", Style::default().fg(Color::Yellow)));
                right_spans.push(Span::styled(
                    loc.clone(),
                    Style::default().fg(Color::Yellow),
                ));
            }

            // 4. Tags (Hide if shadowed by alias)
            for cat in &t.categories {
                if !tags_to_hide.contains(cat) {
                    let (r, g, b) = color_utils::generate_color(cat);
                    if !right_spans.is_empty() {
                        right_spans.push(Span::raw(" "));
                    }
                    right_spans.push(Span::styled(
                        format!("#{}", cat),
                        Style::default().fg(Color::Rgb(
                            (r * 255.0) as u8,
                            (g * 255.0) as u8,
                            (b * 255.0) as u8,
                        )),
                    ));
                }
            }

            // Calculate widths
            let metadata_width: usize = metadata_spans
                .iter()
                .map(|s| s.content.chars().count())
                .sum();
            let right_width: usize = right_spans.iter().map(|s| s.content.chars().count()).sum();

            // Calculate available width for title
            let reserved_width = prefix_width + metadata_width + right_width;
            let available_for_title = if reserved_width + 10 < list_inner_width {
                list_inner_width
                    .saturating_sub(reserved_width)
                    .saturating_sub(1)
            } else {
                30 // minimum
            };

            // Truncate title if necessary
            let title_chars: Vec<char> = t.summary.chars().collect();
            let (display_title, is_truncated) = if title_chars.len() > available_for_title {
                let mut truncated = title_chars
                    .iter()
                    .take(available_for_title.saturating_sub(3))
                    .collect::<String>();
                truncated.push_str("...");
                (truncated, true)
            } else {
                (t.summary.clone(), false)
            };

            // Track if the selected task was truncated
            if is_truncated && Some(idx) == state.list_state.selected() {
                selected_task_was_truncated = true;
            }

            // Build final spans
            let mut spans = vec![
                prefix_indent,
                prefix_bracket_l,
                prefix_inner,
                prefix_bracket_r,
                prefix_blocked,
                Span::styled(display_title, base_style),
            ];
            spans.extend(metadata_spans);

            // Add padding and right-aligned content
            if !right_spans.is_empty() {
                let left_width: usize = spans.iter().map(|s| s.content.chars().count()).sum();
                let total_content = left_width + right_width;

                if total_content < list_inner_width {
                    let padding = list_inner_width - total_content;
                    spans.push(Span::raw(" ".repeat(padding)));
                } else {
                    spans.push(Span::raw(" "));
                }

                spans.extend(right_spans);
            }

            ListItem::new(Line::from(spans))
        })
        .collect();

    // Now build the details with truncation information
    if let Some(task) = state.get_selected_task() {
        // Only show title if it was truncated in the list
        if selected_task_was_truncated && !task.summary.is_empty() {
            full_details.push_str(&task.summary);
            full_details.push_str("\n\n");
        }

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

        // Outgoing relations (this task → others)
        if !task.related_to.is_empty() {
            full_details.push_str("[Related To]:\n");
            for related_uid in &task.related_to {
                let name = state
                    .store
                    .get_summary(related_uid)
                    .unwrap_or_else(|| "Unknown task".to_string());
                full_details.push_str(&format!(" → {}\n", name));
            }
        }

        // Incoming relations (others → this task)
        let incoming_related = state.store.get_tasks_related_to(&task.uid);
        if !incoming_related.is_empty() {
            full_details.push_str("[Related From]:\n");
            for (_related_uid, related_name) in incoming_related {
                full_details.push_str(&format!(" ← {}\n", related_name));
            }
        }
    }
    if full_details.is_empty() {
        full_details = "No details.".to_string();
    }

    let active_count = state.tasks.iter().filter(|t| !t.status.is_done()).count();

    // --- Calculate Dynamic Height for Details ---
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

    // Re-calculate layout with correct details height
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),                       // Task list takes remaining space
            Constraint::Length(final_details_height), // Details takes calculated height
        ])
        .split(h_chunks[1]);

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

            // 1. Calculate available width for the input text
            // Width - 2 (borders) - prefix width - 1 (cursor spacing/padding)
            let inner_width = footer_area.width.saturating_sub(2) as usize;
            let prefix_width = prefix.chars().count();
            let input_area_width = inner_width.saturating_sub(prefix_width).saturating_sub(1);

            // 2. Determine Horizontal Scroll Offset
            // If editing description, we allow wrapping (multiline).
            // For single-line inputs, we scroll.
            let (visible_text, scroll_offset) = if state.mode == InputMode::EditingDescription {
                (state.input_buffer.clone(), 0)
            } else {
                let cursor = state.cursor_position;
                if cursor >= input_area_width {
                    // Shift the view so the cursor is at the end
                    let offset = cursor - input_area_width + 1;
                    let slice: String = state
                        .input_buffer
                        .chars()
                        .skip(offset)
                        .take(input_area_width)
                        .collect();
                    (slice, offset)
                } else {
                    // Start from 0, possibly truncate end if too long (though cursor is within bounds)
                    let slice: String = state.input_buffer.chars().take(input_area_width).collect();
                    (slice, 0)
                }
            };

            let mut input_spans = vec![prefix_span];

            if state.mode == InputMode::EditingDescription {
                input_spans.push(Span::raw(&visible_text));
            } else {
                // Tokenize the *visible* slice.
                // Note: Truncated tokens might lose coloring, which is acceptable behavior during scrolling.
                let tokens = tokenize_smart_input(&visible_text);

                for token in tokens {
                    let text = &visible_text[token.start..token.end];
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
                .wrap(Wrap { trim: false }); // Wrap is fine for Description, ignored for others due to slicing

            f.render_widget(input, footer_area);

            // 3. Render Cursor relative to Scroll Offset
            if state.mode == InputMode::EditingDescription {
                // Multiline cursor logic (simplified: ratatui handles multiline text well,
                // but exact cursor placement on wrapped lines requires more complex logic.
                // For now, we leave the existing simple logic or improve it if needed.
                // The prompt specifically asked about the "long name" bug which is single line).
                // Keep existing logic for Description for now:
                let term_width = footer_area.width.saturating_sub(2) as usize;
                if term_width > 0 {
                    let x = (state.cursor_position % term_width) as u16;
                    let y = (state.cursor_position / term_width) as u16;
                    f.set_cursor_position((
                        footer_area.x + 1 + prefix_width as u16 + x,
                        footer_area.y + 1 + y,
                    ));
                }
            } else {
                // Single line sliding window cursor
                let visual_cursor_offset = state.cursor_position.saturating_sub(scroll_offset);

                let cursor_x = footer_area.x
                    + 1 // Border
                    + prefix_width as u16
                    + visual_cursor_offset as u16;

                f.set_cursor_position((
                    cursor_x.min(footer_area.x + footer_area.width - 2),
                    footer_area.y + 1,
                ));
            }
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
                        if state.yanked_uid.is_some() {
                            "YANK ACTIVE: b:Block c:Child l:Link (Esc:Clear)"
                        } else {
                            "?:Help q:Quit Tab:Side a:Add e:Edit E:Details Spc:Done d:Del y:Yank /:Find"
                        }
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

    if state.mode == InputMode::SelectingExportSource {
        let area = centered_rect(60, 50, f.area());
        let items: Vec<ListItem> = state
            .export_source_calendars
            .iter()
            .map(|c| ListItem::new(c.name.as_str()))
            .collect();
        let popup = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Export: Select Source Local Calendar "),
            )
            .highlight_style(Style::default().bg(Color::Blue));
        f.render_widget(Clear, area);
        f.render_stateful_widget(popup, area, &mut state.export_source_selection_state);
    }

    if state.mode == InputMode::Exporting {
        let area = centered_rect(60, 50, f.area());
        let items: Vec<ListItem> = state
            .export_targets
            .iter()
            .map(|c| ListItem::new(c.name.as_str()))
            .collect();

        let title = if let Some(idx) = state.export_source_selection_state.selected() {
            if let Some(source) = state.export_source_calendars.get(idx) {
                format!(" Export '{}' To ", source.name)
            } else {
                " Export: Select Destination ".to_string()
            }
        } else {
            " Export: Select Destination ".to_string()
        };

        let popup = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(title))
            .highlight_style(Style::default().bg(Color::Blue));
        f.render_widget(Clear, area);
        f.render_stateful_widget(popup, area, &mut state.export_selection_state);
    }

    // --- RELATIONSHIP BROWSING POPUP ---
    if state.mode == InputMode::RelationshipBrowsing {
        let area = centered_rect(70, 60, f.area());
        let items: Vec<ListItem> = state
            .relationship_items
            .iter()
            .map(|(_, display_name)| ListItem::new(display_name.as_str()))
            .collect();
        let popup = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Jump to Related Task "),
            )
            .highlight_style(Style::default().bg(Color::Blue).fg(Color::White));
        f.render_widget(Clear, area);
        f.render_stateful_widget(popup, area, &mut state.relationship_selection_state);
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

        // Format snooze presets
        let short_label = crate::model::parser::format_duration_compact(state.snooze_short_mins);
        let long_label = crate::model::parser::format_duration_compact(state.snooze_long_mins);

        if state.mode == InputMode::Snoozing {
            // Show custom snooze input
            lines.push(Line::from(vec![
                Span::styled(
                    " Custom snooze: ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(&state.input_buffer),
                Span::styled(" █", Style::default().fg(Color::Yellow)),
            ]));
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::raw("Enter duration (e.g., 30m, 2h, 1d) or "),
                Span::styled("[Esc]", Style::default().fg(Color::Yellow)),
                Span::raw(" to cancel"),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::styled(
                    " [d] ",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("Dismiss    "),
                Span::styled(
                    " [1] ",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(format!("Snooze {}    ", short_label)),
                Span::styled(
                    " [2] ",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(format!("Snooze {}    ", long_label)),
                Span::styled(
                    " [s] ",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("Custom"),
            ]));
        }

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
