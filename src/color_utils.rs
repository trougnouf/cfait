//cfait/src/color_utils.rs

// Utilities for generating deterministic colors from strings.
// This file provides HSL-based color generation and priority color mappings.
// It intentionally has NO dependencies on iced or other GUI crates so it can be
// used from non-GUI code (TUI, core logic, tests, etc).

use std::hash::{Hash, Hasher};

/// Generates a deterministic color tuple (r, g, b) in [0.0, 1.0] range based on the input string.
/// Ranges selected produce reasonably saturated and bright colors suitable for UI accents.
pub fn generate_color(tag: &str) -> (f32, f32, f32) {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    tag.hash(&mut hasher);
    let hash = hasher.finish();

    // Hue: 0-360 degrees
    let h = (hash % 360) as f32;

    let hash_s = hash >> 16;
    let hash_l = hash >> 32;

    // Saturation: 40% - 90% (0.40 .. 0.90)
    let s = 0.40 + ((hash_s % 51) as f32 / 100.0);

    // Lightness: 65% - 90% (0.65 .. 0.90)
    let l = 0.65 + ((hash_l % 26) as f32 / 100.0);

    hsl_to_rgb(h, s, l)
}

/// Helper: HSL to RGB conversion
fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (f32, f32, f32) {
    // Implementation based on standard HSL->RGB conversion
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;

    let (r1, g1, b1) = if (0.0..60.0).contains(&h) {
        (c, x, 0.0)
    } else if (60.0..120.0).contains(&h) {
        (x, c, 0.0)
    } else if (120.0..180.0).contains(&h) {
        (0.0, c, x)
    } else if (180.0..240.0).contains(&h) {
        (0.0, x, c)
    } else if (240.0..300.0).contains(&h) {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };

    (r1 + m, g1 + m, b1 + m)
}

/// Determines if text on top of this color should be black or white.
/// Input components are expected in [0.0, 1.0] range.
pub fn is_dark(r: f32, g: f32, b: f32) -> bool {
    // Perceptual luminance approximation
    let brightness = 0.299 * r + 0.587 * g + 0.114 * b;
    brightness < 0.5
}

/// Parse a hex color string like "#RRGGBB" or "RRGGBB" into floats (0.0..1.0).
pub fn parse_hex_to_floats(hex: &str) -> Option<(f32, f32, f32)> {
    let hex = hex.trim_start_matches('#');
    if hex.len() < 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some((r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0))
}

/// Parse a hex color string like "#RRGGBB" or "RRGGBB" into u8 tuple.
pub fn parse_hex_to_u8(hex: &str) -> Option<(u8, u8, u8)> {
    let hex = hex.trim_start_matches('#');
    if hex.len() < 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some((r, g, b))
}

/// Returns the RGB color tuple for a given priority level.
/// Adapts to light/dark themes to ensure readability.
pub fn get_priority_rgb(priority: u8, is_dark_theme: bool) -> (f32, f32, f32) {
    if is_dark_theme {
        // Bright/Pastel colors for Dark Mode
        match priority {
            1 => (1.0, 0.2, 0.2),    // Red
            2 => (1.0, 0.4, 0.2),    // Orange-Red
            3 => (1.0, 0.6, 0.2),    // Orange
            4 => (1.0, 0.8, 0.2),    // Amber
            5 => (1.0, 1.0, 0.2),    // Yellow
            6 => (0.85, 0.85, 0.55), // Pale Khaki
            7 => (0.7, 0.75, 0.85),  // Light Steel Blue
            8 => (0.65, 0.6, 0.8),   // Slate / Muted Purple
            9 => (0.6, 0.55, 0.65),  // Greyish Lavender
            _ => (1.0, 1.0, 1.0),    // White
        }
    } else {
        // Darker/Saturated colors for Light Mode (White Background)
        match priority {
            1 => (0.8, 0.0, 0.0), // Dark Red
            2 => (0.9, 0.3, 0.0), // Burnt Orange
            3 => (0.9, 0.5, 0.0), // Dark Orange
            4 => (0.8, 0.6, 0.0), // Dark Gold
            5 => (0.7, 0.7, 0.0), // Olive/Dark Yellow
            6 => (0.5, 0.5, 0.2), // Olive Drab
            7 => (0.3, 0.4, 0.6), // Navy Blue
            8 => (0.4, 0.4, 0.5), // Dark Slate
            9 => (0.5, 0.5, 0.5), // Grey
            _ => (0.0, 0.0, 0.0), // Black
        }
    }
}
