// SPDX-License-Identifier: GPL-3.0-or-later
// File: ./src/model/display.rs
use crate::model::item::{Task, TaskStatus};
use chrono::Utc; // Import Utc for live calculation

pub trait TaskDisplay {
    fn to_smart_string(&self) -> String;
    fn format_duration_short(&self) -> String;
    fn checkbox_symbol(&self) -> &'static str;
    fn is_paused(&self) -> bool;
}

/// Function to get a random relationship icon based on the relationship pair
/// Takes both UIDs to ensure both sides of the relationship see the same icon
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

    // Deterministic selection among three relationship icons
    match hash % 3 {
        0 => '\u{f0a5a}',
        1 => '\u{f0a5e}',
        _ => '\u{f02e8}',
    }
}

impl TaskDisplay for Task {
    fn is_paused(&self) -> bool {
        // No longer relying exclusively on the 50% hack.
        self.status == TaskStatus::NeedsAction
            && ((self.percent_complete.unwrap_or(0) > 0
                && self.percent_complete.unwrap_or(0) < 100)
                || self.time_spent_seconds > 0
                || !self.sessions.is_empty())
    }

    fn checkbox_symbol(&self) -> &'static str {
        if self.is_paused() {
            return "[‖]";
        }
        match self.status {
            TaskStatus::Completed => "[✔]",
            TaskStatus::Cancelled => "[✘]",
            TaskStatus::InProcess => "[▶]",
            TaskStatus::NeedsAction => "[ ]",
        }
    }

    fn format_duration_short(&self) -> String {
        // Calculate actual spent time (stored + current session)
        let now_ts = Utc::now().timestamp();
        let current_session = self
            .last_started_at
            .map(|start| (now_ts - start).max(0) as u64)
            .unwrap_or(0);
        let total_seconds = self.time_spent_seconds + current_session;
        let total_mins = (total_seconds / 60) as u32;

        let est_str = if let Some(min) = self.estimated_duration {
            if let Some(max) = self.estimated_duration_max
                && max > min
            {
                format!(
                    "~{}-{}",
                    crate::model::parser::format_duration_compact(min),
                    crate::model::parser::format_duration_compact(max)
                )
            } else {
                format!("~{}", crate::model::parser::format_duration_compact(min))
            }
        } else {
            String::new()
        };

        let time_str = if total_mins > 0 || self.last_started_at.is_some() {
            if !est_str.is_empty() {
                format!(
                    "{} / {}",
                    crate::model::parser::format_duration_compact(total_mins),
                    est_str
                )
            } else {
                crate::model::parser::format_duration_compact(total_mins).to_string()
            }
        } else if !est_str.is_empty() {
            est_str.to_string()
        } else {
            String::new()
        };

        // Only display percentage if the task is actively actionable (not completed/cancelled)
        let pc_str = if !self.status.is_done() && self.percent_complete.unwrap_or(0) > 0 {
            format!("{}%", self.percent_complete.unwrap())
        } else {
            String::new()
        };

        if !pc_str.is_empty() && !time_str.is_empty() {
            format!("[{}] | {}", pc_str, time_str)
        } else if !pc_str.is_empty() {
            format!("[{}]", pc_str)
        } else if !time_str.is_empty() {
            format!("[{}]", time_str)
        } else {
            String::new()
        }
    }

    fn to_smart_string(&self) -> String {
        use crate::model::item::AlarmTrigger;
        use chrono::{Duration, Local};

        let mut s = crate::model::parser::escape_summary(&self.summary);
        if self.priority > 0 {
            s.push_str(&format!(" !{}", self.priority));
        }
        if let Some(loc) = &self.location {
            s.push_str(&format!(" @@{}", crate::model::parser::quote_value(loc)));
        }
        if let Some(u) = &self.url {
            s.push_str(&format!(" url:{}", crate::model::parser::quote_value(u)));
        }
        if let Some(g) = &self.geo {
            s.push_str(&format!(" geo:{}", crate::model::parser::quote_value(g)));
        }
        if let Some(start) = &self.dtstart {
            s.push_str(&format!(" ^{}", start.format_smart()));
        }
        if let Some(d) = &self.due {
            s.push_str(&format!(" @{}", d.format_smart()));
        }

        if let Some(min) = self.estimated_duration {
            let fmt_val = |m: u32| -> String {
                if m.is_multiple_of(525600) {
                    format!("{}y", m / 525600)
                } else if m.is_multiple_of(43200) {
                    format!("{}mo", m / 43200)
                } else if m.is_multiple_of(10080) {
                    format!("{}w", m / 10080)
                } else if m.is_multiple_of(1440) {
                    format!("{}d", m / 1440)
                } else if m.is_multiple_of(60) {
                    format!("{}h", m / 60)
                } else {
                    format!("{}m", m)
                }
            };

            if let Some(max) = self.estimated_duration_max {
                if max > min {
                    s.push_str(&format!(" ~{}-{}", fmt_val(min), fmt_val(max)));
                } else {
                    s.push_str(&format!(" ~{}", fmt_val(min)));
                }
            } else {
                s.push_str(&format!(" ~{}", fmt_val(min)));
            }
        }

        if let Some(r) = &self.rrule {
            let pretty = crate::model::parser::prettify_recurrence(r);
            s.push_str(&format!(" {}", pretty));
        }

        for ex in &self.exdates {
            s.push_str(&format!(" except {}", ex.format_smart()));
        }

        for alarm in &self.alarms {
            if alarm.is_snooze() || alarm.acknowledged.is_some() {
                continue;
            }
            match alarm.trigger {
                AlarmTrigger::Relative(offset) => {
                    let mins = -offset;
                    if mins > 0 {
                        if mins % 10080 == 0 {
                            s.push_str(&format!(" rem:{}w", mins / 10080));
                        } else if mins % 1440 == 0 {
                            s.push_str(&format!(" rem:{}d", mins / 1440));
                        } else if mins % 60 == 0 {
                            s.push_str(&format!(" rem:{}h", mins / 60));
                        } else {
                            s.push_str(&format!(" rem:{}m", mins));
                        }
                    } else {
                        s.push_str(&format!(" rem:{}m", mins));
                    }
                }
                AlarmTrigger::Absolute(dt) => {
                    let local = dt.with_timezone(&Local);
                    let now = Local::now();

                    // Check if alarm date perfectly matches the task's own date
                    let task_date = self
                        .due
                        .as_ref()
                        .or(self.dtstart.as_ref())
                        .map(|d| d.to_date_naive());

                    if Some(local.date_naive()) == task_date
                        || local.date_naive() == now.date_naive()
                    {
                        s.push_str(&format!(" rem:{}", local.format("%H:%M")));
                    } else if local.date_naive() == now.date_naive() + Duration::days(1) {
                        s.push_str(&format!(" rem:tomorrow {}", local.format("%H:%M")));
                    } else {
                        s.push_str(&format!(" rem:{}", local.format("%Y-%m-%d %H:%M")));
                    }
                }
            }
        }

        for cat in &self.categories {
            s.push_str(&format!(" #{}", crate::model::parser::quote_value(cat)));
        }

        if let Some(create_event) = self.create_event {
            s.push_str(if create_event { " +cal" } else { " -cal" });
        }

        // Output completion date if present
        if let Some(comp) = self.completion_date() {
            let local = comp.with_timezone(&chrono::Local);
            s.push_str(&format!(" done:{}", local.format("%Y-%m-%d %H:%M")));
        } else if let Some(pc) = self.percent_complete
            && pc > 0
        {
            // New partial completion syntax
            s.push_str(&format!(" done:{}%", pc));
        }

        s
    }
}
