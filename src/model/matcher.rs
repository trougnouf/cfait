// SPDX-License-Identifier: GPL-3.0-or-later
// Logic for checking if tasks match search queries.
//
// This file implements a lexer and recursive-descent parser to support boolean
// search expressions with implicit AND, explicit OR (|), NOT (-), and grouping
// with parentheses.
//
// Syntax:
//   A B       -> A AND B
//   A | B     -> A OR B
//   -A        -> NOT A
//   (A | B) C -> (A OR B) AND C
//   "foo bar" -> Exact phrase match
//
// Each term is compiled once against the lexicon in `Query::new`; evaluation
// then handles specific filters (e.g. #tag, @date, is:done) and substring matching.

use crate::model::item::{Task, TaskStatus};
use chrono::NaiveDate;

/// A search term compiled once at query-parse time, so per-task matching never
/// re-lowercases it or re-scans it against the lexicon.
#[derive(Debug, Clone)]
struct CompiledTerm {
    /// Lowercased, unquoted term; used for the text fallback and for
    /// fall-through when a typed filter fails to parse.
    lower: String,
    /// Location filter (`@@loc` or `loc:loc`).
    loc_query: Option<String>,
    /// Duration filter (`~30m`, `~<1h`); a `None` target means the value
    /// failed to parse and the term falls through to text search.
    duration: Option<(Op, Option<u32>)>,
    /// Priority filter (`!1`, `!<3`).
    priority: Option<(Op, u8)>,
    /// Start date filter (`^monday`, `start:monday`); a `None` date means the
    /// value failed to parse and the term falls through.
    start: Option<(Op, Option<NaiveDate>, bool)>,
    /// Due date filter (`@tomorrow`, `due:tomorrow`).
    due: Option<(Op, Option<NaiveDate>, bool)>,
    /// Tag filter (`#tag`).
    tag_query: Option<String>,
    /// Status filter (`is:done`, ...).
    status: Option<StatusFilter>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    Lt,
    Gt,
    Le,
    Ge,
    Eq,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StatusFilter {
    Done,
    InProcess,
    Active,
    Ready,
    Note,
    Page,
    Permanent,
    Canceled,
    Pinned,
    Recurrent,
}

/// Raw (uncompiled) syntax tree produced by the parser.
#[derive(Debug, Clone)]
enum RawExpr {
    Term(String),
    And(Box<RawExpr>, Box<RawExpr>),
    Or(Box<RawExpr>, Box<RawExpr>),
    Not(Box<RawExpr>),
}

/// Compiled syntax tree evaluated against tasks.
#[derive(Debug, Clone)]
enum SearchExpr {
    Term(CompiledTerm),
    And(Box<SearchExpr>, Box<SearchExpr>),
    Or(Box<SearchExpr>, Box<SearchExpr>),
    Not(Box<SearchExpr>),
}

pub struct Query {
    expr: SearchExpr,
}

pub fn extract_highlight_terms(query: &str) -> Vec<String> {
    let tokens = tokenize_query(query);
    let mut terms = Vec::new();
    let lex_guard = crate::model::parser::LEXICON.read().unwrap();
    for t in tokens {
        if let Token::Text(s) = t {
            let lower = s.to_lowercase();
            if lower.starts_with("is:")
                || lex_guard.search_is_done.contains(&lower)
                || lex_guard.search_is_active.contains(&lower)
                || lex_guard.search_is_started.contains(&lower)
                || lex_guard.search_is_ongoing.contains(&lower)
                || lex_guard.search_is_ready.contains(&lower)
                || lex_guard.search_is_blocked.contains(&lower)
                || lex_guard.search_is_note.contains(&lower)
                || lex_guard.search_is_page.contains(&lower)
                || lex_guard.search_is_permanent.contains(&lower)
                || lex_guard.search_is_canceled.contains(&lower)
                || lex_guard.search_is_recurrent.contains(&lower)
            {
                continue;
            }

            let mut clean_term = s.as_str();
            if let Some((_, _, rem_original)) = lex_guard.extract_prefix(&s, &lower) {
                clean_term = rem_original;
            } else if let Some(stripped) = s.strip_prefix('#') {
                clean_term = stripped;
            } else if let Some(stripped) = s.strip_prefix("@@") {
                clean_term = stripped;
            } else if let Some(stripped) = s.strip_prefix('-') {
                clean_term = stripped;
            } else if let Some(stripped) = s.strip_prefix('!') {
                clean_term = stripped;
            } else if let Some(stripped) = s.strip_prefix('~') {
                clean_term = stripped;
            }

            let clean_term = crate::model::parser::strip_quotes(clean_term);
            if !clean_term.trim().is_empty() {
                terms.push(regex::escape(clean_term.trim()));
            }
        }
    }
    terms
}

impl Query {
    pub fn new(query: &str, lex: &crate::model::parser::ParserLexicon) -> Self {
        let tokens = tokenize_query(query);
        let mut parser = Parser::new(tokens);
        Self {
            expr: compile_expr(parser.parse(), lex),
        }
    }

    pub fn matches(&self, task: &Task, store: &crate::store::TaskStore) -> bool {
        self.expr.matches(task, store)
    }
}

impl SearchExpr {
    fn matches(&self, task: &Task, store: &crate::store::TaskStore) -> bool {
        match self {
            SearchExpr::Term(t) => t.match_task(task, store),
            SearchExpr::And(a, b) => a.matches(task, store) && b.matches(task, store),
            SearchExpr::Or(a, b) => a.matches(task, store) || b.matches(task, store),
            SearchExpr::Not(a) => !a.matches(task, store),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
enum Token {
    Text(String),
    Or,
    LParen,
    RParen,
    NotPrefix, // The '-' character meaning NOT
}

/// Tokenizes the input string into a stream of tokens for the parser.
/// Handles quoted strings, parentheses, pipe operators, and the minus/NOT operator.
fn tokenize_query(input: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();

    while let Some(&c) = chars.peek() {
        match c {
            ' ' | '\t' => {
                chars.next();
            } // Skip whitespace
            '(' => {
                tokens.push(Token::LParen);
                chars.next();
            }
            ')' => {
                tokens.push(Token::RParen);
                chars.next();
            }
            '|' => {
                tokens.push(Token::Or);
                chars.next();
            }
            '-' => {
                // A `-` at the start of a token is the NOT operator when attached to
                // what follows ("-A", "-5", "-("); a bare "-" (before whitespace, a
                // closing paren, a pipe, or end of input) is literal text.
                chars.next(); // Consume '-'
                if let Some(&next_c) = chars.peek() {
                    if next_c.is_whitespace() || next_c == ')' || next_c == '|' {
                        tokens.push(Token::Text("-".to_string()));
                    } else {
                        tokens.push(Token::NotPrefix);
                    }
                } else {
                    tokens.push(Token::Text("-".to_string()));
                }
            }
            _ => {
                // Read text/term
                let mut term = String::new();
                let mut in_quote = false;
                let mut escaped = false; // Track escape state

                while let Some(&c) = chars.peek() {
                    if escaped {
                        // Previous char was backslash, take this one literally
                        term.push(c);
                        chars.next();
                        escaped = false;
                    } else if c == '\\' {
                        // Start escape
                        chars.next(); // Consume backslash
                        escaped = true;
                        // Handle trailing backslash case
                        if chars.peek().is_none() {
                            term.push('\\');
                        }
                    } else if c == '"' {
                        in_quote = !in_quote;
                        term.push(c);
                        chars.next();
                    } else if !in_quote && (c == ' ' || c == '(' || c == ')' || c == '|') {
                        break;
                    } else {
                        term.push(c);
                        chars.next();
                    }
                }

                if !term.is_empty() {
                    tokens.push(Token::Text(term));
                }
            }
        }
    }

    tokens
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn parse(&mut self) -> RawExpr {
        if self.tokens.is_empty() {
            return RawExpr::Term("".to_string());
        }
        self.parse_or()
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) {
        if self.pos < self.tokens.len() {
            self.pos += 1;
        }
    }

    // OR has lowest precedence
    fn parse_or(&mut self) -> RawExpr {
        let mut left = self.parse_and();

        while let Some(Token::Or) = self.peek() {
            self.advance();
            let right = self.parse_and();
            left = RawExpr::Or(Box::new(left), Box::new(right));
        }
        left
    }

    // Implicit AND handles sequences of terms
    fn parse_and(&mut self) -> RawExpr {
        let mut left = self.parse_unary();

        while let Some(token) = self.peek() {
            if matches!(token, Token::Or | Token::RParen) {
                break;
            }
            // Implicit AND between adjacent tokens
            let right = self.parse_unary();
            left = RawExpr::And(Box::new(left), Box::new(right));
        }
        left
    }

    // NOT operator
    fn parse_unary(&mut self) -> RawExpr {
        if let Some(Token::NotPrefix) = self.peek() {
            self.advance();
            let expr = self.parse_primary();
            return RawExpr::Not(Box::new(expr));
        }
        self.parse_primary()
    }

    // Terms or Grouping
    fn parse_primary(&mut self) -> RawExpr {
        match self.peek() {
            Some(Token::LParen) => {
                self.advance();
                let expr = self.parse_or();
                if let Some(Token::RParen) = self.peek() {
                    self.advance();
                }
                expr
            }
            Some(Token::Text(t)) => {
                let term = t.clone();
                self.advance();
                RawExpr::Term(term)
            }
            _ => RawExpr::Term("".to_string()), // Fallback
        }
    }
}

pub fn contains_ignore_case(haystack: &str, needle_lower: &str) -> bool {
    if needle_lower.is_empty() {
        return true;
    }
    let h = haystack.as_bytes();
    let n = needle_lower.as_bytes();
    if haystack.is_ascii() && needle_lower.is_ascii() {
        if h.len() < n.len() {
            return false;
        }
        for i in 0..=(h.len() - n.len()) {
            if h[i..i + n.len()].eq_ignore_ascii_case(n) {
                return true;
            }
        }
        return false;
    }
    haystack.to_lowercase().contains(needle_lower)
}

pub fn starts_with_ignore_case(haystack: &str, prefix_lower: &str) -> bool {
    if prefix_lower.is_empty() {
        return true;
    }
    let h = haystack.as_bytes();
    let p = prefix_lower.as_bytes();
    if haystack.is_ascii() && prefix_lower.is_ascii() {
        if h.len() < p.len() {
            return false;
        }
        return h[..p.len()].eq_ignore_ascii_case(p);
    }
    haystack.to_lowercase().starts_with(prefix_lower)
}

/// Split a leading comparison operator (`<=`, `>=`, `<`, `>`) off `s`,
/// defaulting to equality.
fn split_op(s: &str) -> (Op, &str) {
    if let Some(v) = s.strip_prefix("<=") {
        (Op::Le, v)
    } else if let Some(v) = s.strip_prefix(">=") {
        (Op::Ge, v)
    } else if let Some(v) = s.strip_prefix('<') {
        (Op::Lt, v)
    } else if let Some(v) = s.strip_prefix('>') {
        (Op::Gt, v)
    } else {
        (Op::Eq, s)
    }
}

/// Compile a raw search term once so per-task matching never re-lowercases it
/// or re-scans it against the lexicon.
fn compile_term(part: &str, lex: &crate::model::parser::ParserLexicon) -> CompiledTerm {
    // Trim whitespace and strip surrounding quotes for quoted phrases.
    // We do this to support searches like "exact phrase" or tag:"my tag".
    let part = part.trim();
    let part_unquoted = if part.starts_with('"') && part.ends_with('"') && part.len() >= 2 {
        &part[1..part.len() - 1]
    } else {
        part
    };
    let lower = part_unquoted.to_lowercase();

    let extracted = lex.extract_prefix(part_unquoted, &lower);
    let rem = extracted.map(|(_, r, _)| r).unwrap_or(lower.as_str());
    let pref = extracted.map(|(p, _, _)| p);

    // --- Location Filter (@@loc or loc:loc) ---
    let loc_query =
        if lower.starts_with("@@") || pref == Some(crate::model::parser::PrefixToken::Loc) {
            Some(if lower.starts_with("@@") {
                lower.trim_start_matches('@').to_string()
            } else {
                rem.to_string()
            })
        } else {
            None
        };

    // --- Duration Filter (~30m, ~<1h, ~>2h) ---
    let duration =
        if lower.starts_with('~') || pref == Some(crate::model::parser::PrefixToken::Duration) {
            let content = if lower.starts_with('~') {
                lower.strip_prefix('~').unwrap()
            } else {
                rem
            };
            let (op, val_str) = if let Some(stripped) = content.strip_prefix('=') {
                (Op::Eq, stripped)
            } else {
                split_op(content)
            };
            Some((
                op,
                crate::model::parser::parse_duration_with_lex(val_str, lex),
            ))
        } else {
            None
        };

    // --- Priority Filter (!1, !<3) ---
    let priority = if let Some(stripped) = lower.strip_prefix('!') {
        let (op, val_str) = split_op(stripped);
        val_str.parse::<u8>().ok().map(|v| (op, v))
    } else {
        None
    };

    // --- Date Filters (@due, ^start) ---
    let compile_date = |target_pref: crate::model::parser::PrefixToken,
                        prefix_char: char|
     -> Option<(Op, Option<NaiveDate>, bool)> {
        if pref != Some(target_pref) && !lower.starts_with(prefix_char) {
            return None;
        }
        let raw_val = if pref == Some(target_pref) {
            rem
        } else {
            lower.strip_prefix(prefix_char).unwrap_or("")
        };
        let (val_str_full, include_none) = if let Some(stripped) = raw_val.strip_suffix('!') {
            (stripped, true)
        } else {
            (raw_val, false)
        };
        let (op, date_str) = split_op(val_str_full);
        let target = crate::model::parser::parse_smart_date_with_lex(date_str, lex)
            .map(|d| d.to_date_naive());
        Some((op, target, include_none))
    };
    let start = compile_date(crate::model::parser::PrefixToken::Start, '^');
    let due = compile_date(crate::model::parser::PrefixToken::Due, '@');

    // --- Tag Filter ---
    let tag_query = lower.strip_prefix('#').map(str::to_string);

    // --- Status Filters ---
    let status = if lower == "is:done" || lex.search_is_done.contains(&lower) {
        Some(StatusFilter::Done)
    } else if lower == "is:started"
        || lower == "is:ongoing"
        || lex.search_is_started.contains(&lower)
        || lex.search_is_ongoing.contains(&lower)
    {
        Some(StatusFilter::InProcess)
    } else if lower == "is:active" || lex.search_is_active.contains(&lower) {
        Some(StatusFilter::Active)
    } else if lower == "is:ready"
        || lower == "is:blocked"
        || lex.search_is_ready.contains(&lower)
        || lex.search_is_blocked.contains(&lower)
    {
        Some(StatusFilter::Ready)
    } else if lower == "is:note" || lex.search_is_note.contains(&lower) {
        Some(StatusFilter::Note)
    } else if lower == "is:page" || lower == "is:journal" || lex.search_is_page.contains(&lower) {
        Some(StatusFilter::Page)
    } else if lower == "is:permanent" || lex.search_is_permanent.contains(&lower) {
        Some(StatusFilter::Permanent)
    } else if lower == "is:canceled"
        || lower == "is:cancelled"
        || lex.search_is_canceled.contains(&lower)
    {
        Some(StatusFilter::Canceled)
    } else if lower == "is:recurrent" || lex.search_is_recurrent.contains(&lower) {
        Some(StatusFilter::Recurrent)
    } else if lex.exact.get(&lower) == Some(&crate::model::parser::ExactToken::IsPinned) {
        Some(StatusFilter::Pinned)
    } else {
        None
    };

    CompiledTerm {
        lower,
        loc_query,
        duration,
        priority,
        start,
        due,
        tag_query,
        status,
    }
}

fn compile_expr(raw: RawExpr, lex: &crate::model::parser::ParserLexicon) -> SearchExpr {
    match raw {
        RawExpr::Term(s) => SearchExpr::Term(compile_term(&s, lex)),
        RawExpr::And(a, b) => SearchExpr::And(
            Box::new(compile_expr(*a, lex)),
            Box::new(compile_expr(*b, lex)),
        ),
        RawExpr::Or(a, b) => SearchExpr::Or(
            Box::new(compile_expr(*a, lex)),
            Box::new(compile_expr(*b, lex)),
        ),
        RawExpr::Not(a) => SearchExpr::Not(Box::new(compile_expr(*a, lex))),
    }
}

impl CompiledTerm {
    /// Evaluate the compiled term against a single task.
    fn match_task(&self, task: &Task, store: &crate::store::TaskStore) -> bool {
        if self.lower.is_empty() {
            return true;
        }

        // --- Location Filter (@@loc or loc:loc) ---
        if let Some(loc_query) = &self.loc_query {
            let is_match = |t: &Task| {
                t.locations
                    .iter()
                    .chain(t.transient_desc_locs.iter())
                    .any(|l| contains_ignore_case(l, loc_query))
            };
            return task.any_ancestor_matches(store, is_match);
        }

        // --- Duration Filter (~30m, ~<1h, ~>2h) ---
        if let Some((op, target)) = &self.duration
            && let Some(target) = target
        {
            let t_min = task.estimated_duration.unwrap_or(0);
            let t_max = task.estimated_duration_max.unwrap_or(t_min);

            if task.estimated_duration.is_none() {
                return false;
            }

            return match op {
                Op::Lt => t_min < *target,
                Op::Gt => t_max > *target,
                Op::Le => t_min <= *target,
                Op::Ge => t_max >= *target,
                Op::Eq => *target >= t_min && *target <= t_max,
            };
        }
        // Fall through to text match if parsing failed

        // --- Priority Filter (!1, !<3) ---
        if let Some((op, target)) = self.priority {
            let p = task.priority;
            return match op {
                Op::Lt => p < target,
                Op::Gt => p > target,
                Op::Le => p <= target,
                Op::Ge => p >= target,
                Op::Eq => p == target,
            };
        }

        // Start Date
        if let Some((op, target, include_none)) = &self.start
            && let Some(target) = target
        {
            return match task.dtstart.as_ref().map(|d| d.to_date_naive()) {
                Some(t_date) => match op {
                    Op::Lt => t_date < *target,
                    Op::Gt => t_date > *target,
                    Op::Le => t_date <= *target,
                    Op::Ge => t_date >= *target,
                    Op::Eq => t_date == *target,
                },
                None => *include_none,
            };
        }
        // Unparseable date: fall through

        // Due Date
        if let Some((op, target, include_none)) = &self.due
            && let Some(target) = target
        {
            return match task.due.as_ref().map(|d| d.to_date_naive()) {
                Some(t_date) => match op {
                    Op::Lt => t_date < *target,
                    Op::Gt => t_date > *target,
                    Op::Le => t_date <= *target,
                    Op::Ge => t_date >= *target,
                    Op::Eq => t_date == *target,
                },
                None => *include_none,
            };
        }

        // --- Tag Filter ---
        if let Some(tag_query) = &self.tag_query {
            let is_match = |t: &Task| {
                t.categories
                    .iter()
                    .chain(t.transient_desc_tags.iter())
                    .any(|c| contains_ignore_case(c, tag_query))
            };
            return task.any_ancestor_matches(store, is_match);
        }

        // --- Status Filters ---
        if let Some(status) = self.status {
            return match status {
                StatusFilter::Done => task.status.is_done(),
                StatusFilter::InProcess => task.status == TaskStatus::InProcess,
                StatusFilter::Active => !task.status.is_done(),
                // "ready/blocked" states are computed transiently in store.filter()
                // but for simple text matching here we mostly ignore them or treat as valid.
                StatusFilter::Ready => true,
                StatusFilter::Note => task.is_note,
                StatusFilter::Page => task.is_journal,
                StatusFilter::Permanent => task.permanent,
                StatusFilter::Canceled => task.status == TaskStatus::Cancelled,
                StatusFilter::Pinned => task.pinned,
                StatusFilter::Recurrent => task.rrule.is_some(),
            };
        }

        // --- Fallback: Text Search ---
        // Matches summary, description, categories, or location.
        let is_match = |t: &Task| {
            let summary_match = contains_ignore_case(&t.summary, &self.lower);
            let desc_match = contains_ignore_case(&t.description, &self.lower);
            let cat_match = t
                .categories
                .iter()
                .chain(t.transient_desc_tags.iter())
                .any(|c| contains_ignore_case(c, &self.lower));
            let loc_match = t
                .locations
                .iter()
                .chain(t.transient_desc_locs.iter())
                .any(|l| contains_ignore_case(l, &self.lower));

            summary_match || desc_match || cat_match || loc_match
        };
        task.any_ancestor_matches(store, is_match)
    }
}

impl Task {
    /// Checks if the task matches the given search query using boolean logic.
    /// Supports implicit AND, OR (|), NOT (-), and parentheses.
    pub fn matches_search_term(&self, query: &str, store: &crate::store::TaskStore) -> bool {
        if query.trim().is_empty() {
            return true;
        }

        let lex_guard = crate::model::parser::LEXICON.read().unwrap();
        let q = Query::new(query, &lex_guard);
        q.matches(self, store)
    }

    /// Returns true if `self` or any ancestor (walking `parent_uid` upward,
    /// cycle-guarded) satisfies `matches`.
    fn any_ancestor_matches(
        &self,
        store: &crate::store::TaskStore,
        matches: impl Fn(&Task) -> bool,
    ) -> bool {
        if matches(self) {
            return true;
        }
        let mut curr = self.parent_uid.as_deref();
        let mut visited = std::collections::HashSet::new();
        while let Some(p_uid) = curr {
            if !visited.insert(p_uid) {
                break;
            }
            if let Some(p) = store.get_task_ref(p_uid) {
                if matches(p) {
                    return true;
                }
                curr = p.parent_uid.as_deref();
            } else {
                break;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use crate::model::item::Task;
    use std::collections::HashMap;

    #[test]
    fn test_basic_and_or_not() {
        let store =
            crate::store::TaskStore::new(std::sync::Arc::new(crate::context::TestContext::new()));
        let aliases: HashMap<String, Vec<String>> = HashMap::new();
        let mut t = Task::new("Test", &aliases, None);
        t.summary = "Work today".to_string();
        t.description = "Finish the report".to_string();
        t.categories.push("work".to_string());
        t.categories.push("today".to_string());
        t.locations.push("home office".to_string());

        // Implicit AND (space)
        assert!(t.matches_search_term("work today", &store));

        // OR using |
        assert!(t.matches_search_term("urgent | work", &store));
        // The task location is "home office", which contains "home".
        // The OR expression should evaluate to true.
        assert!(t.matches_search_term("urgent | home", &store));
        assert!(!t.matches_search_term("urgent | beach", &store));

        // NOT prefix '-'
        assert!(!t.matches_search_term("-work", &store));
        assert!(t.matches_search_term("-beach", &store));

        // Grouping and NOT combined: (work OR urgent) AND NOT today -> false
        assert!(!t.matches_search_term("(work | urgent) -today", &store));
    }

    #[test]
    fn test_quotes_and_term() {
        let store =
            crate::store::TaskStore::new(std::sync::Arc::new(crate::context::TestContext::new()));
        let aliases: HashMap<String, Vec<String>> = HashMap::new();
        let mut t = Task::new("Test", &aliases, None);
        t.summary = "Big task".to_string();

        // Exact quoted match
        assert!(t.matches_search_term("\"Big task\"", &store));

        // Partial term match
        assert!(t.matches_search_term("big", &store));

        // Non-matching quoted phrase
        assert!(!t.matches_search_term("\"small task\"", &store));
    }

    #[test]
    fn test_tag_and_location_filters() {
        let store =
            crate::store::TaskStore::new(std::sync::Arc::new(crate::context::TestContext::new()));
        let aliases: HashMap<String, Vec<String>> = HashMap::new();
        let mut t = Task::new("Test", &aliases, None);
        t.categories.push("home".to_string());
        t.locations.push("Kitchen".to_string());

        // Tag filter using '#'
        assert!(t.matches_search_term("#home", &store));

        // Location filters: 'loc:' and '@@' prefixes should both work
        assert!(t.matches_search_term("loc:Kitchen", &store));
        assert!(t.matches_search_term("@@Kitchen", &store));
    }

    #[test]
    fn test_ancestor_search() {
        let mut store =
            crate::store::TaskStore::new(std::sync::Arc::new(crate::context::TestContext::new()));
        let aliases: HashMap<String, Vec<String>> = HashMap::new();

        let mut parent = Task::new("PowerShed", &aliases, None);
        parent.uid = "parent_uid".to_string();

        let mut child = Task::new("BOM", &aliases, None);
        child.uid = "child_uid".to_string();
        child.parent_uid = Some("parent_uid".to_string());

        let mut grandchild = Task::new("Bac d'acier", &aliases, None);
        grandchild.uid = "grandchild_uid".to_string();
        grandchild.parent_uid = Some("child_uid".to_string());

        store.add_task(parent);
        store.add_task(child);

        assert!(grandchild.matches_search_term("PowerShed bom acier", &store));
    }
}
