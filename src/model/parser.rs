// File: src/model/parser.rs
use crate::model::{Alarm, DateType, Task};
use chrono::{Datelike, Duration, Local, NaiveDate, NaiveTime, Utc};
use std::collections::{HashMap, HashSet};

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum SyntaxType {
    Text,
    Priority,
    DueDate,
    StartDate,
    Recurrence,
    Duration,
    Tag,
    Location,
    Url,
    Geo,
    Description,
    Reminder,
}

#[derive(Debug)]
pub struct SyntaxToken {
    pub kind: SyntaxType,
    pub start: usize,
    pub end: usize,
}

pub fn extract_inline_aliases(input: &str) -> (String, HashMap<String, Vec<String>>) {
    let parts = split_input_respecting_quotes(input);
    let mut cleaned_words = Vec::new();
    let mut new_aliases = HashMap::new();

    for (_, _, token) in parts {
        if token.starts_with('#')
            && token.contains(":=")
            && let Some((left, right)) = token.split_once(":=")
        {
            let key = strip_quotes(left.trim_start_matches('#'));
            let tags: Vec<String> = right
                .split(',')
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .collect();
            if !key.is_empty() && !tags.is_empty() {
                new_aliases.insert(key, tags);
                cleaned_words.push(left.to_string());
                continue;
            }
        }
        cleaned_words.push(token);
    }
    (cleaned_words.join(" "), new_aliases)
}

pub fn validate_alias_integrity(
    new_key: &str,
    new_values: &[String],
    current_aliases: &HashMap<String, Vec<String>>,
) -> Result<(), String> {
    if new_values
        .iter()
        .any(|v| strip_quotes(v.trim_start_matches('#')) == new_key)
    {
        return Err(format!("Alias '#{}' cannot refer to itself.", new_key));
    }
    let mut stack: Vec<String> = new_values
        .iter()
        .filter(|t| t.starts_with('#'))
        .map(|t| strip_quotes(t.trim_start_matches('#')))
        .collect();

    let mut visited_path = HashSet::new();

    while let Some(current_ref) = stack.pop() {
        if current_ref == new_key {
            return Err(format!(
                "Circular dependency: '#{}' leads back to itself.",
                new_key
            ));
        }
        if visited_path.contains(&current_ref) {
            continue;
        }
        visited_path.insert(current_ref.clone());

        if let Some(children) = current_aliases.get(&current_ref) {
            for child in children {
                if child.starts_with('#') {
                    stack.push(strip_quotes(child.trim_start_matches('#')));
                }
            }
        }
    }
    Ok(())
}

fn parse_time_string(s: &str) -> Option<NaiveTime> {
    let lower = s.to_lowercase();

    // Helper for 12h
    let parse_12h = |s: &str, is_pm: bool| -> Option<NaiveTime> {
        let (h, m) = if let Some((h_str, m_str)) = s.split_once(':') {
            (h_str.parse::<u32>().ok()?, m_str.parse::<u32>().ok()?)
        } else {
            (s.parse::<u32>().ok()?, 0)
        };
        if !(1..=12).contains(&h) || m > 59 {
            return None;
        }
        let h_24 = if h == 12 {
            if is_pm { 12 } else { 0 }
        } else if is_pm {
            h + 12
        } else {
            h
        };
        NaiveTime::from_hms_opt(h_24, m, 0)
    };

    if let Some(stripped) = lower.strip_suffix("am") {
        return parse_12h(stripped, false);
    }
    if let Some(stripped) = lower.strip_suffix("pm") {
        return parse_12h(stripped, true);
    }

    if let Some((h_str, m_str)) = lower.split_once(':') {
        let h = h_str.parse::<u32>().ok()?;
        let m = m_str.parse::<u32>().ok()?;
        return NaiveTime::from_hms_opt(h, m, 0);
    }

    None
}

fn is_time_format(s: &str) -> bool {
    parse_time_string(s).is_some()
}

pub fn tokenize_smart_input(input: &str) -> Vec<SyntaxToken> {
    let mut tokens = Vec::new();
    let words = split_input_respecting_quotes(input);

    let mut cursor = 0;
    let mut i = 0;

    while i < words.len() {
        let (start, end, word) = &words[i];
        if *start > cursor {
            tokens.push(SyntaxToken {
                kind: SyntaxType::Text,
                start: cursor,
                end: *start,
            });
        }

        let mut matched_kind = None;
        let mut words_consumed = 1;

        let word_lower = word.to_lowercase();

        // 1. Recurrence
        if (word == "@every" || word == "rec:every") && i + 1 < words.len() {
            let next_token_str = words[i + 1].2.as_str();
            let next_next = if i + 2 < words.len() {
                Some(words[i + 2].2.as_str())
            } else {
                None
            };

            if let Some((_, _, consumed)) = parse_amount_and_unit(next_token_str, next_next, false)
            {
                matched_kind = Some(SyntaxType::Recurrence);
                words_consumed = 1 + 1 + consumed;
            } else if parse_weekday_code(next_token_str).is_some() {
                matched_kind = Some(SyntaxType::Recurrence);
                words_consumed = 2;
            }
        }

        // 2. Dates
        if matched_kind.is_none() {
            let (is_start, clean_word) = if let Some(val) = word
                .strip_prefix("start:")
                .or_else(|| word.strip_prefix('^'))
            {
                (true, val)
            } else if let Some(val) = word.strip_prefix("due:").or_else(|| word.strip_prefix('@')) {
                (false, val)
            } else {
                (false, "")
            };

            if !clean_word.is_empty() {
                // Check "next friday"
                if clean_word == "next" && i + 1 < words.len() {
                    let next_str = words[i + 1].2.as_str();
                    let is_weekday = parse_weekday_code(next_str).is_some();
                    if is_date_unit_full(next_str) || is_weekday {
                        matched_kind = Some(if is_start {
                            SyntaxType::StartDate
                        } else {
                            SyntaxType::DueDate
                        });
                        words_consumed = 2;
                        // Peek ahead for time (e.g. @next friday 2pm)
                        if i + 2 < words.len() && is_time_format(&words[i + 2].2) {
                            words_consumed += 1;
                        }
                    }
                }
                // Check "in 2 days"
                else if clean_word == "in" && i + 1 < words.len() {
                    let next_token_str = words[i + 1].2.as_str();
                    let next_next = if i + 2 < words.len() {
                        Some(words[i + 2].2.as_str())
                    } else {
                        None
                    };

                    if let Some((_, _, consumed)) =
                        parse_amount_and_unit(next_token_str, next_next, false)
                    {
                        matched_kind = Some(if is_start {
                            SyntaxType::StartDate
                        } else {
                            SyntaxType::DueDate
                        });
                        words_consumed = 1 + 1 + consumed;
                        // Peek ahead for time
                        if i + words_consumed < words.len()
                            && is_time_format(&words[i + words_consumed].2)
                        {
                            words_consumed += 1;
                        }
                    }
                }
                // Check standard dates (@tomorrow, @2025-01-01)
                else {
                    // Just basic check if parser accepts it
                    if parse_smart_date(clean_word).is_some()
                        || parse_weekday_code(clean_word).is_some()
                        || parse_time_string(clean_word).is_some()
                    {
                        matched_kind = Some(if is_start {
                            SyntaxType::StartDate
                        } else {
                            SyntaxType::DueDate
                        });
                        // Peek ahead for time
                        if i + 1 < words.len() && is_time_format(&words[i + 1].2) {
                            words_consumed = 2;
                        }
                    }
                }
            }
        }

        // 3. Reminders (rem:10m, rem:8am, rem:tomorrow 16:00, rem:in 5m)
        if matched_kind.is_none() && word_lower.starts_with("rem:") {
            matched_kind = Some(SyntaxType::Reminder);

            let clean_val = if word.len() > 4 { &word[4..] } else { "" };

            // Helper to skip whitespace and get next non-whitespace token
            let find_next_token = |start_idx: usize| -> Option<usize> {
                (start_idx..words.len()).find(|&idx| !words[idx].2.trim().is_empty())
            };

            // Check for "rem:in 5m" pattern
            if clean_val.eq_ignore_ascii_case("in") {
                if let Some(next_idx) = find_next_token(i + 1) {
                    let next_token_str = words[next_idx].2.as_str();
                    let next_next_idx = find_next_token(next_idx + 1);
                    let next_next = next_next_idx.map(|idx| words[idx].2.as_str());

                    if let Some((_, _, consumed)) =
                        parse_amount_and_unit(next_token_str, next_next, false)
                    {
                        let last_idx = if consumed > 0 {
                            next_next_idx.unwrap_or(next_idx)
                        } else {
                            next_idx
                        };
                        words_consumed = last_idx - i + 1;
                    }
                }
            }
            // Check for date keywords (rem:tomorrow, rem:today, etc.)
            else if !clean_val.is_empty() && parse_smart_date(clean_val).is_some() {
                // Look ahead for time (rem:tomorrow 16:00)
                if let Some(next_idx) = find_next_token(i + 1)
                    && is_time_format(&words[next_idx].2)
                {
                    words_consumed = next_idx - i + 1;
                }
            }
            // Check for "rem: tomorrow 16:00" (space after colon)
            else if clean_val.is_empty()
                && let Some(next_idx) = find_next_token(i + 1)
            {
                let next_word = &words[next_idx].2;

                // Check if next word is "in" for "rem: in 5m"
                if next_word.eq_ignore_ascii_case("in") {
                    if let Some(amount_idx) = find_next_token(next_idx + 1) {
                        let next_token_str = words[amount_idx].2.as_str();
                        let next_next_idx = find_next_token(amount_idx + 1);
                        let next_next = next_next_idx.map(|idx| words[idx].2.as_str());

                        if let Some((_, _, consumed)) =
                            parse_amount_and_unit(next_token_str, next_next, false)
                        {
                            let last_idx = if consumed > 0 {
                                next_next_idx.unwrap_or(amount_idx)
                            } else {
                                amount_idx
                            };
                            words_consumed = last_idx - i + 1;
                        }
                    }
                }
                // Check if next word is a date keyword
                else if parse_smart_date(next_word).is_some() {
                    words_consumed = next_idx - i + 1;
                    // Look ahead for time
                    if let Some(time_idx) = find_next_token(next_idx + 1)
                        && is_time_format(&words[time_idx].2)
                    {
                        words_consumed = time_idx - i + 1;
                    }
                }
                // Or just a duration/time
                else if parse_duration(next_word).is_some() || is_time_format(next_word) {
                    words_consumed = next_idx - i + 1;
                }
            }
        }

        // 4. Single tokens
        if matched_kind.is_none() {
            if word.starts_with("@@") || word_lower.starts_with("loc:") {
                matched_kind = Some(SyntaxType::Location);
            } else if word_lower.starts_with("url:")
                || (word.starts_with("[[") && word.ends_with("]]"))
            {
                matched_kind = Some(SyntaxType::Url);
            } else if word_lower.starts_with("geo:") {
                matched_kind = Some(SyntaxType::Geo);
                if word.ends_with(',') && i + 1 < words.len() {
                    // consume next if coordinate split by space
                    words_consumed = 2;
                }
            } else if word_lower.starts_with("desc:") {
                matched_kind = Some(SyntaxType::Description);
            } else if word.starts_with('!') && word.len() > 1 && word[1..].parse::<u8>().is_ok() {
                matched_kind = Some(SyntaxType::Priority);
            } else if word.starts_with('~') || word_lower.starts_with("est:") {
                matched_kind = Some(SyntaxType::Duration);
            } else if word.starts_with('#') {
                matched_kind = Some(SyntaxType::Tag);
            }
        }

        if let Some(kind) = matched_kind {
            let final_end = words[i + words_consumed - 1].1;
            tokens.push(SyntaxToken {
                kind,
                start: *start,
                end: final_end,
            });
            cursor = final_end;
            i += words_consumed;
        } else {
            tokens.push(SyntaxToken {
                kind: SyntaxType::Text,
                start: *start,
                end: *end,
            });
            cursor = *end;
            i += 1;
        }
    }
    if cursor < input.len() {
        tokens.push(SyntaxToken {
            kind: SyntaxType::Text,
            start: cursor,
            end: input.len(),
        });
    }
    tokens
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(next) = chars.next() {
                out.push(next);
            } else {
                out.push('\\');
            }
        } else {
            out.push(c);
        }
    }
    out
}

pub fn strip_quotes(s: &str) -> String {
    let s = s.trim();
    if s.len() >= 2
        && ((s.starts_with('"') && s.ends_with('"')) || (s.starts_with('{') && s.ends_with('}')))
    {
        unescape(&s[1..s.len() - 1])
    } else {
        unescape(s)
    }
}

pub fn quote_value(s: &str) -> String {
    if s.contains(' ') || s.contains('"') || s.contains('\\') || s.contains('#') || s.is_empty() {
        let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
        format!("\"{}\"", escaped)
    } else {
        s.to_string()
    }
}

fn split_input_respecting_quotes(input: &str) -> Vec<(usize, usize, String)> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut start_idx = 0;
    let mut in_quote = false;
    let mut in_brace = false;
    let mut escaped = false;
    let chars = input.char_indices().peekable();

    for (idx, c) in chars {
        if current.is_empty() && !in_quote && !in_brace && !c.is_whitespace() {
            start_idx = idx;
        }
        if escaped {
            current.push(c);
            escaped = false;
            continue;
        }
        match c {
            '\\' => {
                escaped = true;
                current.push('\\');
            }
            '"' if !in_brace => {
                in_quote = !in_quote;
                current.push(c);
            }
            '{' if !in_quote => {
                in_brace = true;
                current.push(c);
            }
            '}' if !in_quote => {
                in_brace = false;
                current.push(c);
            }
            ws if ws.is_whitespace() && !in_quote && !in_brace => {
                if !current.is_empty() {
                    parts.push((start_idx, idx, current.clone()));
                    current.clear();
                }
            }
            _ => current.push(c),
        }
    }
    if !current.is_empty() {
        parts.push((start_idx, input.len(), current));
    }
    parts
}

fn collect_alias_expansions(
    token: &str,
    aliases: &HashMap<String, Vec<String>>,
    visited: &mut HashSet<String>,
) -> Vec<String> {
    let mut results = Vec::new();
    if token.starts_with('#') {
        let key = strip_quotes(token.trim_start_matches('#'));
        let mut search = key.as_str();
        let mut found_values = None;
        let mut matched_key = String::new();
        loop {
            if let Some(vals) = aliases.get(search) {
                found_values = Some(vals);
                matched_key = search.to_string();
                break;
            }
            if let Some(idx) = search.rfind(':') {
                search = &search[..idx];
            } else {
                break;
            }
        }
        if let Some(values) = found_values {
            if visited.contains(&matched_key) {
                return results;
            }
            visited.insert(matched_key);
            for val in values {
                let child_expansions = collect_alias_expansions(val, aliases, visited);
                results.extend(child_expansions);
                results.push(val.clone());
            }
        }
    }
    results
}

// --- DATE PARSING HELPERS ---

fn is_date_unit_full(s: &str) -> bool {
    let lower = s.to_lowercase();
    matches!(
        lower.as_str(),
        "day" | "days" | "week" | "weeks" | "month" | "months" | "year" | "years"
    )
}
fn is_date_unit_short(s: &str) -> bool {
    let lower = s.to_lowercase();
    matches!(
        lower.as_str(),
        "m" | "min"
            | "minute"
            | "minutes"
            | "h"
            | "hour"
            | "hours"
            | "d"
            | "day"
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
fn parse_amount_and_unit(
    first: &str,
    second: Option<&str>,
    strict_unit: bool,
) -> Option<(u32, String, usize)> {
    if let Some(next_token) = second
        && let Some(amt) = parse_english_number(first)
    {
        let unit = next_token.to_lowercase();
        let is_valid = if strict_unit {
            is_date_unit_full(&unit)
        } else {
            is_date_unit_short(&unit)
        };
        if is_valid {
            return Some((amt, unit, 1));
        }
    }
    if !strict_unit {
        let lower = first.to_lowercase();
        let (amt_str, unit_str) = if let Some(idx) = lower.find(|c: char| !c.is_numeric()) {
            lower.split_at(idx)
        } else {
            return None;
        };
        if let Ok(amt) = amt_str.parse::<u32>()
            && is_date_unit_short(unit_str)
        {
            return Some((amt, unit_str.to_string(), 0));
        }
    }
    None
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
fn parse_freq_from_unit(u: &str) -> &'static str {
    let s = u.to_lowercase();
    if s.starts_with('d') {
        "DAILY"
    } else if s.starts_with('w') {
        "WEEKLY"
    } else if s.starts_with("mo") {
        "MONTHLY"
    } else if s.starts_with('y') {
        "YEARLY"
    } else {
        ""
    }
}

pub fn prettify_recurrence(rrule: &str) -> String {
    match rrule {
        "FREQ=DAILY" => "@daily".to_string(),
        "FREQ=WEEKLY" => "@weekly".to_string(),
        "FREQ=MONTHLY" => "@monthly".to_string(),
        "FREQ=YEARLY" => "@yearly".to_string(),
        _ => {
            let mut freq = "";
            let mut interval = "";
            let mut byday = "";

            for part in rrule.split(';') {
                if let Some(v) = part.strip_prefix("FREQ=") {
                    freq = v;
                } else if let Some(v) = part.strip_prefix("INTERVAL=") {
                    interval = v;
                } else if let Some(v) = part.strip_prefix("BYDAY=") {
                    byday = v;
                }
            }

            if !byday.is_empty() && freq == "WEEKLY" {
                let day_name = match byday {
                    "MO" => "monday",
                    "TU" => "tuesday",
                    "WE" => "wednesday",
                    "TH" => "thursday",
                    "FR" => "friday",
                    "SA" => "saturday",
                    "SU" => "sunday",
                    _ => "",
                };
                if !day_name.is_empty() {
                    return format!("@every {}", day_name);
                }
            }

            if !freq.is_empty() && !interval.is_empty() {
                let unit = match freq {
                    "DAILY" => "days",
                    "WEEKLY" => "weeks",
                    "MONTHLY" => "months",
                    "YEARLY" => "years",
                    _ => "",
                };
                if !unit.is_empty() {
                    return format!("@every {} {}", interval, unit);
                }
            }
            format!("rec:{}", rrule)
        }
    }
}

pub fn parse_duration(val: &str) -> Option<u32> {
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
    match val.to_lowercase().as_str() {
        "daily" => Some("FREQ=DAILY".to_string()),
        "weekly" => Some("FREQ=WEEKLY".to_string()),
        "monthly" => Some("FREQ=MONTHLY".to_string()),
        "yearly" => Some("FREQ=YEARLY".to_string()),
        _ => {
            if val.to_uppercase().starts_with("FREQ=") {
                Some(val.to_string())
            } else {
                None
            }
        }
    }
}

fn parse_smart_date(val: &str) -> Option<NaiveDate> {
    if let Ok(date) = NaiveDate::parse_from_str(val, "%Y-%m-%d") {
        return Some(date);
    }
    let now = Local::now().date_naive();
    let lower = val.to_lowercase();
    if lower == "today" {
        return Some(now);
    }
    if lower == "tomorrow" {
        return Some(now + Duration::days(1));
    }
    if let Some(n) = lower.strip_suffix('d').and_then(|s| s.parse::<i64>().ok()) {
        return Some(now + Duration::days(n));
    }
    if let Some(n) = lower.strip_suffix('w').and_then(|s| s.parse::<i64>().ok()) {
        return Some(now + Duration::days(n * 7));
    }
    if let Some(n) = lower.strip_suffix("mo").and_then(|s| s.parse::<i64>().ok()) {
        return Some(now + Duration::days(n * 30));
    }
    if let Some(n) = lower.strip_suffix('y').and_then(|s| s.parse::<i64>().ok()) {
        return Some(now + Duration::days(n * 365));
    }
    None
}

fn parse_weekday_code(s: &str) -> Option<&'static str> {
    match s.to_lowercase().as_str() {
        "mo" | "mon" | "monday" => Some("MO"),
        "tu" | "tue" | "tuesday" => Some("TU"),
        "we" | "wed" | "wednesday" => Some("WE"),
        "th" | "thu" | "thursday" => Some("TH"),
        "fr" | "fri" | "friday" => Some("FR"),
        "sa" | "sat" | "saturday" => Some("SA"),
        "su" | "sun" | "sunday" => Some("SU"),
        _ => None,
    }
}

fn parse_weekday_date(s: &str) -> Option<NaiveDate> {
    parse_next_date(s)
}

fn parse_next_date(unit: &str) -> Option<NaiveDate> {
    let now = Local::now().date_naive();
    match unit.to_lowercase().as_str() {
        "week" => Some(now + Duration::days(7)),
        "month" => Some(now + Duration::days(30)),
        "year" => Some(now + Duration::days(365)),
        _ => {
            if let Some(code) = parse_weekday_code(unit) {
                let target = match code {
                    "MO" => chrono::Weekday::Mon,
                    "TU" => chrono::Weekday::Tue,
                    "WE" => chrono::Weekday::Wed,
                    "TH" => chrono::Weekday::Thu,
                    "FR" => chrono::Weekday::Fri,
                    "SA" => chrono::Weekday::Sat,
                    "SU" => chrono::Weekday::Sun,
                    _ => return None,
                };
                return next_weekday(now, target);
            }
            None
        }
    }
}

fn parse_in_date(amount: u32, unit: &str) -> Option<NaiveDate> {
    let now = Local::now().date_naive();
    let days = match unit.to_lowercase().as_str() {
        "d" | "day" | "days" => amount as i64,
        "w" | "week" | "weeks" => amount as i64 * 7,
        "mo" | "month" | "months" => amount as i64 * 30,
        "y" | "year" | "years" => amount as i64 * 365,
        _ => return None,
    };
    Some(now + Duration::days(days))
}

fn next_weekday(from: NaiveDate, target: chrono::Weekday) -> Option<NaiveDate> {
    let mut d = from + Duration::days(1);
    while d.weekday() != target {
        d += Duration::days(1);
    }
    Some(d)
}

// Helper to look ahead for a time string and merge it
fn finalize_date_token(
    d: NaiveDate,
    stream: &[String],
    next_idx: usize,
    consumed: &mut usize,
) -> DateType {
    if next_idx < stream.len() {
        let next_token = &stream[next_idx];
        if let Some(t) = parse_time_string(next_token) {
            *consumed += 1;
            let local_dt = d
                .and_time(t)
                .and_local_timezone(Local)
                .unwrap()
                .with_timezone(&Utc);
            return DateType::Specific(local_dt);
        }
    }
    DateType::AllDay(d)
}

pub fn escape_summary(summary: &str) -> String {
    let mut escaped_words = Vec::new();
    for word in summary.split_whitespace() {
        if is_special_token(word) {
            escaped_words.push(format!("\\{}", word));
        } else {
            escaped_words.push(word.to_string());
        }
    }
    escaped_words.join(" ")
}

fn is_special_token(word: &str) -> bool {
    let lower = word.to_lowercase();
    if word.starts_with('@')
        || word.starts_with('#')
        || word.starts_with('!')
        || word.starts_with('^')
        || word.starts_with('~')
    {
        return true;
    }
    if lower.starts_with("loc:")
        || lower.starts_with("url:")
        || lower.starts_with("geo:")
        || lower.starts_with("desc:")
        || lower.starts_with("due:")
        || lower.starts_with("start:")
        || lower.starts_with("rec:")
        || lower.starts_with("est:")
        || lower.starts_with("rem:")
    {
        return true;
    }
    // FIX: Removed matches!(lower.as_str(), "today" | "tomorrow")
    // Bare words should be treated as text.
    false
}

pub fn apply_smart_input(
    task: &mut Task,
    input: &str,
    aliases: &HashMap<String, Vec<String>>,
    default_reminder_time: Option<NaiveTime>,
) {
    let mut summary_words = Vec::new();
    // Reset fields
    task.priority = 0;
    task.due = None;
    task.dtstart = None;
    task.rrule = None;
    task.estimated_duration = None;
    task.location = None;
    task.url = None;
    task.geo = None;
    task.categories.clear();
    task.alarms.clear(); // Reset alarms

    let user_tokens: Vec<String> = split_input_respecting_quotes(input)
        .into_iter()
        .map(|(_, _, s)| s)
        .collect();

    let mut background_tokens = Vec::new();
    let mut visited = HashSet::new();

    for token in &user_tokens {
        let expanded = collect_alias_expansions(token, aliases, &mut visited);
        background_tokens.extend(expanded);
    }

    let mut stream = background_tokens;
    stream.extend(user_tokens);

    let mut i = 0;
    while i < stream.len() {
        let token = &stream[i];
        let mut consumed = 1;
        let token_lower = token.to_lowercase();

        // 1. Recurrence
        if (token == "rec:every" || token == "@every") && i + 1 < stream.len() {
            let next_token_str = stream[i + 1].as_str();
            let next_next = if i + 2 < stream.len() {
                Some(stream[i + 2].as_str())
            } else {
                None
            };
            if let Some((interval, unit, extra_consumed)) =
                parse_amount_and_unit(next_token_str, next_next, false)
            {
                let freq = parse_freq_from_unit(&unit);
                if !freq.is_empty() {
                    task.rrule = Some(format!("FREQ={};INTERVAL={}", freq, interval));
                    consumed = 1 + 1 + extra_consumed;
                } else {
                    summary_words.push(unescape(token));
                }
            } else if let Some(byday) = parse_weekday_code(next_token_str) {
                task.rrule = Some(format!("FREQ=WEEKLY;BYDAY={}", byday));
                consumed = 2;
            } else {
                summary_words.push(unescape(token));
            }
        }
        // 2. Reminders (rem:) -> UPDATED LOGIC
        else if let Some(val) = token_lower.strip_prefix("rem:") {
            let clean_val = if val.is_empty() && i + 1 < stream.len() {
                // Handle "rem: 10m" (space after colon)
                consumed += 1;
                &stream[i + 1]
            } else {
                // Handle "rem:10m"
                if token.len() > 4 { &token[4..] } else { "" }
            };

            // NEW: Handle "rem:in 5m" or "rem: in 5m"
            if clean_val.eq_ignore_ascii_case("in") && i + consumed < stream.len() {
                let next_str = &stream[i + consumed];
                let next_next = if i + consumed + 1 < stream.len() {
                    Some(stream[i + consumed + 1].as_str())
                } else {
                    None
                };

                // Reuse existing duration parser helper
                if let Some((amt, unit, extra)) = parse_amount_and_unit(next_str, next_next, false)
                {
                    // 1. Calculate Duration in minutes
                    let mins = match unit.as_str() {
                        "d" | "day" | "days" => amt * 1440,
                        "h" | "hour" | "hours" => amt * 60,
                        _ => amt, // assume minutes for m/min or bare numbers
                    };

                    // 2. Create ABSOLUTE alarm (Now + Duration)
                    let now = Local::now();
                    let target = now + Duration::minutes(mins as i64);
                    let target_utc = target.with_timezone(&Utc);

                    task.alarms.push(Alarm::new_absolute(target_utc));

                    consumed += 1 + extra; // Consume amount + unit tokens
                } else {
                    summary_words.push(unescape(token));
                }
            } else if !clean_val.is_empty() {
                // A. Duration (rem:10m)
                if let Some(d) = parse_duration(clean_val) {
                    task.alarms.push(Alarm::new_relative(d));
                }
                // B. Time Only (rem:8pm) -> Today at 8pm
                else if let Some(t) = parse_time_string(clean_val) {
                    let now = Local::now().date_naive();
                    let dt = now
                        .and_time(t)
                        .and_local_timezone(Local)
                        .unwrap()
                        .with_timezone(&Utc);
                    task.alarms.push(Alarm::new_absolute(dt));
                }
                // C. Date + Optional Time (rem:2025-12-27 12:30, rem:tomorrow 9am)
                else if let Some(d) = parse_smart_date(clean_val) {
                    // Look ahead for time
                    let mut time_part = None;

                    // FIX: Check `i + consumed` (next token) for time string
                    if i + consumed < stream.len() {
                        let potential_time = &stream[i + consumed];
                        if let Some(t) = parse_time_string(potential_time) {
                            time_part = Some(t);
                            consumed += 1;
                        }
                    }

                    // USE CONFIG OR DEFAULT TO 08:00
                    let fallback = default_reminder_time
                        .unwrap_or_else(|| NaiveTime::from_hms_opt(8, 0, 0).unwrap());
                    let t = time_part.unwrap_or(fallback);

                    let dt = d
                        .and_time(t)
                        .and_local_timezone(Local)
                        .unwrap()
                        .with_timezone(&Utc);
                    task.alarms.push(Alarm::new_absolute(dt));
                } else {
                    summary_words.push(unescape(token));
                }
            } else {
                summary_words.push(unescape(token));
            }
        }
        // 3. New Fields (@@, loc:, url:, geo:, desc:, !, ~)
        else if token.starts_with("@@") {
            let val = strip_quotes(token.trim_start_matches("@@"));
            if val.is_empty() {
                summary_words.push(unescape(token));
            } else {
                task.location = Some(val);
            }
        } else if token_lower.starts_with("loc:") {
            let val = strip_quotes(&token[4..]);
            if val.is_empty() {
                summary_words.push(unescape(token));
            } else {
                task.location = Some(val);
            }
        } else if token_lower.starts_with("url:") {
            task.url = Some(strip_quotes(&token[4..]));
        } else if token.starts_with("[[") && token.ends_with("]]") {
            task.url = Some(token[2..token.len() - 2].to_string());
        } else if token_lower.starts_with("geo:") {
            let mut raw_val = token[4..].to_string();
            if token.ends_with(',') && i + 1 < stream.len() {
                let next_token = &stream[i + 1];
                if next_token
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_numeric() || c == '-')
                {
                    raw_val.push_str(next_token);
                    consumed = 2;
                }
            }
            task.geo = Some(strip_quotes(&raw_val));
        } else if token_lower.starts_with("desc:") {
            let desc_val = strip_quotes(&token[5..]);
            if task.description.is_empty() {
                task.description = desc_val;
            } else {
                task.description.push_str(&format!("\n{}", desc_val));
            }
        } else if token.starts_with('#') {
            let cat = strip_quotes(token.trim_start_matches('#'));
            if !task.categories.contains(&cat) {
                task.categories.push(cat);
            }
        } else if token.starts_with('!') && token.len() > 1 {
            if let Ok(p) = token[1..].parse::<u8>() {
                task.priority = p;
            } else {
                summary_words.push(unescape(token));
            }
        } else if token.starts_with('~') || token_lower.starts_with("est:") {
            let val = strip_quotes(if let Some(s) = token.strip_prefix('~') {
                s
            } else {
                &token[4..]
            });
            if let Some(d) = parse_duration(&val) {
                task.estimated_duration = Some(d);
            } else {
                summary_words.push(unescape(token));
            }
        }
        // 4. Dates (Multi-word + Time)
        else if (token.starts_with('@')
            || token.starts_with('^')
            || token_lower.starts_with("due:")
            || token_lower.starts_with("start:"))
            && stream.get(i + 1).is_some()
        {
            let (is_start, clean) = if let Some(_v) = token
                .strip_prefix('^')
                .or_else(|| token_lower.strip_prefix("start:"))
            {
                let clean_v = if let Some(s) = token.strip_prefix('^') {
                    s
                } else {
                    &token[6..]
                };
                (true, clean_v)
            } else {
                let clean_v = if let Some(s) = token.strip_prefix('@') {
                    s
                } else if token_lower.starts_with("due:") {
                    &token[4..]
                } else {
                    ""
                };
                (false, clean_v)
            };

            let mut matched_date = false;
            // "next friday"
            if clean == "next" && stream.get(i + 1).is_some() {
                let next_str = &stream[i + 1];
                if let Some(d) = parse_next_date(next_str) {
                    let dt = finalize_date_token(d, &stream, i + 2, &mut consumed);
                    if is_start {
                        task.dtstart = Some(dt);
                    } else {
                        task.due = Some(dt);
                    }
                    consumed += 1; // consumed "next" + unit + (time?)
                    matched_date = true;
                }
            }
            // "in 2 days"
            else if clean == "in" && i + 1 < stream.len() {
                let next_token_str = stream[i + 1].as_str();
                let next_next = if i + 2 < stream.len() {
                    Some(stream[i + 2].as_str())
                } else {
                    None
                };
                if let Some((amount, unit, extra)) =
                    parse_amount_and_unit(next_token_str, next_next, false)
                    && let Some(d) = parse_in_date(amount, &unit)
                {
                    let dt = finalize_date_token(d, &stream, i + 1 + 1 + extra, &mut consumed);
                    if is_start {
                        task.dtstart = Some(dt);
                    } else {
                        task.due = Some(dt);
                    }
                    consumed += 1 + extra;
                    matched_date = true;
                }
            }

            if !matched_date {
                if let Some(d) = parse_smart_date(clean) {
                    let dt = finalize_date_token(d, &stream, i + 1, &mut consumed);
                    if is_start {
                        task.dtstart = Some(dt);
                    } else {
                        task.due = Some(dt);
                    }
                } else if let Some(t) = parse_time_string(clean) {
                    let now = Local::now().date_naive();
                    let dt = now
                        .and_time(t)
                        .and_local_timezone(Local)
                        .unwrap()
                        .with_timezone(&Utc);
                    if is_start {
                        task.dtstart = Some(DateType::Specific(dt));
                    } else {
                        task.due = Some(DateType::Specific(dt));
                    }
                } else if let Some(rrule) = parse_recurrence(clean) {
                    task.rrule = Some(rrule);
                } else if let Some(d) = parse_weekday_date(clean) {
                    let dt = finalize_date_token(d, &stream, i + 1, &mut consumed);
                    if is_start {
                        task.dtstart = Some(dt);
                    } else {
                        task.due = Some(dt);
                    }
                } else {
                    summary_words.push(unescape(token));
                }
            }
        }
        // 5. Single Word Dates
        else if let Some(val) = token
            .strip_prefix("rec:")
            .or_else(|| token.strip_prefix('@'))
        {
            // Recurrence, Date, or Weekday
            if let Some(rrule) = parse_recurrence(val) {
                task.rrule = Some(rrule);
            } else if token.starts_with("rec:") {
                if let Some((interval, unit, _)) = parse_amount_and_unit(val, None, false) {
                    let freq = parse_freq_from_unit(&unit);
                    if !freq.is_empty() {
                        task.rrule = Some(format!("FREQ={};INTERVAL={}", freq, interval));
                    } else {
                        summary_words.push(unescape(token));
                    }
                } else {
                    summary_words.push(unescape(token));
                }
            } else if let Some(d) = parse_smart_date(val) {
                let dt = finalize_date_token(d, &stream, i + 1, &mut consumed);
                task.due = Some(dt);
            } else if let Some(t) = parse_time_string(val) {
                let now = Local::now().date_naive();
                let dt = now
                    .and_time(t)
                    .and_local_timezone(Local)
                    .unwrap()
                    .with_timezone(&Utc);
                task.due = Some(DateType::Specific(dt));
            } else if let Some(d) = parse_weekday_date(val) {
                let dt = finalize_date_token(d, &stream, i + 1, &mut consumed);
                task.due = Some(dt);
            } else if let Some(_stripped) = token_lower.strip_prefix("due:") {
                let real_val = &token[4..];
                if let Some(d) = parse_smart_date(real_val) {
                    let dt = finalize_date_token(d, &stream, i + 1, &mut consumed);
                    task.due = Some(dt);
                } else {
                    summary_words.push(unescape(token));
                }
            } else {
                summary_words.push(unescape(token));
            }
        } else if token_lower.starts_with("due:") {
            let val = &token[4..];
            if let Some(d) = parse_smart_date(val) {
                let dt = finalize_date_token(d, &stream, i + 1, &mut consumed);
                task.due = Some(dt);
            } else {
                summary_words.push(unescape(token));
            }
        } else if let Some(_val) = token
            .strip_prefix('^')
            .or_else(|| token_lower.strip_prefix("start:"))
        {
            let clean_val = if let Some(s) = token.strip_prefix('^') {
                s
            } else {
                &token[6..]
            };
            if let Some(d) = parse_smart_date(clean_val) {
                let dt = finalize_date_token(d, &stream, i + 1, &mut consumed);
                task.dtstart = Some(dt);
            } else if let Some(d) = parse_weekday_date(clean_val) {
                let dt = finalize_date_token(d, &stream, i + 1, &mut consumed);
                task.dtstart = Some(dt);
            } else {
                summary_words.push(unescape(token));
            }
        } else {
            summary_words.push(unescape(token));
        }
        i += consumed;
    }

    task.summary = summary_words.join(" ");
    task.categories.sort();
    task.categories.dedup();
}
