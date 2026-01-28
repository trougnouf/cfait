// File: ./src/model/display.rs
use crate::model::item::{Task, TaskStatus};

pub trait TaskDisplay {
    fn to_smart_string(&self) -> String;
    fn format_duration_short(&self) -> String;
    fn checkbox_symbol(&self) -> &'static str;
    fn is_paused(&self) -> bool;
}

impl TaskDisplay for Task {
    fn is_paused(&self) -> bool {
        self.status == TaskStatus::NeedsAction
            && self.percent_complete.unwrap_or(0) > 0
            && self.percent_complete.unwrap_or(0) < 100
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
        fn fmt_min(m: u32) -> String {
            if m >= 525600 {
                format!("{}y", m / 525600)
            } else if m >= 43200 {
                format!("{}mo", m / 43200)
            } else if m >= 10080 {
                format!("{}w", m / 10080)
            } else if m >= 1440 {
                format!("{}d", m / 1440)
            } else if m >= 60 {
                format!("{}h", m / 60)
            } else {
                format!("{}m", m)
            }
        }

        if let Some(min) = self.estimated_duration {
            if let Some(max) = self.estimated_duration_max
                && max > min
            {
                return format!("[~{}-{}]", fmt_min(min), fmt_min(max));
            }
            return format!("[~{}]", fmt_min(min));
        }
        String::new()
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

                    if local.date_naive() == now.date_naive() {
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

        s
    }
}
