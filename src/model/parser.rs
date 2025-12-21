// File: ./src/model/parser.rs
use crate::model::item::Task;
use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, TimeZone, Utc};
use std::collections::HashMap;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum SyntaxType {
    Text,
    Priority,
    DueDate,
    StartDate,
    Recurrence,
    Duration,
    Tag,
}

#[derive(Debug)]
pub struct SyntaxToken {
    pub kind: SyntaxType,
    pub start: usize,
    pub end: usize, // Exclusive
}

pub fn tokenize_smart_input(input: &str) -> Vec<SyntaxToken> {
    let mut tokens = Vec::new();

    // 1. Identify words and their positions to handle multi-word lookahead
    let mut words = Vec::new();
    let mut current_idx = 0;

    // We use split_inclusive to handle whitespace preservation logic later if needed,
    // but for tokens we mostly care about the content parts.
    for part in input.split_inclusive(char::is_whitespace) {
        let trimmed = part.trim_end();
        if !trimmed.is_empty() {
            let start = current_idx;
            let end = current_idx + trimmed.len();
            words.push((start, end, trimmed));
        }
        current_idx += part.len();
    }

    let mut cursor = 0; // Current byte index in input
    let mut word_idx = 0;

    while word_idx < words.len() {
        let (start, end, word) = words[word_idx];

        // Fill gap with Text
        if start > cursor {
            tokens.push(SyntaxToken {
                kind: SyntaxType::Text,
                start: cursor,
                end: start,
            });
        }

        let mut matched_token = None;

        // --- Logic mirroring apply_smart_input ---

        // 1. Priority (!1 - !9)
        if word.starts_with('!') && word.len() > 1 && word[1..].parse::<u8>().is_ok() {
            matched_token = Some(SyntaxToken {
                kind: SyntaxType::Priority,
                start,
                end,
            });
            word_idx += 1;
        }
        // 2. Duration (~30m, est:30m)
        else if (word.starts_with('~') || word.starts_with("est:"))
            && parse_duration(word.trim_start_matches("est:").trim_start_matches('~')).is_some()
        {
            matched_token = Some(SyntaxToken {
                kind: SyntaxType::Duration,
                start,
                end,
            });
            word_idx += 1;
        }
        // 3. Tags (#tag)
        else if word.starts_with('#') {
            matched_token = Some(SyntaxToken {
                kind: SyntaxType::Tag,
                start,
                end,
            });
            word_idx += 1;
        }
        // 4. Recurrence Multi-word (@every 2 weeks)
        else if (word == "@every" || word == "rec:every") && word_idx + 2 < words.len() {
            let (_, _, amount_str) = words[word_idx + 1];
            let (_, unit_end, unit_str) = words[word_idx + 2];

            if amount_str.parse::<u32>().is_ok() && !parse_freq_unit(unit_str).is_empty() {
                matched_token = Some(SyntaxToken {
                    kind: SyntaxType::Recurrence,
                    start,
                    end: unit_end,
                });
                word_idx += 3;
            }
        }
        // 5. Recurrence Single-word (@daily)
        if matched_token.is_none() && (word.starts_with('@') || word.starts_with("rec:")) {
            let val = word.trim_start_matches("rec:").trim_start_matches('@');
            if parse_recurrence(val).is_some() {
                matched_token = Some(SyntaxToken {
                    kind: SyntaxType::Recurrence,
                    start,
                    end,
                });
                word_idx += 1;
            }
        }

        // 6. Dates Multi-word (@next week, ^next month, @in 2 weeks)
        if matched_token.is_none()
            && (word.starts_with('@')
                || word.starts_with('^')
                || word.starts_with("due:")
                || word.starts_with("start:"))
        {
            let prefix_char = word.chars().next().unwrap_or(' ');
            let is_start = prefix_char == '^' || word.starts_with("start:");
            let clean_word = if is_start {
                word.trim_start_matches("start:").trim_start_matches('^')
            } else {
                word.trim_start_matches("due:").trim_start_matches('@')
            };

            // @next week
            if clean_word == "next" && word_idx + 1 < words.len() {
                let (_, next_end, next_val) = words[word_idx + 1];
                if is_date_unit(next_val) {
                    let kind = if is_start {
                        SyntaxType::StartDate
                    } else {
                        SyntaxType::DueDate
                    };
                    matched_token = Some(SyntaxToken {
                        kind,
                        start,
                        end: next_end,
                    });
                    word_idx += 2;
                }
            }
            // @in 2 weeks
            else if clean_word == "in" && word_idx + 2 < words.len() {
                let (_, _, amount_str) = words[word_idx + 1];
                let (_, unit_end, unit_str) = words[word_idx + 2];

                if parse_english_number(amount_str).is_some() && is_date_duration_unit(unit_str) {
                    let kind = if is_start {
                        SyntaxType::StartDate
                    } else {
                        SyntaxType::DueDate
                    };
                    matched_token = Some(SyntaxToken {
                        kind,
                        start,
                        end: unit_end,
                    });
                    word_idx += 3;
                }
            }
        }

        // 7. Dates Single-word (@tomorrow, ^today)
        if matched_token.is_none() {
            if word.starts_with('^') || word.starts_with("start:") {
                let val = word.trim_start_matches("start:").trim_start_matches('^');
                if parse_smart_date(val, false).is_some() {
                    matched_token = Some(SyntaxToken {
                        kind: SyntaxType::StartDate,
                        start,
                        end,
                    });
                    word_idx += 1;
                }
            } else if word.starts_with('@') || word.starts_with("due:") {
                let val = word.trim_start_matches("due:").trim_start_matches('@');
                if parse_smart_date(val, true).is_some() {
                    matched_token = Some(SyntaxToken {
                        kind: SyntaxType::DueDate,
                        start,
                        end,
                    });
                    word_idx += 1;
                }
            }
        }

        if let Some(token) = matched_token {
            tokens.push(token);
            // Move cursor to the end of the consumed token(s)
            cursor = tokens.last().unwrap().end;
        } else {
            // No match, treat current word as text
            tokens.push(SyntaxToken {
                kind: SyntaxType::Text,
                start,
                end,
            });
            cursor = end;
            word_idx += 1;
        }
    }

    // Fill trailing text if any
    if cursor < input.len() {
        tokens.push(SyntaxToken {
            kind: SyntaxType::Text,
            start: cursor,
            end: input.len(),
        });
    }

    tokens
}

fn is_date_unit(s: &str) -> bool {
    let s = s.to_lowercase();
    matches!(
        s.as_str(),
        "week"
            | "month"
            | "year"
            | "monday"
            | "tuesday"
            | "wednesday"
            | "thursday"
            | "friday"
            | "saturday"
            | "sunday"
    )
}

fn is_date_duration_unit(s: &str) -> bool {
    let s = s.to_lowercase();
    matches!(
        s.as_str(),
        "d" | "day"
            | "days"
            | "w"
            | "week"
            | "weeks"
            | "mo"
            | "month"
            | "months"
            | "y"
            | "year"
            | "years"
    )
}

impl Task {
    pub fn apply_smart_input(&mut self, input: &str, aliases: &HashMap<String, Vec<String>>) {
        let mut summary_words = Vec::new();
        self.priority = 0;
        self.due = None;
        self.dtstart = None;
        self.rrule = None;
        self.estimated_duration = None;
        self.categories.clear();

        let tokens: Vec<&str> = input.split_whitespace().collect();
        let mut i = 0;

        while i < tokens.len() {
            let word = tokens[i];

            if word.starts_with('!')
                && let Ok(p) = word[1..].parse::<u8>()
                && (1..=9).contains(&p)
            {
                self.priority = p;
                i += 1;
                continue;
            }

            if let Some(val) = word.strip_prefix("est:").or_else(|| word.strip_prefix('~'))
                && let Some(m) = parse_duration(val)
            {
                self.estimated_duration = Some(m);
                i += 1;
                continue;
            }

            if let Some(stripped) = word.strip_prefix('#') {
                let cat = stripped.to_string();
                if !cat.is_empty() {
                    if !self.categories.contains(&cat) {
                        self.categories.push(cat.clone());
                    }

                    let mut search = cat.as_str();
                    loop {
                        if let Some(expanded_tags) = aliases.get(search) {
                            for extra_tag in expanded_tags {
                                if !self.categories.contains(extra_tag) {
                                    self.categories.push(extra_tag.clone());
                                }
                            }
                        }
                        if let Some(idx) = search.rfind(':') {
                            search = &search[..idx];
                        } else {
                            break;
                        }
                    }

                    i += 1;
                    continue;
                }
            }

            if (word == "rec:every" || word == "@every") && i + 2 < tokens.len() {
                let amount_str = tokens[i + 1];
                let unit_str = tokens[i + 2];
                if let Ok(interval) = amount_str.parse::<u32>() {
                    let freq = parse_freq_unit(unit_str);
                    if !freq.is_empty() {
                        self.rrule = Some(format!("FREQ={};INTERVAL={}", freq, interval));
                        i += 3;
                        continue;
                    }
                }
            }

            if let Some(val) = word.strip_prefix("rec:").or_else(|| word.strip_prefix('@'))
                && let Some(rrule) = parse_recurrence(val)
            {
                self.rrule = Some(rrule);
                i += 1;
                continue;
            }

            // --- Multi-word Dates (@next week, @in 2 weeks) ---
            if (word.starts_with('@')
                || word.starts_with('^')
                || word.starts_with("due:")
                || word.starts_with("start:"))
                && (i + 1 < tokens.len())
            {
                let prefix_char = word.chars().next().unwrap_or(' ');
                let is_start = prefix_char == '^' || word.starts_with("start:");
                let clean_word = if is_start {
                    word.trim_start_matches("start:").trim_start_matches('^')
                } else {
                    word.trim_start_matches("due:").trim_start_matches('@')
                };

                // @next week
                if clean_word == "next" {
                    let next_unit = tokens[i + 1];
                    if let Some(dt) = parse_next_date(next_unit, !is_start) {
                        if is_start {
                            self.dtstart = Some(dt);
                        } else {
                            self.due = Some(dt);
                        }
                        i += 2;
                        continue;
                    }
                }
                // @in 2 weeks
                else if clean_word == "in" && i + 2 < tokens.len() {
                    let amount_str = tokens[i + 1];
                    let unit_str = tokens[i + 2];
                    if let Some(amount) = parse_english_number(amount_str)
                        && let Some(dt) = parse_in_date(amount, unit_str, !is_start) {
                            if is_start {
                                self.dtstart = Some(dt);
                            } else {
                                self.due = Some(dt);
                            }
                            i += 3;
                            continue;
                        }
                }
            }

            if let Some(val) = word.strip_prefix("due:").or_else(|| word.strip_prefix('@'))
                && let Some(dt) = parse_smart_date(val, true)
            {
                self.due = Some(dt);
                i += 1;
                continue;
            }

            if let Some(val) = word
                .strip_prefix("start:")
                .or_else(|| word.strip_prefix('^'))
                && let Some(dt) = parse_smart_date(val, false)
            {
                self.dtstart = Some(dt);
                i += 1;
                continue;
            }

            summary_words.push(word);
            i += 1;
        }
        self.summary = summary_words.join(" ");
    }

    pub fn to_smart_string(&self) -> String {
        let mut s = self.summary.clone();

        if self.priority > 0 {
            s.push_str(&format!(" !{}", self.priority));
        }

        if let Some(start) = self.dtstart {
            s.push_str(&format!(" ^{}", start.format("%Y-%m-%d")));
        }

        if let Some(d) = self.due {
            s.push_str(&format!(" @{}", d.format("%Y-%m-%d")));
        }

        if let Some(mins) = self.estimated_duration {
            let dur_str = if mins >= 525600 {
                format!("~{}y", mins / 525600)
            } else if mins >= 43200 {
                format!("~{}mo", mins / 43200)
            } else if mins >= 10080 {
                format!("~{}w", mins / 10080)
            } else if mins >= 1440 {
                format!("~{}d", mins / 1440)
            } else if mins >= 60 {
                format!("~{}h", mins / 60)
            } else {
                format!("~{}m", mins)
            };
            s.push_str(&format!(" {}", dur_str));
        }

        if let Some(r) = &self.rrule {
            if r == "FREQ=DAILY" {
                s.push_str(" @daily");
            } else if r == "FREQ=WEEKLY" {
                s.push_str(" @weekly");
            } else if r == "FREQ=MONTHLY" {
                s.push_str(" @monthly");
            } else if r == "FREQ=YEARLY" {
                s.push_str(" @yearly");
            } else if let Some(simple) = reconstruct_simple_rrule(r) {
                s.push_str(&format!(" {}", simple));
            } else {
                s.push_str(" rec:custom");
            }
        }

        for cat in &self.categories {
            s.push_str(&format!(" #{}", cat));
        }
        s
    }
}

pub fn extract_inline_aliases(input: &str) -> (String, HashMap<String, Vec<String>>) {
    let mut cleaned_words = Vec::new();
    let mut new_aliases = HashMap::new();

    for token in input.split_whitespace() {
        if token.starts_with('#')
            && token.contains('=')
            && let Some((left, right)) = token.split_once('=')
        {
            let alias_key = left.trim_start_matches('#').to_string();
            if !alias_key.is_empty() && !right.is_empty() {
                let tags: Vec<String> = right
                    .split(',')
                    .map(|t| t.trim().trim_start_matches('#').to_string())
                    .filter(|t| !t.is_empty())
                    .collect();

                if !tags.is_empty() {
                    new_aliases.insert(alias_key.clone(), tags);
                    cleaned_words.push(left.to_string());
                    continue;
                }
            }
        }
        cleaned_words.push(token.to_string());
    }

    (cleaned_words.join(" "), new_aliases)
}

fn reconstruct_simple_rrule(rrule: &str) -> Option<String> {
    let parts: HashMap<&str, &str> = rrule.split(';').filter_map(|s| s.split_once('=')).collect();

    let freq = parts.get("FREQ")?;
    let interval = parts.get("INTERVAL").unwrap_or(&"1");

    let unit = match *freq {
        "DAILY" => "days",
        "WEEKLY" => "weeks",
        "MONTHLY" => "months",
        "YEARLY" => "years",
        _ => return None,
    };

    Some(format!("@every {} {}", interval, unit))
}

fn parse_duration(val: &str) -> Option<u32> {
    let lower = val.to_lowercase();
    if let Some(n) = lower.strip_suffix("min") {
        return n.parse::<u32>().ok();
    }
    if let Some(n) = lower.strip_suffix('m') {
        return n.parse::<u32>().ok();
    } else if let Some(n) = lower.strip_suffix('h') {
        return n.parse::<u32>().ok().map(|h| h * 60);
    } else if let Some(n) = lower.strip_suffix('d') {
        return n.parse::<u32>().ok().map(|d| d * 24 * 60);
    } else if let Some(n) = lower.strip_suffix('w') {
        return n.parse::<u32>().ok().map(|w| w * 7 * 24 * 60);
    } else if let Some(n) = lower.strip_suffix("mo") {
        return n.parse::<u32>().ok().map(|mo| mo * 30 * 24 * 60);
    } else if let Some(n) = lower.strip_suffix('y') {
        return n.parse::<u32>().ok().map(|y| y * 365 * 24 * 60);
    }
    None
}

fn parse_recurrence(val: &str) -> Option<String> {
    match val {
        "daily" => Some("FREQ=DAILY".to_string()),
        "weekly" => Some("FREQ=WEEKLY".to_string()),
        "monthly" => Some("FREQ=MONTHLY".to_string()),
        "yearly" => Some("FREQ=YEARLY".to_string()),
        _ => None,
    }
}

fn parse_freq_unit(unit: &str) -> &'static str {
    let u = unit.to_lowercase();
    if u.starts_with("day") {
        "DAILY"
    } else if u.starts_with("week") {
        "WEEKLY"
    } else if u.starts_with("month") {
        "MONTHLY"
    } else if u.starts_with("year") {
        "YEARLY"
    } else {
        ""
    }
}

fn parse_smart_date(val: &str, end_of_day: bool) -> Option<DateTime<Utc>> {
    if let Ok(date) = NaiveDate::parse_from_str(val, "%Y-%m-%d") {
        return finalize_date(date, end_of_day);
    }

    let now = Local::now().date_naive();

    if val == "today" {
        return finalize_date(now, end_of_day);
    }
    if val == "tomorrow" {
        return finalize_date(now + Duration::days(1), end_of_day);
    }

    if let Some(n) = val.strip_suffix('d').and_then(|s| s.parse::<i64>().ok()) {
        return finalize_date(now + Duration::days(n), end_of_day);
    }
    if let Some(n) = val.strip_suffix('w').and_then(|s| s.parse::<i64>().ok()) {
        return finalize_date(now + Duration::days(n * 7), end_of_day);
    }
    if let Some(n) = val.strip_suffix("mo").and_then(|s| s.parse::<i64>().ok()) {
        return finalize_date(now + Duration::days(n * 30), end_of_day);
    }
    if let Some(n) = val.strip_suffix('y').and_then(|s| s.parse::<i64>().ok()) {
        return finalize_date(now + Duration::days(n * 365), end_of_day);
    }

    None
}

// Logic for "@next week", "@next month", etc.
fn parse_next_date(unit: &str, end_of_day: bool) -> Option<DateTime<Utc>> {
    let now = Local::now().date_naive();
    match unit.to_lowercase().as_str() {
        "week" => finalize_date(now + Duration::days(7), end_of_day),
        "month" => finalize_date(now + Duration::days(30), end_of_day),
        "year" => finalize_date(now + Duration::days(365), end_of_day),
        // Simple day matching
        "monday" => next_weekday(now, chrono::Weekday::Mon, end_of_day),
        "tuesday" => next_weekday(now, chrono::Weekday::Tue, end_of_day),
        "wednesday" => next_weekday(now, chrono::Weekday::Wed, end_of_day),
        "thursday" => next_weekday(now, chrono::Weekday::Thu, end_of_day),
        "friday" => next_weekday(now, chrono::Weekday::Fri, end_of_day),
        "saturday" => next_weekday(now, chrono::Weekday::Sat, end_of_day),
        "sunday" => next_weekday(now, chrono::Weekday::Sun, end_of_day),
        _ => None,
    }
}

// Logic for "@in X units"
fn parse_in_date(amount: u32, unit: &str, end_of_day: bool) -> Option<DateTime<Utc>> {
    let now = Local::now().date_naive();
    let days = match unit.to_lowercase().as_str() {
        "d" | "day" | "days" => amount as i64,
        "w" | "week" | "weeks" => amount as i64 * 7,
        "mo" | "month" | "months" => amount as i64 * 30,
        "y" | "year" | "years" => amount as i64 * 365,
        _ => return None,
    };
    finalize_date(now + Duration::days(days), end_of_day)
}

fn parse_english_number(s: &str) -> Option<u32> {
    match s.to_lowercase().as_str() {
        "one" | "1" => Some(1),
        "two" | "2" => Some(2),
        "three" | "3" => Some(3),
        "four" | "4" => Some(4),
        "five" | "5" => Some(5),
        "six" | "6" => Some(6),
        "seven" | "7" => Some(7),
        "eight" | "8" => Some(8),
        "nine" | "9" => Some(9),
        "ten" | "10" => Some(10),
        "eleven" | "11" => Some(11),
        "twelve" | "12" => Some(12),
        _ => s.parse::<u32>().ok(),
    }
}

fn next_weekday(
    from: NaiveDate,
    target: chrono::Weekday,
    end_of_day: bool,
) -> Option<DateTime<Utc>> {
    let mut d = from + Duration::days(1);
    while d.weekday() != target {
        d += Duration::days(1);
    }
    finalize_date(d, end_of_day)
}

fn finalize_date(d: NaiveDate, end_of_day: bool) -> Option<DateTime<Utc>> {
    let t = if end_of_day {
        d.and_hms_opt(23, 59, 59)?
    } else {
        d.and_hms_opt(0, 0, 0)?
    };

    // Interpret as Local, then convert to UTC
    match Local.from_local_datetime(&t) {
        chrono::LocalResult::Single(dt) => Some(dt.with_timezone(&Utc)),
        chrono::LocalResult::Ambiguous(dt1, _) => Some(dt1.with_timezone(&Utc)),
        chrono::LocalResult::None => {
            // Fallback for invalid local times
            Some(t.and_utc())
        }
    }
}
