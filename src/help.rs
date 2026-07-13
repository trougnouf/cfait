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
    let get_first = |k: &str| {
        rust_i18n::t!(k)
            .split(',')
            .next()
            .unwrap_or("")
            .trim()
            .to_string()
    };

    let p_due = get_first("parser_due");
    let p_start = get_first("parser_start");
    let p_start_due = get_first("parser_start_due");
    let p_duration = get_first("parser_duration");
    let p_loc = get_first("parser_loc");
    let p_recur = get_first("parser_recur");
    let p_rem = get_first("parser_reminder");
    let p_spent = get_first("parser_spent");
    let p_done = get_first("parser_done");
    let p_desc = get_first("parser_desc");
    let p_goal = get_first("parser_goal");
    let is_permanent = get_first("parser_is_permanent");
    let p_url = get_first("parser_url");
    let p_dep = get_first("parser_dep");
    let p_rel = get_first("parser_rel");

    let e_today = get_first("parser_today");
    let e_tomorrow = get_first("parser_tomorrow");
    let e_yesterday = get_first("parser_yesterday");
    let e_now = get_first("parser_now");
    let e_next = get_first("parser_next");
    let e_in = get_first("parser_in");
    let e_every = get_first("parser_every");
    let e_after = get_first("parser_after");
    let e_until = get_first("parser_until");
    let e_except = get_first("parser_except");

    let u_m = get_first("parser_unit_minutes");
    let u_h = get_first("parser_unit_hours");
    let u_d = get_first("parser_unit_days");
    let u_w = get_first("parser_unit_weeks");
    let u_mo = get_first("parser_unit_months");
    let u_y = get_first("parser_unit_years");

    let is_ready = rust_i18n::t!("search_is_ready");
    let is_done = rust_i18n::t!("search_is_done");
    let is_status = rust_i18n::t!("help_keys_search_status");

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
                    keys: format!("{p_due}date"),
                    desc: rust_i18n::t!("help_timeline_due_date").to_string(),
                    example: format!(
                        "{} {p_due}{e_tomorrow}, {p_due}2025-01-01",
                        rust_i18n::t!("example_meeting")
                    ),
                },
                HelpItem {
                    keys: format!("{p_start}date"),
                    desc: rust_i18n::t!("help_timeline_start_date").to_string(),
                    example: format!("{p_start}{e_next} {u_w}, {p_start}{e_tomorrow}"),
                },
                HelpItem {
                    keys: format!("{p_start_due}date"),
                    desc: rust_i18n::t!("help_timeline_set_both_dates").to_string(),
                    example: format!(
                        "{p_start_due}{e_tomorrow}, {p_start_due}2{u_d}, {p_start_due}2026-06-06 15:00-18:30"
                    ),
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
                    keys: format!("{p_loc}location"),
                    desc: rust_i18n::t!("help_org_location_hierarchy").to_string(),
                    example: format!("{} {p_loc}aldi", rust_i18n::t!("example_buy_cookies")),
                },
                HelpItem {
                    keys: "##tag, @@@loc".to_string(),
                    desc: rust_i18n::t!("help_org_inline_tags").to_string(),
                    example: rust_i18n::t!("example_apply_inline_tags").to_string(),
                },
                HelpItem {
                    keys: format!("{p_duration}duration"),
                    desc: rust_i18n::t!("help_org_estimated_duration").to_string(),
                    example: format!(
                        "{} {p_duration}30{u_m}, {} {p_duration}1{u_h}-2{u_h}",
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
                    keys: format!("{u_m}, {u_h}, {u_d}, {u_w}, {u_mo}, {u_y}"),
                    desc: rust_i18n::t!("help_timeline_units_desc").to_string(),
                    example: format!("15{u_m}, 2{u_h}, 3{u_d}, 1{u_w}, 6{u_mo}, 1{u_y}"),
                },
                HelpItem {
                    keys: rust_i18n::t!("help_key_offsets").to_string(),
                    desc: rust_i18n::t!("help_timeline_offsets_desc").to_string(),
                    example: format!("{p_due}1{u_d}, {p_start}2{u_w}, {p_due}3{u_mo}, ..."),
                },
                HelpItem {
                    keys: rust_i18n::t!("help_key_weekdays").to_string(),
                    desc: rust_i18n::t!("help_timeline_weekdays").to_string(),
                    example: format!("{p_due}friday, {p_due}fri, {p_due}mon"),
                },
                HelpItem {
                    keys: rust_i18n::t!("help_key_next_day").to_string(),
                    desc: rust_i18n::t!("help_timeline_next_day").to_string(),
                    example: format!("{p_due}next 8, {p_start}next 15"),
                },
                HelpItem {
                    keys: rust_i18n::t!("help_key_dates").to_string(),
                    desc: rust_i18n::t!("help_timeline_dates_desc").to_string(),
                    example: format!("{p_due}2025-10-31, {p_due}2026-05, {p_due}2027"),
                },
            ],
        },
        HelpSection {
            title: rust_i18n::t!("recurrence").to_string(),
            items: vec![
                HelpItem {
                    keys: format!("{e_after} X"),
                    desc: rust_i18n::t!("help_recurrence_relative_desc").to_string(),
                    example: format!("{e_after} 1{u_w}, {e_after} 2{u_mo}"),
                },
                HelpItem {
                    keys: format!("{p_due}daily ({} {p_recur}daily)", rust_i18n::t!("or")),
                    desc: rust_i18n::t!("help_recurrence_quick_presets").to_string(),
                    example: format!("{p_due}daily, {p_due}weekly, {p_due}monthly, {p_due}yearly"),
                },
                HelpItem {
                    keys: format!("{e_every} X"),
                    desc: rust_i18n::t!("help_recurrence_custom_intervals").to_string(),
                    example: format!(
                        "{e_every} 3 {u_d}, {e_every} 2 {u_w}, {e_every} tuesday, {e_every} sat,sun"
                    ),
                },
                HelpItem {
                    keys: format!("{e_until} <date>"),
                    desc: rust_i18n::t!("help_recurrence_until").to_string(),
                    example: format!("{p_due}daily {e_until} 2025-12-31"),
                },
                HelpItem {
                    keys: format!("{e_except} <...>"),
                    desc: rust_i18n::t!("help_recurrence_except_dates").to_string(),
                    example: format!("{p_due}daily {e_except} 2025-12-25,sat,sun,dec"),
                },
            ],
        },
        HelpSection {
            title: rust_i18n::t!("notifications_and_reminders").to_string(),
            items: vec![
                HelpItem {
                    keys: format!("{p_rem}10{u_m}"),
                    desc: rust_i18n::t!("help_reminder_relative_due_desc").to_string(),
                    example: format!("{p_rem}15{u_m}, {p_rem}2{u_h}"),
                },
                HelpItem {
                    keys: format!("{p_rem}{e_in} 5{u_m}"),
                    desc: rust_i18n::t!("help_reminder_relative_now_desc").to_string(),
                    example: format!("{p_rem}{e_in} 2{u_h}"),
                },
                HelpItem {
                    keys: format!("{p_rem}date"),
                    desc: rust_i18n::t!("help_metadata_absolute_reminder").to_string(),
                    example: format!("{p_rem}2025-01-20 9am, {p_rem}friday"),
                },
            ],
        },
        HelpSection {
            title: rust_i18n::t!("metadata").to_string(),
            items: vec![
                HelpItem {
                    keys: format!("{p_dep}uid / text"),
                    desc: rust_i18n::t!("help_metadata_dependency").to_string(),
                    example: format!(
                        "{p_dep}\"{}\" {}",
                        rust_i18n::t!("example_phase_1"),
                        rust_i18n::t!("example_phase_2")
                    ),
                },
                HelpItem {
                    keys: format!("{p_rel}uid / text"),
                    desc: rust_i18n::t!("help_metadata_relation").to_string(),
                    example: format!("{p_rel}\"{}\"", rust_i18n::t!("example_meeting")),
                },
                HelpItem {
                    keys: format!("{p_url} or [[ ]]"),
                    desc: rust_i18n::t!("help_metadata_attach_link").to_string(),
                    example: format!("{p_url}https://perdu.com or [[https://perdu.com]]"),
                },
                HelpItem {
                    keys: "geo:".to_string(),
                    desc: rust_i18n::t!("help_metadata_coordinates").to_string(),
                    example: "geo:50.1,4.2".to_string(),
                },
                HelpItem {
                    keys: p_desc.to_string(),
                    desc: rust_i18n::t!("help_metadata_append_description").to_string(),
                    example: format!(
                        "{p_desc}\"{}\" {} {p_desc}{{{}}}",
                        rust_i18n::t!("example_call_back"),
                        rust_i18n::t!("or"),
                        rust_i18n::t!("example_call_back")
                    ),
                },
                HelpItem {
                    keys: "+cal / -cal".to_string(),
                    desc: rust_i18n::t!("help_metadata_force_calendar").to_string(),
                    example: format!("{} {p_due}{e_tomorrow} +cal", rust_i18n::t!("example_task")),
                },
                HelpItem {
                    keys: "is:pinned".to_string(),
                    desc: rust_i18n::t!("help_metadata_pin_task").to_string(),
                    example: format!("{} is:pinned", rust_i18n::t!("example_important_task")),
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
                    keys: is_ready.to_string(),
                    desc: rust_i18n::t!("help_search_is_ready").to_string(),
                    example: is_ready.to_string(),
                },
                HelpItem {
                    keys: is_status.to_string(),
                    desc: rust_i18n::t!("help_search_filter_state").to_string(),
                    example: format!("{}, {}, ...", is_done, rust_i18n::t!("search_is_active")),
                },
                HelpItem {
                    keys: "< > <=".to_string(),
                    desc: rust_i18n::t!("help_search_operators").to_string(),
                    example: format!("{p_duration}<20{u_m}, !<4"),
                },
                HelpItem {
                    keys: "Dates".to_string(),
                    desc: rust_i18n::t!("help_search_dates").to_string(),
                    example: format!("{p_due}<{e_today}, {p_start}>1{u_w}"),
                },
                HelpItem {
                    keys: "(A | B) -C".to_string(),
                    desc: rust_i18n::t!("help_search_combine").to_string(),
                    example: format!("{} (#work | #school) -{p_due}{e_today}", is_ready),
                },
            ],
        },
        HelpSection {
            title: rust_i18n::t!("organization").to_string(),
            items: vec![
                HelpItem {
                    keys: "#a:=#b,#c".to_string(),
                    desc: rust_i18n::t!("help_org_alias_tag_desc").to_string(),
                    example: format!("#gardening:=#home:outside,{p_loc}garden,!4"),
                },
                HelpItem {
                    keys: format!("{p_loc}a:=#b,{p_loc}c"),
                    desc: rust_i18n::t!("help_org_alias_loc_desc").to_string(),
                    example: format!("{p_loc}aldi:=#groceries,{p_loc}shops:supermarkets"),
                },
                HelpItem {
                    keys: "\\#text".to_string(),
                    desc: rust_i18n::t!("help_org_escape_special").to_string(),
                    example: "\\#not-a-tag".to_string(),
                },
                HelpItem {
                    keys: "- / is:note".to_string(),
                    desc: rust_i18n::t!("help_metadata_note").to_string(),
                    example: format!("- {}", rust_i18n::t!("example_pantry")),
                },
                HelpItem {
                    keys: format!("{p_done}date"),
                    desc: rust_i18n::t!("help_org_done").to_string(),
                    example: format!(
                        "{p_done}{e_now}, {p_done}{e_yesterday}, {p_done}2024-01-01 15:30"
                    ),
                },
                HelpItem {
                    keys: format!("{p_done}X%"),
                    desc: rust_i18n::t!("help_org_done_percent").to_string(),
                    example: format!("{p_done}25%"),
                },
                HelpItem {
                    keys: format!("{p_spent} ({} t / Shift+T)", rust_i18n::t!("or")),
                    desc: rust_i18n::t!("help_log_time_syntax").to_string(),
                    example: format!(
                        "{p_spent}30{u_m}, {p_spent}thursday 1{u_h}, {p_spent}14:00-15:30"
                    ),
                },
                HelpItem {
                    keys: is_permanent.to_string(),
                    desc: rust_i18n::t!("help_org_permanent").to_string(),
                    example: format!(
                        "{} {p_duration}30{u_m}-3{u_h} {is_permanent}",
                        rust_i18n::t!("example_develop_photos")
                    ),
                },
            ],
        },
        HelpSection {
            title: rust_i18n::t!("help_goals").to_string(),
            items: vec![
                HelpItem {
                    keys: format!("{p_goal}val/period"),
                    desc: rust_i18n::t!("help_goals_task").to_string(),
                    example: format!(
                        "{} {p_goal}4/{u_mo}, {} {p_duration}1{u_h} {p_goal}2{u_h}/{u_w}",
                        rust_i18n::t!("example_develop_photos"),
                        rust_i18n::t!("example_exercise")
                    ),
                },
                HelpItem {
                    keys: format!("#tag:={p_goal}val/period"),
                    desc: rust_i18n::t!("help_goals_global").to_string(),
                    example: format!(
                        "#read:book:={p_goal}5/{u_y}, {p_loc}outside:={p_goal}2{u_h}/{u_d}"
                    ),
                },
                HelpItem {
                    keys: rust_i18n::t!("help_key_count_vs_duration").to_string(),
                    desc: rust_i18n::t!("help_goals_types").to_string(),
                    example: format!(
                        "{p_goal}weekly {} {p_goal}1/{u_w} ({}), {p_goal}30{u_m}/{u_d} ({})",
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
                        "- [ ] {} {p_due}{e_tomorrow} !1",
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
                    keys: "Ctrl + n".to_string(),
                    desc: rust_i18n::t!("help_keyboard_create_desc").to_string(),
                    example: "".to_string(),
                },
                HelpItem {
                    keys: "e / E".to_string(),
                    desc: "Edit Title / Edit Description".to_string(),
                    example: "".to_string(),
                },
                HelpItem {
                    keys: "Ctrl + e".to_string(),
                    desc: "Edit tree (Markdown)".to_string(),
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
