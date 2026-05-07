// SPDX-License-Identifier: GPL-3.0-or-later
//! Presentation model for rendering tasks in UI layers.

use crate::color_utils;
use crate::model::item::{Task, TaskStatus};
use chrono::Utc;
use std::collections::{HashMap, HashSet};

#[cfg_attr(feature = "mobile", derive(uniffi::Record))]
#[derive(Clone, Debug)]
pub struct RenderableTag {
    pub name: String,
    pub bg_color_hex: String,
    pub text_color_hex: String,
}

#[cfg_attr(feature = "mobile", derive(uniffi::Record))]
#[derive(Clone, Debug)]
pub struct RenderableTask {
    pub uid: String,
    pub summary: String,
    pub is_done: bool,
    pub is_blocked: bool,
    pub depth: u32,
    pub status_string: String, // "Completed", "Cancelled", "InProcess", "NeedsAction"
    pub is_paused: bool,
    pub title_color_hex: String,
    pub date_badge: Option<String>,
    pub date_color_hex: String,
    pub date_icon: String,
    pub duration_badge: Option<String>,
    pub duration_color_hex: String,
    pub has_active_alarm: bool,
    pub tags: Vec<RenderableTag>,
    pub location_badge: Option<String>,
    pub has_subtasks: bool,
    pub is_tree_collapsed: bool,
    pub has_notes_or_deps: bool,
    pub url: Option<String>,
    pub geo: Option<String>,
}

impl Task {
    pub fn to_renderable(
        &self,
        is_dark_theme: bool,
        parent_tags: &HashSet<String>,
        parent_location: &Option<String>,
        aliases: &HashMap<String, Vec<String>>,
        is_tree_collapsed: bool,
    ) -> RenderableTask {
        let (visible_tags, visible_location) =
            self.resolve_visual_attributes(parent_tags, parent_location, aliases);

        let is_done = self.status.is_done();
        let is_trash = self.calendar_href == "local://trash";
        let is_blocked = self.is_blocked;

        let title_color_hex = if is_done || is_trash {
            if is_dark_theme {
                "#888888".to_string()
            } else {
                "#A0A0A0".to_string()
            }
        } else if is_blocked {
            if is_dark_theme {
                "#777777".to_string()
            } else {
                "#808080".to_string()
            }
        } else if self.priority > 0 {
            let (r, g, b) = color_utils::get_priority_rgb(self.priority, is_dark_theme);
            format!(
                "#{:02X}{:02X}{:02X}",
                (r * 255.0) as u8,
                (g * 255.0) as u8,
                (b * 255.0) as u8
            )
        } else {
            if is_dark_theme {
                "#FFFFFF".to_string()
            } else {
                "#000000".to_string()
            }
        };

        let status_string = format!("{:?}", self.status);
        let is_paused = self.status == TaskStatus::NeedsAction
            && ((self.percent_complete.unwrap_or(0) > 0
                && self.percent_complete.unwrap_or(0) < 100)
                || self.time_spent_seconds > 0
                || !self.sessions.is_empty());

        let now = Utc::now();
        let mut date_badge = None;
        let mut date_color_hex = if is_dark_theme {
            "#AAAAAA".to_string()
        } else {
            "#666666".to_string()
        };
        let mut date_icon = "\u{f073}".to_string(); // CALENDAR

        if is_done {
            if let Some(done_dt) = self.completion_date() {
                let local = done_dt.with_timezone(&chrono::Local);
                date_badge = Some(local.format("%Y-%m-%d %H:%M").to_string());
                date_icon = if self.status == TaskStatus::Completed {
                    "\u{f274}".to_string()
                } else {
                    "\u{f273}".to_string()
                };
                date_color_hex = if self.status == TaskStatus::Completed {
                    "#66BB6A".to_string()
                } else {
                    "#EF5350".to_string()
                };
            }
        } else if let Some(start) = &self
            .dtstart
            .as_ref()
            .filter(|s| s.to_start_comparison_time() > now)
        {
            let start_str = start.format_smart();
            date_icon = "\u{f251}".to_string(); // HOURGLASS_START
            if let Some(due) = &self.due {
                if start_str == due.format_smart() {
                    date_badge = Some(start_str);
                } else {
                    date_badge = Some(format!("{} - {}", start_str, due.format_smart()));
                }
            } else {
                date_badge = Some(start_str);
            }
        } else if let Some(d) = &self.due {
            let is_overdue = d.to_comparison_time() < now;
            date_badge = Some(d.format_smart());
            date_icon = "\u{f253}".to_string(); // HOURGLASS_END
            if is_overdue {
                date_color_hex = "#EF5350".to_string();
            }
        }

        let has_active_alarm = self.alarms.iter().any(|a| a.acknowledged.is_none());

        let now_ts = now.timestamp();
        let current_session = self
            .last_started_at
            .map(|s| (now_ts - s).max(0) as u64)
            .unwrap_or(0);
        let total_mins = (self.time_spent_seconds + current_session) / 60;

        let est_str = if let Some(min) = self.estimated_duration {
            if let Some(max) = self.estimated_duration_max.filter(|m| *m > min) {
                format!(
                    "~{}-{}",
                    crate::model::parser::format_duration_compact(min),
                    crate::model::parser::format_duration_compact(max)
                )
            } else {
                format!("~{}", crate::model::parser::format_duration_compact(min))
            }
        } else {
            "".to_string()
        };

        let time_str = if total_mins > 0 || self.last_started_at.is_some() {
            if !est_str.is_empty() {
                format!(
                    "{} / {}",
                    crate::model::parser::format_duration_compact(total_mins as u32),
                    est_str
                )
            } else {
                crate::model::parser::format_duration_compact(total_mins as u32).to_string()
            }
        } else {
            est_str
        };

        let pc_str = if !is_done && self.percent_complete.unwrap_or(0) > 0 {
            format!("{}%", self.percent_complete.unwrap())
        } else {
            "".to_string()
        };

        let duration_badge = if !pc_str.is_empty() && !time_str.is_empty() {
            Some(format!("{} | {}", pc_str, time_str))
        } else if !pc_str.is_empty() {
            Some(pc_str)
        } else if !time_str.is_empty() {
            Some(time_str)
        } else {
            None
        };

        let duration_color_hex = if self.last_started_at.is_some() {
            "#66BB6A".to_string()
        } else {
            if is_dark_theme {
                "#AAAAAA".to_string()
            } else {
                "#666666".to_string()
            }
        };

        let renderable_tags = visible_tags
            .into_iter()
            .map(|name| {
                let (r, g, b) = color_utils::generate_color(&name);
                let bg = format!(
                    "#{:02X}{:02X}{:02X}",
                    (r * 255.0) as u8,
                    (g * 255.0) as u8,
                    (b * 255.0) as u8
                );
                let text = if color_utils::is_dark(r, g, b) {
                    "#FFFFFF".to_string()
                } else {
                    "#000000".to_string()
                };
                RenderableTag {
                    name,
                    bg_color_hex: bg,
                    text_color_hex: text,
                }
            })
            .collect();

        let has_notes_or_deps = !self.description.is_empty()
            || !self.dependencies.is_empty()
            || !self.related_to.is_empty();

        RenderableTask {
            uid: self.uid.clone(),
            summary: self.summary.clone(),
            is_done,
            is_blocked,
            depth: self.depth as u32,
            status_string,
            is_paused,
            title_color_hex,
            date_badge,
            date_color_hex,
            date_icon,
            duration_badge,
            duration_color_hex,
            has_active_alarm,
            tags: renderable_tags,
            location_badge: visible_location,
            has_subtasks: self.has_visible_subtasks,
            is_tree_collapsed,
            has_notes_or_deps,
            url: self.url.clone(),
            geo: self.geo.clone(),
        }
    }
}
