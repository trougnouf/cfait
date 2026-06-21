// File: ./src/help.rs
// SPDX-License-Identifier: GPL-3.0-or-later
//! Dynamic, localized help sections with progressive disclosure.

pub const SUPPORT_ICONS: &[char] = &[
    '\u{f0a52}', // nf-md-hand_peache_variant
    '\u{f185c}', // nf-md-cash_fast
    '\u{f188f}', // nf-md-hand_coin
    '\u{f118}',  // nf-fa-face_smile
    '\u{eda9}',  // nf-fa-face_smile_wink
    '\u{eeed}',  // nf-fa-cat
    '\u{f0b79}', // nf-md-chat
];

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
            title: rust_i18n::t!("help_quick_start").to_string(),
            items: vec![
                HelpItem {
                    keys: "!1..9".to_string(),
                    desc: rust_i18n::t!("help_org_priority").to_string(),
                    example: format!("{} !1", rust_i18n::t!("example_buy_cat_food")),
                },
                HelpItem {
                    keys: "@date (or due:)".to_string(),
                    desc: rust_i18n::t!("help_timeline_due_date").to_string(),
                    example: format!(
                        "{} @tomorrow, due:2025-01-01",
                        rust_i18n::t!("example_meeting")
                    ),
                },
                HelpItem {
                    keys: "^date (or start:)".to_string(),
                    desc: rust_i18n::t!("help_timeline_start_date").to_string(),
                    example: "^next week, start:tomorrow".to_string(),
                },
                HelpItem {
                    keys: "^@date".to_string(),
                    desc: rust_i18n::t!("help_timeline_set_both_dates").to_string(),
                    example: "^@tomorrow, ^@2d, ^@2026-06-06 15:00-18:30".to_string(),
                },
                HelpItem {
                    keys: "#tag".to_string(),
                    desc: rust_i18n::t!("help_org_add_category").to_string(),
                    example: format!(
                        "{} #breathe #gardening:trees",
                        rust_i18n::t!("example_plant_tree")
                    ),
                },
                HelpItem {
                    keys: "#tag{a,b}".to_string(),
                    desc: rust_i18n::t!("help_org_grouping").to_string(),
                    example: "#gaming{solo,genre=rpg}".to_string(),
                },
                HelpItem {
                    keys: "@@location (or loc:)".to_string(),
                    desc: rust_i18n::t!("help_org_location_hierarchy").to_string(),
                    example: format!("{} @@aldi", rust_i18n::t!("example_buy_cookies")),
                },
                HelpItem {
                    keys: "##tag, @@@loc".to_string(),
                    desc: rust_i18n::t!("help_org_inline_tags").to_string(),
                    example: rust_i18n::t!("example_apply_inline_tags").to_string(),
                },
                HelpItem {
                    keys: "~duration (or est:duration)".to_string(),
                    desc: rust_i18n::t!("help_org_estimated_duration").to_string(),
                    example: format!(
                        "{} ~30m, {} ~1h-2h",
                        rust_i18n::t!("example_exercise"),
                        rust_i18n::t!("example_read_book")
                    ),
                },
            ],
        },
        HelpSection {
            title: rust_i18n::t!("timeline").to_string(),
            items: vec![
                HelpItem {
                    keys: "m, h, d, w, mo, y".to_string(),
                    desc: rust_i18n::t!("help_timeline_units_desc").to_string(),
                    example: "15m, 2h, 3d, 1w, 6mo, 1y".to_string(),
                },
                HelpItem {
                    keys: rust_i18n::t!("help_key_offsets").to_string(),
                    desc: rust_i18n::t!("help_timeline_offsets_desc").to_string(),
                    example: "@1d, ^2w, @3mo, ...".to_string(),
                },
                HelpItem {
                    keys: rust_i18n::t!("help_key_weekdays").to_string(),
                    desc: rust_i18n::t!("help_timeline_weekdays").to_string(),
                    example: "@friday, @fri, @mon".to_string(),
                },
                HelpItem {
                    keys: rust_i18n::t!("help_key_dates").to_string(),
                    desc: rust_i18n::t!("help_timeline_dates_desc").to_string(),
                    example: "@2025-10-31, @2026-05, @2027".to_string(),
                },
            ],
        },
        HelpSection {
            title: rust_i18n::t!("recurrence").to_string(),
            items: vec![
                HelpItem {
                    keys: "@after X".to_string(),
                    desc: rust_i18n::t!("help_recurrence_relative_desc").to_string(),
                    example: "@after 1w, @after 2mo".to_string(),
                },
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
                    desc: rust_i18n::t!("help_reminder_relative_due_desc").to_string(),
                    example: "rem:15m, rem:2h".to_string(),
                },
                HelpItem {
                    keys: "rem:in 5m".to_string(),
                    desc: rust_i18n::t!("help_reminder_relative_now_desc").to_string(),
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
                    example: format!(
                        "desc:\"{}\" {} desc:{{{}}}",
                        rust_i18n::t!("example_call_back"),
                        rust_i18n::t!("or"),
                        rust_i18n::t!("example_call_back")
                    ),
                },
                HelpItem {
                    keys: "+cal / -cal".to_string(),
                    desc: rust_i18n::t!("help_metadata_force_calendar").to_string(),
                    example: format!("{} @tomorrow +cal", rust_i18n::t!("example_task")),
                },
                HelpItem {
                    keys: "+pin / -pin".to_string(),
                    desc: rust_i18n::t!("help_metadata_pin_task").to_string(),
                    example: format!("{} +pin", rust_i18n::t!("example_important_task")),
                },
            ],
        },
        HelpSection {
            title: rust_i18n::t!("search_and_filtering").to_string(),
            items: vec![
                HelpItem {
                    keys: "text".to_string(),
                    desc: rust_i18n::t!("help_search_matches").to_string(),
                    example: rust_i18n::t!("example_buy_cat_food").to_string(),
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
                    desc: rust_i18n::t!("help_org_alias_tag_desc").to_string(),
                    example: "#gardening:=#home:outside,@@garden,!4".to_string(),
                },
                HelpItem {
                    keys: "@@a:=#b,@@c".to_string(),
                    desc: rust_i18n::t!("help_org_alias_loc_desc").to_string(),
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
                    keys: "spent: (or t / Shift+T)".to_string(),
                    desc: rust_i18n::t!("help_log_time_syntax").to_string(),
                    example: "spent:30m, spent:thursday 1h, spent:14:00-15:30".to_string(),
                },
                HelpItem {
                    keys: "#permanent".to_string(),
                    desc: rust_i18n::t!("help_org_permanent").to_string(),
                    example: format!(
                        "{} ~30m-3h #permanent",
                        rust_i18n::t!("example_develop_photos")
                    ),
                },
            ],
        },
        HelpSection {
            title: rust_i18n::t!("help_goals").to_string(),
            items: vec![
                HelpItem {
                    keys: "goal:val/period".to_string(),
                    desc: rust_i18n::t!("help_goals_task").to_string(),
                    example: format!(
                        "{} goal:4/mo, {} ~1h goal:2h/w",
                        rust_i18n::t!("example_develop_photos"),
                        rust_i18n::t!("example_exercise")
                    ),
                },
                HelpItem {
                    keys: "#tag:=goal:val/period".to_string(),
                    desc: rust_i18n::t!("help_goals_global").to_string(),
                    example: "#read:book:=goal:5/y, @@outside:=goal:2h/d".to_string(),
                },
                HelpItem {
                    keys: rust_i18n::t!("help_key_count_vs_duration").to_string(),
                    desc: rust_i18n::t!("help_goals_types").to_string(),
                    example: format!(
                        "goal:weekly {} goal:1/w ({}), goal:30m/d ({})",
                        rust_i18n::t!("or"),
                        rust_i18n::t!("example_1_instance_week"),
                        rust_i18n::t!("example_30_mins_day")
                    ),
                },
                HelpItem {
                    keys: rust_i18n::t!("help_key_implicit_goals").to_string(),
                    desc: rust_i18n::t!("help_goals_implicit_desc").to_string(),
                    example: rust_i18n::t!("help_goals_implicit_example").to_string(),
                },
            ],
        },
        HelpSection {
            title: format!(
                "{} (inside {})",
                rust_i18n::t!("help_md_title"),
                '\u{f01c6}'
            ),
            items: vec![
                HelpItem {
                    keys: "text".to_string(),
                    desc: rust_i18n::t!("help_md_text_desc").to_string(),
                    example: rust_i18n::t!("example_remember_cat").to_string(),
                },
                HelpItem {
                    keys: "- [ ]".to_string(),
                    desc: rust_i18n::t!("help_md_subtask_desc").to_string(),
                    example: format!(
                        "- [ ] {} @tomorrow !1",
                        rust_i18n::t!("example_buy_cookies")
                    ),
                },
                HelpItem {
                    keys: "1. [ ]".to_string(),
                    desc: rust_i18n::t!("help_md_numbered_desc").to_string(),
                    example: format!(
                        "2. [ ] {} ({})",
                        rust_i18n::t!("example_phase_2"),
                        rust_i18n::t!("example_blocked_by_1")
                    ),
                },
                HelpItem {
                    keys: "  (indent)".to_string(),
                    desc: rust_i18n::t!("help_md_notes_desc").to_string(),
                    example: format!("  {}", rust_i18n::t!("example_notes_cookies")),
                },
                HelpItem {
                    keys: rust_i18n::t!("help_md_inherit_key").to_string(),
                    desc: rust_i18n::t!("help_md_inherit_desc").to_string(),
                    example: "".to_string(),
                },
            ],
        },
    ]
}

pub fn get_shortcuts_help(is_gui: bool) -> Vec<HelpSection> {
    let mut nav_items = vec![
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
    ];

    if is_gui {
        nav_items.push(HelpItem {
            keys: "Ctrl + / - / 0".to_string(),
            desc: rust_i18n::t!("help_keyboard_zoom_ui").to_string(),
            example: rust_i18n::t!("help_keyboard_zoom_note").to_string(),
        });
    }

    nav_items.push(HelpItem {
        keys: "z".to_string(),
        desc: "Fold / Unfold Task Tree".to_string(),
        example: "".to_string(),
    });
    nav_items.push(HelpItem {
        keys: "Shift + r".to_string(),
        desc: rust_i18n::t!("jump_to_random_task").to_string(),
        example: "".to_string(),
    });

    if is_gui {
        nav_items.push(HelpItem {
            keys: "Ctrl + ,".to_string(),
            desc: rust_i18n::t!("settings").to_string(),
            example: "".to_string(),
        });
    }

    vec![
        HelpSection {
            title: "Navigation & general".to_string(),
            items: nav_items,
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
                    keys: "Shift + Space".to_string(),
                    desc: "Complete & shift schedule to today".to_string(),
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
                    keys: "t".to_string(),
                    desc: rust_i18n::t!("help_metadata_log_time").to_string(),
                    example: "".to_string(),
                },
                HelpItem {
                    keys: "Shift + t".to_string(),
                    desc: rust_i18n::t!("help_metadata_manage_sessions").to_string(),
                    example: "".to_string(),
                },
                HelpItem {
                    keys: "+ / -".to_string(),
                    desc: rust_i18n::t!("increase_priority").to_string(),
                    example: "".to_string(),
                },
                HelpItem {
                    keys: "M".to_string(),
                    desc: rust_i18n::t!("menu_move").to_string(),
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
                    desc: "Selected is Blocked by / Child of / Related to Yanked".to_string(),
                    example: "".to_string(),
                },
                HelpItem {
                    keys: "C".to_string(),
                    desc: "Create Subtask".to_string(),
                    example: "".to_string(),
                },
                HelpItem {
                    keys: "Y".to_string(),
                    desc: "Toggle Yank Lock (keep yanked task)".to_string(),
                    example: "".to_string(),
                },
                HelpItem {
                    keys: "> / <".to_string(),
                    desc: "Indent / Outdent Task".to_string(),
                    example: "".to_string(),
                },
                HelpItem {
                    keys: "L".to_string(),
                    desc: "Browse relationships / Toggle details".to_string(),
                    example: "".to_string(),
                },
                HelpItem {
                    keys: "Enter".to_string(),
                    desc: "Action Menu / Context Menu".to_string(),
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
                    keys: "1, 2, 3, 4".to_string(),
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
                HelpItem {
                    keys: "w".to_string(),
                    desc: "Toggle Quick Filter (is:ready)".to_string(),
                    example: "".to_string(),
                },
            ],
        },
    ]
}
