// File: ./src/model/parser.rs
use crate::model::item::Task;
use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, Utc};
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

// --- PUBLIC API EXPORTS ---

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

        // 1. Recurrence: @every (5d / 5 days / two months)
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

        // 2. Dates: @in / ^in / @next / ^next
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
                    let next_str = words[i + 1].2.as_str();
                    let is_weekday = matches!(
                        next_str.to_lowercase().as_str(),
                        "monday"
                            | "tuesday"
                            | "wednesday"
                            | "thursday"
                            | "friday"
                            | "saturday"
                            | "sunday"
                    );

                    if is_date_unit_full(next_str) || is_weekday {
                        matched_kind = Some(if is_start {
                            SyntaxType::StartDate
                        } else {
                            SyntaxType::DueDate
                        });
                        words_consumed = 2;
                    }
                } else if clean_word == "in" && i + 1 < words.len() {
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
                    }
                }
            }
        }

        // 3. Single tokens
        if matched_kind.is_none() {
            if word.starts_with("@@") || word_lower.starts_with("loc:") {
                let val = if word.starts_with("@@") {
                    strip_quotes(word.trim_start_matches("@@"))
                } else {
                    strip_quotes(&word[4..])
                };
                if !val.is_empty() {
                    matched_kind = Some(SyntaxType::Location);
                }
            } else if word_lower.starts_with("url:")
                || (word.starts_with("[[") && word.ends_with("]]"))
            {
                matched_kind = Some(SyntaxType::Url);
            } else if word_lower.starts_with("geo:") {
                matched_kind = Some(SyntaxType::Geo);
                if word.ends_with(',') && i + 1 < words.len() {
                    let next_val = &words[i + 1].2;
                    if next_val
                        .chars()
                        .next()
                        .map(|c| c.is_numeric() || c == '-')
                        .unwrap_or(false)
                    {
                        words_consumed = 2;
                    }
                }
            } else if word_lower.starts_with("desc:") {
                matched_kind = Some(SyntaxType::Description);
            } else if word.starts_with('!') && word.len() > 1 && word[1..].parse::<u8>().is_ok() {
                matched_kind = Some(SyntaxType::Priority);
            } else if word.starts_with('~') || word_lower.starts_with("est:") {
                let clean_val = if let Some(stripped) = word.strip_prefix('~') {
                    stripped
                } else {
                    &word[4..]
                };
                if parse_duration(&strip_quotes(clean_val)).is_some() {
                    matched_kind = Some(SyntaxType::Duration);
                }
            } else if word.starts_with('#') {
                matched_kind = Some(SyntaxType::Tag);
            } else if let Some(val) = word.strip_prefix("rec:").or_else(|| word.strip_prefix('@')) {
                if parse_recurrence(val).is_some() {
                    matched_kind = Some(SyntaxType::Recurrence);
                } else if parse_weekday_code(val).is_some() {
                    matched_kind = Some(SyntaxType::DueDate);
                } else if parse_amount_and_unit(val, None, false).is_some() {
                    if word.starts_with("rec:") {
                        matched_kind = Some(SyntaxType::Recurrence);
                    } else {
                        matched_kind = Some(SyntaxType::DueDate);
                    }
                } else if parse_smart_date(val, true).is_some() {
                    matched_kind = Some(SyntaxType::DueDate);
                } else if let Some(_stripped) = word_lower.strip_prefix("due:")
                    && (parse_smart_date(&word[4..], true).is_some()
                        || parse_weekday_code(&word[4..]).is_some())
                    {
                        matched_kind = Some(SyntaxType::DueDate);
                    }
            } else if let Some(_stripped) = word_lower.strip_prefix("due:") {
                if parse_smart_date(&word[4..], true).is_some()
                    || parse_weekday_code(&word[4..]).is_some()
                {
                    matched_kind = Some(SyntaxType::DueDate);
                }
            } else if let Some(_val) = word
                .strip_prefix('^')
                .or_else(|| word_lower.strip_prefix("start:"))
            {
                let clean_val = if let Some(stripped) = word.strip_prefix('^') {
                    stripped
                } else {
                    &word[6..]
                };
                if parse_smart_date(clean_val, false).is_some()
                    || parse_weekday_code(clean_val).is_some()
                {
                    matched_kind = Some(SyntaxType::StartDate);
                }
            } else if word_lower == "today" || word_lower == "tomorrow" {
                matched_kind = Some(SyntaxType::DueDate);
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
    let inner = if s.len() >= 2
        && ((s.starts_with('"') && s.ends_with('"')) || (s.starts_with('{') && s.ends_with('}')))
    {
        &s[1..s.len() - 1]
    } else {
        s
    };
    unescape(inner)
}

fn quote_value(s: &str) -> String {
    if s.contains(' ') || s.contains('"') || s.contains('\\') || s.contains('#') || s.is_empty() {
        let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
        format!("\"{}\"", escaped)
    } else {
        s.to_string()
    }
}

// --- TASK IMPLEMENTATION ---

impl Task {
    pub fn to_smart_string(&self) -> String {
        let mut s = escape_summary(&self.summary);
        if self.priority > 0 {
            s.push_str(&format!(" !{}", self.priority));
        }
        if let Some(loc) = &self.location {
            s.push_str(&format!(" @@{}", quote_value(loc)));
        }
        if let Some(u) = &self.url {
            s.push_str(&format!(" url:{}", quote_value(u)));
        }
        if let Some(g) = &self.geo {
            s.push_str(&format!(" geo:{}", quote_value(g)));
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
            let pretty = prettify_recurrence(r);
            s.push_str(&format!(" {}", pretty));
        }
        for cat in &self.categories {
            s.push_str(&format!(" #{}", quote_value(cat)));
        }
        s
    }

    pub fn apply_smart_input(&mut self, input: &str, aliases: &HashMap<String, Vec<String>>) {
        let mut summary_words = Vec::new();
        // Reset fields
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
                        self.rrule = Some(format!("FREQ={};INTERVAL={}", freq, interval));
                        consumed = 1 + 1 + extra_consumed;
                    } else {
                        summary_words.push(unescape(token));
                    }
                } else if let Some(byday) = parse_weekday_code(next_token_str) {
                    self.rrule = Some(format!("FREQ=WEEKLY;BYDAY={}", byday));
                    consumed = 2;
                } else {
                    summary_words.push(unescape(token));
                }
            }
            // 2. New Fields
            else if token.starts_with("@@") {
                let val = strip_quotes(token.trim_start_matches("@@"));
                if val.is_empty() {
                    summary_words.push(unescape(token));
                } else {
                    self.location = Some(val);
                }
            } else if token_lower.starts_with("loc:") {
                let val = strip_quotes(&token[4..]);
                if val.is_empty() {
                    summary_words.push(unescape(token));
                } else {
                    self.location = Some(val);
                }
            } else if token_lower.starts_with("url:") {
                self.url = Some(strip_quotes(&token[4..]));
            } else if token.starts_with("[[") && token.ends_with("]]") {
                self.url = Some(token[2..token.len() - 2].to_string());
            } else if token_lower.starts_with("geo:") {
                let mut raw_val = token[4..].to_string();
                if token.ends_with(',') && i + 1 < stream.len() {
                    let next_token = &stream[i + 1];
                    if next_token
                        .chars()
                        .next()
                        .map(|c| c.is_numeric() || c == '-')
                        .unwrap_or(false)
                    {
                        raw_val.push_str(next_token);
                        consumed = 2;
                    }
                }
                let raw_geo = strip_quotes(&raw_val);
                if raw_geo.contains(',')
                    && raw_geo
                        .chars()
                        .all(|c| c.is_numeric() || c == ',' || c == '.' || c == '-' || c == ' ')
                {
                    self.geo = Some(raw_geo);
                } else {
                    summary_words.push(unescape(token));
                    if consumed == 2 {
                        summary_words.push(unescape(&stream[i + 1]));
                    }
                }
            } else if token_lower.starts_with("desc:") {
                let desc_val = strip_quotes(&token[5..]);
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
                } else {
                    summary_words.push(unescape(token));
                }
            } else if token.starts_with('~') || token_lower.starts_with("est:") {
                let val = strip_quotes(if let Some(stripped) = token.strip_prefix('~') {
                    stripped
                } else {
                    &token[4..]
                });
                if let Some(d) = parse_duration(&val) {
                    self.estimated_duration = Some(d);
                } else {
                    summary_words.push(unescape(token));
                }
            }
            // 3. Dates (Multi-word)
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
                    let clean_v = if let Some(stripped) = token.strip_prefix('^') {
                        stripped
                    } else {
                        &token[6..]
                    };
                    (true, clean_v)
                } else {
                    let clean_v = if let Some(stripped) = token.strip_prefix('@') {
                        stripped
                    } else if token_lower.starts_with("due:") {
                        &token[4..]
                    } else {
                        ""
                    };
                    (false, clean_v)
                };

                let mut matched_date = false;
                if clean == "next" && stream.get(i + 1).is_some() {
                    let next_str = &stream[i + 1];
                    if let Some(dt) = parse_next_date(next_str, !is_start) {
                        if is_start {
                            self.dtstart = Some(dt)
                        } else {
                            self.due = Some(dt)
                        }
                        consumed = 2;
                        matched_date = true;
                    }
                } else if clean == "in" && i + 1 < stream.len() {
                    let next_token_str = stream[i + 1].as_str();
                    let next_next = if i + 2 < stream.len() {
                        Some(stream[i + 2].as_str())
                    } else {
                        None
                    };

                    if let Some((amount, unit, extra_consumed)) =
                        parse_amount_and_unit(next_token_str, next_next, false)
                        && let Some(dt) = parse_in_date(amount, &unit, !is_start)
                    {
                        if is_start {
                            self.dtstart = Some(dt)
                        } else {
                            self.due = Some(dt)
                        }
                        consumed = 1 + 1 + extra_consumed;
                        matched_date = true;
                    }
                }

                if !matched_date {
                    if let Some(dt) = parse_smart_date(clean, !is_start) {
                        if is_start {
                            self.dtstart = Some(dt)
                        } else {
                            self.due = Some(dt)
                        };
                    } else if let Some(rrule) = parse_recurrence(clean) {
                        self.rrule = Some(rrule);
                    } else if let Some(dt) = parse_weekday_date(clean, !is_start) {
                        if is_start {
                            self.dtstart = Some(dt)
                        } else {
                            self.due = Some(dt)
                        }
                    } else {
                        summary_words.push(unescape(token));
                    }
                }
            }
            // 4. Recurrence / Dates (Single-word)
            else if let Some(val) = token
                .strip_prefix("rec:")
                .or_else(|| token.strip_prefix('@'))
            {
                if let Some(rrule) = parse_recurrence(val) {
                    self.rrule = Some(rrule);
                } else if token.starts_with("rec:")
                    && let Some((interval, unit, _)) = parse_amount_and_unit(val, None, false)
                {
                    let freq = parse_freq_from_unit(&unit);
                    if !freq.is_empty() {
                        self.rrule = Some(format!("FREQ={};INTERVAL={}", freq, interval));
                    } else {
                        summary_words.push(unescape(token));
                    }
                } else if let Some(dt) = parse_smart_date(val, true) {
                    self.due = Some(dt);
                } else if let Some(dt) = parse_weekday_date(val, true) {
                    self.due = Some(dt);
                } else if let Some(_stripped) = token_lower.strip_prefix("due:") {
                    let real_val = &token[4..];
                    if let Some(dt) = parse_smart_date(real_val, true) {
                        self.due = Some(dt);
                    } else if let Some(dt) = parse_weekday_date(real_val, true) {
                        self.due = Some(dt);
                    } else {
                        summary_words.push(unescape(token));
                    }
                } else {
                    summary_words.push(unescape(token));
                }
            } else if token_lower.strip_prefix("due:").is_some() {
                let val = &token[4..];
                if let Some(dt) = parse_smart_date(val, true) {
                    self.due = Some(dt);
                } else if let Some(dt) = parse_weekday_date(val, true) {
                    self.due = Some(dt);
                } else {
                    summary_words.push(unescape(token));
                }
            } else if let Some(_val) = token
                .strip_prefix('^')
                .or_else(|| token_lower.strip_prefix("start:"))
            {
                let clean_val = if let Some(stripped) = token.strip_prefix('^') {
                    stripped
                } else {
                    &token[6..]
                };
                if let Some(dt) = parse_smart_date(clean_val, false) {
                    self.dtstart = Some(dt);
                } else if let Some(dt) = parse_weekday_date(clean_val, false) {
                    self.dtstart = Some(dt);
                } else {
                    summary_words.push(unescape(token));
                }
            } else if token_lower == "today" {
                if let Some(dt) = parse_smart_date("today", true) {
                    self.due = Some(dt);
                }
            } else if token_lower == "tomorrow" {
                if let Some(dt) = parse_smart_date("tomorrow", true) {
                    self.due = Some(dt);
                }
            } else {
                summary_words.push(unescape(token));
            }
            i += consumed;
        }

        self.summary = summary_words.join(" ");
        self.categories.sort();
        self.categories.dedup();
    }
}

fn escape_summary(summary: &str) -> String {
    let mut escaped_words = Vec::new();
    let words = summary.split_whitespace(); // Simple split for escaping text

    for word in words {
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
    // 1. Sigils
    if word.starts_with('@')
        || word.starts_with('#')
        || word.starts_with('!')
        || word.starts_with('^')
        || word.starts_with('~')
    {
        return true;
    }
    // 2. Keywords
    if lower.starts_with("loc:")
        || lower.starts_with("url:")
        || lower.starts_with("geo:")
        || lower.starts_with("desc:")
        || lower.starts_with("due:")
        || lower.starts_with("start:")
        || lower.starts_with("rec:")
        || lower.starts_with("est:")
    {
        return true;
    }
    // 3. Strict words
    matches!(lower.as_str(), "today" | "tomorrow")
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

fn prettify_recurrence(rrule: &str) -> String {
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

fn parse_smart_date(val: &str, end_of_day: bool) -> Option<DateTime<Utc>> {
    if let Ok(date) = NaiveDate::parse_from_str(val, "%Y-%m-%d") {
        return finalize_date(date, end_of_day);
    }

    let now = Local::now().date_naive();
    let lower = val.to_lowercase();

    if lower == "today" {
        return finalize_date(now, end_of_day);
    }
    if lower == "tomorrow" {
        return finalize_date(now + Duration::days(1), end_of_day);
    }

    if let Some(n) = lower.strip_suffix('d').and_then(|s| s.parse::<i64>().ok()) {
        return finalize_date(now + Duration::days(n), end_of_day);
    }
    if let Some(n) = lower.strip_suffix('w').and_then(|s| s.parse::<i64>().ok()) {
        return finalize_date(now + Duration::days(n * 7), end_of_day);
    }
    if let Some(n) = lower.strip_suffix("mo").and_then(|s| s.parse::<i64>().ok()) {
        return finalize_date(now + Duration::days(n * 30), end_of_day);
    }
    if let Some(n) = lower.strip_suffix('y').and_then(|s| s.parse::<i64>().ok()) {
        return finalize_date(now + Duration::days(n * 365), end_of_day);
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

fn parse_weekday_date(s: &str, end_of_day: bool) -> Option<DateTime<Utc>> {
    parse_next_date(s, end_of_day)
}

fn parse_next_date(unit: &str, end_of_day: bool) -> Option<DateTime<Utc>> {
    let now = Local::now().date_naive();
    match unit.to_lowercase().as_str() {
        "week" => finalize_date(now + Duration::days(7), end_of_day),
        "month" => finalize_date(now + Duration::days(30), end_of_day),
        "year" => finalize_date(now + Duration::days(365), end_of_day),
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
                return next_weekday(now, target, end_of_day);
            }
            None
        }
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
    Some(t.and_utc())
}
