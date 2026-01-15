/*
File: cfait/src/gui/icon.rs

This file defines icon font loading and codepoint constants used by the GUI.
It mirrors the project's existing icon mappings with added tag/check variants
used by the sidebar and Android clients.
*/

use iced::Font;
use iced::widget::{Text, text};

pub const FONT_BYTES: &[u8] = include_bytes!("../../assets/fonts/SymbolsNerdFont-Regular.ttf");
pub const FONT: Font = Font::with_name("Symbols Nerd Font");

// Load the Logo
pub const LOGO: &[u8] = include_bytes!("../../assets/cfait.svg");
// Load the Help Icons (21:8 ratio)
pub const HELP_ICON_QUESTION: &[u8] =
    include_bytes!("../../assets/nf-cod-question+breeze-face-hugs.svg");
pub const HELP_ICON_ROBOT: &[u8] =
    include_bytes!("../../assets/nf-md-robot_confused+breeze-face-hugs.svg");
pub const HELP_ICON_ROBOT_HELP: &[u8] =
    include_bytes!("../../assets/nf-md-robot_confused+help+breeze-face-hugs.svg");

pub fn icon<'a>(codepoint: char) -> Text<'a> {
    text(codepoint.to_string()).font(FONT)
}

// --- NERD FONT MAPPING ---

pub const CALENDAR: char = '\u{f073}'; // 
pub const TAG: char = '\u{f02b}'; //  (generic tag)
pub const TAG_OUTLINE: char = '\u{f04fc}'; // nf-md-tag_outline
pub const TAG_CHECK: char = '\u{f1a7a}'; // nf-md-tag_check
pub const SETTINGS: char = '\u{f013}'; // 
pub const REFRESH: char = '\u{f0450}'; // nf-md-refresh
pub const UNSYNCED: char = '\u{f0c2}'; //  (Cloud)
pub const PLUS: char = '\u{f0603}'; // nf-md-priority_high
pub const MINUS: char = '\u{f0604}'; // nf-md-priority_low
pub const TRASH: char = '\u{f1f8}'; // 
pub const CHECK: char = '\u{f00c}'; // 
pub const CHECK_CIRCLE: char = '\u{f058}'; // nf-fa-check_circle
pub const CROSS: char = '\u{f00d}'; // 
pub const EDIT: char = '\u{f040}'; // 
pub const PLAY: char = '\u{eb2c}'; // nf-cod-play
pub const PLAY_FA: char = '\u{f04b}'; // nf-fa-play
pub const PAUSE: char = '\u{f04c}'; //  (Added)
pub const DEBUG_STOP: char = '\u{ead7}'; // nf-cod-debug_stop (Added)
pub const STOP: char = '\u{f04d}'; // 
pub const LOCK: char = '\u{f023}'; // 
pub const LINK: char = '\u{f0c1}'; // 
pub const UNLINK: char = '\u{f127}'; // 
pub const SHIELD: char = '\u{f32a}'; // 
pub const CHILD_ARROW: char = '\u{f149}'; // 
pub const INFO: char = '\u{f129}'; // 
pub const REPEAT: char = '\u{f0b6}'; // 
pub const ARROW_RIGHT: char = '\u{f061}'; // 
pub const ARROW_LEFT: char = '\u{f060}'; //
pub const CHECK_SQUARE: char = '\u{f14a}'; //
pub const SQUARE: char = '\u{f096}'; //
pub const EXPORT: char = '\u{f093}'; // nf-fa-upload
pub const IMPORT: char = '\u{f019}'; // nf-fa-download
pub const BLOCKED: char = '\u{f479}'; // nf-oct-blocked
pub const CHILD: char = '\u{f0a89}'; // nf-md-account_child
pub const CREATE_CHILD: char = '\u{f0014}'; // nf-md-account_plus
pub const CLEAR_ALL: char = '\u{eabf}'; // nf-cod-clear_all
pub const MAP_PIN: char = '\u{f276}'; // nf-fa-map_pin
pub const ELEVATOR_UP: char = '\u{f12c1}'; // nf-md-elevator_up

pub const SETTINGS_GEAR: char = '\u{e690}'; // nf-seti-settings
pub const HELP_RHOMBUS: char = '\u{f0625}'; // nf-md-help_circle_outline
pub const SEARCH_STOP: char = '\u{eb4e}'; // nf-cod-search_stop
pub const SEARCH: char = '\u{ea6d}'; // nf-cod-search

// Window Controls
pub const WINDOW_MINIMIZE: char = '\u{f2d1}'; // nf-fa-window_minimize

// Calendar State Icons
pub const CONTENT_SAVE_EDIT: char = '\u{f0cfb}'; // nf-md-content_save_edit
pub const EYE: char = '\u{ea70}'; // nf-cod-eye
pub const EYE_CLOSED: char = '\u{eae7}'; // nf-cod-eye_closed

// --- SUPPORT / DONATION ICONS ---
pub const HEART_HAND: char = '\u{ed9b}'; // nf-fa-hand_holding_heart
pub const CREDIT_CARD: char = '\u{f09d}'; // nf-fa-credit_card
pub const BANK: char = '\u{f0a27}'; // nf-md-bank_transfer
pub const BITCOIN: char = '\u{f10f}'; // nf-fa-bitcoin
pub const LITECOIN: char = '\u{f0a61}'; // nf-md-litecoin
pub const ETHEREUM: char = '\u{ed58}'; // nf-fa-ethereum

// --- NEW FIELD ICONS (Updated) ---
pub const LOCATION: char = '\u{ef4b}'; // Default European Earth
pub const URL: char = '\u{f0c1}'; // Generic Link
pub const URL_CHECK: char = '\u{f0789}'; // nf-md-web_check
pub const MAP_LOCATION_DOT: char = '\u{ee69}'; // nf-fa-map_location_dot
pub const GEO: char = '\u{f041}'; // Map Marker

// Location Tab Variations
pub const EARTH_ASIA: char = '\u{ee47}';
pub const EARTH_AMERICAS: char = '\u{ee46}';
pub const EARTH_AFRICA: char = '\u{ee45}';
pub const EARTH_GENERIC: char = '\u{f01e7}'; // nf-md-earth
pub const PLANET: char = '\u{e22e}'; // nf-fae-planet
pub const GALAXY: char = '\u{e243}'; // nf-fae-galaxy
pub const ISLAND: char = '\u{f104f}'; // nf-md-island
pub const COMPASS: char = '\u{ebd5}'; // nf-cod-compass
pub const MOUNTAINS: char = '\u{e2a6}'; // nf-fae-mountains
pub const GLOBE: char = '\u{f0ac}'; // nf-fa-globe
pub const GLOBEMODEL: char = '\u{f08e9}'; // nf-md-globe_model

// NEW
pub const BELL: char = '\u{f0f3}'; // nf-fa-bell
pub const PALETTE_COLOR: char = '\u{e22b}'; // nf-fae-palette_color
pub const HOURGLASS_START: char = '\u{f251}'; // nf-fa-hourglass_start

// NEW HEADER ICONS per request
pub const CALENDARS_HEADER: char = '\u{f00f2}'; // nf-md-calendar_multiple_check
pub const TAGS_HEADER: char = '\u{f04fb}'; // nf-md-tag_multiple

// RELATIONSHIP ICONS (for generic related-to with random selection)
pub const RELATED_FEMALE_FEMALE: char = '\u{f0a5a}'; // nf-md-human_female_female
pub const RELATED_MALE_MALE: char = '\u{f0a5e}'; // nf-md-human_male_male
pub const RELATED_MALE_FEMALE: char = '\u{f02e8}'; // nf-md-human_male_female

// Function to get a random relationship icon based on the relationship pair
// Takes both UIDs to ensure both sides of the relationship see the same icon
pub fn random_related_icon(uid1: &str, uid2: &str) -> char {
    // Sort UIDs to ensure consistent ordering regardless of direction
    let (first, second) = if uid1 < uid2 {
        (uid1, uid2)
    } else {
        (uid2, uid1)
    };

    // Hash the sorted pair
    let hash: u32 = first
        .bytes()
        .chain(second.bytes())
        .fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32));

    match hash % 3 {
        0 => RELATED_FEMALE_FEMALE,
        1 => RELATED_MALE_MALE,
        _ => RELATED_MALE_FEMALE,
    }
}
