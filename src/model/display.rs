// SPDX-License-Identifier: GPL-3.0-or-later
// File: ./src/model/display.rs
use crate::model::item::{Task, TaskStatus};
use chrono::Utc; // Import Utc for live calculation

/// Generate a random example string for the session-logging syntax, used by the
/// TUI and GUI input placeholders.
pub fn random_session_example() -> String {
    const DURATIONS: &[&str] = &["30m", "1h", "2h", "6h", "14:00-15:30", "09:00-10:15"];
    DURATIONS[fastrand::usize(..DURATIONS.len())].to_string()
}

/// Capitalize the first letter of a localized name for display.
fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

/// The parser month/weekday locale keys hold comma-separated aliases, the
/// last of which is the full name. Returns it with a capitalized first
/// letter. Some locales list the plural form last (en "wednesday,wednesdays");
/// when the last alias is just the previous one plus an "s", the previous
/// alias is used instead. A singular name that merely ends in "s" (es
/// "lunes") is kept as-is.
fn full_alias(key: &str) -> String {
    let raw = rust_i18n::t!(key);
    let aliases: Vec<&str> = raw
        .split(',')
        .map(str::trim)
        .filter(|a| !a.is_empty())
        .collect();
    let full = match aliases.len() {
        0 => String::new(),
        1 => aliases[0].to_string(),
        _ => {
            let last = aliases[aliases.len() - 1];
            let prev = aliases[aliases.len() - 2];
            if last.ends_with('s') && &last[..last.len() - 1] == prev {
                prev.to_string()
            } else {
                last.to_string()
            }
        }
    };
    capitalize_first(&full)
}

const MONTH_KEYS: &[&str] = &[
    "parser_months_jan",
    "parser_months_feb",
    "parser_months_mar",
    "parser_months_apr",
    "parser_months_may",
    "parser_months_jun",
    "parser_months_jul",
    "parser_months_aug",
    "parser_months_sep",
    "parser_months_oct",
    "parser_months_nov",
    "parser_months_dec",
];

/// Localized full month name for a 1-12 month number ("September", "septembre").
pub fn local_month_name(month: u32) -> String {
    full_alias(MONTH_KEYS[(month.clamp(1, 12) - 1) as usize])
}

/// Localized short month name for a 1-12 month number ("Sep", "sept").
pub fn local_month_abbr(month: u32) -> String {
    let raw = rust_i18n::t!(MONTH_KEYS[(month.clamp(1, 12) - 1) as usize]);
    let abbr = raw.split(',').next().unwrap_or("").trim();
    if abbr.is_empty() {
        format!("{:02}", month)
    } else {
        capitalize_first(abbr)
    }
}

/// Localized full weekday name ("Wednesday", "mercredi").
pub fn local_weekday_name(weekday: chrono::Weekday) -> String {
    let key = match weekday {
        chrono::Weekday::Mon => "parser_weekdays_mo",
        chrono::Weekday::Tue => "parser_weekdays_tu",
        chrono::Weekday::Wed => "parser_weekdays_we",
        chrono::Weekday::Thu => "parser_weekdays_th",
        chrono::Weekday::Fri => "parser_weekdays_fr",
        chrono::Weekday::Sat => "parser_weekdays_sa",
        chrono::Weekday::Sun => "parser_weekdays_su",
    };
    full_alias(key)
}

/// Field names accepted by `cfait list --fields` / `cfait search --fields`,
/// canonical names first followed by their aliases.
pub const TASK_FIELDS: &[&str] = &[
    "uid",
    "col",
    "summary",
    "desc",
    "status",
    "due",
    "start",
    "priority",
    "percent",
    "categories",
    "locations",
    "url",
    "parent",
    "related",
    "dependencies",
    "sequence",
    "time_spent",
    "estimate",
    "rrule",
    "colid",
    "collection",
    "title",
    "description",
    "dtstart",
    "pct",
    "tags",
];

/// Resolve one task field to its string value for `--fields` output.
/// Returns `None` for unknown field names. Unset fields yield an empty
/// string. Tabs and newlines are replaced with spaces so each task stays on
/// a single TSV line.
pub fn task_field_value(t: &Task, field: &str) -> Option<String> {
    let value = match field {
        "uid" => t.uid.clone(),
        "col" | "colid" | "collection" => t.calendar_href.clone(),
        "summary" | "title" => t.summary.clone(),
        "desc" | "description" => t.description.clone(),
        "status" => match t.status {
            TaskStatus::NeedsAction => "needs_action",
            TaskStatus::InProcess => "in_process",
            TaskStatus::Completed => "completed",
            TaskStatus::Cancelled => "cancelled",
        }
        .to_string(),
        "due" => t.due.as_ref().map(|d| d.format_smart()).unwrap_or_default(),
        "start" | "dtstart" => t
            .dtstart
            .as_ref()
            .map(|d| d.format_smart())
            .unwrap_or_default(),
        "priority" => t.priority.to_string(),
        "percent" | "pct" => t
            .percent_complete
            .map(|p| p.to_string())
            .unwrap_or_default(),
        "categories" | "tags" => t.categories.join(","),
        "locations" => t.locations.join(","),
        "url" => t.url.clone().unwrap_or_default(),
        "parent" => t.parent_uid.clone().unwrap_or_default(),
        "related" => t.related_to.join(","),
        "dependencies" => t.dependencies.join(","),
        "sequence" => t.sequence.to_string(),
        "time_spent" => t.time_spent_seconds.to_string(),
        "estimate" => t
            .estimated_duration
            .map(|d| d.to_string())
            .unwrap_or_default(),
        "rrule" => t.rrule.clone().unwrap_or_default(),
        _ => return None,
    };
    Some(value.replace(['\t', '\n', '\r'], " "))
}

pub trait TaskDisplay {
    fn to_smart_string(&self) -> String;
    fn format_duration_short(&self, store: Option<&crate::store::TaskStore>) -> String;
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

    fn format_duration_short(&self, store: Option<&crate::store::TaskStore>) -> String {
        if let Some(goal) = &self.goal {
            let current = if let Some(s) = store {
                s.calculate_goal_progress(&format!("task:{}", self.uid), goal)
            } else {
                0 // Fallback if store is not provided
            };
            let (c_str, t_str) = if goal.goal_type == crate::config::GoalType::Duration {
                crate::model::parser::format_goal_duration(current, goal.target)
            } else {
                (current.to_string(), goal.target.to_string())
            };
            return format!("[ {} / {}/{}]", c_str, t_str, goal.interval.format_short());
        }

        // Calculate actual spent time — aggregated across the subtree when a
        // store is available, so a parent's badge reflects all work in its
        // tree. Falls back to this task's own time if no store.
        let now_ts = Utc::now().timestamp();
        let total_seconds = if let Some(s) = store {
            s.get_aggregated_time_seconds(&self.uid)
        } else {
            let current_session = self
                .last_started_at
                .map(|start| (now_ts - start).max(0) as u64)
                .unwrap_or(0);
            self.time_spent_seconds + current_session
        };
        let total_mins = (total_seconds / 60) as u32;

        let combined_time_str = if let Some(min) = self.estimated_duration {
            let max = self.estimated_duration_max.unwrap_or(min).max(min);
            if total_mins > 0 || self.last_started_at.is_some() {
                let (c_str, max_str) = crate::model::parser::format_goal_duration(total_mins, max);
                let est_display = if max > min {
                    let (_, min_str) = crate::model::parser::format_goal_duration(total_mins, min);
                    format!("~{}-{}", min_str, max_str)
                } else {
                    format!("~{}", max_str)
                };
                format!("{} / {}", c_str, est_display)
            } else {
                if max > min {
                    format!(
                        "~{}-{}",
                        crate::model::parser::format_duration_compact(min),
                        crate::model::parser::format_duration_compact(max)
                    )
                } else {
                    format!("~{}", crate::model::parser::format_duration_compact(min))
                }
            }
        } else {
            if total_mins > 0 || self.last_started_at.is_some() {
                crate::model::parser::format_duration_compact(total_mins)
            } else {
                String::new()
            }
        };

        // Only display percentage if the task is actively actionable (not completed/cancelled)
        let pc_str = if !self.status.is_done() && self.percent_complete.unwrap_or(0) > 0 {
            if let Some(pc) = self.percent_complete {
                format!("{}%", pc)
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        if !pc_str.is_empty() && !combined_time_str.is_empty() {
            format!("[{}] | {}", pc_str, combined_time_str)
        } else if !pc_str.is_empty() {
            format!("[{}]", pc_str)
        } else if !combined_time_str.is_empty() {
            format!("[{}]", combined_time_str)
        } else {
            String::new()
        }
    }

    fn to_smart_string(&self) -> String {
        use crate::model::item::AlarmTrigger;
        use chrono::{Duration, Local};

        let mut s = crate::model::parser::escape_summary(&self.summary);

        if self.is_note || self.is_journal {
            if s.is_empty() {
                s = "-".to_string();
            } else {
                s = format!("- {}", s);
            }
        }
        if self.priority > 0 {
            s.push_str(&format!(" !{}", self.priority));
        }
        for loc in &self.locations {
            s.push_str(&format!(" @@{}", crate::model::parser::quote_value(loc)));
        }
        if let Some(u) = &self.url {
            s.push_str(&format!(" url:{}", crate::model::parser::quote_value(u)));
        }
        if let Some(g) = &self.geo {
            s.push_str(&format!(" geo:{}", crate::model::parser::quote_value(g)));
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
            let is_relative = self
                .unmapped_properties
                .iter()
                .any(|p| p.key == "X-CFAIT-RECUR-FROM-COMPLETION");
            let pretty = crate::model::parser::prettify_recurrence(r, is_relative);
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

        if self.pinned {
            s.push_str(" is:pinned");
        }

        if self.is_journal {
            s.push_str(" is:page");
        }

        if self.manual_block {
            let block_str = rust_i18n::t!("search_is_blocked");
            if block_str == "search_is_blocked" || block_str.is_empty() {
                s.push_str(" is:blocked");
            } else {
                s.push_str(&format!(
                    " {}",
                    block_str.split(',').next().unwrap_or("is:blocked").trim()
                ));
            }
        }

        if self.permanent {
            let perm_str = rust_i18n::t!("parser_is_permanent");
            if perm_str == "parser_is_permanent" || perm_str.is_empty() {
                s.push_str(" is:permanent");
            } else {
                s.push_str(&format!(
                    " {}",
                    perm_str.split(',').next().unwrap_or("is:permanent").trim()
                ));
            }
        }

        if let Some(goal) = &self.goal {
            let type_str = if goal.goal_type == crate::config::GoalType::Duration {
                crate::model::parser::format_duration_compact(goal.target)
            } else {
                goal.target.to_string()
            };
            s.push_str(&format!(
                " goal:{}/{}",
                type_str,
                goal.interval.format_short()
            ));
        }

        if let (Some(start), Some(due)) = (&self.dtstart, &self.due) {
            if start == due {
                s.push_str(&format!(" ^@{}", start.format_smart()));
            } else if let (
                crate::model::DateType::Specific(s_dt),
                crate::model::DateType::Specific(d_dt),
            ) = (start, due)
            {
                let s_loc = s_dt.with_timezone(&chrono::Local);
                let d_loc = d_dt.with_timezone(&chrono::Local);
                if s_loc.date_naive() == d_loc.date_naive() {
                    s.push_str(&format!(
                        " ^@{} {}-{}",
                        s_loc.format("%Y-%m-%d"),
                        s_loc.format("%H:%M"),
                        d_loc.format("%H:%M")
                    ));
                } else {
                    s.push_str(&format!(" ^{}", start.format_smart()));
                    s.push_str(&format!(" @{}", due.format_smart()));
                }
            } else {
                s.push_str(&format!(" ^{}", start.format_smart()));
                s.push_str(&format!(" @{}", due.format_smart()));
            }
        } else {
            if let Some(start) = &self.dtstart {
                s.push_str(&format!(" ^{}", start.format_smart()));
            }
            if let Some(d) = &self.due {
                s.push_str(&format!(" @{}", d.format_smart()));
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::item::DateType;
    use chrono::NaiveDate;
    use std::collections::HashMap;

    #[test]
    fn local_month_names() {
        assert_eq!(local_month_name(1), "January");
        assert_eq!(local_month_name(9), "September");
        assert_eq!(local_month_name(12), "December");
        // Out-of-range months clamp instead of panicking.
        assert_eq!(local_month_name(0), "January");
        assert_eq!(local_month_name(13), "December");
        assert_eq!(local_month_abbr(1), "Jan");
        assert_eq!(local_month_abbr(9), "Sep");
    }

    #[test]
    fn local_weekday_names() {
        // The en alias list ends in the plural ("mon,monday,mondays");
        // the plural must not leak into the display name.
        assert_eq!(local_weekday_name(chrono::Weekday::Mon), "Monday");
        assert_eq!(local_weekday_name(chrono::Weekday::Wed), "Wednesday");
        assert_eq!(local_weekday_name(chrono::Weekday::Sun), "Sunday");
    }

    #[test]
    fn task_field_values() {
        let mut t = Task::new("water the ferns", &HashMap::new(), None);
        t.uid = "uid-1".to_string();
        t.summary = "water the ferns".to_string();
        t.calendar_href = "https://cal.example/ferns.ics".to_string();
        t.status = TaskStatus::InProcess;
        t.due = Some(DateType::AllDay(
            NaiveDate::from_ymd_opt(2026, 9, 30).unwrap(),
        ));
        t.priority = 3;
        t.categories = vec!["garden".to_string()];
        t.time_spent_seconds = 3600;

        assert_eq!(task_field_value(&t, "uid").as_deref(), Some("uid-1"));
        assert_eq!(
            task_field_value(&t, "col").as_deref(),
            Some("https://cal.example/ferns.ics")
        );
        assert_eq!(
            task_field_value(&t, "colid").as_deref(),
            Some("https://cal.example/ferns.ics")
        );
        assert_eq!(
            task_field_value(&t, "summary").as_deref(),
            Some("water the ferns")
        );
        assert_eq!(
            task_field_value(&t, "status").as_deref(),
            Some("in_process")
        );
        assert_eq!(task_field_value(&t, "due").as_deref(), Some("2026-09-30"));
        assert_eq!(task_field_value(&t, "priority").as_deref(), Some("3"));
        assert_eq!(
            task_field_value(&t, "categories").as_deref(),
            Some("garden")
        );
        assert_eq!(task_field_value(&t, "time_spent").as_deref(), Some("3600"));
        // Unset fields come back empty; unknown fields return None.
        assert_eq!(task_field_value(&t, "desc").as_deref(), Some(""));
        assert_eq!(task_field_value(&t, "parent").as_deref(), Some(""));
        assert_eq!(task_field_value(&t, "bogus"), None);
        // Tabs and newlines are flattened so the value stays on one line.
        t.summary = "a\tb\nc".to_string();
        assert_eq!(task_field_value(&t, "summary").as_deref(), Some("a b c"));
    }
}
