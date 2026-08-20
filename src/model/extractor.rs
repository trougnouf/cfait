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

fn compute_task_lines(input: &str, is_journal: bool) -> Vec<bool> {
    let lines_vec: Vec<&str> = input.lines().collect();
    let mut is_task_line = vec![false; lines_vec.len()];
    let mut indents = vec![0; lines_vec.len()];
    let mut is_list = vec![false; lines_vec.len()];

    for (i, line) in lines_vec.iter().enumerate() {
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
        indents[i] = indent;
        let rest = &line[byte_offset..];

        let mut list_marker = false;
        let mut after_marker = rest;

        if rest.starts_with("- ") || rest.starts_with("* ") || rest.starts_with("+ ") {
            list_marker = true;
            after_marker = &rest[2..];
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
                list_marker = true;
                after_marker = &rest[digit_bytes + 2..];
            }
        }

        is_list[i] = list_marker;

        if list_marker {
            let has_checkbox = parse_checkbox(after_marker).is_some();
            let has_uid = after_marker.contains("<!-- uid:");
            let has_is_note = after_marker.contains("is:note")
                || after_marker.contains("is:page")
                || after_marker.contains("is:journal");
            if has_checkbox || has_uid || has_is_note || (!is_journal && !has_checkbox) {
                is_task_line[i] = true;
            }
        }
    }

    // Phase 2: Backwards propagate `is_task` to implicit structural parents
    for i in (0..lines_vec.len()).rev() {
        if is_list[i] && !is_task_line[i] {
            let curr_indent = indents[i];
            for j in (i + 1)..lines_vec.len() {
                if !lines_vec[j].trim().is_empty() {
                    // ignore empty lines for indent checks
                    if indents[j] <= curr_indent {
                        break; // Hit a sibling or outdent, so no children
                    }
                    if is_task_line[j] {
                        is_task_line[i] = true;
                        break;
                    }
                }
            }
        }
    }

    is_task_line
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

pub fn has_extractable_subtasks(input: &str, is_journal: bool) -> bool {
    let is_task = compute_task_lines(input, is_journal);
    is_task.into_iter().any(|b| b)
}

/// Takes a raw markdown string.
/// Returns (Cleaned Root Description, List of Extracted Subtasks).
pub fn extract_markdown_tasks(input: &str, is_journal: bool) -> (String, Vec<ExtractedTask>) {
    let mut cleaned_root_desc = String::new();
    let mut extracted: Vec<ExtractedTask> = Vec::new();

    let lines_vec: Vec<&str> = input.lines().collect();
    let is_task_line = compute_task_lines(input, is_journal);

    let mut indent_stack: Vec<(usize, String, usize)> = Vec::new(); // (indent, uid, extracted_idx)
    let mut item_kind_at_indent: HashMap<usize, usize> = HashMap::new(); // indent -> block_id
    let mut next_block_id = 0;
    let mut numbered_tasks: Vec<(usize, usize, usize)> = Vec::new(); // (block_id, parsed_num, extracted_idx)

    let mut active_task_idx: Option<usize> = None;
    let mut active_task_indent = 0;

    for (line_idx, line) in lines_vec.into_iter().enumerate() {
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
            if let Some(idx) = active_task_idx {
                extracted[idx].description.push('\n');
            } else {
                cleaned_root_desc.push('\n');
            }
            continue;
        }

        item_kind_at_indent.retain(|&k, _| k <= indent);

        if is_task_line[line_idx] {
            let mut is_numbered = false;
            let mut parsed_num = 0;
            let mut parsed_status = crate::model::TaskStatus::NeedsAction;
            let mut parsed_pc = None;
            let mut is_note = true;
            let mut raw_text = rest;

            if rest.starts_with("- ") || rest.starts_with("* ") || rest.starts_with("+ ") {
                let after_marker = &rest[2..];
                if let Some((status, pc, r)) = parse_checkbox(after_marker) {
                    is_note = false;
                    parsed_status = status;
                    parsed_pc = pc;
                    raw_text = r;
                } else {
                    raw_text = after_marker;
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
                    if let Some((status, pc, r)) = parse_checkbox(after_marker) {
                        is_numbered = true;
                        is_note = false;
                        parsed_num = rest[..digit_bytes].parse::<usize>().unwrap_or(1);
                        parsed_status = status;
                        parsed_pc = pc;
                        raw_text = r;
                    } else {
                        is_numbered = true;
                        parsed_num = rest[..digit_bytes].parse::<usize>().unwrap_or(1);
                        raw_text = after_marker;
                    }
                }
            }

            let (clean_text, parsed_uid) = extract_uid_tag(raw_text);
            let uid = parsed_uid
                .clone()
                .unwrap_or_else(|| Uuid::new_v4().to_string());

            while let Some(&(stack_indent, _, _)) = indent_stack.last() {
                if stack_indent >= indent {
                    indent_stack.pop();
                } else {
                    break;
                }
            }

            let parent_uid = indent_stack.last().map(|(_, id, _)| id.clone());
            let new_idx = extracted.len();

            if is_numbered {
                let block_id = match item_kind_at_indent.get(&indent) {
                    Some(&b) => b,
                    _ => {
                        let b = next_block_id;
                        next_block_id += 1;
                        b
                    }
                };
                item_kind_at_indent.insert(indent, block_id);
                numbered_tasks.push((block_id, parsed_num, new_idx));
            } else {
                // Remove entry to break numbering blocks
                item_kind_at_indent.remove(&indent);
            }

            indent_stack.push((indent, uid.clone(), new_idx));

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
            active_task_indent = indent;
        } else {
            // Not a task line -> treat as plain text.
            item_kind_at_indent.remove(&indent);

            while let Some(&(stack_indent, _, _)) = indent_stack.last() {
                if stack_indent >= indent {
                    indent_stack.pop();
                } else {
                    break;
                }
            }

            let target_idx = indent_stack.last().map(|&(_, _, idx)| idx);

            let strip_amount = if target_idx.is_some() {
                active_task_indent + 2
            } else {
                0
            };

            let mut bytes_to_strip = 0;
            let mut spaces_seen = 0;
            for c in line.chars() {
                if spaces_seen >= strip_amount {
                    break;
                }
                if c == ' ' {
                    spaces_seen += 1;
                    bytes_to_strip += c.len_utf8();
                } else if c == '\t' {
                    spaces_seen += 4;
                    bytes_to_strip += c.len_utf8();
                } else {
                    break;
                }
            }
            let line_content = &line[bytes_to_strip..];

            if let Some(idx) = target_idx {
                if !extracted[idx].description.is_empty()
                    && !extracted[idx].description.ends_with('\n')
                {
                    extracted[idx].description.push('\n');
                }
                extracted[idx].description.push_str(line_content);
                extracted[idx].description.push('\n');
                active_task_idx = Some(idx);
            } else {
                if !cleaned_root_desc.is_empty() && !cleaned_root_desc.ends_with('\n') {
                    cleaned_root_desc.push('\n');
                }
                cleaned_root_desc.push_str(line_content);
                cleaned_root_desc.push('\n');
                active_task_idx = None;
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

pub fn serialize_task_tree(
    store: &crate::store::TaskStore,
    root_uid: &str,
    calendars: &[crate::model::CalendarListEntry],
    is_journal: bool,
) -> String {
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
    // We pre-sort deterministically (by created date, then summary) to ensure stable
    // git diffs and consistent publication output, rather than volatile priority/status sorting.
    for list in children_map.values_mut() {
        list.sort_by_cached_key(|t| (t.created_date(), t.summary.clone(), t.uid.clone()));

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

    struct SerializeContext<'a> {
        children_map: &'a std::collections::HashMap<String, Vec<&'a crate::model::Task>>,
        store: &'a crate::store::TaskStore,
        calendars: &'a [crate::model::CalendarListEntry],
    }

    fn serialize_node(
        ctx: &SerializeContext,
        task: &crate::model::Task,
        depth: usize,
        out: &mut String,
        prefix: &str,
        parent_href: &str,
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
        if task.calendar_href != parent_href {
            let cal_name = ctx
                .calendars
                .iter()
                .find(|c| c.href == task.calendar_href)
                .map(|c| c.name.as_str())
                .unwrap_or(task.calendar_href.as_str());

            smart_string.push_str(&format!(
                " col:{}",
                crate::model::parser::quote_value(cal_name)
            ));
        }

        let uid_tag = format!("<!-- uid:{} -->", task.uid);
        let indent = "    ".repeat(depth);

        // Output short UID dependencies and relations to guarantee they are never ambiguous upon re-parsing
        let mut dep_str = String::new();

        let process_relations = |uids: &[String], prefix: &str, out: &mut String| {
            for uid in uids {
                // Skip trashed/recovered/missing references so they self-heal (disappear) on save
                if let Some(target_task) = ctx.store.get_task_ref(uid) {
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

        if let Some(children) = ctx.children_map.get(&task.uid) {
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
                serialize_node(ctx, child, depth + 1, out, prefix, &task.calendar_href);
            }
        }
    }

    let ctx = SerializeContext {
        children_map: &children_map,
        store,
        calendars,
    };

    if is_journal {
        if !root.description.is_empty() {
            out.push_str(&root.description);
            out.push('\n');
        }
        if let Some(children) = children_map.get(&root.uid) {
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
                serialize_node(&ctx, child, 0, &mut out, prefix, &root.calendar_href);
            }
        }
    } else {
        serialize_node(&ctx, root, 0, &mut out, "-", &root.calendar_href);
    }

    out.trim_end().to_string()
}
