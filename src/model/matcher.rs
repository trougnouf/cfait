// Logic for checking if tasks match search queries.
use crate::model::item::{Task, TaskStatus};
use chrono::{Duration, Local, NaiveDate};

impl Task {
    pub fn matches_search_term(&self, term: &str) -> bool {
        if term.is_empty() {
            return true;
        }

        let term_lower = term.to_lowercase();
        let parts: Vec<&str> = term_lower.split_whitespace().collect();

        for part in parts {
            // --- Location Filter ---
            if let Some(loc_query) = part
                .strip_prefix("@@")
                .or_else(|| part.strip_prefix("loc:"))
            {
                if let Some(t_loc) = &self.location {
                    if !t_loc.to_lowercase().contains(loc_query) {
                        return false;
                    }
                } else {
                    return false;
                }
                continue;
            }

            // 1. Duration Filter (~30m, ~<1h, ~>2h)
            if part.starts_with('~') {
                let (op, val_str) = if let Some(stripped) = part.strip_prefix("~<=") {
                    ("<=", stripped)
                } else if let Some(stripped) = part.strip_prefix("~>=") {
                    (">=", stripped)
                } else if let Some(stripped) = part.strip_prefix("~<") {
                    ("<", stripped)
                } else if let Some(stripped) = part.strip_prefix("~>") {
                    (">", stripped)
                } else if let Some(stripped) = part.strip_prefix('~') {
                    ("=", stripped)
                } else {
                    continue;
                };

                let mins = if let Some(n) = val_str.strip_suffix('m') {
                    n.parse::<u32>().ok()
                } else if let Some(n) = val_str.strip_suffix('h') {
                    n.parse::<u32>().ok().map(|h| h * 60)
                } else if let Some(n) = val_str.strip_suffix('d') {
                    n.parse::<u32>().ok().map(|d| d * 1440)
                } else if let Some(n) = val_str.strip_suffix('w') {
                    n.parse::<u32>().ok().map(|w| w * 10080)
                } else if let Some(n) = val_str.strip_suffix("mo") {
                    n.parse::<u32>().ok().map(|m| m * 43200)
                } else if let Some(n) = val_str.strip_suffix('y') {
                    n.parse::<u32>().ok().map(|y| y * 525600)
                } else {
                    None
                };

                if let Some(target) = mins {
                    let t_min = self.estimated_duration.unwrap_or(0);
                    let t_max = self.estimated_duration_max.unwrap_or(t_min);

                    if self.estimated_duration.is_none() {
                        return false;
                    }

                    match op {
                        "<" => {
                            if t_min >= target {
                                return false;
                            }
                        }
                        ">" => {
                            if t_max <= target {
                                return false;
                            }
                        }
                        "<=" => {
                            if t_min > target {
                                return false;
                            }
                        }
                        ">=" => {
                            if t_max < target {
                                return false;
                            }
                        }
                        _ => {
                            if target < t_min || target > t_max {
                                return false;
                            }
                        }
                    }
                    continue;
                }
            }

            if part.starts_with('!') {
                let (op, val_str) = if let Some(stripped) = part.strip_prefix("!<=") {
                    ("<=", stripped)
                } else if let Some(stripped) = part.strip_prefix("!>=") {
                    (">=", stripped)
                } else if let Some(stripped) = part.strip_prefix("!<") {
                    ("<", stripped)
                } else if let Some(stripped) = part.strip_prefix("!>") {
                    (">", stripped)
                } else if let Some(stripped) = part.strip_prefix('!') {
                    ("=", stripped)
                } else {
                    continue;
                };

                if let Ok(target) = val_str.parse::<u8>() {
                    let p = self.priority;
                    match op {
                        "<" => {
                            if p >= target {
                                return false;
                            }
                        }
                        ">" => {
                            if p <= target {
                                return false;
                            }
                        }
                        "<=" => {
                            if p > target {
                                return false;
                            }
                        }
                        ">=" => {
                            if p < target {
                                return false;
                            }
                        }
                        _ => {
                            if p != target {
                                return false;
                            }
                        }
                    }
                    continue;
                }
            }

            let check_date_filter = |prefix_char: char,
                                     alt_prefix: &str,
                                     task_date: Option<NaiveDate>|
             -> Option<bool> {
                if !part.starts_with(prefix_char) && !part.starts_with(alt_prefix) {
                    return None;
                }

                let raw_val = part
                    .strip_prefix(alt_prefix)
                    .or_else(|| part.strip_prefix(prefix_char))
                    .unwrap();

                let (val_str_full, include_none) = if let Some(stripped) = raw_val.strip_suffix('!')
                {
                    (stripped, true)
                } else {
                    (raw_val, false)
                };

                let (op, date_str) = if let Some(s) = val_str_full.strip_prefix("<=") {
                    ("<=", s)
                } else if let Some(s) = val_str_full.strip_prefix(">=") {
                    (">=", s)
                } else if let Some(s) = val_str_full.strip_prefix('<') {
                    ("<", s)
                } else if let Some(s) = val_str_full.strip_prefix('>') {
                    (">", s)
                } else {
                    ("=", val_str_full)
                };

                let now = Local::now().date_naive();

                let target_date = if date_str == "today" {
                    Some(now)
                } else if date_str == "tomorrow" {
                    Some(now + Duration::days(1))
                } else if date_str == "yesterday" {
                    Some(now - Duration::days(1))
                } else if let Ok(date) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
                    Some(date)
                } else {
                    let offset = if let Some(n) = date_str.strip_suffix('d') {
                        n.parse::<i64>().ok()
                    } else if let Some(n) = date_str.strip_suffix('w') {
                        n.parse::<i64>().ok().map(|w| w * 7)
                    } else if let Some(n) = date_str.strip_suffix("mo") {
                        n.parse::<i64>().ok().map(|m| m * 30)
                    } else if let Some(n) = date_str.strip_suffix('y') {
                        n.parse::<i64>().ok().map(|y| y * 365)
                    } else {
                        None
                    };
                    offset.map(|days| now + Duration::days(days))
                };

                if let Some(target) = target_date {
                    match task_date {
                        Some(t_date) => match op {
                            "<" => {
                                if t_date >= target {
                                    return Some(false);
                                }
                            }
                            ">" => {
                                if t_date <= target {
                                    return Some(false);
                                }
                            }
                            "<=" => {
                                if t_date > target {
                                    return Some(false);
                                }
                            }
                            ">=" => {
                                if t_date < target {
                                    return Some(false);
                                }
                            }
                            _ => {
                                if t_date != target {
                                    return Some(false);
                                }
                            }
                        },
                        None => {
                            if !include_none {
                                return Some(false);
                            }
                        }
                    }
                    return Some(true);
                }

                None
            };

            let t_start = self.dtstart.as_ref().map(|d| d.to_date_naive());
            if let Some(passed) = check_date_filter('^', "start:", t_start) {
                if !passed {
                    return false;
                }
                continue;
            }

            let t_due = self.due.as_ref().map(|d| d.to_date_naive());
            if let Some(passed) = check_date_filter('@', "due:", t_due) {
                if !passed {
                    return false;
                }
                continue;
            }

            if let Some(tag_query) = part.strip_prefix('#') {
                if !self
                    .categories
                    .iter()
                    .any(|c| c.to_lowercase().contains(tag_query))
                {
                    return false;
                }
                continue;
            }

            if part == "is:done" {
                if !self.status.is_done() {
                    return false;
                }
                continue;
            }

            if part == "is:started" || part == "is:ongoing" {
                if self.status != TaskStatus::InProcess {
                    return false;
                }
                continue;
            }

            if part == "is:active" {
                if self.status.is_done() {
                    return false;
                }
                continue;
            }

            if part == "is:ready" || part == "is:blocked" {
                continue;
            }

            if !self.summary.to_lowercase().contains(part)
                && !self.description.to_lowercase().contains(part)
                && !self
                    .categories
                    .iter()
                    .any(|c| c.to_lowercase().contains(part))
                && !self
                    .location
                    .as_deref()
                    .is_some_and(|l| l.to_lowercase().contains(part))
            {
                return false;
            }
        }
        true
    }
}
