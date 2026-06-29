// SPDX-License-Identifier: GPL-3.0-or-later
// File: ./src/model/extractor.rs
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug)]
pub struct ExtractedTask {
    pub uid: String,
    pub parsed_existing_uid: Option<String>, // Found via <!-- uid:... -->
    pub parent_uid: Option<String>,
    pub dependencies: Vec<String>,
    pub raw_text: String,
    pub description: String,
    pub is_completed: bool,
}

fn extract_uid_tag(line: &str) -> (String, Option<String>) {
    if let Some(idx) = line.rfind("<!-- uid:")
        && let Some(end_idx) = line[idx..].find("-->")
    {
        let uid = line[idx + 9..idx + end_idx].trim().to_string();
        let clean_line = line[..idx].trim().to_string();
        return (clean_line, Some(uid));
    }
    (line.trim_end().to_string(), None)
}

pub fn has_extractable_subtasks(input: &str) -> bool {
    for line in input.lines() {
        let mut byte_offset = 0;
        for c in line.chars() {
            if c == ' ' || c == '\t' {
                byte_offset += c.len_utf8();
            } else {
                break;
            }
        }
        let rest = &line[byte_offset..];

        // Check for headers
        if rest.starts_with("# ") || rest.starts_with("## ") || rest.starts_with("### ") {
            return true;
        }

        if rest.starts_with("- ") || rest.starts_with("* ") || rest.starts_with("+ ") {
            let after_marker = &rest[2..];
            if after_marker.starts_with("[ ] ")
                || after_marker.starts_with("[x] ")
                || after_marker.starts_with("[X] ")
            {
                return true;
            }
        } else {
            let mut digit_bytes = 0;
            for c in rest.chars() {
                if c.is_ascii_digit() {
                    digit_bytes += c.len_utf8();
                } else {
                    break;
                }
            }
            if digit_bytes > 0 && rest[digit_bytes..].starts_with(". ") {
                let after_marker = &rest[digit_bytes + 2..];
                if after_marker.starts_with("[ ] ")
                    || after_marker.starts_with("[x] ")
                    || after_marker.starts_with("[X] ")
                {
                    return true;
                }
            }
        }
    }
    false
}

/// Takes a raw markdown string.
/// Returns (Cleaned Root Description, List of Extracted Subtasks).
pub fn extract_markdown_tasks(input: &str) -> (String, Vec<ExtractedTask>) {
    let mut cleaned_root_desc = String::new();
    let mut extracted: Vec<ExtractedTask> = Vec::new();

    // Stack stores (indent_level, task_uid)
    let mut indent_stack: Vec<(usize, String)> = Vec::new();
    // Map stores indent_level -> uid of last numbered task
    let mut last_numbered_at_indent: HashMap<usize, String> = HashMap::new();

    let mut active_task_idx: Option<usize> = None;

    for line in input.lines() {
        let mut indent = 0;
        let mut byte_offset = 0;
        for c in line.chars() {
            if c == ' ' {
                indent += 1;
                byte_offset += c.len_utf8();
            } else if c == '\t' {
                indent += 4;
                byte_offset += c.len_utf8();
            } else {
                break;
            }
        }

        let rest = &line[byte_offset..];

        if rest.is_empty() {
            // Empty line: append to active task if exists, else root
            if let Some(idx) = active_task_idx {
                extracted[idx].description.push('\n');
            } else {
                cleaned_root_desc.push('\n');
            }
            continue;
        }

        // Check if it's a valid Markdown task list
        let mut is_task = false;
        let mut is_numbered = false;
        let mut is_completed = false;
        let mut raw_text = "";
        let mut header_depth = 0;

        if let Some(stripped) = rest.strip_prefix("# ") {
            is_task = true;
            raw_text = stripped;
            header_depth = 1;
        } else if let Some(stripped) = rest.strip_prefix("## ") {
            is_task = true;
            raw_text = stripped;
            header_depth = 2;
        } else if let Some(stripped) = rest.strip_prefix("### ") {
            is_task = true;
            raw_text = stripped;
            header_depth = 3;
        } else if rest.starts_with("- ") || rest.starts_with("* ") || rest.starts_with("+ ") {
            let after_marker = &rest[2..];
            if let Some(stripped) = after_marker.strip_prefix("[ ] ") {
                is_task = true;
                is_completed = false;
                raw_text = stripped;
            } else if let Some(stripped) = after_marker.strip_prefix("[x] ") {
                is_task = true;
                is_completed = true;
                raw_text = stripped;
            } else if let Some(stripped) = after_marker.strip_prefix("[X] ") {
                is_task = true;
                is_completed = true;
                raw_text = stripped;
            }
        } else {
            // Check for numbered lists (e.g., "1. [ ] ")
            let mut digit_bytes = 0;
            for c in rest.chars() {
                if c.is_ascii_digit() {
                    digit_bytes += c.len_utf8();
                } else {
                    break;
                }
            }
            if digit_bytes > 0 && rest[digit_bytes..].starts_with(". ") {
                let after_marker = &rest[digit_bytes + 2..];
                if let Some(stripped) = after_marker.strip_prefix("[ ] ") {
                    is_task = true;
                    is_numbered = true;
                    is_completed = false;
                    raw_text = stripped;
                } else if let Some(stripped) = after_marker.strip_prefix("[x] ") {
                    is_task = true;
                    is_numbered = true;
                    is_completed = true;
                    raw_text = stripped;
                } else if let Some(stripped) = after_marker.strip_prefix("[X] ") {
                    is_task = true;
                    is_numbered = true;
                    is_completed = true;
                    raw_text = stripped;
                }
            }
        }

        if is_task {
            let (clean_text, parsed_uid) = extract_uid_tag(raw_text);
            let uid = parsed_uid
                .clone()
                .unwrap_or_else(|| Uuid::new_v4().to_string());

            // For headers, depth is absolute. For lists, it's relative to indentation.
            let effective_indent = if header_depth > 0 {
                (header_depth - 1) * 4 // Treat H1 as 0, H2 as 4, H3 as 8
            } else {
                indent
            };

            // Pop stack until we find a parent that has a strictly smaller indentation
            while let Some(&(stack_indent, _)) = indent_stack.last() {
                if stack_indent >= effective_indent {
                    indent_stack.pop();
                } else {
                    break;
                }
            }
            let parent_uid = indent_stack.last().map(|(_, id)| id.clone());

            // Determine dependencies using the last numbered task at THIS indentation level
            let mut dependencies = Vec::new();
            if is_numbered {
                if let Some(dep_uid) = last_numbered_at_indent.get(&effective_indent) {
                    dependencies.push(dep_uid.clone());
                }
                last_numbered_at_indent.insert(effective_indent, uid.clone());
            } else {
                // Breaking the numbered chain
                last_numbered_at_indent.remove(&effective_indent);
            }

            // Push ourselves to the stack to become a potential parent for the next lines
            indent_stack.push((effective_indent, uid.clone()));

            extracted.push(ExtractedTask {
                uid,
                parsed_existing_uid: parsed_uid,
                parent_uid,
                dependencies,
                raw_text: clean_text,
                description: String::new(),
                is_completed,
            });
            active_task_idx = Some(extracted.len() - 1);
        } else {
            // Not a task line. Append it to the relevant description.
            if indent == 0 || header_depth > 0 {
                // Indent 0 or headers break the list completely. Back to root parent notes.
                active_task_idx = None;
                indent_stack.clear();
                last_numbered_at_indent.clear();

                if !cleaned_root_desc.is_empty() && !cleaned_root_desc.ends_with('\n') {
                    cleaned_root_desc.push('\n');
                }
                cleaned_root_desc.push_str(rest);
                cleaned_root_desc.push('\n');
            } else if let Some(idx) = active_task_idx {
                // Belongs to the active subtask's notes
                if !extracted[idx].description.is_empty()
                    && !extracted[idx].description.ends_with('\n')
                {
                    extracted[idx].description.push('\n');
                }
                extracted[idx].description.push_str(rest);
                extracted[idx].description.push('\n');
            } else {
                // Indented, but no active task -> Belongs to root parent
                if !cleaned_root_desc.is_empty() && !cleaned_root_desc.ends_with('\n') {
                    cleaned_root_desc.push('\n');
                }
                cleaned_root_desc.push_str(rest);
                cleaned_root_desc.push('\n');
            }
        }
    }

    // Clean up trailing newlines
    let cleaned_root_desc = cleaned_root_desc.trim_end().to_string();
    for task in &mut extracted {
        task.description = task.description.trim_end().to_string();
    }

    (cleaned_root_desc, extracted)
}
