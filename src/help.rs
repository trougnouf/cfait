// File: ./src/help.rs
//! Dynamic, localized help sections.
//!
//! This replaces the previous static `&'static str` tables with runtime-generated
//! `String` values so that the help content reflects the active locale (via
//! `rust_i18n::t!()`).
//!
//! Consumers should call `get_syntax_help()` and `get_keyboard_help()` to obtain
//! the current localized content. Callers that previously referenced the
//! `SYNTAX_HELP` / `KEYBOARD_HELP` statics must be updated to use these
//! functions.

#[cfg_attr(feature = "mobile", derive(uniffi::Enum))]
#[derive(PartialEq, Clone, Copy, Debug, Default)]
pub enum HelpTab {
    #[default]
    Syntax,
    Keyboard,
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

/// Returns localized syntax-oriented help sections.
///
/// This function uses `rust_i18n::t!()` for all translatable strings so the
/// result reflects the currently active locale.
pub fn get_syntax_help() -> Vec<HelpSection> {
    vec![
        HelpSection {
            title: rust_i18n::t!("organization").to_string(),
            items: vec![
                HelpItem {
                    keys: "!1..9".to_string(),
                    desc: rust_i18n::t!("help_org_priority").to_string(),
                    example: "!1, !5, !9".to_string(),
                },
                HelpItem {
                    keys: "#tag".to_string(),
                    desc: rust_i18n::t!("help_org_add_category").to_string(),
                    example: "#work, #dev:backend".to_string(),
                },
                HelpItem {
                    keys: "@@loc".to_string(),
                    desc: rust_i18n::t!("help_org_location_hierarchy").to_string(),
                    example: "@@home, @@store:aldi".to_string(),
                },
                HelpItem {
                    keys: "~duration".to_string(),
                    desc: rust_i18n::t!("help_org_estimated_duration").to_string(),
                    example: "~30m, ~1.5h, ~15m-45m".to_string(),
                },
                HelpItem {
                    keys: "spent:X".to_string(),
                    desc: rust_i18n::t!("help_org_spent").to_string(), // Fixed mapping!
                    example: "spent:1h, spent:30m".to_string(),
                },
                HelpItem {
                    keys: "done:date".to_string(),
                    desc: rust_i18n::t!("help_org_done").to_string(), // Fixed mapping!
                    example: "done:2024-01-01 15:30".to_string(),
                },
                HelpItem {
                    keys: "#a:=#b".to_string(),
                    desc: rust_i18n::t!("help_org_define_alias").to_string(),
                    example: "#tree:=#gardening,@@home".to_string(),
                },
                HelpItem {
                    keys: "@@a:=#b".to_string(),
                    desc: rust_i18n::t!("help_org_location_alias").to_string(),
                    example: "@@aldi:=#groceries".to_string(),
                },
                HelpItem {
                    keys: "\\#text".to_string(),
                    desc: rust_i18n::t!("help_org_escape_special").to_string(),
                    example: "\\#not-a-tag".to_string(),
                },
            ],
        },
        HelpSection {
            title: rust_i18n::t!("timeline").to_string(),
            items: vec![
                HelpItem {
                    keys: "@date".to_string(),
                    desc: rust_i18n::t!("help_timeline_due_date").to_string(),
                    example: "@tomorrow, @2025-12-31".to_string(),
                },
                HelpItem {
                    keys: "^date".to_string(),
                    desc: rust_i18n::t!("help_timeline_start_date").to_string(),
                    example: "^next week, ^2025-01-01".to_string(),
                },
                HelpItem {
                    keys: "Offsets".to_string(),
                    desc: rust_i18n::t!("help_timeline_offsets").to_string(),
                    example: "1d, 2w, 3mo".to_string(),
                },
                HelpItem {
                    keys: "Weekdays".to_string(),
                    desc: rust_i18n::t!("help_timeline_weekdays").to_string(),
                    example: "@friday, @next monday".to_string(),
                },
                HelpItem {
                    keys: "Next period".to_string(),
                    desc: rust_i18n::t!("help_timeline_next_period").to_string(),
                    example: "@next week, @next month".to_string(),
                },
                HelpItem {
                    keys: "Keywords".to_string(),
                    desc: rust_i18n::t!("help_timeline_keywords").to_string(),
                    example: "today, tomorrow".to_string(),
                },
                HelpItem {
                    keys: "^@date".to_string(),
                    desc: rust_i18n::t!("help_timeline_set_both_dates").to_string(),
                    example: "^@tomorrow, ^@2d".to_string(),
                },
            ],
        },
        HelpSection {
            title: rust_i18n::t!("recurrence").to_string(),
            items: vec![
                HelpItem {
                    keys: "@daily".to_string(),
                    desc: rust_i18n::t!("help_recurrence_quick_presets").to_string(),
                    example: "@daily, @weekly, @monthly, @yearly".to_string(),
                },
                HelpItem {
                    keys: "@every X".to_string(),
                    desc: rust_i18n::t!("help_recurrence_custom_intervals").to_string(),
                    example: "@every 3 days, @every 2 weeks".to_string(),
                },
                HelpItem {
                    keys: "@every <day>".to_string(),
                    desc: rust_i18n::t!("help_recurrence_specific_weekdays").to_string(),
                    example: "@every monday,wednesday".to_string(),
                },
                HelpItem {
                    keys: "until <date>".to_string(),
                    desc: rust_i18n::t!("help_recurrence_until").to_string(),
                    example: "@daily until 2025-12-31".to_string(),
                },
                HelpItem {
                    keys: "except <date>".to_string(),
                    desc: rust_i18n::t!("help_recurrence_except_dates").to_string(),
                    example: "@daily except 2025-12-25".to_string(),
                },
                HelpItem {
                    keys: "except day".to_string(),
                    desc: rust_i18n::t!("help_recurrence_except_day").to_string(),
                    example: "except mo,tue".to_string(),
                },
                HelpItem {
                    keys: "except month".to_string(),
                    desc: rust_i18n::t!("help_recurrence_except_month").to_string(),
                    example: "except oct,nov".to_string(),
                },
            ],
        },
        HelpSection {
            title: rust_i18n::t!("metadata").to_string(),
            items: vec![
                HelpItem {
                    keys: "url:".to_string(),
                    desc: rust_i18n::t!("help_metadata_attach_link").to_string(),
                    example: "url:https://perdu.com".to_string(),
                },
                HelpItem {
                    keys: "geo:".to_string(),
                    desc: rust_i18n::t!("help_metadata_coordinates").to_string(),
                    example: "geo:53.04,-121.10".to_string(),
                },
                HelpItem {
                    keys: "desc:".to_string(),
                    desc: rust_i18n::t!("help_metadata_append_description").to_string(),
                    example: "desc:\"Call back later\"".to_string(),
                },
                HelpItem {
                    keys: "rem:10m".to_string(),
                    desc: rust_i18n::t!("help_metadata_relative_reminder").to_string(),
                    example: rust_i18n::t!("help_metadata_adjusts_if_due_changes").to_string(),
                },
                HelpItem {
                    keys: "rem:in 5m".to_string(),
                    desc: rust_i18n::t!("help_metadata_relative_from_now").to_string(),
                    example: "rem:in 2h".to_string(),
                },
                HelpItem {
                    keys: "rem:date".to_string(),
                    desc: rust_i18n::t!("help_metadata_absolute_reminder").to_string(),
                    example: "rem:2025-01-20 9am".to_string(),
                },
                HelpItem {
                    keys: "+cal".to_string(),
                    desc: rust_i18n::t!("help_metadata_force_calendar").to_string(),
                    example: "Task @tomorrow +cal".to_string(),
                },
                HelpItem {
                    keys: "-cal".to_string(),
                    desc: rust_i18n::t!("help_metadata_prevent_calendar").to_string(),
                    example: "Task @tomorrow -cal".to_string(),
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
                    keys: "#tag".to_string(),
                    desc: rust_i18n::t!("help_search_filter_tag").to_string(),
                    example: "#gardening".to_string(),
                },
                HelpItem {
                    keys: "@@loc".to_string(),
                    desc: rust_i18n::t!("help_search_location").to_string(),
                    example: "@@home".to_string(),
                },
                HelpItem {
                    keys: "is:ready".to_string(),
                    desc: rust_i18n::t!("help_search_is_ready").to_string(),
                    example: rust_i18n::t!("help_search_is_ready_explain").to_string(),
                },
                HelpItem {
                    keys: "is:status".to_string(),
                    desc: rust_i18n::t!("help_search_filter_state").to_string(),
                    example: "is:done, is:started, is:active, is:blocked".to_string(),
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
                    keys: "Date!".to_string(),
                    desc: rust_i18n::t!("help_search_date_exclaim").to_string(),
                    example: "@<today!".to_string(),
                },
                HelpItem {
                    keys: "(A | B) -C".to_string(),
                    desc: rust_i18n::t!("help_search_combine").to_string(),
                    example: "(#work | #school) -is:done".to_string(),
                },
            ],
        },
    ]
}

/// Returns localized keyboard help sections.
///
/// Many of the keyboard descriptions are short action names; where a translation
/// key exists we use it, otherwise we fall back to concise translation keys so
/// the text can be localized.
pub fn get_keyboard_help() -> Vec<HelpSection> {
    vec![
        HelpSection {
            title: rust_i18n::t!("keyboard_shortcuts").to_string(),
            items: vec![
                HelpItem {
                    keys: "?".to_string(),
                    // prefer the "about" label already in translations
                    desc: rust_i18n::t!("help_about").to_string(),
                    example: "".to_string(),
                },
                HelpItem {
                    keys: "q".to_string(),
                    desc: rust_i18n::t!("quit_application").to_string(),
                    example: "".to_string(),
                },
                HelpItem {
                    keys: "Tab".to_string(),
                    desc: rust_i18n::t!("toggle_matching_logic").to_string(), // best-effort mapping
                    example: rust_i18n::t!("help_keyboard_switch_focus").to_string(),
                },
                HelpItem {
                    keys: "j / k".to_string(),
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
                    keys: "e".to_string(),
                    desc: rust_i18n::t!("edit").to_string(),
                    example: "".to_string(),
                },
                HelpItem {
                    keys: "E".to_string(),
                    desc: rust_i18n::t!("edit_task_title").to_string(),
                    example: "".to_string(),
                },
                HelpItem {
                    keys: "Space".to_string(),
                    desc: rust_i18n::t!("done").to_string(),
                    example: "".to_string(),
                },
                HelpItem {
                    keys: "Delete".to_string(),
                    desc: rust_i18n::t!("delete").to_string(),
                    example: "".to_string(),
                },
                HelpItem {
                    keys: "s".to_string(),
                    desc: rust_i18n::t!("start_task").to_string(),
                    example: "".to_string(),
                },
                HelpItem {
                    keys: "S".to_string(),
                    desc: rust_i18n::t!("stop_reset").to_string(),
                    example: "".to_string(),
                },
                HelpItem {
                    keys: "x".to_string(),
                    desc: rust_i18n::t!("cancel").to_string(),
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
                    keys: "y".to_string(),
                    desc: rust_i18n::t!("yank_link").to_string(),
                    example: "".to_string(),
                },
            ],
        },
        HelpSection {
            title: rust_i18n::t!("metadata").to_string(),
            items: vec![
                HelpItem {
                    keys: "c".to_string(),
                    desc: rust_i18n::t!("help_keyboard_link_yanked_child").to_string(),
                    example: "".to_string(),
                },
                HelpItem {
                    keys: "C".to_string(),
                    desc: rust_i18n::t!("create_subtask").to_string(),
                    example: "".to_string(),
                },
                HelpItem {
                    keys: "b".to_string(),
                    desc: rust_i18n::t!("help_keyboard_mark_blocked_by_yanked").to_string(),
                    example: "".to_string(),
                },
                HelpItem {
                    keys: "l".to_string(),
                    desc: rust_i18n::t!("help_keyboard_relate_to_yanked").to_string(),
                    example: "".to_string(),
                },
                HelpItem {
                    keys: "> / .".to_string(),
                    desc: rust_i18n::t!("make_child").to_string(),
                    example: "".to_string(),
                },
                HelpItem {
                    keys: "< / ,".to_string(),
                    desc: rust_i18n::t!("promote_remove_parent").to_string(),
                    example: "".to_string(),
                },
                HelpItem {
                    keys: "L".to_string(),
                    desc: rust_i18n::t!("help_keyboard_jump_related_tasks").to_string(),
                    example: "".to_string(),
                },
                HelpItem {
                    keys: "Enter".to_string(),
                    desc: rust_i18n::t!("help_keyboard_toggle_subtasks_expansion").to_string(),
                    example: "".to_string(),
                },
            ],
        },
        HelpSection {
            title: rust_i18n::t!("support_card_title").to_string(),
            items: vec![
                HelpItem {
                    keys: "/".to_string(),
                    desc: rust_i18n::t!("search").to_string(),
                    example: "".to_string(),
                },
                HelpItem {
                    keys: "1, 2, 3".to_string(),
                    desc: rust_i18n::t!("menu_move").to_string(),
                    example: rust_i18n::t!("help_keyboard_switch_sidebar_tab").to_string(),
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
                    desc: rust_i18n::t!("help_keyboard_clear_filters").to_string(),
                    example: "".to_string(),
                },
                HelpItem {
                    keys: "Right".to_string(),
                    desc: rust_i18n::t!("help_keyboard_isolate_calendar").to_string(),
                    example: "".to_string(),
                },
            ],
        },
    ]
}
