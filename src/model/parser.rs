// SPDX-License-Identifier: GPL-3.0-or-later
// File: ./src/model/parser.rs
/*
File: cfait/src/model/parser.rs
Logic for parsing smart input strings into task properties.
This file is recreated with updated handling for `done:` tokens to accept
either a full datetime in the token (`done:YYYY-MM-DD HH:MM`) or the older
date-only form with an optional separate time token following it.
*/

use crate::model::{Alarm, DateType, Task};
use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, NaiveTime, Utc};
use std::collections::{HashMap, HashSet};
use std::sync::RwLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LexiconUnit {
    Minutes,
    Hours,
    Days,
    Weeks,
    Months,
    Years,
}

impl LexiconUnit {
    pub fn to_freq(&self) -> &'static str {
        match self {
            LexiconUnit::Days => "DAILY",
            LexiconUnit::Weeks => "WEEKLY",
            LexiconUnit::Months => "MONTHLY",
            LexiconUnit::Years => "YEARLY",
            _ => "",
        }
    }
    pub fn to_canonical(&self) -> &'static str {
        match self {
            LexiconUnit::Minutes => "m",
            LexiconUnit::Hours => "h",
            LexiconUnit::Days => "d",
            LexiconUnit::Weeks => "w",
            LexiconUnit::Months => "mo",
            LexiconUnit::Years => "y",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExactToken {
    Today,
    Tomorrow,
    Yesterday,
    Now,
    Next,
    In,
    Every,
    After,
    Until,
    Except,
    Unit(LexiconUnit),
    Weekday(&'static str),
    Month(u32),
    Number(u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrefixToken {
    Due,
    Start,
    StartDue,
    Duration,
    Reminder,
    Spent,
    Done,
    Loc,
    Desc,
    Goal,
    Recur,
    Url,
    Geo,
}

pub struct ParserLexicon {
    pub exact: HashMap<String, ExactToken>,
    pub prefixes: Vec<(String, PrefixToken)>,
    pub date_format: String,
    pub unit_m: String,
    pub unit_h: String,
    pub unit_d: String,
    pub unit_w: String,
    pub unit_mo: String,
    pub unit_y: String,
}

impl ParserLexicon {
    pub fn match_prefix<'a>(&'a self, word: &'a str) -> Option<(&'a str, PrefixToken, &'a str)> {
        for (p, kind) in &self.prefixes {
            if word.starts_with(p.as_str()) {
                return Some((p.as_str(), *kind, &word[p.len()..]));
            }
        }
        None
    }

    pub fn build() -> Self {
        let mut exact_en = HashMap::new();
        let mut exact_loc = HashMap::new();
        let mut prefixes_en = Vec::new();
        let mut prefixes_loc = Vec::new();

        let mut add_exact = |loc_key: &str, default_en: &str, token: ExactToken| {
            for k in default_en.split(',') {
                exact_en.insert(k.trim().to_lowercase(), token);
            }
            let localized = rust_i18n::t!(loc_key);
            if localized != loc_key && !localized.is_empty() {
                for k in localized.split(',') {
                    exact_loc.insert(k.trim().to_lowercase(), token);
                }
            }
        };

        let mut add_prefix = |loc_key: &str, default_en: &str, token: PrefixToken| {
            for k in default_en.split(',') {
                let clean = k.trim().to_lowercase();
                if !clean.is_empty() {
                    prefixes_en.push((clean, token));
                }
            }
            let localized = rust_i18n::t!(loc_key);
            if localized != loc_key && !localized.is_empty() {
                for k in localized.split(',') {
                    let clean = k.trim().to_lowercase();
                    if !clean.is_empty() {
                        prefixes_loc.push((clean, token));
                    }
                }
            }
        };

        let get_first = |loc_key: &str, def: &str| -> String {
            let t = rust_i18n::t!(loc_key);
            if t != loc_key && !t.is_empty() {
                t.split(',').next().unwrap_or(def).trim().to_string()
            } else {
                def.to_string()
            }
        };

        add_exact("parser_today", "today,tdy", ExactToken::Today);
        add_exact("parser_tomorrow", "tomorrow,tmr", ExactToken::Tomorrow);
        add_exact("parser_yesterday", "yesterday,yst", ExactToken::Yesterday);
        add_exact("parser_now", "now", ExactToken::Now);
        add_exact("parser_next", "next", ExactToken::Next);
        add_exact("parser_in", "in", ExactToken::In);
        add_exact("parser_every", "every", ExactToken::Every);
        add_exact("parser_after", "after", ExactToken::After);
        add_exact("parser_until", "until", ExactToken::Until);
        add_exact("parser_except", "except", ExactToken::Except);

        add_exact(
            "parser_unit_days",
            "d,day,days,daily",
            ExactToken::Unit(LexiconUnit::Days),
        );
        add_exact(
            "parser_unit_weeks",
            "w,week,weeks,weekly",
            ExactToken::Unit(LexiconUnit::Weeks),
        );
        add_exact(
            "parser_unit_months",
            "mo,month,months,monthly",
            ExactToken::Unit(LexiconUnit::Months),
        );
        add_exact(
            "parser_unit_years",
            "y,year,years,yearly",
            ExactToken::Unit(LexiconUnit::Years),
        );
        add_exact(
            "parser_unit_hours",
            "h,hour,hours",
            ExactToken::Unit(LexiconUnit::Hours),
        );
        add_exact(
            "parser_unit_minutes",
            "m,min,minute,minutes",
            ExactToken::Unit(LexiconUnit::Minutes),
        );

        add_exact(
            "parser_weekdays_mo",
            "mon,monday,mondays",
            ExactToken::Weekday("MO"),
        );
        add_exact(
            "parser_weekdays_tu",
            "tu,tue,tuesday,tuesdays",
            ExactToken::Weekday("TU"),
        );
        add_exact(
            "parser_weekdays_we",
            "we,wed,wednesday,wednesdays",
            ExactToken::Weekday("WE"),
        );
        add_exact(
            "parser_weekdays_th",
            "th,thu,thursday,thursdays",
            ExactToken::Weekday("TH"),
        );
        add_exact(
            "parser_weekdays_fr",
            "fr,fri,friday,fridays",
            ExactToken::Weekday("FR"),
        );
        add_exact(
            "parser_weekdays_sa",
            "sa,sat,saturday,saturdays",
            ExactToken::Weekday("SA"),
        );
        add_exact(
            "parser_weekdays_su",
            "su,sun,sunday,sundays",
            ExactToken::Weekday("SU"),
        );

        add_exact("parser_months_jan", "jan,january", ExactToken::Month(1));
        add_exact("parser_months_feb", "feb,february", ExactToken::Month(2));
        add_exact("parser_months_mar", "mar,march", ExactToken::Month(3));
        add_exact("parser_months_apr", "apr,april", ExactToken::Month(4));
        add_exact("parser_months_may", "may", ExactToken::Month(5));
        add_exact("parser_months_jun", "jun,june", ExactToken::Month(6));
        add_exact("parser_months_jul", "jul,july", ExactToken::Month(7));
        add_exact("parser_months_aug", "aug,august", ExactToken::Month(8));
        add_exact(
            "parser_months_sep",
            "sep,sept,september",
            ExactToken::Month(9),
        );
        add_exact("parser_months_oct", "oct,october", ExactToken::Month(10));
        add_exact("parser_months_nov", "nov,november", ExactToken::Month(11));
        add_exact("parser_months_dec", "dec,december", ExactToken::Month(12));

        let nums_en = "one,two,three,four,five,six,seven,eight,nine,ten,eleven,twelve";
        let nums_loc = rust_i18n::t!("parser_numbers_1_to_12");
        for (i, w) in nums_en.split(',').enumerate() {
            if i < 12 {
                exact_en.insert(w.trim().to_lowercase(), ExactToken::Number((i + 1) as u32));
            }
        }
        if nums_loc != "parser_numbers_1_to_12" && !nums_loc.is_empty() {
            for (i, w) in nums_loc.split(',').enumerate() {
                if i < 12 {
                    exact_loc.insert(w.trim().to_lowercase(), ExactToken::Number((i + 1) as u32));
                }
            }
        }

        add_prefix("parser_due", "@,due:", PrefixToken::Due);
        add_prefix("parser_start", "^,start:", PrefixToken::Start);
        add_prefix("parser_start_due", "^@", PrefixToken::StartDue);
        add_prefix("parser_duration", "~,est:", PrefixToken::Duration);
        add_prefix("parser_reminder", "rem:", PrefixToken::Reminder);
        add_prefix("parser_spent", "spent:", PrefixToken::Spent);
        add_prefix("parser_done", "done:", PrefixToken::Done);
        add_prefix("parser_loc", "@@,loc:", PrefixToken::Loc);
        add_prefix("parser_desc", "desc:", PrefixToken::Desc);
        add_prefix("parser_goal", "goal:", PrefixToken::Goal);
        add_prefix("parser_recur", "rec:", PrefixToken::Recur);
        add_prefix("parser_url", "url:", PrefixToken::Url);

        // Merge logic: Localized translations unconditionally overwrite English canonicals.
        // This ensures e.g., French "mar" (Mardi) overwrites English "mar" (March).
        let mut exact = exact_en;
        exact.extend(exact_loc);

        // For prefixes, we append English canonicals *after* localized to ensure localized
        // matches take precedence if string lengths are identical.
        let mut raw_prefixes = prefixes_loc;
        raw_prefixes.extend(prefixes_en);
        raw_prefixes.push(("geo:".to_string(), PrefixToken::Geo));

        // Stable sort by reverse length so that longer prefixes trigger first,
        // while preserving the Localized > English tie-breaking order.
        raw_prefixes.sort_by_key(|b| std::cmp::Reverse(b.0.len()));

        // Deduplicate
        let mut prefixes = Vec::new();
        let mut seen = HashSet::new();
        for p in raw_prefixes {
            if seen.insert(p.0.clone()) {
                prefixes.push(p);
            }
        }

        let df = rust_i18n::t!("parser_date_format");
        let date_format = if df != "parser_date_format" && !df.is_empty() {
            df.to_string()
        } else {
            "%Y-%m-%d".to_string()
        };

        Self {
            exact,
            prefixes,
            date_format,
            unit_m: get_first("parser_unit_minutes", "m"),
            unit_h: get_first("parser_unit_hours", "h"),
            unit_d: get_first("parser_unit_days", "d"),
            unit_w: get_first("parser_unit_weeks", "w"),
            unit_mo: get_first("parser_unit_months", "mo"),
            unit_y: get_first("parser_unit_years", "y"),
        }
    }
}

pub static LEXICON: once_cell::sync::Lazy<RwLock<ParserLexicon>> =
    once_cell::sync::Lazy::new(|| RwLock::new(ParserLexicon::build()));

pub fn rebuild_lexicon() {
    if let Ok(mut lex) = LEXICON.write() {
        *lex = ParserLexicon::build();
    }
}

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
    Calendar, // +cal, -cal
    Pin,      // +pin, -pin
    Filter,   // is:done, < / > operators, duration filters, etc.
    Operator, // Boolean / operator tokens: |, -, (, ), AND/OR/NOT
    Goal,     // goal:
}

#[derive(Debug)]
pub struct SyntaxToken {
    pub kind: SyntaxType,
    pub start: usize,
    pub end: usize,
}

pub fn parse_alias_values(input: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_quote = false;
    let mut escaped = false;
    let mut in_geo = false;

    for c in input.chars() {
        if escaped {
            current.push(c);
            escaped = false;
        } else if c == '\\' {
            escaped = true;
            current.push(c);
        } else if c == '"' {
            in_quote = !in_quote;
            current.push(c);
        } else if c == ',' && !in_quote {
            let trimmed = current.trim_start();
            if trimmed.starts_with("geo:") && !in_geo {
                in_geo = true;
                current.push(c);
            } else {
                parts.push(current.trim().to_string());
                current.clear();
                in_geo = false;
            }
        } else {
            current.push(c);
        }
    }
    if !current.trim().is_empty() {
        parts.push(current.trim().to_string());
    }
    parts.into_iter().filter(|s| !s.is_empty()).collect()
}

fn merge_assignment_tokens(parts: &[(usize, usize, String)]) -> Vec<String> {
    let mut merged_tokens: Vec<String> = Vec::new();
    let mut i = 0;
    while i < parts.len() {
        let mut tok = parts[i].2.clone();

        if tok == ":=" {
            if let Some(prev) = merged_tokens.last_mut() {
                prev.push_str(":=");
                if i + 1 < parts.len() {
                    prev.push_str(&parts[i + 1].2);
                    i += 1;
                }
            }
            i += 1;
            continue;
        } else if i + 1 < parts.len() && (tok.ends_with(":=") || parts[i + 1].2.starts_with(":=")) {
            tok.push_str(&parts[i + 1].2);
            i += 1;
        }

        merged_tokens.push(tok);
        i += 1;
    }

    let mut final_tokens = Vec::new();
    let mut j = 0;
    while j < merged_tokens.len() {
        let mut tok = merged_tokens[j].clone();
        if tok.contains(":=") {
            while j + 1 < merged_tokens.len() {
                let next = &merged_tokens[j + 1];
                if tok.ends_with(',') || next == "," || next.starts_with(',') {
                    tok.push_str(next);
                    j += 1;
                } else {
                    break;
                }
            }
        }
        final_tokens.push(tok);
        j += 1;
    }

    final_tokens
}

pub fn extract_inline_aliases(input: &str) -> (String, HashMap<String, Vec<String>>) {
    let parts = split_input_respecting_quotes(input);
    let merged = merge_assignment_tokens(&parts);
    let mut cleaned_words = Vec::new();
    let mut new_aliases = HashMap::new();

    let lex_guard = LEXICON.read().unwrap();
    let lex = &*lex_guard;

    for token in merged {
        if token.contains(":=")
            && !token.starts_with('\\')
            && let Some((left, right)) = token.split_once(":=")
        {
            let mut key = String::new();
            let mut is_valid = false;

            // Case 1: Tag Alias (#tag:=...)
            if left.starts_with('#') {
                key = strip_quotes(left.trim_start_matches('#'));
                is_valid = !key.is_empty();
            }
            // Case 2: Location Alias (@@loc:=... or loc:loc:=...)
            else if left.starts_with("@@") {
                let raw = left.trim_start_matches('@');
                let clean = strip_quotes(raw);
                if !clean.is_empty() {
                    key = format!("@@{}", clean);
                    is_valid = true;
                }
            } else if let Some((p, PrefixToken::Loc, _)) = lex.match_prefix(&left.to_lowercase()) {
                let char_count = p.chars().count();
                let byte_idx = left
                    .char_indices()
                    .nth(char_count)
                    .map(|(i, _)| i)
                    .unwrap_or(left.len());
                let clean = strip_quotes(&left[byte_idx..]);
                if !clean.is_empty() {
                    key = format!("@@{}", clean);
                    is_valid = true;
                }
            }

            let right_lower = right.to_lowercase();
            if is_valid
                && lex.match_prefix(&right_lower).map(|(_, p, _)| p) != Some(PrefixToken::Goal)
            {
                let tags = parse_alias_values(right);

                if !tags.is_empty() {
                    new_aliases.insert(key, tags);
                    // Keep the key part in the description as the 'primary' value
                    cleaned_words.push(left.to_string());
                    continue;
                }
            }
        }
        cleaned_words.push(token);
    }
    (cleaned_words.join(" "), new_aliases)
}

pub fn extract_inline_goals(input: &str) -> (String, HashMap<String, crate::config::Goal>) {
    let parts = split_input_respecting_quotes(input);
    let merged = merge_assignment_tokens(&parts);
    let mut cleaned_words = Vec::new();
    let mut new_goals = HashMap::new();

    let lex_guard = LEXICON.read().unwrap();
    let lex = &*lex_guard;

    for token in merged {
        if token.contains(":=")
            && !token.starts_with('\\')
            && let Some((left, right)) = token.split_once(":=")
        {
            let mut key = String::new();
            let mut is_valid = false;

            if left.starts_with('#') {
                key = strip_quotes(left.trim_start_matches('#'));
                is_valid = !key.is_empty();
                key = format!("#{}", key);
            } else if left.starts_with("@@") {
                let raw = left.trim_start_matches('@');
                let clean = strip_quotes(raw);
                if !clean.is_empty() {
                    key = format!("@@{}", clean);
                    is_valid = true;
                }
            } else if let Some((p, PrefixToken::Loc, _)) = lex.match_prefix(&left.to_lowercase()) {
                let char_count = p.chars().count();
                let byte_idx = left
                    .char_indices()
                    .nth(char_count)
                    .map(|(i, _)| i)
                    .unwrap_or(left.len());
                let clean = strip_quotes(&left[byte_idx..]);
                if !clean.is_empty() {
                    key = format!("@@{}", clean);
                    is_valid = true;
                }
            }

            let right_lower = right.to_lowercase();
            if is_valid && let Some((p, PrefixToken::Goal, _)) = lex.match_prefix(&right_lower) {
                let char_count = p.chars().count();
                let byte_idx = right
                    .char_indices()
                    .nth(char_count)
                    .map(|(i, _)| i)
                    .unwrap_or(right.len());
                let goal_str = &right[byte_idx..];
                let (target_str, period_str) = if let Some(idx) = goal_str.find('/') {
                    (&goal_str[..idx], &goal_str[idx + 1..])
                } else if let Some(idx) = goal_str.find(':') {
                    (&goal_str[..idx], &goal_str[idx + 1..])
                } else {
                    let lower = goal_str.to_lowercase();
                    if let Some(ExactToken::Unit(u)) = lex.exact.get(&lower) {
                        match u {
                            LexiconUnit::Days
                            | LexiconUnit::Weeks
                            | LexiconUnit::Months
                            | LexiconUnit::Years => ("1", goal_str),
                            _ => ("", ""),
                        }
                    } else if matches!(
                        lower.as_str(),
                        "daily" | "weekly" | "monthly" | "yearly" | "d" | "w" | "mo" | "y"
                    ) {
                        ("1", goal_str)
                    } else {
                        ("", "")
                    }
                };

                if !target_str.is_empty() && !period_str.is_empty() {
                    let period_str = period_str.trim_start_matches('@');

                    let mut goal_type = crate::config::GoalType::Count;
                    let target = if let Some(dur) = parse_duration_with_lex(target_str, lex) {
                        goal_type = crate::config::GoalType::Duration;
                        dur
                    } else {
                        target_str.parse().unwrap_or(0)
                    };

                    let (mut amount, unit, _) = parse_amount_and_unit_with_lex(
                        period_str, None, false, lex,
                    )
                    .unwrap_or((1, period_str.to_string(), 0));
                    let interval_unit = if let Some(ExactToken::Unit(u)) =
                        lex.exact.get(&unit.to_lowercase())
                    {
                        match u {
                            LexiconUnit::Days => crate::config::IntervalUnit::Days,
                            LexiconUnit::Weeks => crate::config::IntervalUnit::Weeks,
                            LexiconUnit::Months => crate::config::IntervalUnit::Months,
                            LexiconUnit::Years => crate::config::IntervalUnit::Years,
                            _ => crate::config::IntervalUnit::Weeks,
                        }
                    } else {
                        match unit.to_lowercase().as_str() {
                            "d" | "day" | "days" | "daily" => crate::config::IntervalUnit::Days,
                            "w" | "week" | "weeks" | "weekly" => crate::config::IntervalUnit::Weeks,
                            "m" | "mo" | "month" | "months" | "monthly" => {
                                crate::config::IntervalUnit::Months
                            }
                            "q" | "quarter" | "quarterly" => {
                                amount *= 3;
                                crate::config::IntervalUnit::Months
                            }
                            "h" | "hy" | "halfyearly" | "semiannual" => {
                                amount *= 6;
                                crate::config::IntervalUnit::Months
                            }
                            "y" | "year" | "years" | "yearly" => crate::config::IntervalUnit::Years,
                            _ => crate::config::IntervalUnit::Weeks,
                        }
                    };

                    if target > 0 {
                        new_goals.insert(
                            key,
                            crate::config::Goal {
                                goal_type,
                                target,
                                interval: crate::config::Interval {
                                    amount,
                                    unit: interval_unit,
                                },
                            },
                        );
                        cleaned_words.push(left.to_string());
                        continue;
                    }
                }
            }
        }
        cleaned_words.push(token);
    }
    (cleaned_words.join(" "), new_goals)
}

pub fn is_valid_geo(val: &str) -> bool {
    let stripped = strip_quotes(val);
    let s = stripped.trim();
    if s.eq_ignore_ascii_case("here") {
        return true;
    }

    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 2 {
        return false;
    }

    let valid_part = |p: &str| -> bool {
        let p = p.trim();
        !p.is_empty()
            && p.chars()
                .all(|c| c.is_ascii_digit() || " .-°NSEWnsew".contains(c))
    };

    valid_part(parts[0]) && valid_part(parts[1])
}

pub fn validate_alias_integrity(
    new_key: &str,
    new_values: &[String],
    current_aliases: &HashMap<String, Vec<String>>,
) -> Result<(), String> {
    // 1. Normalize values into Keys (strip #, keep @@, check hierarchy)
    fn to_key(val: &str) -> Option<String> {
        if val.starts_with('#') {
            Some(strip_quotes(val.trim_start_matches('#')))
        } else if val.starts_with("@@") {
            Some(format!("@@{}", strip_quotes(val.trim_start_matches('@'))))
        } else if val.to_lowercase().starts_with("loc:") {
            Some(format!("@@{}", strip_quotes(&val[4..])))
        } else {
            None
        }
    }

    // 2. Check immediate self-reference
    if new_values
        .iter()
        .any(|v| to_key(v).as_deref() == Some(new_key))
    {
        return Err(format!("Alias '{}' cannot refer to itself.", new_key));
    }

    // 3. DFS Traversal to find cycles
    let mut stack: Vec<String> = new_values.iter().filter_map(|v| to_key(v)).collect();
    let mut visited_path = HashSet::new();

    while let Some(current_ref) = stack.pop() {
        if current_ref == new_key {
            return Err(format!(
                "Circular dependency: '{}' leads back to itself.",
                new_key
            ));
        }
        if visited_path.contains(&current_ref) {
            continue;
        }
        visited_path.insert(current_ref.clone());

        // --- HIERARCHY LOGIC START ---
        // We must mimic the runtime parser: if exact key missing, try parent key.
        let mut search = current_ref.as_str();
        loop {
            if let Some(children) = current_aliases.get(search) {
                // Found a definition! Add its children to stack.
                for child in children {
                    if let Some(k) = to_key(child) {
                        stack.push(k);
                    }
                }
                // In runtime, we stop after finding the first match in the hierarchy.
                // We must mirror that behavior here.
                break;
            }

            // Fallback: Try stripping the last segment (e.g., "A:B" -> "A")
            if let Some(idx) = search.rfind(':') {
                // Safety: Don't split inside the "@@" prefix if it's a location
                // e.g. "@@Home:Kitchen" -> "@@Home" (OK)
                // e.g. "@@:Kitchen" -> "@@" (Valid Key? Yes, theoretically)
                if search.starts_with("@@") && idx < 2 {
                    break;
                }
                search = &search[..idx];
            } else {
                break;
            }
        }
        // --- HIERARCHY LOGIC END ---
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

pub fn parse_time_range(s: &str) -> Option<(NaiveTime, NaiveTime)> {
    let (start_str, end_str) = s.split_once('-')?;
    let start = parse_time_string(start_str)?;
    let end = parse_time_string(end_str)?;
    Some((start, end))
}

fn is_time_format(s: &str) -> bool {
    parse_time_string(s).is_some() || parse_time_range(s).is_some()
}

pub fn tokenize_smart_input(input: &str, is_search_query: bool) -> Vec<SyntaxToken> {
    let mut tokens = Vec::new();
    let words = split_input_respecting_quotes(input);

    let mut cursor = 0;
    let mut i = 0;
    let mut has_recurrence = false;

    let lex_guard = LEXICON.read().unwrap();
    let lex = &*lex_guard;

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

        let pref_match = lex.match_prefix(&word_lower);
        let rem = pref_match.map(|(_, _, r)| r).unwrap_or(word_lower.as_str());
        let pref = pref_match.map(|(_, p, _)| p);

        let rem_original = if let Some((prefix_str, _, _)) = pref_match {
            let char_count = prefix_str.chars().count();
            let byte_idx = word
                .char_indices()
                .nth(char_count)
                .map(|(i, _)| i)
                .unwrap_or(word.len());
            &word[byte_idx..]
        } else {
            word.as_str()
        };

        let exact = lex.exact.get(rem);

        if is_search_query {
            if word == "|"
                || word == "("
                || word == ")"
                || word.eq_ignore_ascii_case("and")
                || word.eq_ignore_ascii_case("or")
                || word.eq_ignore_ascii_case("not")
            {
                matched_kind = Some(SyntaxType::Operator);
            } else if word.starts_with('-') && word.len() > 1
                || word_lower.starts_with("is:")
                || ((word.starts_with('!')
                    || word.starts_with('~')
                    || word.starts_with('@')
                    || word.starts_with("due:")
                    || word.starts_with('^')
                    || word.starts_with("start:"))
                    && (word.contains('<') || word.contains('>')))
            {
                matched_kind = Some(SyntaxType::Filter);
            }
        }

        let is_due_or_recur = pref == Some(PrefixToken::Due) || pref == Some(PrefixToken::Recur);

        let is_every = is_due_or_recur && exact == Some(&ExactToken::Every);
        let is_after = pref == Some(PrefixToken::Due) && exact == Some(&ExactToken::After);

        if matched_kind.is_none() && (is_every || is_after) && i + 1 < words.len() {
            let next_token_str = words[i + 1].2.as_str();
            let next_next = if i + 2 < words.len() {
                Some(words[i + 2].2.as_str())
            } else {
                None
            };

            if let Some((_, _, consumed)) =
                parse_amount_and_unit_with_lex(next_token_str, next_next, false, lex)
            {
                matched_kind = Some(SyntaxType::Recurrence);
                words_consumed = 1 + 1 + consumed;
            } else {
                let parts: Vec<&str> = next_token_str.split(',').map(|s| s.trim()).collect();
                let all_weekdays = parts
                    .iter()
                    .all(|part| parse_weekday_code_with_lex(part, lex).is_some());

                if all_weekdays && !parts.is_empty() {
                    matched_kind = Some(SyntaxType::Recurrence);
                    words_consumed = 2;
                }
            }
        } else if matched_kind.is_none()
            && ((is_due_or_recur && matches!(exact, Some(&ExactToken::Unit(_))))
                || pref == Some(PrefixToken::Recur))
        {
            matched_kind = Some(SyntaxType::Recurrence);
            has_recurrence = true;
        }

        if matched_kind == Some(SyntaxType::Recurrence) {
            has_recurrence = true;
        }

        if matched_kind == Some(SyntaxType::Recurrence) && i + words_consumed < words.len() {
            let potential_time = &words[i + words_consumed].2;
            if is_time_format(potential_time) {
                words_consumed += 1;
            }
        }

        if matched_kind.is_none()
            && has_recurrence
            && (exact == Some(&ExactToken::Until) || exact == Some(&ExactToken::Except))
            && i + 1 < words.len()
        {
            let next_token_str = words[i + 1].2.as_str();

            let is_list = if next_token_str.contains(',') {
                true
            } else if parse_smart_date_with_lex(next_token_str, lex).is_some()
                || parse_next_date_with_lex(next_token_str, lex).is_some()
            {
                if i + 2 < words.len() && is_time_format(&words[i + 2].2) {
                    words_consumed = 3;
                }
                true
            } else {
                parse_weekday_code_with_lex(next_token_str, lex).is_some()
                    || parse_month_code_with_lex(next_token_str, lex).is_some()
            };

            if is_list {
                matched_kind = Some(SyntaxType::Recurrence);
                if words_consumed == 1 {
                    words_consumed = 2;
                }
            }
        }

        if matched_kind.is_none() {
            let (is_start, clean_word) = match pref {
                Some(PrefixToken::StartDue) => (true, rem),
                Some(PrefixToken::Start) => (true, rem),
                Some(PrefixToken::Due) => (false, rem),
                _ => (false, ""),
            };

            let actual_clean = clean_word;

            if !actual_clean.is_empty() {
                let is_next = lex.exact.get(clean_word) == Some(&ExactToken::Next);
                let is_in = lex.exact.get(clean_word) == Some(&ExactToken::In);

                if is_next && i + 1 < words.len() {
                    let next_str = words[i + 1].2.as_str();
                    let is_weekday = parse_weekday_code_with_lex(next_str, lex).is_some();
                    let is_unit = lex
                        .exact
                        .get(&next_str.to_lowercase())
                        .map(|t| matches!(t, ExactToken::Unit(_)))
                        .unwrap_or(false);
                    if is_unit || is_weekday {
                        matched_kind = Some(if is_start {
                            SyntaxType::StartDate
                        } else {
                            SyntaxType::DueDate
                        });
                        words_consumed = 2;
                        if i + 2 < words.len() && is_time_format(&words[i + 2].2) {
                            words_consumed += 1;
                        }
                    }
                } else if is_in && i + 1 < words.len() {
                    let next_token_str = words[i + 1].2.as_str();
                    let next_next = if i + 2 < words.len() {
                        Some(words[i + 2].2.as_str())
                    } else {
                        None
                    };

                    if let Some((_, _, consumed)) =
                        parse_amount_and_unit_with_lex(next_token_str, next_next, false, lex)
                    {
                        matched_kind = Some(if is_start {
                            SyntaxType::StartDate
                        } else {
                            SyntaxType::DueDate
                        });
                        words_consumed = 1 + 1 + consumed;
                        if i + words_consumed < words.len()
                            && is_time_format(&words[i + words_consumed].2)
                        {
                            words_consumed += 1;
                        }
                    }
                } else {
                    if parse_smart_date_with_lex(clean_word, lex).is_some()
                        || parse_weekday_code_with_lex(clean_word, lex).is_some()
                        || is_time_format(clean_word)
                    {
                        matched_kind = Some(if is_start {
                            SyntaxType::StartDate
                        } else {
                            SyntaxType::DueDate
                        });
                        if i + 1 < words.len() && is_time_format(&words[i + 1].2) {
                            words_consumed = 2;
                        }
                    }
                }
            }
        }

        if matched_kind.is_none() && pref == Some(PrefixToken::Reminder) {
            matched_kind = Some(SyntaxType::Reminder);

            let clean_val = if rem.is_empty() && i + 1 < words.len() {
                words_consumed += 1;
                &words[i + 1].2
            } else {
                rem_original
            };

            let find_next_token = |start_idx: usize| -> Option<usize> {
                (start_idx..words.len()).find(|&idx| !words[idx].2.trim().is_empty())
            };

            let is_in = lex.exact.get(&clean_val.to_lowercase()) == Some(&ExactToken::In);
            let is_next = lex.exact.get(&clean_val.to_lowercase()) == Some(&ExactToken::Next);

            if is_in
                || (clean_val.is_empty()
                    && find_next_token(i + words_consumed)
                        .map(|idx| {
                            let w = words[idx].2.to_lowercase();
                            lex.exact.get(&w) == Some(&ExactToken::In)
                        })
                        .unwrap_or(false))
            {
                let mut offset = 0;
                if clean_val.is_empty() {
                    offset = 1;
                }

                if let Some(next_idx) = find_next_token(i + words_consumed + offset) {
                    let next_token_str = words[next_idx].2.as_str();
                    let next_next_idx = find_next_token(next_idx + 1);
                    let next_next = next_next_idx.map(|idx| words[idx].2.as_str());

                    if let Some((_, _, consumed)) =
                        parse_amount_and_unit_with_lex(next_token_str, next_next, false, lex)
                    {
                        let last_idx = if consumed > 0 {
                            next_next_idx.unwrap_or(next_idx)
                        } else {
                            next_idx
                        };
                        words_consumed = last_idx - i + 1;
                    }
                }
            } else if is_next
                || (clean_val.is_empty()
                    && find_next_token(i + words_consumed)
                        .map(|idx| {
                            let w = words[idx].2.to_lowercase();
                            lex.exact.get(&w) == Some(&ExactToken::Next)
                        })
                        .unwrap_or(false))
            {
                let mut offset = 0;
                if clean_val.is_empty() {
                    offset = 1;
                }

                if let Some(next_idx) = find_next_token(i + words_consumed + offset) {
                    let next_word = &words[next_idx].2;
                    let is_unit = lex
                        .exact
                        .get(&next_word.to_lowercase())
                        .map(|t| matches!(t, ExactToken::Unit(_)))
                        .unwrap_or(false);
                    if parse_weekday_code_with_lex(next_word, lex).is_some() || is_unit {
                        words_consumed = next_idx - i + 1;

                        if let Some(time_idx) = find_next_token(next_idx + 1)
                            && is_time_format(&words[time_idx].2)
                        {
                            words_consumed = time_idx - i + 1;
                        }
                    }
                }
            } else if !clean_val.is_empty()
                && (parse_smart_date_with_lex(clean_val, lex).is_some()
                    || parse_next_date_with_lex(clean_val, lex).is_some())
            {
                if let Some(next_idx) = find_next_token(i + words_consumed)
                    && is_time_format(&words[next_idx].2)
                {
                    words_consumed = next_idx - i + 1;
                }
            } else if clean_val.is_empty()
                && let Some(next_idx) = find_next_token(i + words_consumed)
            {
                let next_word = &words[next_idx].2;
                if parse_smart_date_with_lex(next_word, lex).is_some()
                    || parse_next_date_with_lex(next_word, lex).is_some()
                {
                    words_consumed = next_idx - i + 1;
                    if let Some(time_idx) = find_next_token(next_idx + 1)
                        && is_time_format(&words[time_idx].2)
                    {
                        words_consumed = time_idx - i + 1;
                    }
                } else if parse_duration_with_lex(next_word, lex).is_some()
                    || is_time_format(next_word)
                {
                    words_consumed = next_idx - i + 1;
                }
            }
        }

        if matched_kind.is_none() {
            if word.starts_with("@@") || pref == Some(PrefixToken::Loc) {
                matched_kind = Some(SyntaxType::Location);
            } else if pref == Some(PrefixToken::Url)
                || (word.starts_with("[[") && word.ends_with("]]"))
            {
                matched_kind = Some(SyntaxType::Url);
            } else if word_lower == "+cal" || word_lower == "-cal" {
                matched_kind = Some(SyntaxType::Calendar);
            } else if word_lower == "+pin" || word_lower == "-pin" {
                matched_kind = Some(SyntaxType::Pin);
            } else if pref == Some(PrefixToken::Geo) {
                let mut temp_consumed = 1;
                let mut raw_val = rem_original.to_string();
                let is_geo_word = |w: &str| -> bool {
                    let upper = w.to_uppercase();
                    if matches!(
                        upper.as_str(),
                        "N" | "S" | "E" | "W" | "N," | "S," | "E," | "W,"
                    ) {
                        return true;
                    }
                    if !w.chars().any(|c| c.is_ascii_digit()) {
                        return false;
                    }
                    w.chars()
                        .all(|c| c.is_ascii_digit() || " .,-°NSEWnsew".contains(c))
                };

                while i + temp_consumed < words.len() {
                    let next_word = &words[i + temp_consumed].2;
                    if is_geo_word(next_word) {
                        raw_val.push(' ');
                        raw_val.push_str(next_word);
                        temp_consumed += 1;
                    } else {
                        break;
                    }
                }

                if is_valid_geo(&raw_val) {
                    matched_kind = Some(SyntaxType::Geo);
                    words_consumed = temp_consumed;
                }
            } else if pref == Some(PrefixToken::Desc) {
                matched_kind = Some(SyntaxType::Description);
            } else if pref == Some(PrefixToken::Goal) {
                matched_kind = Some(SyntaxType::Goal);
            } else if word.starts_with('!') && word.len() > 1 && word[1..].parse::<u8>().is_ok() {
                matched_kind = Some(SyntaxType::Priority);
            } else if word.starts_with('~')
                || pref == Some(PrefixToken::Duration)
                || pref == Some(PrefixToken::Spent)
            {
                matched_kind = Some(SyntaxType::Duration);
            } else if pref == Some(PrefixToken::Done) {
                matched_kind = Some(SyntaxType::DueDate);
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
    let mut brace_depth: usize = 0;
    let mut escaped = false;
    let chars = input.char_indices().peekable();

    for (idx, c) in chars {
        if current.is_empty() && !in_quote && brace_depth == 0 && !c.is_whitespace() {
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
            '"' if brace_depth == 0 => {
                in_quote = !in_quote;
                current.push(c);
            }
            '{' if !in_quote => {
                brace_depth += 1;
                current.push(c);
            }
            '}' if !in_quote => {
                brace_depth = brace_depth.saturating_sub(1);
                current.push(c);
            }
            ws if ws.is_whitespace() && !in_quote && brace_depth == 0 => {
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

pub fn expand_braces(input: &str) -> Vec<String> {
    let mut results = Vec::new();
    expand_braces_recursive("", input.trim(), &mut results);
    results
}

fn split_by_comma_at_depth_zero(input: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0;
    let mut last_idx = 0;
    let mut in_quote = false;
    let mut escaped = false;
    for (i, c) in input.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if c == '\\' {
            escaped = true;
            continue;
        }
        if c == '"' {
            in_quote = !in_quote;
        }
        if !in_quote {
            if c == '{' {
                depth += 1;
            } else if c == '}' {
                if depth > 0 {
                    depth -= 1;
                }
            } else if c == ',' && depth == 0 {
                parts.push(&input[last_idx..i]);
                last_idx = i + c.len_utf8();
            }
        }
    }
    parts.push(&input[last_idx..]);
    parts
}

fn expand_braces_recursive(prefix: &str, input: &str, results: &mut Vec<String>) {
    let parts = split_by_comma_at_depth_zero(input);
    for part in parts {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        let mut brace_start = None;
        let mut depth = 0;
        let mut in_quote = false;
        let mut escaped = false;
        for (i, c) in part.char_indices() {
            if escaped {
                escaped = false;
                continue;
            }
            if c == '\\' {
                escaped = true;
                continue;
            }
            if c == '"' {
                in_quote = !in_quote;
            }
            if !in_quote {
                if c == '{' {
                    if depth == 0 {
                        brace_start = Some(i);
                        break;
                    }
                    depth += 1;
                } else if c == '}' && depth > 0 {
                    depth -= 1;
                }
            }
        }

        if let Some(start) = brace_start
            && part.ends_with('}')
        {
            let base = part[..start].trim();
            let inner = &part[start + 1..part.len() - 1];

            let new_prefix = if base.is_empty() {
                prefix.to_string()
            } else {
                let sep = if base.ends_with('=')
                    || base.ends_with(':')
                    || prefix.is_empty()
                    || prefix.ends_with(':')
                {
                    ""
                } else {
                    ":"
                };
                format!("{}{}{}", prefix, sep, base)
            };

            let pass_prefix = if new_prefix.is_empty() {
                String::new()
            } else if new_prefix.ends_with('=') || new_prefix.ends_with(':') {
                new_prefix
            } else {
                format!("{}:", new_prefix)
            };

            expand_braces_recursive(&pass_prefix, inner, results);
            continue;
        }

        results.push(format!("{}{}", prefix, part));
    }
}

fn collect_alias_expansions(
    token: &str,
    aliases: &HashMap<String, Vec<String>>,
    visited: &mut HashSet<String>,
    depth: usize,
) -> Vec<String> {
    // Hard limit to prevent stack overflows from manually corrupted config files
    if depth > 10 {
        return Vec::new();
    }

    let mut results = Vec::new();
    let mut search_key: Option<String> = None;

    // 1. Tag Lookup
    if token.starts_with('#') {
        search_key = Some(strip_quotes(token.trim_start_matches('#')));
    }
    // 2. Location Lookup
    else if token.starts_with("@@") || token.to_lowercase().starts_with("loc:") {
        let raw = if token.starts_with("@@") {
            token.trim_start_matches("@@")
        } else {
            &token[4..]
        };
        let clean = strip_quotes(raw);
        if !clean.is_empty() {
            search_key = Some(format!("@@{}", clean));
        }
    }

    if let Some(key) = search_key {
        let mut search = key.as_str();
        let mut found_values = None;
        let mut matched_key = String::new();

        loop {
            if let Some(vals) = aliases.get(search) {
                found_values = Some(vals);
                matched_key = search.to_string();
                break;
            }

            // Handle hierarchy (tag:subtag or @@loc:subloc)
            if let Some(idx) = search.rfind(':') {
                if idx > 0 {
                    search = &search[..idx];
                } else {
                    break;
                }
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
                let child_expansions = collect_alias_expansions(val, aliases, visited, depth + 1);
                results.extend(child_expansions);
                results.push(val.clone());
            }
        }
    }
    results
}

pub fn parse_amount_and_unit(
    first: &str,
    second: Option<&str>,
    strict_unit: bool,
) -> Option<(u32, String, usize)> {
    let lex_guard = LEXICON.read().unwrap();
    parse_amount_and_unit_with_lex(first, second, strict_unit, &lex_guard)
}

pub fn parse_amount_and_unit_with_lex(
    first: &str,
    second: Option<&str>,
    _strict_unit: bool,
    lex: &ParserLexicon,
) -> Option<(u32, String, usize)> {
    if let Some(next_token) = second
        && let Some(amt) = parse_english_number_with_lex(first, lex)
    {
        let unit = next_token.to_lowercase();
        if let Some(ExactToken::Unit(u)) = lex.exact.get(&unit) {
            return Some((amt, u.to_canonical().to_string(), 1));
        }
    }
    let lower = first.to_lowercase();
    let (amt_str, unit_str) = if let Some(idx) = lower.find(|c: char| !c.is_numeric()) {
        lower.split_at(idx)
    } else {
        return None;
    };
    if let Ok(amt) = amt_str.parse::<u32>()
        && let Some(ExactToken::Unit(u)) = lex.exact.get(unit_str)
    {
        return Some((amt, u.to_canonical().to_string(), 0));
    }
    None
}

fn parse_english_number_with_lex(s: &str, lex: &ParserLexicon) -> Option<u32> {
    if let Ok(n) = s.parse::<u32>() {
        return Some(n);
    }
    if let Some(ExactToken::Number(n)) = lex.exact.get(&s.to_lowercase()) {
        return Some(*n);
    }
    None
}

fn parse_freq_from_unit(u: &str) -> &'static str {
    let s = u.to_uppercase();
    match s.as_str() {
        "D" | "DAILY" => "DAILY",
        "W" | "WEEKLY" => "WEEKLY",
        "MO" | "MONTHLY" => "MONTHLY",
        "Y" | "YEARLY" => "YEARLY",
        _ => "",
    }
}

fn code_to_full_day(code: &str) -> &'static str {
    match code {
        "MO" => "monday",
        "TU" => "tuesday",
        "WE" => "wednesday",
        "TH" => "thursday",
        "FR" => "friday",
        "SA" => "saturday",
        "SU" => "sunday",
        _ => "",
    }
}

fn month_num_to_short_name(num: u32) -> &'static str {
    match num {
        1 => "jan",
        2 => "feb",
        3 => "mar",
        4 => "apr",
        5 => "may",
        6 => "jun",
        7 => "jul",
        8 => "aug",
        9 => "sep",
        10 => "oct",
        11 => "nov",
        12 => "dec",
        _ => "",
    }
}

pub fn prettify_recurrence(rrule: &str, is_relative: bool) -> String {
    let mut freq = "";
    let mut interval = "";
    let mut byday = "";
    let mut bymonth = "";
    let mut until = "";

    // Parse RRULE components
    for part in rrule.split(';') {
        if let Some(v) = part.strip_prefix("FREQ=") {
            freq = v;
        } else if let Some(v) = part.strip_prefix("INTERVAL=") {
            interval = v;
        } else if let Some(v) = part.strip_prefix("BYDAY=") {
            byday = v;
        } else if let Some(v) = part.strip_prefix("BYMONTH=") {
            bymonth = v;
        } else if let Some(v) = part.strip_prefix("UNTIL=") {
            until = v;
        }
    }

    // Format UNTIL if present
    let mut until_str = String::new();
    if !until.is_empty() {
        let date_part = until.split('T').next().unwrap_or(until);
        if date_part.len() >= 8 {
            until_str = format!(
                " until {}-{}-{}",
                &date_part[0..4],
                &date_part[4..6],
                &date_part[6..8]
            );
        } else {
            until_str = format!(" until {}", until);
        }
    }

    // 1. Handle Weekday Logic (Inclusions vs Exclusions)
    // Only apply if interval is 1 (standard) or empty
    if freq == "WEEKLY" && !byday.is_empty() && (interval.is_empty() || interval == "1") {
        let days: Vec<&str> = byday.split(',').collect();
        let all_codes = ["MO", "TU", "WE", "TH", "FR", "SA", "SU"];

        // A. Single Day -> @every monday or @after monday
        if days.len() == 1 && bymonth.is_empty() {
            let d_name = code_to_full_day(days[0]);
            if !d_name.is_empty() {
                let prefix = if is_relative { "@after" } else { "@every" };
                return format!("{} {}{}", prefix, d_name, until_str);
            }
        }

        // B. 2-3 Days -> @every monday,tuesday,wednesday or @after monday,tuesday,wednesday
        if days.len() >= 2 && days.len() <= 3 && bymonth.is_empty() {
            let day_names: Vec<String> = days
                .iter()
                .map(|code| code_to_full_day(code).to_string())
                .filter(|name| !name.is_empty())
                .collect();

            if day_names.len() == days.len() {
                let prefix = if is_relative { "@after" } else { "@every" };
                return format!("{} {}{}", prefix, day_names.join(","), until_str);
            }
        }

        // C. Majority Days (4+) -> @daily except ...
        // If we have >= 4 days, listing exclusions is cleaner than listing inclusions
        if days.len() >= 4 {
            let missing_days: Vec<String> = all_codes
                .iter()
                .filter(|code| !days.contains(code))
                .map(|code| code_to_full_day(code).to_string())
                .collect();

            // Check if we also have month exclusions to combine
            if !bymonth.is_empty() {
                let months: Vec<u32> = bymonth
                    .split(',')
                    .filter_map(|m| m.parse::<u32>().ok())
                    .collect();

                let missing_months: Vec<String> = (1..=12)
                    .filter(|m| !months.contains(m))
                    .map(|m| month_num_to_short_name(m).to_string())
                    .collect();

                if !missing_months.is_empty() {
                    // Combine both day and month exclusions
                    let mut combined = missing_days;
                    combined.extend(missing_months);
                    return format!("@daily{} except {}", until_str, combined.join(","));
                }
            }

            if missing_days.is_empty() {
                // All days present -> @daily (or check month logic if bymonth present)
                if bymonth.is_empty() {
                    return format!("@daily{}", until_str);
                }
                // If bymonth is present, fall through to month logic section
            } else {
                // @daily except x,y
                return format!("@daily{} except {}", until_str, missing_days.join(","));
            }
        }
    }

    // 2. Handle Month Logic (Exclusions only)
    // Show except format whenever BYMONTH is present (more user-friendly than raw RRULE)
    // Works for DAILY, WEEKLY, MONTHLY, and YEARLY frequencies
    if !bymonth.is_empty() && (interval.is_empty() || interval == "1") {
        let months: Vec<u32> = bymonth
            .split(',')
            .filter_map(|m| m.parse::<u32>().ok())
            .collect();

        let missing: Vec<String> = (1..=12)
            .filter(|m| !months.contains(m))
            .map(|m| month_num_to_short_name(m).to_string())
            .collect();

        if missing.is_empty() {
            // All 12 months present -> use base frequency without except
            // Continue to section 4 to handle the base frequency
        } else {
            // 1-11 months excluded -> @<freq> except x,y,z
            let base = match freq {
                "DAILY" => "@daily",
                "WEEKLY" => "@weekly",
                "MONTHLY" => "@monthly",
                "YEARLY" => "@yearly",
                _ => "",
            };
            if !base.is_empty() {
                return format!("{}{} except {}", base, until_str, missing.join(","));
            }
        }
    }

    // 3. Handle Custom Intervals (e.g. @every 3 days or @after 3d)
    if !freq.is_empty() && !interval.is_empty() && interval != "1" {
        let unit = match freq {
            "DAILY" => {
                if is_relative {
                    "d"
                } else {
                    "days"
                }
            }
            "WEEKLY" => {
                if is_relative {
                    "w"
                } else {
                    "weeks"
                }
            }
            "MONTHLY" => {
                if is_relative {
                    "mo"
                } else {
                    "months"
                }
            }
            "YEARLY" => {
                if is_relative {
                    "y"
                } else {
                    "years"
                }
            }
            _ => "",
        };
        if !unit.is_empty() {
            if is_relative {
                return format!("@after {}{}{}", interval, unit, until_str);
            } else {
                return format!("@every {} {}{}", interval, unit, until_str);
            }
        }
    }

    // 4. Handle Standard Presets (@daily, @monthly... or @after 1d, @after 1w)
    if !freq.is_empty() {
        if is_relative {
            let unit = match freq {
                "DAILY" => "1d",
                "WEEKLY" => "1w",
                "MONTHLY" => "1mo",
                "YEARLY" => "1y",
                _ => "",
            };
            if !unit.is_empty() {
                return format!("@after {}{}", unit, until_str);
            }
        } else {
            let base = match freq {
                "DAILY" => "@daily",
                "WEEKLY" => "@weekly",
                "MONTHLY" => "@monthly",
                "YEARLY" => "@yearly",
                _ => "",
            };
            if !base.is_empty() {
                return format!("{}{}", base, until_str);
            }
        }
    }

    // 5. Fallback to raw string
    format!("rec:{}", rrule)
}

pub fn parse_duration(val: &str) -> Option<u32> {
    let lex_guard = LEXICON.read().unwrap();
    parse_duration_with_lex(val, &lex_guard)
}

pub fn parse_duration_with_lex(val: &str, lex: &ParserLexicon) -> Option<u32> {
    let lower = val.to_lowercase();
    let (amt_str, unit_str) = if let Some(idx) = lower.find(|c: char| !c.is_numeric()) {
        lower.split_at(idx)
    } else {
        return None;
    };
    if let Ok(n) = amt_str.parse::<u32>() {
        match lex.exact.get(unit_str) {
            Some(ExactToken::Unit(LexiconUnit::Minutes)) => return Some(n),
            Some(ExactToken::Unit(LexiconUnit::Hours)) => return Some(n * 60),
            Some(ExactToken::Unit(LexiconUnit::Days)) => return Some(n * 24 * 60),
            Some(ExactToken::Unit(LexiconUnit::Weeks)) => return Some(n * 7 * 24 * 60),
            Some(ExactToken::Unit(LexiconUnit::Months)) => return Some(n * 30 * 24 * 60),
            Some(ExactToken::Unit(LexiconUnit::Years)) => return Some(n * 365 * 24 * 60),
            _ => {}
        }
    }
    None
}

pub fn parse_duration_range(val: &str) -> Option<(u32, Option<u32>)> {
    let lex_guard = LEXICON.read().unwrap();
    parse_duration_range_with_lex(val, &lex_guard)
}

pub fn parse_duration_range_with_lex(val: &str, lex: &ParserLexicon) -> Option<(u32, Option<u32>)> {
    if let Some((left, right)) = val.split_once('-') {
        let min = parse_duration_with_lex(left, lex)?;
        let max = parse_duration_with_lex(right, lex)?;
        if max >= min {
            return Some((min, Some(max)));
        }
        return Some((min, None));
    }
    let single = parse_duration_with_lex(val, lex)?;
    Some((single, None))
}

pub fn format_goal_duration(current_mins: u32, target_mins: u32) -> (String, String) {
    let (c_str, t_str) = if target_mins > 0 && target_mins.is_multiple_of(1440) {
        let t = target_mins / 1440;
        let c = current_mins as f32 / 1440.0;
        (format!("{:.1}d", c).replace(".0d", "d"), format!("{}d", t))
    } else if target_mins > 0 && target_mins.is_multiple_of(60) {
        let t = target_mins / 60;
        let c = current_mins as f32 / 60.0;
        (format!("{:.1}h", c).replace(".0h", "h"), format!("{}h", t))
    } else {
        (format!("{}m", current_mins), format!("{}m", target_mins))
    };

    let c_suffix: String = c_str.chars().filter(|c| c.is_ascii_alphabetic()).collect();
    let t_suffix: String = t_str.chars().filter(|c| c.is_ascii_alphabetic()).collect();

    if c_suffix == t_suffix && !c_suffix.is_empty() {
        let stripped = c_str.trim_end_matches(&c_suffix);
        (stripped.to_string(), t_str)
    } else {
        if current_mins == 0 {
            ("0".to_string(), t_str)
        } else {
            (c_str, t_str)
        }
    }
}

pub fn format_duration_human(mins: u32) -> String {
    let lex = LEXICON.read().unwrap();
    if mins == 0 {
        return format!("0{}", lex.unit_m);
    }
    let mut parts = Vec::new();
    let mut remaining = mins;

    let years = remaining / 525600;
    if years > 0 {
        parts.push(format!("{}{}", years, lex.unit_y));
        remaining %= 525600;
    }
    let months = remaining / 43200;
    if months > 0 {
        parts.push(format!("{}{}", months, lex.unit_mo));
        remaining %= 43200;
    }
    let weeks = remaining / 10080;
    if weeks > 0 {
        parts.push(format!("{}{}", weeks, lex.unit_w));
        remaining %= 10080;
    }
    let days = remaining / 1440;
    if days > 0 {
        parts.push(format!("{}{}", days, lex.unit_d));
        remaining %= 1440;
    }
    let hours = remaining / 60;
    if hours > 0 {
        parts.push(format!("{}{}", hours, lex.unit_h));
        remaining %= 60;
    }
    if remaining > 0 || parts.is_empty() {
        parts.push(format!("{}{}", remaining, lex.unit_m));
    }

    parts.join(" ")
}

pub fn format_duration_compact(mins: u32) -> String {
    if mins == 0 {
        return "".to_string();
    }
    if mins.is_multiple_of(525600) {
        format!("{}y", mins / 525600)
    } else if mins.is_multiple_of(43200) {
        format!("{}mo", mins / 43200)
    } else if mins.is_multiple_of(10080) {
        format!("{}w", mins / 10080)
    } else if mins.is_multiple_of(1440) {
        format!("{}d", mins / 1440)
    } else if mins.is_multiple_of(60) {
        format!("{}h", mins / 60)
    } else {
        format!("{}m", mins)
    }
}

fn parse_recurrence(val: &str) -> Option<String> {
    let upper = val.to_uppercase();
    match upper.as_str() {
        "DAILY" => Some("FREQ=DAILY".to_string()),
        "WEEKLY" => Some("FREQ=WEEKLY".to_string()),
        "MONTHLY" => Some("FREQ=MONTHLY".to_string()),
        "YEARLY" => Some("FREQ=YEARLY".to_string()),
        _ => {
            if upper.starts_with("FREQ=") {
                Some(upper)
            } else {
                None
            }
        }
    }
}

pub fn parse_smart_date(val: &str) -> Option<DateType> {
    let lex_guard = LEXICON.read().unwrap();
    parse_smart_date_with_lex(val, &lex_guard)
}

pub fn parse_smart_date_with_lex(val: &str, lex: &ParserLexicon) -> Option<DateType> {
    if let Ok(date) = NaiveDate::parse_from_str(val, "%Y-%m-%d") {
        return Some(DateType::AllDay(date));
    }
    if val.len() == 8
        && val.chars().all(|c| c.is_numeric())
        && let Ok(date) = NaiveDate::parse_from_str(val, "%Y%m%d")
    {
        return Some(DateType::AllDay(date));
    }

    if let Ok(date) = NaiveDate::parse_from_str(val, &lex.date_format) {
        return Some(DateType::AllDay(date));
    }

    if val.len() == 7 && val.chars().nth(4) == Some('-') {
        let y = val[0..4].parse::<i32>().ok();
        let m = val[5..7].parse::<u32>().ok();
        if let (Some(year), Some(month)) = (y, m)
            && (1..=12).contains(&month)
        {
            return Some(DateType::Month(year, month));
        }
    }
    if val.len() == 4
        && val.chars().all(|c| c.is_numeric())
        && let Ok(y) = val.parse::<i32>()
    {
        return Some(DateType::Year(y));
    }

    let now = Local::now().date_naive();
    let lower = val.to_lowercase();

    match lex.exact.get(&lower) {
        Some(ExactToken::Now) => return Some(DateType::Specific(Utc::now())),
        Some(ExactToken::Today) => return Some(DateType::AllDay(now)),
        Some(ExactToken::Tomorrow) => return Some(DateType::AllDay(now + Duration::days(1))),
        Some(ExactToken::Yesterday) => return Some(DateType::AllDay(now - Duration::days(1))),
        _ => {}
    }

    let (amt_str, unit_str) = if let Some(idx) = lower.find(|c: char| !c.is_numeric()) {
        lower.split_at(idx)
    } else {
        return None;
    };
    if let Ok(n) = amt_str.parse::<i64>() {
        match lex.exact.get(unit_str) {
            Some(ExactToken::Unit(LexiconUnit::Days)) => {
                return Some(DateType::AllDay(now + Duration::days(n)));
            }
            Some(ExactToken::Unit(LexiconUnit::Weeks)) => {
                return Some(DateType::AllDay(now + Duration::days(n * 7)));
            }
            Some(ExactToken::Unit(LexiconUnit::Months)) => {
                return Some(DateType::AllDay(now + Duration::days(n * 30)));
            }
            Some(ExactToken::Unit(LexiconUnit::Years)) => {
                return Some(DateType::AllDay(now + Duration::days(n * 365)));
            }
            _ => {}
        }
    }
    None
}

pub fn parse_weekday_code(s: &str) -> Option<&'static str> {
    let lex_guard = LEXICON.read().unwrap();
    parse_weekday_code_with_lex(s, &lex_guard)
}

pub fn parse_weekday_code_with_lex(s: &str, lex: &ParserLexicon) -> Option<&'static str> {
    if let Some(ExactToken::Weekday(c)) = lex.exact.get(&s.to_lowercase()) {
        Some(c)
    } else {
        None
    }
}

pub fn parse_month_code(s: &str) -> Option<u32> {
    let lex_guard = LEXICON.read().unwrap();
    parse_month_code_with_lex(s, &lex_guard)
}

pub fn parse_month_code_with_lex(s: &str, lex: &ParserLexicon) -> Option<u32> {
    if let Some(ExactToken::Month(m)) = lex.exact.get(&s.to_lowercase()) {
        Some(*m)
    } else {
        None
    }
}

pub fn parse_weekday_date(s: &str) -> Option<NaiveDate> {
    let lex_guard = LEXICON.read().unwrap();
    parse_next_date_with_lex(s, &lex_guard)
}

pub fn parse_weekday_date_with_lex(s: &str, lex: &ParserLexicon) -> Option<NaiveDate> {
    parse_next_date_with_lex(s, lex)
}

pub fn parse_next_date(unit: &str) -> Option<NaiveDate> {
    let lex_guard = LEXICON.read().unwrap();
    parse_next_date_with_lex(unit, &lex_guard)
}

pub fn parse_next_date_with_lex(unit: &str, lex: &ParserLexicon) -> Option<NaiveDate> {
    let now = Local::now().date_naive();
    let lower = unit.to_lowercase();

    if let Some(ExactToken::Unit(u)) = lex.exact.get(&lower) {
        match u {
            LexiconUnit::Weeks => return Some(now + Duration::days(7)),
            LexiconUnit::Months => return Some(now + Duration::days(30)),
            LexiconUnit::Years => return Some(now + Duration::days(365)),
            _ => {}
        }
    }

    if let Some(code) = parse_weekday_code_with_lex(unit, lex) {
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

pub fn parse_in_date(amount: u32, unit: &str) -> Option<NaiveDate> {
    let lex_guard = LEXICON.read().unwrap();
    parse_in_date_with_lex(amount, unit, &lex_guard)
}

pub fn parse_in_date_with_lex(amount: u32, unit: &str, lex: &ParserLexicon) -> Option<NaiveDate> {
    let now = Local::now().date_naive();
    if let Some(ExactToken::Unit(u)) = lex.exact.get(&unit.to_lowercase()) {
        let days = match u {
            LexiconUnit::Days => amount as i64,
            LexiconUnit::Weeks => amount as i64 * 7,
            LexiconUnit::Months => amount as i64 * 30,
            LexiconUnit::Years => amount as i64 * 365,
            _ => return None,
        };
        return Some(now + Duration::days(days));
    }
    None
}

fn next_weekday(from: NaiveDate, target: chrono::Weekday) -> Option<NaiveDate> {
    let mut d = from + Duration::days(1);
    while d.weekday() != target {
        d += Duration::days(1);
    }
    Some(d)
}

fn calculate_first_occurrence(rrule: &str, today: NaiveDate) -> NaiveDate {
    // Parse RRULE to determine first occurrence date
    let mut freq = "";
    let mut byday = "";
    let mut bymonth = "";

    for part in rrule.split(';') {
        if let Some(v) = part.strip_prefix("FREQ=") {
            freq = v;
        } else if let Some(v) = part.strip_prefix("BYDAY=") {
            byday = v;
        } else if let Some(v) = part.strip_prefix("BYMONTH=") {
            bymonth = v;
        }
    }

    // For WEEKLY with BYDAY, find the next occurrence of any listed day
    if freq == "WEEKLY" && !byday.is_empty() {
        let days: Vec<&str> = byday.split(',').collect();

        // Parse BYMONTH constraint if present
        let allowed_months: Vec<u32> = if !bymonth.is_empty() {
            bymonth
                .split(',')
                .filter_map(|m| m.parse::<u32>().ok())
                .collect()
        } else {
            vec![]
        };

        // Helper to check if a date is in an allowed month
        let is_month_allowed = |date: NaiveDate| -> bool {
            allowed_months.is_empty() || allowed_months.contains(&date.month())
        };

        let mut earliest: Option<NaiveDate> = None;

        for day_code in days {
            let weekday = match day_code {
                "MO" => chrono::Weekday::Mon,
                "TU" => chrono::Weekday::Tue,
                "WE" => chrono::Weekday::Wed,
                "TH" => chrono::Weekday::Thu,
                "FR" => chrono::Weekday::Fri,
                "SA" => chrono::Weekday::Sat,
                "SU" => chrono::Weekday::Sun,
                _ => continue,
            };

            // Check if today matches (both weekday AND month if BYMONTH is present)
            if today.weekday() == weekday && is_month_allowed(today) {
                return today;
            }

            // Otherwise find next occurrence of this weekday that's in an allowed month
            let mut candidate = today;
            for _ in 0..60 {
                // Check up to ~8 weeks ahead to find valid occurrence
                if let Some(next) = next_weekday(candidate, weekday) {
                    if is_month_allowed(next) {
                        if earliest.is_none() || next < earliest.unwrap() {
                            earliest = Some(next);
                        }
                        break;
                    }
                    candidate = next;
                } else {
                    break;
                }
            }
        }

        if let Some(date) = earliest {
            return date;
        }
    }

    // For MONTHLY with BYMONTH, find the next occurrence in an allowed month
    if (freq == "MONTHLY" || freq == "DAILY" || freq == "WEEKLY") && !bymonth.is_empty() {
        let allowed_months: Vec<u32> = bymonth
            .split(',')
            .filter_map(|m| m.parse::<u32>().ok())
            .collect();

        if !allowed_months.is_empty() {
            let current_month = today.month();

            // Check if current month is allowed
            if allowed_months.contains(&current_month) {
                return today;
            }

            // Find next allowed month
            let mut check_date = today;
            for _ in 0..12 {
                // Move to first day of next month
                if check_date.month() == 12 {
                    check_date = NaiveDate::from_ymd_opt(check_date.year() + 1, 1, 1).unwrap();
                } else {
                    check_date =
                        NaiveDate::from_ymd_opt(check_date.year(), check_date.month() + 1, 1)
                            .unwrap();
                }

                if allowed_months.contains(&check_date.month()) {
                    return check_date;
                }
            }
        }
    }

    // For DAILY or other patterns, default to today
    today
}

fn finalize_date_token(
    d: DateType,
    stream: &[String],
    next_idx: usize,
    consumed: &mut usize,
) -> (DateType, Option<DateType>) {
    match d {
        DateType::AllDay(nd) => {
            // Only try to merge a time token if it's a specific day
            if next_idx < stream.len() {
                let next_token = &stream[next_idx];
                if let Some((t1, t2)) = parse_time_range(next_token) {
                    *consumed += 1;
                    let mut end_nd = nd;
                    if t2 < t1 {
                        end_nd += Duration::days(1);
                    }
                    return (
                        DateType::Specific(crate::model::item::safe_local_to_utc(nd, t1)),
                        Some(DateType::Specific(crate::model::item::safe_local_to_utc(
                            end_nd, t2,
                        ))),
                    );
                } else if let Some(t) = parse_time_string(next_token) {
                    *consumed += 1;
                    return (
                        DateType::Specific(crate::model::item::safe_local_to_utc(nd, t)),
                        None,
                    );
                }
            }
            (DateType::AllDay(nd), None)
        }
        // Months and Years don't support specific times (14:00)
        _ => (d, None),
    }
}

pub fn normalize_geo(val: String) -> String {
    if val.eq_ignore_ascii_case("here") {
        return val;
    }
    if !val.contains('°')
        && !val.contains('N')
        && !val.contains('S')
        && !val.contains('E')
        && !val.contains('W')
        && !val.contains('n')
        && !val.contains('s')
        && !val.contains('e')
        && !val.contains('w')
    {
        return val.replace(' ', "");
    }
    let clean = val.replace(['°', ' '], "");
    let mut parts = clean.split(',');
    if let (Some(lat_str), Some(lon_str)) = (parts.next(), parts.next()) {
        let parse_part = |s: &str| -> String {
            let mut num = String::new();
            let mut sign = "";
            for c in s.chars() {
                if c == 'S' || c == 's' || c == 'W' || c == 'w' {
                    sign = "-";
                } else if c.is_numeric() || c == '.' || c == '-' {
                    num.push(c);
                }
            }
            format!("{}{}", sign, num)
        };
        return format!("{},{}", parse_part(lat_str), parse_part(lon_str));
    }
    val.replace(' ', "")
}

pub fn escape_summary(summary: &str) -> String {
    let lex_guard = LEXICON.read().unwrap();
    let mut escaped_words = Vec::new();
    for word in summary.split_whitespace() {
        if is_special_token_with_lex(word, &lex_guard) {
            escaped_words.push(format!("\\{}", word));
        } else {
            escaped_words.push(word.to_string());
        }
    }
    escaped_words.join(" ")
}

pub fn is_special_token(word: &str) -> bool {
    let lex_guard = LEXICON.read().unwrap();
    is_special_token_with_lex(word, &lex_guard)
}

pub fn is_special_token_with_lex(word: &str, lex: &ParserLexicon) -> bool {
    let lower = word.to_lowercase();
    if word.starts_with('#')
        || word.starts_with('!')
        || lower.starts_with("geo:")
        || lower == "+pin"
        || lower == "-pin"
        || lower == "+cal"
        || lower == "-cal"
    {
        return true;
    }
    if lex.match_prefix(&lower).is_some() {
        return true;
    }
    let exact = lex.exact.get(&lower);
    if exact == Some(&ExactToken::Until) || exact == Some(&ExactToken::Except) {
        return true;
    }
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
    task.unmapped_properties
        .retain(|p| p.key != "X-CFAIT-RECUR-FROM-COMPLETION");
    task.priority = 0;
    task.due = None;
    task.dtstart = None;
    task.rrule = None;
    task.estimated_duration = None;
    task.estimated_duration_max = None;
    task.location = None;
    task.url = None;
    task.geo = None;
    task.create_event = None;
    task.goal = None;
    task.categories.clear();
    task.alarms.clear();
    task.exdates.clear();
    task.percent_complete = None;

    enum PendingAlarm {
        Relative(u32),
        TimeOnly(NaiveTime),
        Absolute(DateTime<Utc>),
    }
    let mut pending_alarms = Vec::new();

    let user_tokens: Vec<String> = split_input_respecting_quotes(input)
        .into_iter()
        .map(|(_, _, s)| s)
        .collect();

    let mut background_tokens = Vec::new();
    let mut visited = HashSet::new();

    for token in &user_tokens {
        let expanded = collect_alias_expansions(token, aliases, &mut visited, 0);
        background_tokens.extend(expanded);
    }

    let mut stream = background_tokens;
    stream.extend(user_tokens);

    let mut blocked_weekdays = HashSet::new();
    let mut blocked_months = HashSet::new();
    let mut has_recurrence = false;

    let lex_guard = LEXICON.read().unwrap();
    let lex = &*lex_guard;

    let mut i = 0;
    while i < stream.len() {
        let token = &stream[i];
        let mut consumed = 1;
        let token_lower = token.to_lowercase();

        let pref_match = lex.match_prefix(&token_lower);
        let rem = pref_match
            .map(|(_, _, r)| r)
            .unwrap_or(token_lower.as_str());
        let pref = pref_match.map(|(_, p, _)| p);

        let rem_original = if let Some((p, _, _)) = pref_match {
            let char_count = p.chars().count();
            let byte_idx = token
                .char_indices()
                .nth(char_count)
                .map(|(i, _)| i)
                .unwrap_or(token.len());
            &token[byte_idx..]
        } else {
            token.as_str()
        };
        let exact = lex.exact.get(rem);

        let is_due_or_recur = pref == Some(PrefixToken::Due) || pref == Some(PrefixToken::Recur);

        // 1. Recurrence
        let is_every = is_due_or_recur && exact == Some(&ExactToken::Every);
        let is_after = pref == Some(PrefixToken::Due) && exact == Some(&ExactToken::After);

        if is_every || is_after {
            if i + 1 < stream.len() {
                let next_token_str = stream[i + 1].as_str();
                let next_next = if i + 2 < stream.len() {
                    Some(stream[i + 2].as_str())
                } else {
                    None
                };
                if let Some((interval, unit, extra_consumed)) =
                    parse_amount_and_unit_with_lex(next_token_str, next_next, false, lex)
                {
                    let freq = parse_freq_from_unit(&unit);
                    if !freq.is_empty() {
                        task.rrule = Some(format!("FREQ={};INTERVAL={}", freq, interval));
                        if is_after
                            && !task
                                .unmapped_properties
                                .iter()
                                .any(|p| p.key == "X-CFAIT-RECUR-FROM-COMPLETION")
                        {
                            task.unmapped_properties.push(crate::model::RawProperty {
                                key: "X-CFAIT-RECUR-FROM-COMPLETION".to_string(),
                                value: "TRUE".to_string(),
                                params: vec![],
                            });
                        }
                        has_recurrence = true;
                        consumed = 1 + 1 + extra_consumed;
                    } else {
                        summary_words.push(unescape(token));
                    }
                } else {
                    // Weekdays
                    let parts: Vec<&str> = next_token_str.split(',').map(|s| s.trim()).collect();
                    let weekday_codes: Vec<String> = parts
                        .iter()
                        .filter_map(|part| {
                            parse_weekday_code_with_lex(part, lex).map(|s| s.to_string())
                        })
                        .collect();

                    if !weekday_codes.is_empty() && weekday_codes.len() == parts.len() {
                        task.rrule = Some(format!("FREQ=WEEKLY;BYDAY={}", weekday_codes.join(",")));
                        if is_after
                            && !task
                                .unmapped_properties
                                .iter()
                                .any(|p| p.key == "X-CFAIT-RECUR-FROM-COMPLETION")
                        {
                            task.unmapped_properties.push(crate::model::RawProperty {
                                key: "X-CFAIT-RECUR-FROM-COMPLETION".to_string(),
                                value: "TRUE".to_string(),
                                params: vec![],
                            });
                        }
                        has_recurrence = true;
                        consumed = 2;
                    } else {
                        summary_words.push(unescape(token));
                    }
                }

                if let Some(rrule_str) = task.rrule.clone()
                    && i + consumed < stream.len()
                {
                    let potential_time = &stream[i + consumed];
                    if let Some(t) = parse_time_string(potential_time) {
                        let today = Local::now().date_naive();
                        let first_date = calculate_first_occurrence(&rrule_str, today);
                        let dt_specific = crate::model::item::safe_local_to_utc(first_date, t);
                        let date_val = DateType::Specific(dt_specific);
                        task.due = Some(date_val.clone());
                        task.dtstart = Some(date_val);
                        consumed += 1;
                    }
                }
            } else {
                summary_words.push(unescape(token));
            }
        } else if is_due_or_recur && matches!(exact, Some(&ExactToken::Unit(_))) {
            if let Some(ExactToken::Unit(u)) = exact {
                let freq = u.to_freq();
                task.rrule = Some(format!("FREQ={}", freq));
                has_recurrence = true;

                if i + consumed < stream.len() {
                    let potential_time = &stream[i + consumed];
                    if let Some(t) = parse_time_string(potential_time) {
                        let today = Local::now().date_naive();
                        let first_date =
                            calculate_first_occurrence(task.rrule.as_ref().unwrap(), today);
                        let dt_specific = crate::model::item::safe_local_to_utc(first_date, t);
                        let date_val = DateType::Specific(dt_specific);
                        task.due = Some(date_val.clone());
                        task.dtstart = Some(date_val);
                        consumed += 1;
                    }
                }
            }
        } else if pref == Some(PrefixToken::Recur) {
            if let Some(rrule) = parse_recurrence(rem) {
                task.rrule = Some(rrule.clone());
                has_recurrence = true;
                if i + consumed < stream.len() {
                    let potential_time = &stream[i + consumed];
                    if let Some(t) = parse_time_string(potential_time) {
                        let today = Local::now().date_naive();
                        let first_date = calculate_first_occurrence(&rrule, today);
                        let dt_specific = crate::model::item::safe_local_to_utc(first_date, t);
                        let date_val = DateType::Specific(dt_specific);
                        task.due = Some(date_val.clone());
                        task.dtstart = Some(date_val);
                        consumed += 1;
                    }
                }
            } else if let Some((interval, unit, _)) =
                parse_amount_and_unit_with_lex(rem, None, false, lex)
            {
                let freq = parse_freq_from_unit(&unit);
                if !freq.is_empty() {
                    task.rrule = Some(format!("FREQ={};INTERVAL={}", freq, interval));
                    has_recurrence = true;
                } else {
                    summary_words.push(unescape(token));
                }
            } else {
                summary_words.push(unescape(token));
            }
        } else if has_recurrence && exact == Some(&ExactToken::Until) && i + 1 < stream.len() {
            let next_token = &stream[i + 1];
            if let Some(d) = parse_smart_date_with_lex(next_token, lex) {
                if let Some(mut rr) = task.rrule.take() {
                    if !rr.contains("UNTIL=") {
                        let date_str = match d {
                            DateType::AllDay(nd) => nd.format("%Y%m%d").to_string(),
                            DateType::Specific(dt) => dt.format("%Y%m%d").to_string(),
                            DateType::Month(y, m) => format!("{:04}{:02}01", y, m),
                            DateType::Year(y) => format!("{:04}0101", y),
                        };
                        rr.push_str(&format!(";UNTIL={}", date_str));
                    }
                    task.rrule = Some(rr);
                }
                consumed = 2;
            } else {
                summary_words.push(unescape(token));
            }
        } else if has_recurrence && exact == Some(&ExactToken::Except) && i + 1 < stream.len() {
            let next_token = &stream[i + 1];
            let parts: Vec<&str> = next_token.split(',').map(|s| s.trim()).collect();
            let mut matched_any = false;

            if parts.len() == 1 {
                let part = parts[0];
                let mut temp_consumed = 1;
                if let Some(d) = parse_smart_date_with_lex(part, lex) {
                    let (dt, _) = finalize_date_token(d, &stream, i + 2, &mut temp_consumed);
                    task.exdates.push(dt);
                    matched_any = true;
                } else if let Some(code) = parse_weekday_code_with_lex(part, lex) {
                    blocked_weekdays.insert(code.to_string());
                    matched_any = true;
                } else if let Some(m) = parse_month_code_with_lex(part, lex) {
                    blocked_months.insert(m);
                    matched_any = true;
                }
                if matched_any {
                    consumed = 1 + temp_consumed;
                }
            } else {
                for part in parts {
                    if let Some(d) = parse_smart_date_with_lex(part, lex) {
                        let all_day_date = match d {
                            DateType::AllDay(nd) => DateType::AllDay(nd),
                            DateType::Specific(dt) => DateType::AllDay(dt.date_naive()),
                            DateType::Month(y, m) => {
                                DateType::AllDay(NaiveDate::from_ymd_opt(y, m, 1).unwrap())
                            }
                            DateType::Year(y) => {
                                DateType::AllDay(NaiveDate::from_ymd_opt(y, 1, 1).unwrap())
                            }
                        };
                        task.exdates.push(all_day_date);
                        matched_any = true;
                    } else if let Some(code) = parse_weekday_code_with_lex(part, lex) {
                        blocked_weekdays.insert(code.to_string());
                        matched_any = true;
                    } else if let Some(m) = parse_month_code_with_lex(part, lex) {
                        blocked_months.insert(m);
                        matched_any = true;
                    }
                }
                if matched_any {
                    consumed = 2;
                }
            }
            if !matched_any {
                summary_words.push(unescape(token));
            }
        } else if pref == Some(PrefixToken::Reminder) {
            let clean_val = if rem.is_empty() && i + 1 < stream.len() {
                consumed += 1;
                &stream[i + 1]
            } else {
                rem
            };

            let is_in = lex.exact.get(&clean_val.to_lowercase()) == Some(&ExactToken::In)
                || clean_val.eq_ignore_ascii_case("in");
            let is_next = lex.exact.get(&clean_val.to_lowercase()) == Some(&ExactToken::Next)
                || clean_val.eq_ignore_ascii_case("next");

            if is_in && i + consumed < stream.len() {
                let next_str = &stream[i + consumed];
                let next_next = if i + consumed + 1 < stream.len() {
                    Some(stream[i + consumed + 1].as_str())
                } else {
                    None
                };

                if let Some((amt, unit, extra)) =
                    parse_amount_and_unit_with_lex(next_str, next_next, false, lex)
                {
                    let mins = match unit.as_str() {
                        "d" | "day" | "days" => amt * 1440,
                        "w" | "week" | "weeks" => amt * 10080,
                        "h" | "hour" | "hours" => amt * 60,
                        _ => amt,
                    };
                    let now = Local::now();
                    let target = now + Duration::minutes(mins as i64);
                    pending_alarms.push(PendingAlarm::Absolute(target.with_timezone(&Utc)));
                    consumed += 1 + extra;
                } else {
                    summary_words.push(unescape(token));
                }
            } else if is_next && i + consumed < stream.len() {
                let next_str = &stream[i + consumed];
                if let Some(target_date) = parse_next_date_with_lex(next_str, lex) {
                    consumed += 1;
                    let mut time = default_reminder_time
                        .unwrap_or_else(|| NaiveTime::from_hms_opt(9, 0, 0).unwrap());
                    if i + consumed < stream.len()
                        && let Some(t) = parse_time_string(&stream[i + consumed])
                    {
                        time = t;
                        consumed += 1;
                    }
                    let local_dt = crate::model::item::safe_local_to_utc(target_date, time);
                    pending_alarms.push(PendingAlarm::Absolute(local_dt));
                } else {
                    summary_words.push(unescape(token));
                }
            } else if !clean_val.is_empty() {
                if let Some(d) = parse_duration_with_lex(clean_val, lex) {
                    pending_alarms.push(PendingAlarm::Relative(d));
                } else if let Some(t) = parse_time_string(clean_val) {
                    pending_alarms.push(PendingAlarm::TimeOnly(t));
                } else if let Some(d) = parse_smart_date_with_lex(clean_val, lex)
                    .or_else(|| parse_next_date_with_lex(clean_val, lex).map(DateType::AllDay))
                {
                    let mut time_part = None;
                    if i + consumed < stream.len() {
                        let potential_time = &stream[i + consumed];
                        if let Some(t) = parse_time_string(potential_time) {
                            time_part = Some(t);
                            consumed += 1;
                        }
                    }
                    let fallback = default_reminder_time
                        .unwrap_or_else(|| NaiveTime::from_hms_opt(8, 0, 0).unwrap());
                    let t = time_part.unwrap_or(fallback);

                    let dt = match d {
                        DateType::AllDay(nd) => crate::model::item::safe_local_to_utc(nd, t),
                        DateType::Specific(dt) => dt,
                        DateType::Month(y, m) => crate::model::item::safe_local_to_utc(
                            NaiveDate::from_ymd_opt(y, m, 1).unwrap(),
                            t,
                        ),
                        DateType::Year(y) => crate::model::item::safe_local_to_utc(
                            NaiveDate::from_ymd_opt(y, 1, 1).unwrap(),
                            t,
                        ),
                    };
                    pending_alarms.push(PendingAlarm::Absolute(dt));
                } else {
                    summary_words.push(unescape(token));
                }
            } else {
                summary_words.push(unescape(token));
            }
        } else if pref == Some(PrefixToken::Done) {
            let clean_val = rem;
            let mut matched = false;

            if !clean_val.is_empty() {
                if clean_val.ends_with('%') {
                    if let Ok(pc) = clean_val.trim_end_matches('%').parse::<u8>() {
                        task.percent_complete = Some(pc.min(100));
                        matched = true;
                    }
                } else if let Ok(ndt) =
                    chrono::NaiveDateTime::parse_from_str(clean_val, "%Y-%m-%d %H:%M")
                {
                    let utc_dt = crate::model::item::safe_local_to_utc(ndt.date(), ndt.time());
                    task.set_completion_date(Some(utc_dt));
                    matched = true;
                } else if let Some(d) = parse_smart_date_with_lex(clean_val, lex) {
                    let mut temp_consumed = 1;
                    let (dt, _) =
                        finalize_date_token(d, &stream, i + temp_consumed, &mut temp_consumed);

                    let utc_dt = match dt {
                        DateType::Specific(t) => t,
                        DateType::AllDay(d) => crate::model::item::safe_local_to_utc(
                            d,
                            chrono::NaiveTime::from_hms_opt(12, 0, 0).unwrap(),
                        ),
                        DateType::Month(y, m) => crate::model::item::safe_local_to_utc(
                            NaiveDate::from_ymd_opt(y, m, 1).unwrap(),
                            chrono::NaiveTime::from_hms_opt(12, 0, 0).unwrap(),
                        ),
                        DateType::Year(y) => crate::model::item::safe_local_to_utc(
                            NaiveDate::from_ymd_opt(y, 1, 1).unwrap(),
                            chrono::NaiveTime::from_hms_opt(12, 0, 0).unwrap(),
                        ),
                    };

                    task.set_completion_date(Some(utc_dt));
                    consumed = temp_consumed;
                    matched = true;
                }
            }
            if !matched {
                summary_words.push(unescape(token));
            }
        } else if pref == Some(PrefixToken::Spent) {
            if let Some(mins) = parse_duration_with_lex(rem, lex) {
                task.time_spent_seconds = (mins as u64) * 60;
            } else {
                summary_words.push(unescape(token));
            }
        } else if token.starts_with("@@") {
            let is_triple = token.starts_with("@@@");
            let val = strip_quotes(token.trim_start_matches('@'));
            if val.is_empty() {
                summary_words.push(unescape(token));
            } else {
                task.location = Some(val.clone());
                if is_triple {
                    summary_words.push(val);
                }
            }
        } else if pref == Some(PrefixToken::Loc) {
            let val = strip_quotes(rem_original);
            if val.is_empty() {
                summary_words.push(unescape(token));
            } else {
                task.location = Some(val);
            }
        } else if pref == Some(PrefixToken::Url) {
            task.url = Some(strip_quotes(rem_original));
        } else if token.starts_with("[[") && token.ends_with("]]") {
            task.url = Some(token[2..token.len() - 2].to_string());
        } else if token_lower == "+cal" {
            task.create_event = Some(true);
        } else if token_lower == "-cal" {
            task.create_event = Some(false);
        } else if token_lower == "+pin" {
            task.pinned = true;
        } else if token_lower == "-pin" {
            task.pinned = false;
        } else if pref == Some(PrefixToken::Geo) {
            let mut raw_val = rem_original.to_string();
            let mut temp_consumed = 1;
            let is_geo_word = |w: &str| -> bool {
                let upper = w.to_uppercase();
                if matches!(
                    upper.as_str(),
                    "N" | "S" | "E" | "W" | "N," | "S," | "E," | "W,"
                ) {
                    return true;
                }
                if !w.chars().any(|c| c.is_ascii_digit()) {
                    return false;
                }
                w.chars()
                    .all(|c| c.is_ascii_digit() || " .,-°NSEWnsew".contains(c))
            };
            while i + temp_consumed < stream.len() {
                let next_token = &stream[i + temp_consumed];
                if is_geo_word(next_token) {
                    raw_val.push(' ');
                    raw_val.push_str(next_token);
                    temp_consumed += 1;
                } else {
                    break;
                }
            }
            if is_valid_geo(&raw_val) {
                task.geo = Some(normalize_geo(strip_quotes(&raw_val)));
                consumed = temp_consumed;
            } else {
                summary_words.push(unescape(token));
            }
        } else if pref == Some(PrefixToken::Desc) {
            let desc_val = strip_quotes(rem_original);
            if task.description.is_empty() {
                task.description = desc_val;
            } else {
                task.description.push_str(&format!("\n{}", desc_val));
            }
        } else if token.starts_with('#') {
            let is_double = token.starts_with("##");
            let cat_expr = token.trim_start_matches('#');
            if !cat_expr.is_empty() {
                let expanded_cats = expand_braces(cat_expr);
                for cat in expanded_cats {
                    let clean_cat = strip_quotes(&cat);
                    if !clean_cat.is_empty() && !task.categories.contains(&clean_cat) {
                        task.categories.push(clean_cat);
                    }
                }
                if is_double {
                    summary_words.push(strip_quotes(cat_expr));
                }
            }
        } else if token.starts_with('!') && token.len() > 1 {
            if let Ok(p) = token[1..].parse::<u8>() {
                task.priority = p.min(9);
            } else {
                summary_words.push(unescape(token));
            }
        } else if token.starts_with('~') || pref == Some(PrefixToken::Duration) {
            let val = strip_quotes(if let Some(s) = token.strip_prefix('~') {
                s
            } else {
                rem
            });
            if let Some((min, max_opt)) = parse_duration_range_with_lex(&val, lex) {
                task.estimated_duration = Some(min);
                task.estimated_duration_max = max_opt;
            } else {
                summary_words.push(unescape(token));
            }
        } else if pref == Some(PrefixToken::StartDue)
            || pref == Some(PrefixToken::Start)
            || pref == Some(PrefixToken::Due)
        {
            let set_start = pref == Some(PrefixToken::StartDue) || pref == Some(PrefixToken::Start);
            let set_due = pref == Some(PrefixToken::StartDue) || pref == Some(PrefixToken::Due);
            let clean = rem;

            let is_next = lex.exact.get(&clean.to_lowercase()) == Some(&ExactToken::Next)
                || clean.eq_ignore_ascii_case("next");
            let is_in = lex.exact.get(&clean.to_lowercase()) == Some(&ExactToken::In)
                || clean.eq_ignore_ascii_case("in");

            let mut matched_date = false;

            if is_next && i + 1 < stream.len() {
                let next_str = &stream[i + 1];
                if let Some(d) = parse_next_date_with_lex(next_str, lex) {
                    let mut temp_consumed = 2;
                    let (dt, dt_end) = finalize_date_token(
                        DateType::AllDay(d),
                        &stream,
                        i + temp_consumed,
                        &mut temp_consumed,
                    );
                    if let Some(end) = dt_end {
                        task.dtstart = Some(dt);
                        task.due = Some(end);
                    } else {
                        if set_start {
                            task.dtstart = Some(dt.clone());
                        }
                        if set_due {
                            task.due = Some(dt);
                        }
                    }
                    consumed = temp_consumed;
                    matched_date = true;
                }
            } else if is_in && i + 1 < stream.len() {
                let next_token_str = stream[i + 1].as_str();
                let next_next = if i + 2 < stream.len() {
                    Some(stream[i + 2].as_str())
                } else {
                    None
                };
                if let Some((amount, unit, extra)) =
                    parse_amount_and_unit_with_lex(next_token_str, next_next, false, lex)
                    && let Some(d) = parse_in_date_with_lex(amount, &unit, lex)
                {
                    let mut temp_consumed = 1 + 1 + extra;
                    let (dt, dt_end) = finalize_date_token(
                        DateType::AllDay(d),
                        &stream,
                        i + temp_consumed,
                        &mut temp_consumed,
                    );
                    if let Some(end) = dt_end {
                        task.dtstart = Some(dt);
                        task.due = Some(end);
                    } else {
                        if set_start {
                            task.dtstart = Some(dt.clone());
                        }
                        if set_due {
                            task.due = Some(dt);
                        }
                    }
                    consumed = temp_consumed;
                    matched_date = true;
                }
            }
            if !matched_date {
                if let Some(d) = parse_smart_date_with_lex(clean, lex) {
                    let mut temp_consumed = 1;
                    let (dt, dt_end) =
                        finalize_date_token(d, &stream, i + temp_consumed, &mut temp_consumed);
                    if let Some(end) = dt_end {
                        task.dtstart = Some(dt);
                        task.due = Some(end);
                    } else {
                        if set_start {
                            task.dtstart = Some(dt.clone());
                        }
                        if set_due {
                            task.due = Some(dt);
                        }
                    }
                    consumed = temp_consumed;
                } else if let Some((t1, t2)) = parse_time_range(clean) {
                    let now_local = Local::now();
                    let mut target_date = now_local.date_naive();
                    if t1 <= now_local.time() {
                        target_date += Duration::days(1);
                    }
                    let dt1 =
                        DateType::Specific(crate::model::item::safe_local_to_utc(target_date, t1));
                    let mut target_date_end = target_date;
                    if t2 < t1 {
                        target_date_end += Duration::days(1);
                    }
                    let dt2 = DateType::Specific(crate::model::item::safe_local_to_utc(
                        target_date_end,
                        t2,
                    ));
                    task.dtstart = Some(dt1);
                    task.due = Some(dt2);
                } else if let Some(t) = parse_time_string(clean) {
                    let now_local = Local::now();
                    let mut target_date = now_local.date_naive();
                    if t <= now_local.time() {
                        target_date += Duration::days(1);
                    }
                    let dt = crate::model::item::safe_local_to_utc(target_date, t);
                    let dt_type = DateType::Specific(dt);
                    if set_start {
                        task.dtstart = Some(dt_type.clone());
                    }
                    if set_due {
                        task.due = Some(dt_type);
                    }
                } else if let Some(d) = parse_weekday_date_with_lex(clean, lex) {
                    let mut temp_consumed = 1;
                    let (dt, dt_end) = finalize_date_token(
                        DateType::AllDay(d),
                        &stream,
                        i + temp_consumed,
                        &mut temp_consumed,
                    );
                    if let Some(end) = dt_end {
                        task.dtstart = Some(dt);
                        task.due = Some(end);
                    } else {
                        if set_start {
                            task.dtstart = Some(dt.clone());
                        }
                        if set_due {
                            task.due = Some(dt);
                        }
                    }
                    consumed = temp_consumed;
                } else {
                    summary_words.push(unescape(token));
                }
            }
        } else if pref == Some(PrefixToken::Goal) {
            let val = rem;
            let (target_str, period_str) = if let Some(idx) = val.find('/') {
                (&val[..idx], &val[idx + 1..])
            } else if let Some(idx) = val.find(':') {
                (&val[..idx], &val[idx + 1..])
            } else {
                if matches!(
                    val,
                    "daily" | "weekly" | "monthly" | "yearly" | "d" | "w" | "mo" | "y"
                ) {
                    ("1", val)
                } else {
                    ("", "")
                }
            };
            if !target_str.is_empty() && !period_str.is_empty() {
                let period_str = period_str.trim_start_matches('@');
                let mut goal_type = crate::config::GoalType::Count;
                let target = if let Some(dur) = parse_duration_with_lex(target_str, lex) {
                    goal_type = crate::config::GoalType::Duration;
                    dur
                } else {
                    target_str.parse().unwrap_or(0)
                };
                let (mut amount, unit, _) = parse_amount_and_unit_with_lex(
                    period_str, None, false, lex,
                )
                .unwrap_or((1, period_str.to_string(), 0));
                let interval_unit = match unit.to_lowercase().as_str() {
                    "d" | "day" | "days" | "daily" => crate::config::IntervalUnit::Days,
                    "w" | "week" | "weeks" | "weekly" => crate::config::IntervalUnit::Weeks,
                    "m" | "mo" | "month" | "months" | "monthly" => {
                        crate::config::IntervalUnit::Months
                    }
                    "q" | "quarter" | "quarterly" => {
                        amount *= 3;
                        crate::config::IntervalUnit::Months
                    }
                    "h" | "hy" | "halfyearly" | "semiannual" => {
                        amount *= 6;
                        crate::config::IntervalUnit::Months
                    }
                    "y" | "year" | "years" | "yearly" => crate::config::IntervalUnit::Years,
                    _ => crate::config::IntervalUnit::Weeks,
                };
                if target > 0 {
                    task.goal = Some(crate::config::Goal {
                        goal_type,
                        target,
                        interval: crate::config::Interval {
                            amount,
                            unit: interval_unit,
                        },
                    });
                } else {
                    summary_words.push(unescape(token));
                }
            } else {
                summary_words.push(unescape(token));
            }
        } else {
            summary_words.push(unescape(token));
        }
        i += consumed;
    }

    if (!blocked_weekdays.is_empty() || !blocked_months.is_empty())
        && let Some(mut rrule) = task.rrule.take()
    {
        if !blocked_weekdays.is_empty() {
            if rrule.contains("FREQ=DAILY") {
                rrule = rrule.replace("FREQ=DAILY", "FREQ=WEEKLY");
            }
            if !rrule.contains("BYDAY=") {
                let all_days = vec!["MO", "TU", "WE", "TH", "FR", "SA", "SU"];
                let allowed: Vec<&str> = all_days
                    .into_iter()
                    .filter(|d| !blocked_weekdays.contains(*d))
                    .collect();
                if !allowed.is_empty() {
                    rrule.push_str(&format!(";BYDAY={}", allowed.join(",")));
                }
            }
        }
        if !blocked_months.is_empty() && !rrule.contains("BYMONTH=") {
            let allowed: Vec<String> = (1..=12)
                .filter(|m| !blocked_months.contains(m))
                .map(|m| m.to_string())
                .collect();
            if !allowed.is_empty() {
                rrule.push_str(&format!(";BYMONTH={}", allowed.join(",")));
            }
        }
        task.rrule = Some(rrule);
    }

    task.summary = summary_words.join(" ");
    task.categories.sort();
    task.categories.dedup();

    if let Some(ref rrule) = task.rrule
        && task.dtstart.is_none()
        && task.due.is_none()
    {
        let today = Local::now().date_naive();
        let first_occurrence = calculate_first_occurrence(rrule, today);
        task.dtstart = Some(DateType::AllDay(first_occurrence));
        task.due = Some(DateType::AllDay(first_occurrence));
    }

    if let (Some(start), Some(due)) = (&task.dtstart, &task.due)
        && start.to_start_comparison_time() > due.to_comparison_time()
    {
        task.dtstart = Some(due.clone());
    }

    for pending in pending_alarms {
        match pending {
            PendingAlarm::Relative(mins) => task.alarms.push(Alarm::new_relative(mins)),
            PendingAlarm::Absolute(dt) => task.alarms.push(Alarm::new_absolute(dt)),
            PendingAlarm::TimeOnly(t) => {
                let anchor_date = task
                    .due
                    .as_ref()
                    .or(task.dtstart.as_ref())
                    .map(|d| d.to_date_naive());
                if let Some(target_date) = anchor_date {
                    let dt = crate::model::item::safe_local_to_utc(target_date, t);
                    task.alarms.push(Alarm::new_absolute(dt));
                } else {
                    let now_local = Local::now();
                    let mut target_date = now_local.date_naive();
                    if t <= now_local.time() {
                        target_date += Duration::days(1);
                    }
                    let dt = crate::model::item::safe_local_to_utc(target_date, t);
                    task.alarms.push(Alarm::new_absolute(dt));
                }
            }
        }
    }
}

pub fn parse_session_input(input: &str) -> Option<crate::model::item::WorkSession> {
    use chrono::Local;
    let now = Local::now();
    let mut target_date = now.date_naive();

    let tokens: Vec<&str> = input.split_whitespace().collect();
    if tokens.is_empty() {
        return None;
    }

    let mut duration_mins = None;
    let mut start_time = None;
    let mut end_time = None;

    let lex_guard = LEXICON.read().unwrap();
    let lex = &*lex_guard;

    for token in tokens {
        let lower = token.to_lowercase();
        if lex.exact.get(&lower) == Some(&ExactToken::Today) || lower == "today" {
            target_date = now.date_naive();
        } else if lex.exact.get(&lower) == Some(&ExactToken::Yesterday) || lower == "yesterday" {
            target_date = now.date_naive() - chrono::Duration::days(1);
        } else if let Some(code) = parse_weekday_code_with_lex(&lower, lex) {
            let target_wd = match code {
                "MO" => chrono::Weekday::Mon,
                "TU" => chrono::Weekday::Tue,
                "WE" => chrono::Weekday::Wed,
                "TH" => chrono::Weekday::Thu,
                "FR" => chrono::Weekday::Fri,
                "SA" => chrono::Weekday::Sat,
                "SU" => chrono::Weekday::Sun,
                _ => chrono::Weekday::Mon,
            };
            let mut d = now.date_naive();
            while d.weekday() != target_wd {
                d -= chrono::Duration::days(1);
            }
            target_date = d;
        } else if let Ok(d) = chrono::NaiveDate::parse_from_str(&lower, "%Y-%m-%d") {
            target_date = d;
        } else if lower.contains('-') && lower.split('-').count() == 2 {
            let parts: Vec<&str> = lower.split('-').collect();
            if let (Some(t1), Some(t2)) = (parse_time_string(parts[0]), parse_time_string(parts[1]))
            {
                start_time = Some(t1);
                end_time = Some(t2);
            }
        } else if let Some(mins) = parse_duration_with_lex(&lower, lex) {
            duration_mins = Some(mins);
        } else if let Some(t) = parse_time_string(&lower) {
            start_time = Some(t);
        }
    }

    if let (Some(s), Some(e)) = (start_time, end_time) {
        let start_dt = crate::model::item::safe_local_to_utc(target_date, s);
        let mut end_target_date = target_date;
        if e < s {
            end_target_date += chrono::Duration::days(1);
        }
        let end_dt = crate::model::item::safe_local_to_utc(end_target_date, e);
        return Some(crate::model::item::WorkSession {
            start: start_dt.timestamp(),
            end: end_dt.timestamp(),
        });
    }

    if let Some(dur) = duration_mins {
        if let Some(s) = start_time {
            let start_dt = crate::model::item::safe_local_to_utc(target_date, s);
            let end_dt = start_dt + chrono::Duration::minutes(dur as i64);
            return Some(crate::model::item::WorkSession {
                start: start_dt.timestamp(),
                end: end_dt.timestamp(),
            });
        } else if target_date == now.date_naive() {
            let end_dt = now.with_timezone(&chrono::Utc);
            let start_dt = end_dt - chrono::Duration::minutes(dur as i64);
            return Some(crate::model::item::WorkSession {
                start: start_dt.timestamp(),
                end: end_dt.timestamp(),
            });
        } else {
            let s = chrono::NaiveTime::from_hms_opt(12, 0, 0).unwrap();
            let start_dt = crate::model::item::safe_local_to_utc(target_date, s);
            let end_dt = start_dt + chrono::Duration::minutes(dur as i64);
            return Some(crate::model::item::WorkSession {
                start: start_dt.timestamp(),
                end: end_dt.timestamp(),
            });
        }
    }

    None
}
