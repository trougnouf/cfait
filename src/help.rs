// File: ./src/help.rs
// SPDX-License-Identifier: GPL-3.0-or-later
//! Dynamic, localized help sections with progressive disclosure.

#[cfg_attr(feature = "mobile", derive(uniffi::Enum))]
#[derive(PartialEq, Eq, Hash, Clone, Copy, Debug, Default)]
pub enum HelpTab {
    #[default]
    Syntax,
    Shortcuts,
    About,
}

#[derive(Clone, Debug)]
pub struct HelpItem {
    pub keys: String,
    pub desc: String,
    pub example: String,
}

#[derive(Clone, Debug)]
pub struct HelpSection {
    pub title: String,
    pub items: Vec<HelpItem>,
}

pub fn get_syntax_help() -> Vec<HelpSection> {
    vec![
        HelpSection {
            title: "Quick start".to_string(),
            items: vec![
                HelpItem {
                    keys: "!1..9".to_string(),
                    desc: rust_i18n::t!("help_org_priority").to_string(),
                    example: "Buy cat food !1".to_string(),
                },
                HelpItem {
                    keys: "@date (or due:)".to_string(),
                    desc: rust_i18n::t!("help_timeline_due_date").to_string(),
                    example: "Meeting @tomorrow, due:2025-01-01".to_string(),
                },
                HelpItem {
                    keys: "^date (or start:)".to_string(),
                    desc: rust_i18n::t!("help_timeline_start_date").to_string(),
                    example: "^next week, start:tomorrow".to_string(),
                },
                HelpItem {
                    keys: "^@date".to_string(),
                    desc: rust_i18n::t!("help_timeline_set_both_dates").to_string(),
                    example: "^@tomorrow, ^@2d".to_string(),
                },
                HelpItem {
                    keys: "#tag".to_string(),
                    desc: rust_i18n::t!("help_org_add_category").to_string(),
                    example: "Plant tree #breathe #gardening:trees".to_string(),
                },
                HelpItem {
                    keys: "@@location (or loc:)".to_string(),
                    desc: rust_i18n::t!("help_org_location_hierarchy").to_string(),
                    example: "Buy cookies @@aldi".to_string(),
                },
                HelpItem {
                    keys: "~duration (or est:duration)".to_string(),
                    desc: rust_i18n::t!("help_org_estimated_duration").to_string(),
                    example: "Exercise ~30m, Read a book ~1h-2h".to_string(),
                },
            ],
        },
        HelpSection {
            title: rust_i18n::t!("timeline").to_string(),
            items: vec![
                HelpItem {
                    keys: "m, h, d, w, mo, y".to_string(),
                    desc: "Valid time units for duration or offsets".to_string(),
                    example: "15m, 2h, 3d, 1w, 6mo, 1y".to_string(),
                },
                HelpItem {
                    keys: "Offsets".to_string(),
                    desc: "Relative offset from today".to_string(),
                    example: "@1d, ^2w, @3mo, ...".to_string(),
                },
                HelpItem {
                    keys: "Weekdays".to_string(),
                    desc: rust_i18n::t!("help_timeline_weekdays").to_string(),
                    example: "@friday, @fri, @mon".to_string(),
                },
                HelpItem {
                    keys: "Dates".to_string(),
                    desc: "Absolute dates and months/years".to_string(),
                    example: "@2025-10-31, @2026-05, @2027".to_string(),
                },
            ],
        },
        HelpSection {
            title: rust_i18n::t!("recurrence").to_string(),
            items: vec![
                HelpItem {
                    keys: "@daily (or rec:)".to_string(),
                    desc: rust_i18n::t!("help_recurrence_quick_presets").to_string(),
                    example: "@daily, @weekly, @monthly, @yearly".to_string(),
                },
                HelpItem {
                    keys: "@every X".to_string(),
                    desc: rust_i18n::t!("help_recurrence_custom_intervals").to_string(),
                    example: "@every 3 days, @every 2 weeks, @every tuesday, @every sat,sun"
                        .to_string(),
                },
                HelpItem {
                    keys: "until <date>".to_string(),
                    desc: rust_i18n::t!("help_recurrence_until").to_string(),
                    example: "@daily until 2025-12-31".to_string(),
                },
                HelpItem {
                    keys: "except <...>".to_string(),
                    desc: rust_i18n::t!("help_recurrence_except_dates").to_string(),
                    example: "@daily except 2025-12-25,sat,sun,dec".to_string(),
                },
            ],
        },
        HelpSection {
            title: rust_i18n::t!("notifications_and_reminders").to_string(),
            items: vec![
                HelpItem {
                    keys: "rem:10m".to_string(),
                    desc: "Relative to due/start".to_string(),
                    example: "rem:15m, rem:2h".to_string(),
                },
                HelpItem {
                    keys: "rem:in 5m".to_string(),
                    desc: "Relative from *now* (becomes an absolute time)".to_string(),
                    example: "rem:in 2h".to_string(),
                },
                HelpItem {
                    keys: "rem:date".to_string(),
                    desc: rust_i18n::t!("help_metadata_absolute_reminder").to_string(),
                    example: "rem:2025-01-20 9am, rem:friday".to_string(),
                },
            ],
        },
        HelpSection {
            title: rust_i18n::t!("metadata").to_string(),
            items: vec![
                HelpItem {
                    keys: "url: or [[ ]]".to_string(),
                    desc: rust_i18n::t!("help_metadata_attach_link").to_string(),
                    example: "url:https://perdu.com or [[https://perdu.com]]".to_string(),
                },
                HelpItem {
                    keys: "geo:".to_string(),
                    desc: rust_i18n::t!("help_metadata_coordinates").to_string(),
                    example: "geo:50.1,4.2".to_string(),
                },
                HelpItem {
                    keys: "desc:".to_string(),
                    desc: rust_i18n::t!("help_metadata_append_description").to_string(),
                    example: "desc:\"Call back\" or desc:{Call back}".to_string(),
                },
                HelpItem {
                    keys: "+cal / -cal".to_string(),
                    desc: rust_i18n::t!("help_metadata_force_calendar").to_string(),
                    example: "Task @tomorrow +cal".to_string(),
                },
            ],
        },
        HelpSection {
            title: rust_i18n::t!("search_and_filtering").to_string(),
            items: vec![
                HelpItem {
                    keys: "text".to_string(),
                    desc: rust_i18n::t!("help_search_matches").to_string(),
                    example: "buy cat food".to_string(),
                },
                HelpItem {
                    keys: "is:ready".to_string(),
                    desc: rust_i18n::t!("help_search_is_ready").to_string(),
                    example: "is:ready".to_string(),
                },
                HelpItem {
                    keys: "is:status".to_string(),
                    desc: rust_i18n::t!("help_search_filter_state").to_string(),
                    example: "is:done, is:started, is:active".to_string(),
                },
                HelpItem {
                    keys: "< > <=".to_string(),
                    desc: rust_i18n::t!("help_search_operators").to_string(),
                    example: "~<20m, !<4".to_string(),
                },
                HelpItem {
                    keys: "Dates".to_string(),
                    desc: rust_i18n::t!("help_search_dates").to_string(),
                    example: "@<today (Overdue), ^>1w".to_string(),
                },
                HelpItem {
                    keys: "(A | B) -C".to_string(),
                    desc: rust_i18n::t!("help_search_combine").to_string(),
                    example: "is:ready (#work | #school) -@today".to_string(),
                },
            ],
        },
        HelpSection {
            title: rust_i18n::t!("organization").to_string(),
            items: vec![
                HelpItem {
                    keys: "#a:=#b,#c".to_string(),
                    desc: "Define tag alias (supports tags, loc, and priority)".to_string(),
                    example: "#gardening:=#home:outside,@@garden,!4".to_string(),
                },
                HelpItem {
                    keys: "@@a:=#b,@@c".to_string(),
                    desc: "Define location alias (inherits tags & priority too)".to_string(),
                    example: "@@aldi:=#groceries,@@shops:supermarkets".to_string(),
                },
                HelpItem {
                    keys: "\\#text".to_string(),
                    desc: rust_i18n::t!("help_org_escape_special").to_string(),
                    example: "\\#not-a-tag".to_string(),
                },
                HelpItem {
                    keys: "done:date".to_string(),
                    desc: rust_i18n::t!("help_org_done").to_string(),
                    example: "done:2024-01-01 15:30".to_string(),
                },
                HelpItem {
                    keys: "done:X%".to_string(),
                    desc: rust_i18n::t!("help_org_done_percent").to_string(),
                    example: "done:25%".to_string(),
                },
                HelpItem {
                    keys: "spent:".to_string(),
                    desc: "Log time already spent on a task".to_string(),
                    example: "spent:1h, spent:30m".to_string(),
                },
            ],
        },
        HelpSection {
            title: format!("Description & Sub-tasks (inside {})", '\u{f01c6}'),
            items: vec![
                HelpItem {
                    keys: "text".to_string(),
                    desc: "Unindented text is added to the main description".to_string(),
                    example: "Remember to feed the cat".to_string(),
                },
                HelpItem {
                    keys: "- [ ]".to_string(),
                    desc: "Create a sub-task (mention - [x] to mark as completed)".to_string(),
                    example: "- [ ] Buy cookies @tomorrow !1".to_string(),
                },
                HelpItem {
                    keys: "1. [ ]".to_string(),
                    desc: "Numbered dependency (2 depends on 1)".to_string(),
                    example: "2. [ ] Phase 2 (blocked by 1)".to_string(),
                },
                HelpItem {
                    keys: "  (indent)".to_string(),
                    desc: "Notes for the sub-task above".to_string(),
                    example: "  White chocolate chips cookies with marzipan".to_string(),
                },
                HelpItem {
                    keys: "Inheritance".to_string(),
                    desc: "Sub-tasks inherit tags, location, and priority from their parent"
                        .to_string(),
                    example: "".to_string(),
                },
            ],
        },
    ]
}

pub fn get_shortcuts_help() -> Vec<HelpSection> {
    vec![
        HelpSection {
            title: "Navigation & general".to_string(),
            items: vec![
                HelpItem {
                    keys: "?".to_string(),
                    desc: rust_i18n::t!("help_about").to_string(),
                    example: "".to_string(),
                },
                HelpItem {
                    keys: "Tab".to_string(),
                    desc: rust_i18n::t!("help_keyboard_switch_focus").to_string(),
                    example: "".to_string(),
                },
                HelpItem {
                    keys: "j / k / Dn / Up".to_string(),
                    desc: rust_i18n::t!("help_keyboard_move_selection").to_string(),
                    example: "".to_string(),
                },
                HelpItem {
                    keys: "PgDn / PgUp".to_string(),
                    desc: rust_i18n::t!("help_keyboard_scroll_page").to_string(),
                    example: "".to_string(),
                },
                HelpItem {
                    keys: "Ctrl + / - / 0".to_string(),
                    desc: rust_i18n::t!("help_keyboard_zoom_ui").to_string(),
                    example: rust_i18n::t!("help_keyboard_zoom_note").to_string(),
                },
            ],
        },
        HelpSection {
            title: rust_i18n::t!("edit").to_string(),
            items: vec![
                HelpItem {
                    keys: "a".to_string(),
                    desc: rust_i18n::t!("add").to_string(),
                    example: "".to_string(),
                },
                HelpItem {
                    keys: "e / E".to_string(),
                    desc: "Edit Title / Edit Description".to_string(),
                    example: "".to_string(),
                },
                HelpItem {
                    keys: "Ctrl + e".to_string(),
                    desc: rust_i18n::t!("help_keyboard_create_desc").to_string(),
                    example: "".to_string(),
                },
                HelpItem {
                    keys: "Space".to_string(),
                    desc: rust_i18n::t!("done").to_string(),
                    example: "".to_string(),
                },
                HelpItem {
                    keys: "s / S".to_string(),
                    desc: rust_i18n::t!("start_task").to_string(),
                    example: "".to_string(),
                },
                HelpItem {
                    keys: "x".to_string(),
                    desc: rust_i18n::t!("cancel").to_string(),
                    example: "".to_string(),
                },
                HelpItem {
                    keys: "Del".to_string(),
                    desc: rust_i18n::t!("delete").to_string(),
                    example: "".to_string(),
                },
                HelpItem {
                    keys: "Ctrl + Del".to_string(),
                    desc: rust_i18n::t!("delete_task_tree").to_string(),
                    example: "".to_string(),
                },
                HelpItem {
                    keys: "Ctrl + d".to_string(),
                    desc: rust_i18n::t!("duplicate_task").to_string(),
                    example: "".to_string(),
                },
                HelpItem {
                    keys: "+ / -".to_string(),
                    desc: rust_i18n::t!("increase_priority").to_string(),
                    example: "".to_string(),
                },
                HelpItem {
                    keys: "M".to_string(),
                    desc: rust_i18n::t!("move_label").to_string(),
                    example: "".to_string(),
                },
                HelpItem {
                    keys: "g".to_string(),
                    desc: format!(
                        "{}/ {}",
                        rust_i18n::t!("open_coordinates"),
                        rust_i18n::t!("action_open_locations")
                    ),
                    example: "".to_string(),
                },
                HelpItem {
                    keys: "o".to_string(),
                    desc: rust_i18n::t!("open_url").to_string(),
                    example: "".to_string(),
                },
            ],
        },
        HelpSection {
            title: rust_i18n::t!("metadata").to_string(),
            items: vec![
                HelpItem {
                    keys: "y".to_string(),
                    desc: rust_i18n::t!("yank_copy_id").to_string(),
                    example: "".to_string(),
                },
                HelpItem {
                    keys: "b / c / l".to_string(),
                    desc: "Block / Child / Relate to Yanked".to_string(),
                    example: "".to_string(),
                },
                HelpItem {
                    keys: "> / <".to_string(),
                    desc: "Indent / Outdent Task".to_string(),
                    example: "".to_string(),
                },
                HelpItem {
                    keys: "L".to_string(),
                    desc: rust_i18n::t!("help_metadata_jump_related").to_string(),
                    example: "".to_string(),
                },
            ],
        },
        HelpSection {
            title: rust_i18n::t!("search_and_filtering").to_string(),
            items: vec![
                HelpItem {
                    keys: "/".to_string(),
                    desc: rust_i18n::t!("search").to_string(),
                    example: "".to_string(),
                },
                HelpItem {
                    keys: "1, 2, 3".to_string(),
                    desc: rust_i18n::t!("support_switch_sidebar_tab").to_string(),
                    example: "".to_string(),
                },
                HelpItem {
                    keys: "m".to_string(),
                    desc: rust_i18n::t!("toggle_matching_logic").to_string(),
                    example: "".to_string(),
                },
                HelpItem {
                    keys: "H".to_string(),
                    desc: rust_i18n::t!("hide_completed_and_canceled_tasks").to_string(),
                    example: "".to_string(),
                },
                HelpItem {
                    keys: "*".to_string(),
                    desc: rust_i18n::t!("support_clear_filters").to_string(),
                    example: "".to_string(),
                },
                HelpItem {
                    keys: "Right".to_string(),
                    desc: rust_i18n::t!("support_isolate_calendar").to_string(),
                    example: "".to_string(),
                },
            ],
        },
    ]
}
