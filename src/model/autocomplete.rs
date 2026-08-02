// SPDX-License-Identifier: GPL-3.0-or-later
use crate::model::CalendarListEntry;
use crate::model::parser::{LEXICON, PrefixToken, quote_value, split_input_respecting_quotes};
use crate::store::TaskStore;
use std::collections::HashMap;
use std::ops::Range;

#[derive(Debug, Clone, PartialEq)]
pub struct Suggestion {
    pub replacement: String,
    pub display: String,
    pub description: String,
}

pub fn suggest(
    input: &str,
    cursor_byte_idx: usize,
    store: &TaskStore,
    aliases: &HashMap<String, Vec<String>>,
    calendars: &[CalendarListEntry],
) -> Option<(Range<usize>, Vec<Suggestion>)> {
    let line_start = input[..cursor_byte_idx]
        .rfind('\n')
        .map(|i| i + 1)
        .unwrap_or(0);
    let line_end = input[cursor_byte_idx..]
        .find('\n')
        .map(|i| cursor_byte_idx + i)
        .unwrap_or(input.len());
    let current_line = &input[line_start..line_end];
    let local_cursor = cursor_byte_idx - line_start;

    let parts = split_input_respecting_quotes(current_line);

    let mut current_part = None;
    for (start, end, word) in parts {
        if local_cursor >= start && local_cursor <= end {
            current_part = Some((start, end, word));
            break;
        }
    }

    let (local_start, local_end, word) = current_part?;
    let start = local_start + line_start;
    let end = local_end + line_start;
    if word.is_empty() {
        return None;
    }

    let lex_guard = LEXICON.read().unwrap();
    let lex = &*lex_guard;

    let lower = word.to_lowercase();

    // 0. Commands
    if lower.starts_with(':') && !lower.contains(' ') && start == 0 {
        let cmds = vec![
            (":undo", "Undo last action"),
            (":redo", "Redo last undone action"),
            (":empty-trash", "Empty the local trash"),
        ];
        let mut suggestions = Vec::new();
        for (cmd, desc) in cmds {
            if cmd.starts_with(&lower) {
                suggestions.push(Suggestion {
                    replacement: cmd.to_string(),
                    display: cmd.to_string(),
                    description: desc.to_string(),
                });
            }
        }
        if !suggestions.is_empty() {
            return Some((start..end, suggestions));
        }
    }

    // 1. Tags
    if word.starts_with('#') {
        let query = &lower[1..];
        let mut tag_counts: HashMap<String, usize> = HashMap::new();

        for k in aliases.keys() {
            if let Some(clean) = k.strip_prefix('#')
                && clean.to_lowercase().starts_with(query)
                && clean != "cfait-internal"
            {
                tag_counts.insert(clean.to_string(), 0);
            }
        }
        for map in store.calendars.values() {
            for t in map.values() {
                for c in &t.categories {
                    let c_lower = c.to_lowercase();
                    if c_lower.starts_with(query) {
                        *tag_counts.entry(c.clone()).or_insert(0) += 1;
                    }
                }
            }
        }

        let mut tags: Vec<_> = tag_counts.into_iter().collect();
        tags.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

        let suggestions: Vec<_> = tags
            .into_iter()
            .take(10)
            .map(|(t, _)| Suggestion {
                replacement: format!("#{}", quote_value(&t)),
                display: format!("#{}", t),
                description: String::new(),
            })
            .collect();

        if !suggestions.is_empty() {
            return Some((start..end, suggestions));
        }
    }

    // 2. Locations
    let is_loc = word.starts_with("@@")
        || lex.match_prefix(&lower).map(|(_, k, _)| k) == Some(PrefixToken::Loc);
    if is_loc {
        let (prefix_str, query) = if let Some(stripped) = word.strip_prefix("@@") {
            ("@@", stripped)
        } else {
            let match_res = lex.match_prefix(&lower).unwrap();
            (match_res.0, &word[match_res.0.len()..])
        };
        let query_lower = query.to_lowercase();
        let mut loc_counts: HashMap<String, usize> = HashMap::new();

        for k in aliases.keys() {
            if let Some(clean) = k.strip_prefix("@@")
                && clean.to_lowercase().starts_with(&query_lower)
            {
                loc_counts.insert(clean.to_string(), 0);
            }
        }
        for map in store.calendars.values() {
            for t in map.values() {
                if let Some(l) = &t.location
                    && l.to_lowercase().starts_with(&query_lower)
                {
                    *loc_counts.entry(l.clone()).or_insert(0) += 1;
                }
            }
        }

        let mut loc_list: Vec<_> = loc_counts.into_iter().collect();
        loc_list.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

        let suggestions: Vec<_> = loc_list
            .into_iter()
            .take(10)
            .map(|(l, _)| Suggestion {
                replacement: format!("{}{}", prefix_str, quote_value(&l)),
                display: format!("@@{}", l),
                description: String::new(),
            })
            .collect();

        if !suggestions.is_empty() {
            return Some((start..end, suggestions));
        }
    }

    // 3. Dependencies & Relations
    #[allow(clippy::collapsible_if)]
    if let Some((p_str, kind, rem)) = lex.match_prefix(&lower) {
        if kind == PrefixToken::Dependency || kind == PrefixToken::Rel {
            let original_prefix = &word[..p_str.len()];
            let query = rem.to_lowercase();
            let query_clean = crate::model::parser::strip_quotes(&query).to_lowercase();

            if !query_clean.is_empty() {
                let mut matches = Vec::new();
                for map in store.calendars.values() {
                    for t in map.values() {
                        if t.status.is_done() || t.calendar_href == crate::storage::LOCAL_TRASH_HREF
                        {
                            continue;
                        }
                        if t.summary.to_lowercase().contains(&query_clean)
                            || t.uid.to_lowercase().starts_with(&query_clean)
                        {
                            matches.push(t.clone());
                        }
                    }
                }

                matches.sort_by(|a, b| {
                    let a_starts = a.summary.to_lowercase().starts_with(&query_clean);
                    let b_starts = b.summary.to_lowercase().starts_with(&query_clean);
                    b_starts
                        .cmp(&a_starts)
                        .then_with(|| a.summary.cmp(&b.summary))
                });
                matches.dedup_by(|a, b| a.uid == b.uid);
                matches.truncate(10);

                let suggestions: Vec<_> = matches
                    .into_iter()
                    .map(|t| Suggestion {
                        replacement: format!("{}{}", original_prefix, quote_value(&t.summary)),
                        display: t.summary.clone(),
                        description: String::new(),
                    })
                    .collect();

                if !suggestions.is_empty() {
                    return Some((start..end, suggestions));
                }
            }
        } else if kind == PrefixToken::Collection {
            let original_prefix = &word[..p_str.len()];
            let query_clean = crate::model::parser::strip_quotes(rem).to_lowercase();

            let mut matches = Vec::new();
            for cal in calendars {
                if cal.href == "local://trash" || cal.href == "local://recovery" {
                    continue;
                }
                if cal.name.to_lowercase().contains(&query_clean) {
                    matches.push(cal.clone());
                }
            }

            matches.sort_by(|a, b| {
                let a_starts = a.name.to_lowercase().starts_with(&query_clean);
                let b_starts = b.name.to_lowercase().starts_with(&query_clean);

                let count_a = store.calendars.get(&a.href).map_or(0, |m| m.len());
                let count_b = store.calendars.get(&b.href).map_or(0, |m| m.len());

                b_starts
                    .cmp(&a_starts)
                    .then_with(|| count_b.cmp(&count_a))
                    .then_with(|| a.name.cmp(&b.name))
            });

            matches.truncate(10);

            let suggestions: Vec<_> = matches
                .into_iter()
                .map(|c| Suggestion {
                    replacement: format!("{}{}", original_prefix, quote_value(&c.name)),
                    display: c.name.clone(),
                    description: String::new(),
                })
                .collect();

            if !suggestions.is_empty() {
                return Some((start..end, suggestions));
            }
        }
    }

    None
}
