use crate::model::item::Task;
use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, TimeZone, Utc};
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
}

#[derive(Debug)]
pub struct SyntaxToken {
    pub kind: SyntaxType,
    pub start: usize,
    pub end: usize,
}

/// Tokenizes input while preserving quoted strings.
/// Handles escaping via backslash (e.g. \" inside a string).
fn split_input_respecting_quotes(input: &str) -> Vec<(usize, usize, String)> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut start_idx = 0;
    let mut in_quote = false;
    let mut in_brace = false;
    let mut escaped = false;
    let chars = input.char_indices().peekable();

    for (idx, c) in chars {
        // Mark start of a new token
        if current.is_empty() && !in_quote && !in_brace && !c.is_whitespace() {
            start_idx = idx;
        }

        if escaped {
            current.push(c);
            escaped = false;
            continue;
        }

        match c {
            '\\' => escaped = true,
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

fn strip_quotes(s: &str) -> String {
    let s = s.trim();
    if s.len() >= 2
        && ((s.starts_with('"') && s.ends_with('"')) || (s.starts_with('{') && s.ends_with('}')))
    {
        return s[1..s.len() - 1].to_string();
    }
    s.to_string()
}

// --- MISSING HELPERS ADDED HERE ---

fn is_date_unit(s: &str) -> bool {
    let lower = s.to_lowercase();
    matches!(
        lower.as_str(),
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
    let lower = s.to_lowercase();
    matches!(
        lower.as_str(),
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

// ----------------------------------

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

        // New fields
        if word.starts_with("@@") || word.starts_with("loc:") {
            matched_kind = Some(SyntaxType::Location);
        } else if word.starts_with("url:") || (word.starts_with("[[") && word.ends_with("]]")) {
            matched_kind = Some(SyntaxType::Url);
        } else if word.starts_with("geo:") {
            matched_kind = Some(SyntaxType::Geo);
        } else if word.starts_with("desc:") {
            matched_kind = Some(SyntaxType::Description);
        }
        // Existing fields
        else if word.starts_with('!') && word.len() > 1 && word[1..].parse::<u8>().is_ok() {
            matched_kind = Some(SyntaxType::Priority);
        } else if (word.starts_with('~') || word.starts_with("est:"))
            && parse_duration(&strip_quotes(
                word.trim_start_matches("est:").trim_start_matches('~'),
            ))
            .is_some()
        {
            matched_kind = Some(SyntaxType::Duration);
        } else if word.starts_with('#') {
            matched_kind = Some(SyntaxType::Tag);
        } else if (word == "@every" || word == "rec:every") && i + 2 < words.len() {
            let amount_str = &words[i + 1].2;
            let unit_str = &words[i + 2].2;
            if amount_str.parse::<u32>().is_ok() && !parse_freq_unit(unit_str).is_empty() {
                matched_kind = Some(SyntaxType::Recurrence);
                words_consumed = 3;
            }
        } else if let Some(val) = word.strip_prefix("rec:").or_else(|| word.strip_prefix('@')) {
            if parse_recurrence(val).is_some() {
                matched_kind = Some(SyntaxType::Recurrence);
            } else if parse_smart_date(val, true).is_some() {
                matched_kind = Some(SyntaxType::DueDate);
            } else if let Some(stripped) = word.strip_prefix("due:")
                && parse_smart_date(stripped, true).is_some()
            {
                matched_kind = Some(SyntaxType::DueDate);
            }
        }

        if matched_kind.is_none()
            && let Some(val) = word
                .strip_prefix('^')
                .or_else(|| word.strip_prefix("start:"))
            && parse_smart_date(val, false).is_some()
        {
            matched_kind = Some(SyntaxType::StartDate);
        }

        // Multi-word dates
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
                if clean_word == "next" && i + 1 < words.len() {
                    if is_date_unit(&words[i + 1].2) {
                        matched_kind = Some(if is_start {
                            SyntaxType::StartDate
                        } else {
                            SyntaxType::DueDate
                        });
                        words_consumed = 2;
                    }
                } else if clean_word == "in" && i + 2 < words.len() {
                    let amount_str = &words[i + 1].2;
                    let unit_str = &words[i + 2].2;
                    if parse_english_number(amount_str).is_some() && is_date_duration_unit(unit_str)
                    {
                        matched_kind = Some(if is_start {
                            SyntaxType::StartDate
                        } else {
                            SyntaxType::DueDate
                        });
                        words_consumed = 3;
                    }
                }
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

// --- RECURSIVE ALIAS EXPANSION ---

fn collect_alias_expansions(
    token: &str,
    aliases: &HashMap<String, Vec<String>>,
    visited: &mut HashSet<String>,
) -> Vec<String> {
    let mut results = Vec::new();

    if token.starts_with('#') {
        let key = strip_quotes(token.trim_start_matches('#'));

        // Handle hierarchy (e.g., #work:project might match #work alias)
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
            // Cycle detection
            if visited.contains(&matched_key) {
                return results;
            }
            visited.insert(matched_key);

            for val in values {
                // Recursively expand child values
                let child_expansions = collect_alias_expansions(val, aliases, visited);
                results.extend(child_expansions);
                results.push(val.clone());
            }
        }
    }
    results
}

impl Task {
    pub fn apply_smart_input(&mut self, input: &str, aliases: &HashMap<String, Vec<String>>) {
        let mut summary_words = Vec::new();
        self.priority = 0;
        self.due = None;
        self.dtstart = None;
        self.rrule = None;
        self.estimated_duration = None;
        self.location = None;
        self.url = None;
        self.geo = None;
        self.categories.clear();

        let user_tokens: Vec<String> = split_input_respecting_quotes(input)
            .into_iter()
            .map(|(_, _, s)| s)
            .collect();

        // 1. Expand aliases recursively
        let mut background_tokens = Vec::new();
        let mut visited = HashSet::new();

        for token in &user_tokens {
            let expanded = collect_alias_expansions(token, aliases, &mut visited);
            background_tokens.extend(expanded);
        }

        // 2. Create priority stream: Alias expansions (background) first, then User inputs (override)
        let mut stream = background_tokens;
        stream.extend(user_tokens);

        let mut i = 0;
        while i < stream.len() {
            let token = &stream[i];
            let mut consumed = 1;

            if token.starts_with("@@") {
                self.location = Some(strip_quotes(token.trim_start_matches("@@")));
            } else if token.starts_with("loc:") {
                self.location = Some(strip_quotes(token.trim_start_matches("loc:")));
            } else if token.starts_with("url:") {
                self.url = Some(strip_quotes(token.trim_start_matches("url:")));
            } else if token.starts_with("[[") && token.ends_with("]]") {
                self.url = Some(token[2..token.len() - 2].to_string());
            } else if token.starts_with("geo:") {
                self.geo = Some(strip_quotes(token.trim_start_matches("geo:")));
            } else if token.starts_with("desc:") {
                let desc_val = strip_quotes(token.trim_start_matches("desc:"));
                if self.description.is_empty() {
                    self.description = desc_val;
                } else {
                    self.description.push_str(&format!("\n{}", desc_val));
                }
            } else if token.starts_with('#') {
                let cat = strip_quotes(token.trim_start_matches('#'));
                if !self.categories.contains(&cat) {
                    self.categories.push(cat);
                }
            } else if token.starts_with('!') && token.len() > 1 {
                if let Ok(p) = token[1..].parse::<u8>() {
                    self.priority = p;
                }
            } else if token.starts_with('~') || token.starts_with("est:") {
                let val = strip_quotes(token.trim_start_matches("est:").trim_start_matches('~'));
                if let Some(d) = parse_duration(&val) {
                    self.estimated_duration = Some(d);
                }
            } else if (token == "rec:every" || token == "@every") && i + 2 < stream.len() {
                if let (Ok(interval), freq) = (
                    stream[i + 1].parse::<u32>(),
                    parse_freq_unit(&stream[i + 2]),
                ) && !freq.is_empty()
                {
                    self.rrule = Some(format!("FREQ={};INTERVAL={}", freq, interval));
                    consumed = 3;
                }
            } else if let Some(val) = token
                .strip_prefix("rec:")
                .or_else(|| token.strip_prefix('@'))
            {
                if let Some(rrule) = parse_recurrence(val) {
                    self.rrule = Some(rrule);
                } else if let Some(dt) = parse_smart_date(val, true) {
                    self.due = Some(dt);
                } else if let Some(stripped) = token.strip_prefix("due:")
                    && let Some(dt) = parse_smart_date(stripped, true)
                {
                    self.due = Some(dt);
                }
            } else if (token.starts_with('@')
                || token.starts_with('^')
                || token.starts_with("due:")
                || token.starts_with("start:"))
                && stream.get(i + 1).is_some()
            {
                let (is_start, clean) = if let Some(v) = token
                    .strip_prefix('^')
                    .or_else(|| token.strip_prefix("start:"))
                {
                    (true, v)
                } else {
                    (
                        false,
                        token
                            .strip_prefix('@')
                            .or_else(|| token.strip_prefix("due:"))
                            .unwrap_or(""),
                    )
                };

                if clean == "next" && is_date_unit(&stream[i + 1]) {
                    if let Some(dt) = parse_next_date(&stream[i + 1], !is_start) {
                        if is_start {
                            self.dtstart = Some(dt)
                        } else {
                            self.due = Some(dt)
                        }
                        consumed = 2;
                    }
                } else if clean == "in" && stream.get(i + 2).is_some() {
                    if let Some(amount) = parse_english_number(&stream[i + 1])
                        && let Some(dt) = parse_in_date(amount, &stream[i + 2], !is_start)
                    {
                        if is_start {
                            self.dtstart = Some(dt)
                        } else {
                            self.due = Some(dt)
                        }
                        consumed = 3;
                    }
                } else if let Some(dt) = parse_smart_date(clean, !is_start) {
                    if is_start {
                        self.dtstart = Some(dt)
                    } else {
                        self.due = Some(dt)
                    };
                } else {
                    summary_words.push(token.clone());
                }
            } else if let Some(dt) = parse_smart_date(token.trim_start_matches('^'), false) {
                self.dtstart = Some(dt);
            } else if let Some(dt) = token
                .strip_prefix("start:")
                .and_then(|v| parse_smart_date(v, false))
            {
                self.dtstart = Some(dt);
            } else {
                summary_words.push(token.clone());
            }
            i += consumed;
        }

        self.summary = summary_words.join(" ");
        self.categories.sort();
        self.categories.dedup();
    }

    pub fn to_smart_string(&self) -> String {
        let mut s = self.summary.clone();
        if self.priority > 0 {
            s.push_str(&format!(" !{}", self.priority));
        }
        if let Some(loc) = &self.location {
            if loc.contains(' ') {
                s.push_str(&format!(" @@\"{}\"", loc));
            } else {
                s.push_str(&format!(" @@{}", loc));
            }
        }
        if let Some(u) = &self.url {
            s.push_str(&format!(" url:{}", u));
        }
        if let Some(g) = &self.geo {
            s.push_str(&format!(" geo:{}", g));
        }
        if let Some(start) = self.dtstart {
            s.push_str(&format!(" ^{}", start.format("%Y-%m-%d")));
        }
        if let Some(d) = self.due {
            s.push_str(&format!(" @{}", d.format("%Y-%m-%d")));
        }
        if let Some(mins) = self.estimated_duration {
            s.push_str(&format!(" ~{}m", mins));
        }
        if let Some(r) = &self.rrule {
            s.push_str(&format!(" rec:{}", r));
        }
        for cat in &self.categories {
            // FIX: Don't quote colons, only spaces.
            if cat.contains(' ') {
                s.push_str(&format!(" #\"{}\"", cat));
            } else {
                s.push_str(&format!(" #{}", cat));
            }
        }
        s
    }
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

    // Track visited nodes for the current traversal path.
    // We don't start with new_key in visited, because we want to see if we reach it.
    let mut visited_path = HashSet::new();

    while let Some(current_ref) = stack.pop() {
        // If we loop back to the key being defined, it's a cycle.
        if current_ref == new_key {
            return Err(format!(
                "Circular dependency: '#{}' leads back to itself.",
                new_key
            ));
        }

        // Standard DFS visited check to prevent infinite loops on existing valid cycles or diamond paths
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

fn parse_next_date(unit: &str, end_of_day: bool) -> Option<DateTime<Utc>> {
    let now = Local::now().date_naive();
    match unit.to_lowercase().as_str() {
        "week" => finalize_date(now + Duration::days(7), end_of_day),
        "month" => finalize_date(now + Duration::days(30), end_of_day),
        "year" => finalize_date(now + Duration::days(365), end_of_day),
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

    match Local.from_local_datetime(&t) {
        chrono::LocalResult::Single(dt) => Some(dt.with_timezone(&Utc)),
        chrono::LocalResult::Ambiguous(dt1, _) => Some(dt1.with_timezone(&Utc)),
        chrono::LocalResult::None => Some(t.and_utc()),
    }
}
