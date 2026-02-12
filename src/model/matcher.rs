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
// The evaluation delegates to `matches_primitive` which handles specific filters
// (e.g. #tag, @date, is:done) and substring matching.

use crate::model::item::{Task, TaskStatus};
use chrono::{Duration, Local, NaiveDate};

#[derive(Debug, Clone)]
enum SearchExpr {
    Term(String),
    And(Box<SearchExpr>, Box<SearchExpr>),
    Or(Box<SearchExpr>, Box<SearchExpr>),
    Not(Box<SearchExpr>),
}

impl SearchExpr {
    fn matches(&self, task: &Task) -> bool {
        match self {
            SearchExpr::Term(s) => {
                if s.is_empty() {
                    true
                } else {
                    task.matches_primitive(s)
                }
            }
            SearchExpr::And(a, b) => a.matches(task) && b.matches(task),
            SearchExpr::Or(a, b) => a.matches(task) || b.matches(task),
            SearchExpr::Not(a) => !a.matches(task),
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
                // Check if it's a negative number (e.g. !-1, or text like rem:-10m) or a NOT operator.
                // Heuristic: If followed by a space or another operator, treat as text "-" or implicit term.
                // But generally, `-` at the start of a term is NOT.
                // We peek next.
                chars.next(); // Consume '-'
                if let Some(&next_c) = chars.peek() {
                    if next_c.is_whitespace() || next_c == '(' || next_c == ')' || next_c == '|' {
                        // " - " or "-(" -> Treat as text "-" (which might be ignored or matched)
                        // Actually, standard search engines treat "- (" as NOT ( ... ).
                        // But for simplicity, we enforce attached NOT: "-term".
                        // Wait, "-(" should be NotPrefix, LParen.
                        if next_c == '(' {
                            tokens.push(Token::NotPrefix);
                        } else {
                            tokens.push(Token::Text("-".to_string()));
                        }
                    } else {
                        // Attached to something, e.g. "-A" or "-5".
                        // We treat it as NOT operator.
                        // Exception: if it looks like part of a range or negative number inside a filter?
                        // E.g. "!-1" (Priority -1).
                        // Our tokenizer splits operators. So "!-1" is '!' then '-' then '1'?
                        // No, the catch-all `_` block handles text including punctuation.
                        // So `!-1` starts with `!`, goes to `_` block, consumes `-`.
                        // This `-` branch is only hit if `-` is the START of a token.
                        // So `-5` -> NotPrefix, Text("5"). matches_primitive("5")=true/false. Not(..) inverts it.
                        // Searching `-5` excludes "5". This is standard.
                        tokens.push(Token::NotPrefix);
                    }
                } else {
                    // Trailing dash
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

    fn parse(&mut self) -> SearchExpr {
        if self.tokens.is_empty() {
            return SearchExpr::Term("".to_string());
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
    fn parse_or(&mut self) -> SearchExpr {
        let mut left = self.parse_and();

        while let Some(Token::Or) = self.peek() {
            self.advance();
            let right = self.parse_and();
            left = SearchExpr::Or(Box::new(left), Box::new(right));
        }
        left
    }

    // Implicit AND handles sequences of terms
    fn parse_and(&mut self) -> SearchExpr {
        let mut left = self.parse_unary();

        while let Some(token) = self.peek() {
            if matches!(token, Token::Or | Token::RParen) {
                break;
            }
            // Implicit AND between adjacent tokens
            let right = self.parse_unary();
            left = SearchExpr::And(Box::new(left), Box::new(right));
        }
        left
    }

    // NOT operator
    fn parse_unary(&mut self) -> SearchExpr {
        if let Some(Token::NotPrefix) = self.peek() {
            self.advance();
            let expr = self.parse_primary();
            return SearchExpr::Not(Box::new(expr));
        }
        self.parse_primary()
    }

    // Terms or Grouping
    fn parse_primary(&mut self) -> SearchExpr {
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
                SearchExpr::Term(term)
            }
            _ => SearchExpr::Term("".to_string()), // Fallback
        }
    }
}

impl Task {
    /// Checks if the task matches the given search query using boolean logic.
    /// Supports implicit AND, OR (|), NOT (-), and parentheses.
    pub fn matches_search_term(&self, query: &str) -> bool {
        if query.trim().is_empty() {
            return true;
        }

        let tokens = tokenize_query(query);
        let mut parser = Parser::new(tokens);
        let expr = parser.parse();

        expr.matches(self)
    }

    /// Evaluates a single primitive search term (e.g., "#tag", "is:done", or "text").
    /// Returns true if the task matches this specific term.
    fn matches_primitive(&self, part: &str) -> bool {
        if part.is_empty() {
            return true;
        }

        // Trim whitespace and strip surrounding quotes for quoted phrases.
        // We do this to support searches like "exact phrase" or tag:"my tag".
        let part = part.trim();
        let part_unquoted = if part.starts_with('"') && part.ends_with('"') && part.len() >= 2 {
            &part[1..part.len() - 1]
        } else {
            part
        };
        let part_lower = part_unquoted.to_lowercase();

        // --- Location Filter (@@loc or loc:loc) ---
        if let Some(loc_query) = part_lower
            .strip_prefix("@@")
            .or_else(|| part_lower.strip_prefix("loc:"))
        {
            if let Some(t_loc) = &self.location {
                return t_loc.to_lowercase().contains(loc_query);
            } else {
                return false;
            }
        }

        // --- Duration Filter (~30m, ~<1h, ~>2h) ---
        if part_lower.starts_with('~') {
            let (op, val_str) = if let Some(stripped) = part_lower.strip_prefix("~<=") {
                ("<=", stripped)
            } else if let Some(stripped) = part_lower.strip_prefix("~>=") {
                (">=", stripped)
            } else if let Some(stripped) = part_lower.strip_prefix("~<") {
                ("<", stripped)
            } else if let Some(stripped) = part_lower.strip_prefix("~>") {
                (">", stripped)
            } else if let Some(stripped) = part_lower.strip_prefix('~') {
                ("=", stripped)
            } else {
                ("", "")
            };

            if !op.is_empty() {
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

                    let ok = match op {
                        "<" => t_min < target,
                        ">" => t_max > target,
                        "<=" => t_min <= target,
                        ">=" => t_max >= target,
                        _ => target >= t_min && target <= t_max,
                    };
                    return ok;
                }
            }
            // Fall through to text match if parsing failed
        }

        // --- Priority Filter (!1, !<3) ---
        if part_lower.starts_with('!') {
            let (op, val_str) = if let Some(stripped) = part_lower.strip_prefix("!<=") {
                ("<=", stripped)
            } else if let Some(stripped) = part_lower.strip_prefix("!>=") {
                (">=", stripped)
            } else if let Some(stripped) = part_lower.strip_prefix("!<") {
                ("<", stripped)
            } else if let Some(stripped) = part_lower.strip_prefix("!>") {
                (">", stripped)
            } else if let Some(stripped) = part_lower.strip_prefix('!') {
                ("=", stripped)
            } else {
                ("", "")
            };

            if !op.is_empty()
                && let Ok(target) = val_str.parse::<u8>()
            {
                let p = self.priority;
                let ok = match op {
                    "<" => p < target,
                    ">" => p > target,
                    "<=" => p <= target,
                    ">=" => p >= target,
                    _ => p == target,
                };
                return ok;
            }
        }

        // --- Date Filters (@due, ^start) ---
        let check_date_filter =
            |prefix_char: char, alt_prefix: &str, task_date: Option<NaiveDate>| -> Option<bool> {
                if !part_lower.starts_with(prefix_char) && !part_lower.starts_with(alt_prefix) {
                    return None;
                }

                let raw_val = part_lower
                    .strip_prefix(alt_prefix)
                    .or_else(|| part_lower.strip_prefix(prefix_char))
                    .unwrap_or("");

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
                        Some(t_date) => {
                            let ok = match op {
                                "<" => t_date < target,
                                ">" => t_date > target,
                                "<=" => t_date <= target,
                                ">=" => t_date >= target,
                                _ => t_date == target,
                            };
                            return Some(ok);
                        }
                        None => {
                            if include_none {
                                return Some(true);
                            } else {
                                return Some(false);
                            }
                        }
                    }
                }
                None
            };

        // Start Date
        let t_start = self.dtstart.as_ref().map(|d| d.to_date_naive());
        if let Some(passed) = check_date_filter('^', "start:", t_start) {
            return passed;
        }

        // Due Date
        let t_due = self.due.as_ref().map(|d| d.to_date_naive());
        if let Some(passed) = check_date_filter('@', "due:", t_due) {
            return passed;
        }

        // --- Tag Filter ---
        if let Some(tag_query) = part_lower.strip_prefix('#') {
            return self
                .categories
                .iter()
                .any(|c| c.to_lowercase().contains(tag_query));
        }

        // --- Status Filters ---
        if part_lower == "is:done" {
            return self.status.is_done();
        }
        if part_lower == "is:started" || part_lower == "is:ongoing" {
            return self.status == TaskStatus::InProcess;
        }
        if part_lower == "is:active" {
            return !self.status.is_done();
        }
        if part_lower == "is:ready" || part_lower == "is:blocked" {
            // "ready/blocked" states are computed transiently in store.filter()
            // but for simple text matching here we mostly ignore them or treat as valid.
            // (Note: full filtering support for these requires Context from store)
            return true;
        }

        // --- Fallback: Text Search ---
        // Matches summary, description, categories, or location.
        let summary_match = self.summary.to_lowercase().contains(&part_lower);
        let desc_match = self.description.to_lowercase().contains(&part_lower);
        let cat_match = self
            .categories
            .iter()
            .any(|c| c.to_lowercase().contains(&part_lower));
        let loc_match = self
            .location
            .as_deref()
            .is_some_and(|l| l.to_lowercase().contains(&part_lower));

        summary_match || desc_match || cat_match || loc_match
    }
}

#[cfg(test)]
mod tests {
    use crate::model::item::Task;
    use std::collections::HashMap;

    #[test]
    fn test_basic_and_or_not() {
        let aliases: HashMap<String, Vec<String>> = HashMap::new();
        let mut t = Task::new("Test", &aliases, None);
        t.summary = "Work today".to_string();
        t.description = "Finish the report".to_string();
        t.categories.push("work".to_string());
        t.categories.push("today".to_string());
        t.location = Some("home office".to_string());

        // Implicit AND (space)
        assert!(t.matches_search_term("work today"));

        // OR using |
        assert!(t.matches_search_term("urgent | work"));
        // The task location is "home office", which contains "home".
        // The OR expression should evaluate to true.
        assert!(t.matches_search_term("urgent | home"));
        assert!(!t.matches_search_term("urgent | beach"));

        // NOT prefix '-'
        assert!(!t.matches_search_term("-work"));
        assert!(t.matches_search_term("-beach"));

        // Grouping and NOT combined: (work OR urgent) AND NOT today -> false
        assert!(!t.matches_search_term("(work | urgent) -today"));
    }

    #[test]
    fn test_quotes_and_term() {
        let aliases: HashMap<String, Vec<String>> = HashMap::new();
        let mut t = Task::new("Test", &aliases, None);
        t.summary = "Big task".to_string();

        // Exact quoted match
        assert!(t.matches_search_term("\"Big task\""));

        // Partial term match
        assert!(t.matches_search_term("big"));

        // Non-matching quoted phrase
        assert!(!t.matches_search_term("\"small task\""));
    }

    #[test]
    fn test_tag_and_location_filters() {
        let aliases: HashMap<String, Vec<String>> = HashMap::new();
        let mut t = Task::new("Test", &aliases, None);
        t.categories.push("home".to_string());
        t.location = Some("Kitchen".to_string());

        // Tag filter using '#'
        assert!(t.matches_search_term("#home"));

        // Location filters: 'loc:' and '@@' prefixes should both work
        assert!(t.matches_search_term("loc:Kitchen"));
        assert!(t.matches_search_term("@@Kitchen"));
    }
}
