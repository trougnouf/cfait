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
    pub status: crate::model::TaskStatus,
    pub percent_complete: Option<u8>,
    pub is_note: bool,
}

fn parse_checkbox(s: &str) -> Option<(crate::model::TaskStatus, Option<u8>, &str)> {
    if s.len() < 4 || !s.starts_with('[') {
        return None;
    }
    let mut chars = s.chars();
    chars.next(); // '['
    let inner = chars.next()?;
    if chars.next()? != ']' || chars.next()? != ' ' {
        return None;
    }
    let rest = chars.as_str();
    match inner {
        ' ' => Some((crate::model::TaskStatus::NeedsAction, None, rest)),
        'x' | 'X' | '*' => Some((crate::model::TaskStatus::Completed, Some(100), rest)),
        '/' => Some((crate::model::TaskStatus::NeedsAction, Some(50), rest)),
        '>' | '▶' => Some((crate::model::TaskStatus::InProcess, None, rest)),
        '<' => Some((crate::model::TaskStatus::NeedsAction, Some(50), rest)),
        '-' | '~' => Some((crate::model::TaskStatus::Cancelled, None, rest)),
        _ => None,
    }
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

pub fn extract_list_prefix(line: &str) -> String {
    let mut prefix = String::new();
    let mut byte_offset = 0;
    let chars = line.chars();

    // Extract leading whitespace
    for c in chars {
        if c == ' ' || c == '\t' {
            prefix.push(c);
            byte_offset += c.len_utf8();
        } else {
            break;
        }
    }

    let rest = &line[byte_offset..];
    if rest.starts_with("- [ ] ")
        || rest.starts_with("- [x] ")
        || rest.starts_with("- [X] ")
        || rest.starts_with("- [/] ")
        || rest.starts_with("- [-] ")
        || rest.starts_with("- [<] ")
        || rest.starts_with("- [>] ")
    {
        prefix.push_str("- [ ] ");
    } else if rest.starts_with("* [ ] ")
        || rest.starts_with("* [x] ")
        || rest.starts_with("* [X] ")
        || rest.starts_with("* [/] ")
        || rest.starts_with("* [-] ")
        || rest.starts_with("* [<] ")
        || rest.starts_with("* [>] ")
    {
        prefix.push_str("* [ ] ");
    } else if rest.starts_with("- ") {
        prefix.push_str("- ");
    } else if rest.starts_with("* ") {
        prefix.push_str("* ");
    } else {
        let mut digit_bytes = 0;
        for c in rest.chars() {
            if c.is_ascii_digit() {
                digit_bytes += c.len_utf8();
            } else {
                break;
            }
        }
        if digit_bytes > 0 {
            let after = &rest[digit_bytes..];
            if after.starts_with(". [ ] ")
                || after.starts_with(". [x] ")
                || after.starts_with(". [X] ")
                || after.starts_with(". [/] ")
                || after.starts_with(". [-] ")
                || after.starts_with(". [<] ")
                || after.starts_with(". [>] ")
            {
                let num_str = &rest[..digit_bytes];
                let num: usize = num_str.parse().unwrap_or(1);
                prefix.push_str(&format!("{}. [ ] ", num + 1));
            } else if after.starts_with(". ") {
                let num_str = &rest[..digit_bytes];
                let num: usize = num_str.parse().unwrap_or(1);
                prefix.push_str(&format!("{}. ", num + 1));
            }
        }
    }
    prefix
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
            return true;
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
                return true;
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

    #[derive(PartialEq, Clone, Copy, Debug)]
    enum StackItemKind {
        Heading(usize), // level 1, 2, 3...
        List(usize),    // indent in spaces
    }

    #[derive(PartialEq, Clone, Copy, Debug)]
    enum ItemKind {
        NumberedTask(usize), // block_id
        Other,
    }

    // Stack stores (StackItemKind, task_uid, extracted_idx)
    let mut indent_stack: Vec<(StackItemKind, String, usize)> = Vec::new();
    let mut item_kind_at_indent: HashMap<usize, ItemKind> = HashMap::new();
    let mut next_block_id = 0;
    let mut numbered_tasks: Vec<(usize, usize, usize)> = Vec::new(); // (block_id, parsed_num, extracted_idx)

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

        item_kind_at_indent.retain(|&k, _| k <= indent);

        // Check if it's a valid Markdown task list
        let mut is_task = false;
        let mut is_numbered = false;
        let mut parsed_num = 0;
        let mut parsed_status = crate::model::TaskStatus::NeedsAction;
        let mut parsed_pc = None;
        let mut is_note = false;
        let mut raw_text = "";
        let mut header_depth = 0;

        if let Some(stripped) = rest.strip_prefix("# ") {
            is_task = true;
            is_note = true;
            header_depth = 1;
            raw_text = stripped;
        } else if let Some(stripped) = rest.strip_prefix("## ") {
            is_task = true;
            is_note = true;
            header_depth = 2;
            raw_text = stripped;
        } else if let Some(stripped) = rest.strip_prefix("### ") {
            is_task = true;
            is_note = true;
            header_depth = 3;
            raw_text = stripped;
        }

        if is_task {
            if let Some((status, pc, r)) = parse_checkbox(raw_text) {
                is_note = false; // Header with a checkbox overrides is_note
                parsed_status = status;
                parsed_pc = pc;
                raw_text = r;
            }
        } else if rest.starts_with("- ") || rest.starts_with("* ") || rest.starts_with("+ ") {
            let after_marker = &rest[2..];
            if let Some((status, pc, r)) = parse_checkbox(after_marker) {
                is_task = true;
                is_note = false;
                parsed_status = status;
                parsed_pc = pc;
                raw_text = r;
            } else {
                is_task = true;
                is_note = true;
                raw_text = after_marker;
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
                if let Some((status, pc, r)) = parse_checkbox(after_marker) {
                    is_task = true;
                    is_numbered = true;
                    is_note = false;
                    parsed_num = rest[..digit_bytes].parse::<usize>().unwrap_or(1);
                    parsed_status = status;
                    parsed_pc = pc;
                    raw_text = r;
                } else {
                    is_task = true;
                    is_numbered = true;
                    is_note = true;
                    parsed_num = rest[..digit_bytes].parse::<usize>().unwrap_or(1);
                    raw_text = after_marker;
                }
            }
        }

        if is_task {
            let (clean_text, parsed_uid) = extract_uid_tag(raw_text);
            let uid = parsed_uid
                .clone()
                .unwrap_or_else(|| Uuid::new_v4().to_string());

            let current_kind = if header_depth > 0 {
                StackItemKind::Heading(header_depth)
            } else {
                StackItemKind::List(indent)
            };

            // Pop stack until we find a valid parent
            while let Some(&(kind, _, _)) = indent_stack.last() {
                match current_kind {
                    StackItemKind::Heading(curr_lvl) => {
                        match kind {
                            StackItemKind::Heading(stack_lvl) => {
                                if stack_lvl >= curr_lvl {
                                    indent_stack.pop();
                                } else {
                                    break;
                                }
                            }
                            StackItemKind::List(_) => {
                                indent_stack.pop(); // Headings always pop lists
                            }
                        }
                    }
                    StackItemKind::List(curr_indent) => {
                        match kind {
                            StackItemKind::Heading(_) => {
                                break; // Lists nest under headings
                            }
                            StackItemKind::List(stack_indent) => {
                                if stack_indent >= curr_indent {
                                    indent_stack.pop();
                                } else {
                                    break;
                                }
                            }
                        }
                    }
                }
            }

            let parent_uid = indent_stack.last().map(|(_, id, _)| id.clone());

            let new_idx = extracted.len();

            if is_numbered {
                let block_id = match item_kind_at_indent.get(&indent) {
                    Some(ItemKind::NumberedTask(b)) => *b,
                    _ => {
                        let b = next_block_id;
                        next_block_id += 1;
                        b
                    }
                };
                item_kind_at_indent.insert(indent, ItemKind::NumberedTask(block_id));
                numbered_tasks.push((block_id, parsed_num, new_idx));
            } else {
                item_kind_at_indent.insert(indent, ItemKind::Other);
            }

            // Push ourselves to the stack to become a potential parent for the next lines
            indent_stack.push((current_kind, uid.clone(), new_idx));

            extracted.push(ExtractedTask {
                uid,
                parsed_existing_uid: parsed_uid,
                parent_uid,
                dependencies: Vec::new(),
                raw_text: clean_text,
                description: String::new(),
                status: parsed_status,
                percent_complete: parsed_pc,
                is_note,
            });
            active_task_idx = Some(new_idx);
        } else {
            // Not a task line.
            item_kind_at_indent.insert(indent, ItemKind::Other);

            // Pop any List items from the stack that are at the same or deeper indentation
            // because text belonging to a List item MUST be indented more than the item itself.
            while let Some(&(kind, _, _)) = indent_stack.last() {
                if let StackItemKind::List(stack_indent) = kind {
                    if stack_indent >= indent {
                        indent_stack.pop();
                    } else {
                        break;
                    }
                } else {
                    break; // Stop at Headings
                }
            }

            let target_idx = indent_stack.last().map(|&(_, _, idx)| idx);

            if let Some(idx) = target_idx {
                if !extracted[idx].description.is_empty()
                    && !extracted[idx].description.ends_with('\n')
                {
                    extracted[idx].description.push('\n');
                }
                extracted[idx].description.push_str(rest);
                extracted[idx].description.push('\n');
                active_task_idx = Some(idx); // Update active_task_idx so empty lines go here
            } else {
                if !cleaned_root_desc.is_empty() && !cleaned_root_desc.ends_with('\n') {
                    cleaned_root_desc.push('\n');
                }
                cleaned_root_desc.push_str(rest);
                cleaned_root_desc.push('\n');
                active_task_idx = None; // Update active_task_idx so empty lines go here
            }
        }
    }

    // Second pass: resolve out-of-order numbered dependencies
    let mut blocks: HashMap<usize, Vec<(usize, usize)>> = HashMap::new();
    for (b_id, p_num, e_idx) in numbered_tasks {
        blocks.entry(b_id).or_default().push((p_num, e_idx));
    }

    for (_, list) in blocks {
        let mut uids_by_num: HashMap<usize, Vec<String>> = HashMap::new();
        for &(num, e_idx) in &list {
            uids_by_num
                .entry(num)
                .or_default()
                .push(extracted[e_idx].uid.clone());
        }

        let mut unique_nums: Vec<usize> = uids_by_num.keys().copied().collect();
        unique_nums.sort_unstable();

        for (num, e_idx) in list {
            let prev_num = unique_nums.iter().rev().find(|&&n| n < num).copied();
            if let Some(p_num) = prev_num
                && let Some(deps) = uids_by_num.get(&p_num)
            {
                extracted[e_idx].dependencies.extend(deps.iter().cloned());
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

pub fn serialize_task_tree(store: &crate::store::TaskStore, root_uid: &str) -> String {
    let mut out = String::new();
    let root = if let Some(r) = store.get_task_ref(root_uid) {
        r
    } else {
        return out;
    };

    let mut children_map: std::collections::HashMap<String, Vec<&crate::model::Task>> =
        std::collections::HashMap::new();
    for map in store.calendars.values() {
        for t in map.values() {
            if let Some(p) = &t.parent_uid {
                // Skip trashed/recovered tasks so they don't appear as ghost subtasks,
                // unless we are explicitly serializing a tree that is ALREADY in the trash.
                if (t.calendar_href == crate::storage::LOCAL_TRASH_HREF
                    || t.calendar_href == "local://recovery")
                    && t.calendar_href != root.calendar_href
                {
                    continue;
                }
                children_map.entry(p.clone()).or_default().push(t);
            }
        }
    }

    // Topologically sort children so that blocked tasks inherently follow their dependencies.
    // This perfectly preserves sequence ordering (1., 2., 3.) when re-extracting markdown.
    for list in children_map.values_mut() {
        list.sort_by(|a, b| {
            a.compare_for_sort(b, 5, false, crate::config::SortPreset::UrgentStartedDue)
        });

        if list.len() <= 1 {
            continue;
        }

        let mut uids_in_list = std::collections::HashSet::new();
        for t in list.iter() {
            uids_in_list.insert(t.uid.as_str());
        }

        let mut needs_sort = false;
        for t in list.iter() {
            for dep in &t.dependencies {
                if uids_in_list.contains(dep.as_str()) {
                    needs_sort = true;
                    break;
                }
            }
            if needs_sort {
                break;
            }
        }

        if !needs_sort {
            continue;
        }

        let mut in_degree: HashMap<&str, usize> = HashMap::new();
        let mut graph: HashMap<&str, Vec<&str>> = HashMap::new();

        for t in list.iter() {
            in_degree.insert(t.uid.as_str(), 0);
        }

        for t in list.iter() {
            for dep in &t.dependencies {
                if uids_in_list.contains(dep.as_str()) {
                    *in_degree.entry(t.uid.as_str()).or_insert(0) += 1;
                    graph.entry(dep.as_str()).or_default().push(t.uid.as_str());
                }
            }
        }

        let mut result = Vec::with_capacity(list.len());
        let mut remaining = list.clone();

        while !remaining.is_empty() {
            let mut progressed = false;
            for i in 0..remaining.len() {
                let uid = remaining[i].uid.as_str();
                if *in_degree.get(uid).unwrap_or(&0) == 0 {
                    let task = remaining.remove(i);
                    if let Some(dependents) = graph.get(task.uid.as_str()) {
                        for dep in dependents {
                            if let Some(deg) = in_degree.get_mut(*dep) {
                                *deg = deg.saturating_sub(1);
                            }
                        }
                    }
                    result.push(task);
                    progressed = true;
                    break;
                }
            }
            if !progressed {
                let task = remaining.remove(0);
                if let Some(dependents) = graph.get(task.uid.as_str()) {
                    for dep in dependents {
                        if let Some(deg) = in_degree.get_mut(*dep) {
                            *deg = deg.saturating_sub(1);
                        }
                    }
                }
                result.push(task);
            }
        }
        *list = result;
    }

    fn serialize_node(
        task: &crate::model::Task,
        children_map: &std::collections::HashMap<String, Vec<&crate::model::Task>>,
        depth: usize,
        out: &mut String,
        prefix: &str,
        root_href: &str,
        store: &crate::store::TaskStore,
    ) {
        let status_str = if task.is_note {
            String::new()
        } else {
            format!(
                "{} ",
                match task.status {
                    crate::model::TaskStatus::NeedsAction => {
                        if task.is_paused() { "[/]" } else { "[ ]" }
                    }
                    crate::model::TaskStatus::InProcess => "[>]",
                    crate::model::TaskStatus::Completed => "[x]",
                    crate::model::TaskStatus::Cancelled => "[-]",
                }
            )
        };
        let mut smart_string = task.to_smart_string();
        if task.is_note {
            if smart_string.starts_with("- ") || smart_string.starts_with("* ") {
                smart_string = smart_string[2..].trim_start().to_string();
            } else if smart_string == "-" || smart_string == "*" {
                smart_string = String::new();
            }
        }
        if task.calendar_href != root_href {
            smart_string.push_str(&format!(
                " col:{}",
                crate::model::parser::quote_value(&task.calendar_href)
            ));
        }

        let uid_tag = format!("<!-- uid:{} -->", task.uid);
        let indent = "    ".repeat(depth);

        // Output short UID dependencies and relations to guarantee they are never ambiguous upon re-parsing
        let mut dep_str = String::new();

        let process_relations = |uids: &[String], prefix: &str, out: &mut String| {
            for uid in uids {
                // Skip trashed/recovered/missing references so they self-heal (disappear) on save
                if let Some(target_task) = store.get_task_ref(uid) {
                    if target_task.calendar_href == crate::storage::LOCAL_TRASH_HREF
                        || target_task.calendar_href == "local://recovery"
                    {
                        continue;
                    }
                } else {
                    // Task is completely missing (hard-deleted). Skip to self-heal.
                    continue;
                }

                // Only truncate if it is actually a valid UUID. If another client injected a raw string,
                // quote it so it can be cleanly resolved upon re-parsing.
                let display_val = if uid.len() == 36 && uuid::Uuid::parse_str(uid).is_ok() {
                    &uid[..8]
                } else {
                    uid
                };
                out.push_str(&format!(
                    " {}:{}",
                    prefix,
                    crate::model::parser::quote_value(display_val)
                ));
            }
        };

        process_relations(&task.dependencies, "dep", &mut dep_str);
        process_relations(&task.related_to, "rel", &mut dep_str);

        out.push_str(&format!(
            "{}{}{}{}{}{} {}\n",
            indent,
            prefix,
            if prefix.ends_with(' ') { "" } else { " " },
            status_str,
            smart_string,
            dep_str,
            uid_tag
        ));

        if !task.description.is_empty() {
            for line in task.description.lines() {
                out.push_str(&format!("{}  {}\n", indent, line));
            }
        }

        if let Some(children) = children_map.get(&task.uid) {
            let mut prefixes = Vec::new();
            let mut current_number = 1;
            let mut uses_number_prev = false;

            for i in 0..children.len() {
                let child = children[i];
                let mut uses_number = false;
                if i > 0 {
                    let prev_child = children[i - 1];
                    if child.dependencies.contains(&prev_child.uid) {
                        current_number += 1;
                        uses_number = true;
                    } else if prev_child.dependencies == child.dependencies && uses_number_prev {
                        uses_number = true;
                    } else {
                        current_number = 1;
                        let has_successor = children
                            .iter()
                            .skip(i + 1)
                            .any(|c| c.dependencies.contains(&child.uid));
                        if has_successor {
                            uses_number = true;
                        }
                    }
                } else {
                    let has_successor = children
                        .iter()
                        .skip(1)
                        .any(|c| c.dependencies.contains(&child.uid));
                    if has_successor {
                        uses_number = true;
                    }
                }

                uses_number_prev = uses_number;
                if uses_number {
                    prefixes.push(format!("{}.", current_number));
                } else {
                    prefixes.push("-".to_string());
                }
            }

            for (child, prefix) in children.iter().zip(prefixes.iter()) {
                serialize_node(
                    child,
                    children_map,
                    depth + 1,
                    out,
                    prefix,
                    root_href,
                    store,
                );
            }
        }
    }

    serialize_node(
        root,
        &children_map,
        0,
        &mut out,
        "-",
        &root.calendar_href,
        store,
    );

    out.trim_end().to_string()
}
