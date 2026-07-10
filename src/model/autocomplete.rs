// SPDX-License-Identifier: GPL-3.0-or-later
use crate::store::TaskStore;
use crate::model::parser::{LEXICON, PrefixToken, split_input_respecting_quotes, quote_value};
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
) -> Option<(Range<usize>, Vec<Suggestion>)> {
    let parts = split_input_respecting_quotes(input);
    
    let mut current_part = None;
    for (start, end, word) in parts {
        if cursor_byte_idx >= start && cursor_byte_idx <= end {
            current_part = Some((start, end, word));
            break;
        }
    }
    
    let (start, end, word) = current_part?;
    if word.is_empty() { return None; }
    
    let lex_guard = LEXICON.read().unwrap();
    let lex = &*lex_guard;
    
    let lower = word.to_lowercase();
    
    // 1. Tags
    if word.starts_with('#') {
        let query = &lower[1..];
        let mut tags = std::collections::HashSet::new();
        
        for k in aliases.keys() {
            if let Some(clean) = k.strip_prefix('#')
                && clean.to_lowercase().starts_with(query) {
                tags.insert(clean.to_string());
            }
        }
        for map in store.calendars.values() {
            for t in map.values() {
                for c in &t.categories {
                    if c.to_lowercase().starts_with(query) {
                        tags.insert(c.clone());
                    }
                }
            }
        }
        
        let mut suggestions: Vec<_> = tags.into_iter().map(|t| {
            Suggestion {
                replacement: format!("#{}", quote_value(&t)),
                display: format!("#{}", t),
                description: "Tag".to_string(),
            }
        }).collect();
        suggestions.sort_by(|a, b| a.display.cmp(&b.display));
        suggestions.truncate(10);
        
        if !suggestions.is_empty() {
            return Some((start..end, suggestions));
        }
    }
    
    // 2. Locations
    let is_loc = word.starts_with("@@") || lex.match_prefix(&lower).map(|(_, k, _)| k) == Some(PrefixToken::Loc);
    if is_loc {
        let (prefix_str, query) = if let Some(stripped) = word.strip_prefix("@@") {
            ("@@", stripped)
        } else {
            let match_res = lex.match_prefix(&lower).unwrap();
            (match_res.0, &word[match_res.0.len()..])
        };
        let query_lower = query.to_lowercase();
        let mut locs = std::collections::HashSet::new();
        
        for k in aliases.keys() {
            if let Some(clean) = k.strip_prefix("@@")
                && clean.to_lowercase().starts_with(&query_lower) {
                locs.insert(clean.to_string());
            }
        }
        for map in store.calendars.values() {
            for t in map.values() {
                if let Some(l) = &t.location
                    && l.to_lowercase().starts_with(&query_lower) {
                    locs.insert(l.clone());
                }
            }
        }
        
        let mut suggestions: Vec<_> = locs.into_iter().map(|l| {
            Suggestion {
                replacement: format!("{}{}", prefix_str, quote_value(&l)),
                display: format!("@@{}", l),
                description: "Location".to_string(),
            }
        }).collect();
        suggestions.sort_by(|a, b| a.display.cmp(&b.display));
        suggestions.truncate(10);
        
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
                        if t.status.is_done() || t.calendar_href == crate::storage::LOCAL_TRASH_HREF {
                            continue;
                        }
                        if t.summary.to_lowercase().contains(&query_clean) || t.uid.to_lowercase().starts_with(&query_clean) {
                            matches.push(t.clone());
                        }
                    }
                }
                
                matches.sort_by(|a, b| {
                    let a_starts = a.summary.to_lowercase().starts_with(&query_clean);
                    let b_starts = b.summary.to_lowercase().starts_with(&query_clean);
                    b_starts.cmp(&a_starts).then_with(|| a.summary.cmp(&b.summary))
                });
                matches.dedup_by(|a, b| a.uid == b.uid);
                matches.truncate(10);
                
                let suggestions: Vec<_> = matches.into_iter().map(|t| {
                    Suggestion {
                        replacement: format!("{}{}", original_prefix, quote_value(&t.summary)),
                        display: t.summary.clone(),
                        description: if kind == PrefixToken::Dependency { "Depends On".to_string() } else { "Related To".to_string() },
                    }
                }).collect();
                
                if !suggestions.is_empty() {
                    return Some((start..end, suggestions));
                }
            }
        }
    }

    None
}
