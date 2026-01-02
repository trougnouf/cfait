// File: ./src/store.rs
use crate::cache::Cache;
use crate::model::{Task, TaskStatus};
use crate::storage::LocalStorage;
use chrono::{DateTime, Utc};
use std::collections::{HashMap, HashSet};

pub const UNCATEGORIZED_ID: &str = ":::uncategorized:::";

#[derive(Debug, Clone, Default)]
pub struct TaskStore {
    pub calendars: HashMap<String, Vec<Task>>,
    pub index: HashMap<String, String>,
    /// Reverse index: TargetUID -> Vec<SourceUID> for related_to relationships
    /// Allows O(1) lookup of "which tasks link to this task"
    pub related_from_index: HashMap<String, Vec<String>>,
}

pub struct FilterOptions<'a> {
    pub active_cal_href: Option<&'a str>,
    pub hidden_calendars: &'a std::collections::HashSet<String>,
    pub selected_categories: &'a HashSet<String>,
    pub selected_locations: &'a HashSet<String>, // NEW
    pub match_all_categories: bool,
    pub search_term: &'a str,
    pub hide_completed_global: bool,
    pub cutoff_date: Option<DateTime<Utc>>,
    pub min_duration: Option<u32>,
    pub max_duration: Option<u32>,
    pub include_unset_duration: bool,
    pub urgent_days: u32,
    pub urgent_prio: u8,
}

impl TaskStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, calendar_href: String, tasks: Vec<Task>) {
        for task in &tasks {
            self.index.insert(task.uid.clone(), calendar_href.clone());
        }
        self.calendars.insert(calendar_href, tasks);
        self.rebuild_relation_index();
    }

    pub fn add_task(&mut self, task: Task) {
        let href = task.calendar_href.clone();
        self.index.insert(task.uid.clone(), href.clone());
        self.calendars.entry(href).or_default().push(task);
        self.rebuild_relation_index();
    }

    pub fn update_or_add_task(&mut self, task: Task) {
        let href = task.calendar_href.clone();
        self.index.insert(task.uid.clone(), href.clone());
        let list = self.calendars.entry(href.clone()).or_default();

        if let Some(idx) = list.iter().position(|t| t.uid == task.uid) {
            list[idx] = task;
        } else {
            list.push(task);
        }

        if href.starts_with("local://") {
            let _ = LocalStorage::save_for_href(&href, list);
        } else {
            let (_, token) = Cache::load(&href).unwrap_or((vec![], None));
            let _ = Cache::save(&href, list, token);
        }

        self.rebuild_relation_index();
    }

    pub fn clear(&mut self) {
        self.calendars.clear();
        self.index.clear();
        self.related_from_index.clear();
    }

    pub fn remove(&mut self, calendar_href: &str) {
        // Remove all tasks from this calendar from the index
        if let Some(tasks) = self.calendars.remove(calendar_href) {
            for task in tasks {
                self.index.remove(&task.uid);
            }
        }
        self.rebuild_relation_index();
    }

    pub fn get_task_mut(&mut self, uid: &str) -> Option<(&mut Task, String)> {
        let href = self.index.get(uid)?.clone();
        if let Some(tasks) = self.calendars.get_mut(&href)
            && let Some(task) = tasks.iter_mut().find(|t| t.uid == uid)
        {
            return Some((task, href));
        }
        self.index.remove(uid);
        None
    }

    pub fn toggle_task(&mut self, uid: &str) -> Option<Task> {
        if let Some((task, _)) = self.get_task_mut(uid) {
            if task.status == TaskStatus::Completed {
                task.status = TaskStatus::NeedsAction;
                task.percent_complete = None;
            } else {
                task.status = TaskStatus::Completed;
                task.percent_complete = Some(100);
            }
            return Some(task.clone());
        }
        None
    }

    pub fn set_status(&mut self, uid: &str, status: TaskStatus) -> Option<Task> {
        if let Some((task, _)) = self.get_task_mut(uid) {
            if task.status == status {
                task.status = TaskStatus::NeedsAction;
            } else {
                task.status = status;
            }
            return Some(task.clone());
        }
        None
    }

    pub fn set_status_in_process(&mut self, uid: &str) -> Option<Task> {
        if let Some((task, _)) = self.get_task_mut(uid) {
            task.status = TaskStatus::InProcess;
            return Some(task.clone());
        }
        None
    }

    pub fn pause_task(&mut self, uid: &str) -> Option<Task> {
        if let Some((task, _)) = self.get_task_mut(uid) {
            task.status = TaskStatus::NeedsAction;
            let current = task.percent_complete.unwrap_or(0);
            if current == 0 {
                task.percent_complete = Some(50);
            }
            return Some(task.clone());
        }
        None
    }

    pub fn stop_task(&mut self, uid: &str) -> Option<Task> {
        if let Some((task, _)) = self.get_task_mut(uid) {
            task.status = TaskStatus::NeedsAction;
            task.percent_complete = None;
            return Some(task.clone());
        }
        None
    }

    pub fn change_priority(&mut self, uid: &str, delta: i8) -> Option<Task> {
        if let Some((task, _)) = self.get_task_mut(uid) {
            task.priority = if delta > 0 {
                match task.priority {
                    0 => 9,
                    9 => 5,
                    5 => 1,
                    1 => 1,
                    _ => 5,
                }
            } else {
                match task.priority {
                    1 => 5,
                    5 => 9,
                    9 => 0,
                    0 => 0,
                    _ => 0,
                }
            };
            return Some(task.clone());
        }
        None
    }

    pub fn delete_task(&mut self, uid: &str) -> Option<(Task, String)> {
        let href = self.index.get(uid)?.clone();
        if let Some(tasks) = self.calendars.get_mut(&href)
            && let Some(idx) = tasks.iter().position(|t| t.uid == uid)
        {
            let task = tasks.remove(idx);
            self.index.remove(uid);
            if href.starts_with("local://") {
                let _ = LocalStorage::save_for_href(&href, tasks);
            } else {
                let (_, token) = Cache::load(&href).unwrap_or((vec![], None));
                let _ = Cache::save(&href, tasks, token);
            }
            self.rebuild_relation_index();
            return Some((task, href));
        }
        None
    }

    pub fn set_parent(&mut self, child_uid: &str, parent_uid: Option<String>) -> Option<Task> {
        if let Some((task, _)) = self.get_task_mut(child_uid) {
            task.parent_uid = parent_uid;
            return Some(task.clone());
        }
        None
    }

    pub fn add_dependency(&mut self, task_uid: &str, dep_uid: String) -> Option<Task> {
        if let Some((task, _)) = self.get_task_mut(task_uid)
            && !task.dependencies.contains(&dep_uid)
        {
            task.dependencies.push(dep_uid);
            return Some(task.clone());
        }
        None
    }

    pub fn remove_dependency(&mut self, task_uid: &str, dep_uid: &str) -> Option<Task> {
        if let Some((task, _)) = self.get_task_mut(task_uid)
            && let Some(pos) = task.dependencies.iter().position(|d| d == dep_uid)
        {
            task.dependencies.remove(pos);
            return Some(task.clone());
        }
        None
    }

    pub fn add_related_to(&mut self, task_uid: &str, related_uid: String) -> Option<Task> {
        let result = if let Some((task, _)) = self.get_task_mut(task_uid)
            && !task.related_to.contains(&related_uid)
        {
            task.related_to.push(related_uid.clone());
            Some(task.clone())
        } else {
            None
        };

        // Update reverse index after dropping the mutable borrow
        if result.is_some() {
            self.related_from_index
                .entry(related_uid)
                .or_default()
                .push(task_uid.to_string());
        }

        result
    }

    pub fn remove_related_to(&mut self, task_uid: &str, related_uid: &str) -> Option<Task> {
        let result = if let Some((task, _)) = self.get_task_mut(task_uid)
            && let Some(pos) = task.related_to.iter().position(|r| r == related_uid)
        {
            task.related_to.remove(pos);
            Some(task.clone())
        } else {
            None
        };

        // Update reverse index after dropping the mutable borrow
        if result.is_some()
            && let Some(sources) = self.related_from_index.get_mut(related_uid)
        {
            sources.retain(|uid| uid != task_uid);
            if sources.is_empty() {
                self.related_from_index.remove(related_uid);
            }
        }

        result
    }

    pub fn move_task(&mut self, uid: &str, target_href: String) -> Option<Task> {
        if let Some((mut task, old_href)) = self.delete_task(uid) {
            if old_href == target_href {
                self.add_task(task);
                return None;
            }
            task.calendar_href = target_href.clone();
            self.add_task(task.clone());
            if target_href.starts_with("local://") {
                if let Some(local_tasks) = self.calendars.get(&target_href) {
                    let _ = LocalStorage::save_for_href(&target_href, local_tasks);
                }
            } else if let Some(target_list) = self.calendars.get(&target_href) {
                let (_, token) = Cache::load(&target_href).unwrap_or((vec![], None));
                let _ = Cache::save(&target_href, target_list, token);
            }
            return Some(task);
        }
        None
    }

    pub fn apply_alias_retroactively(
        &mut self,
        alias_key: &str,
        raw_values: &[String],
    ) -> Vec<Task> {
        let mut uids_to_update: Vec<String> = Vec::new();

        let is_location_alias = alias_key.starts_with("@@");

        // Prepare prefix for hierarchy check
        // For tags: "tag" -> "tag:"
        // For locs: "@@loc" -> "loc:" (we check against clean value)
        let (clean_key, alias_prefix) = if is_location_alias {
            let clean = alias_key.trim_start_matches("@@");
            (clean, format!("{}:", clean))
        } else {
            (alias_key, format!("{}:", alias_key))
        };

        for tasks in self.calendars.values() {
            for task in tasks {
                let has_alias_or_child = if is_location_alias {
                    // Check Location field
                    if let Some(loc) = &task.location {
                        loc == clean_key || loc.starts_with(&alias_prefix)
                    } else {
                        false
                    }
                } else {
                    // Check Categories
                    task.categories
                        .iter()
                        .any(|cat| cat == clean_key || cat.starts_with(&alias_prefix))
                };

                if has_alias_or_child {
                    let mut needs_update = false;
                    for val in raw_values {
                        if let Some(tag) = val.strip_prefix('#') {
                            let clean = crate::model::parser::strip_quotes(tag);
                            if !task.categories.contains(&clean) {
                                needs_update = true;
                                break;
                            }
                        } else if let Some(loc) = val.strip_prefix("@@") {
                            let clean = crate::model::parser::strip_quotes(loc);
                            if task.location.as_ref() != Some(&clean) {
                                needs_update = true;
                                break;
                            }
                        } else if let Some(prio) = val.strip_prefix('!')
                            && let Ok(p) = prio.parse::<u8>()
                            && task.priority != p
                        {
                            needs_update = true;
                            break;
                        }
                    }
                    if needs_update {
                        uids_to_update.push(task.uid.clone());
                    }
                }
            }
        }

        if uids_to_update.is_empty() {
            return Vec::new();
        }

        let mut modified_tasks = Vec::new();
        for uid in uids_to_update {
            if let Some((task, _)) = self.get_task_mut(&uid) {
                for val in raw_values {
                    if let Some(tag) = val.strip_prefix('#') {
                        let clean = crate::model::parser::strip_quotes(tag);
                        if !task.categories.contains(&clean) {
                            task.categories.push(clean);
                        }
                    } else if let Some(loc) = val.strip_prefix("@@") {
                        task.location = Some(crate::model::parser::strip_quotes(loc));
                    } else if let Some(loc) = val.strip_prefix("loc:") {
                        task.location = Some(crate::model::parser::strip_quotes(loc));
                    } else if let Some(prio) = val.strip_prefix('!') {
                        if let Ok(p) = prio.parse::<u8>() {
                            task.priority = p;
                        }
                    } else if let Some(url) = val.strip_prefix("url:") {
                        task.url = Some(crate::model::parser::strip_quotes(url));
                    } else if let Some(geo) = val.strip_prefix("geo:") {
                        task.geo = Some(crate::model::parser::strip_quotes(geo));
                    } else if val.starts_with('~') || val.starts_with("est:") {
                        // Duration handling if needed in future
                    } else if !task.categories.contains(val) {
                        task.categories.push(val.clone());
                    }
                }
                task.categories.sort();
                task.categories.dedup();
                modified_tasks.push(task.clone());
            }
        }
        modified_tasks
    }

    pub fn get_all_categories(
        &self,
        _hide_completed: bool,
        hide_fully_completed_tags: bool,
        forced_includes: &HashSet<String>,
        hidden_calendars: &HashSet<String>,
    ) -> Vec<(String, usize)> {
        let mut active_counts: HashMap<String, usize> = HashMap::new();
        let mut present_tags: HashSet<String> = HashSet::new();
        let mut has_uncategorized_active = false;
        let mut has_uncategorized_any = false;

        for (href, tasks) in &self.calendars {
            if hidden_calendars.contains(href) {
                continue;
            }
            for task in tasks {
                let is_active = !task.status.is_done();

                if task.categories.is_empty() {
                    has_uncategorized_any = true;
                    if is_active {
                        has_uncategorized_active = true;
                    }
                } else {
                    for cat in &task.categories {
                        let parts: Vec<&str> = cat.split(':').collect();
                        let mut current_hierarchy = String::with_capacity(cat.len());

                        for (i, part) in parts.iter().enumerate() {
                            if i > 0 {
                                current_hierarchy.push(':');
                            }
                            current_hierarchy.push_str(part);
                            present_tags.insert(current_hierarchy.clone());
                            if is_active {
                                *active_counts.entry(current_hierarchy.clone()).or_insert(0) += 1;
                            }
                        }
                    }
                }
            }
        }

        let mut result = Vec::new();
        for tag in present_tags {
            let count = *active_counts.get(&tag).unwrap_or(&0);
            let should_show = if hide_fully_completed_tags {
                count > 0 || forced_includes.contains(&tag)
            } else {
                true
            };
            if should_show {
                result.push((tag, count));
            }
        }

        let show_uncategorized = if hide_fully_completed_tags {
            has_uncategorized_active || forced_includes.contains(UNCATEGORIZED_ID)
        } else {
            has_uncategorized_any || forced_includes.contains(UNCATEGORIZED_ID)
        };

        if show_uncategorized {
            let count = if has_uncategorized_active {
                self.count_uncategorized_active(hidden_calendars)
            } else {
                0
            };
            result.push((UNCATEGORIZED_ID.to_string(), count));
        }

        result.sort_by(|a, b| a.0.cmp(&b.0));
        result
    }

    // --- CHANGED: Location Aggregation (Hierarchy Support) ---
    pub fn get_all_locations(
        &self,
        _hide_completed: bool,
        hidden_calendars: &HashSet<String>,
    ) -> Vec<(String, usize)> {
        let mut active_counts: HashMap<String, usize> = HashMap::new();
        let mut present_locations: HashSet<String> = HashSet::new();

        for (href, tasks) in &self.calendars {
            if hidden_calendars.contains(href) {
                continue;
            }
            for task in tasks {
                let is_active = !task.status.is_done();

                if let Some(loc) = &task.location {
                    // Split the location string to handle hierarchy (e.g. "home:kitchen")
                    let parts: Vec<&str> = loc.split(':').collect();
                    let mut current_hierarchy = String::with_capacity(loc.len());

                    for (i, part) in parts.iter().enumerate() {
                        if i > 0 {
                            current_hierarchy.push(':');
                        }
                        current_hierarchy.push_str(part);
                        present_locations.insert(current_hierarchy.clone());
                        if is_active {
                            *active_counts.entry(current_hierarchy.clone()).or_insert(0) += 1;
                        }
                    }
                }
            }
        }

        let mut result = Vec::new();
        for loc in present_locations {
            let count = *active_counts.get(&loc).unwrap_or(&0);
            // Only show locations that have at least one active task
            if count > 0 {
                result.push((loc, count));
            }
        }

        result.sort_by(|a, b| a.0.cmp(&b.0));
        result
    }

    fn count_uncategorized_active(&self, hidden_calendars: &HashSet<String>) -> usize {
        let mut count = 0;
        for (href, tasks) in &self.calendars {
            if hidden_calendars.contains(href) {
                continue;
            }
            for task in tasks {
                if task.categories.is_empty() && !task.status.is_done() {
                    count += 1;
                }
            }
        }
        count
    }

    pub fn filter(&self, options: FilterOptions) -> Vec<Task> {
        let mut raw_tasks = Vec::new();

        if let Some(href) = options.active_cal_href {
            if !options.hidden_calendars.contains(href)
                && let Some(tasks) = self.calendars.get(href)
            {
                raw_tasks.extend(tasks.clone());
            }
        } else {
            for (href, tasks) in &self.calendars {
                if !options.hidden_calendars.contains(href) {
                    raw_tasks.extend(tasks.clone());
                }
            }
        }

        // Pre-calculate is:ready flag
        let search_lower = options.search_term.to_lowercase();
        let is_ready_mode = search_lower.contains("is:ready");
        let now = Utc::now();

        // Pass 1: Filter by everything EXCEPT the search text match
        // This creates a pool of "Candidate Tasks" that satisfy calendar, status, tag, and logic filters.
        let visible_candidates: Vec<Task> = raw_tasks
            .into_iter()
            .filter(|t| {
                let has_status_filter = search_lower.contains("is:done")
                    || search_lower.contains("is:active")
                    || search_lower.contains("is:started")
                    || search_lower.contains("is:ongoing");

                if !has_status_filter && t.status.is_done() && options.hide_completed_global {
                    return false;
                }

                // --- Work Mode (is:ready) Logic ---
                if is_ready_mode {
                    // 1. Must not be completed/cancelled
                    if t.status.is_done() {
                        return false;
                    }

                    // 2. Start Date must not be in the future
                    if let Some(start) = &t.dtstart {
                        // Use to_comparison_time to handle AllDay correctly vs Now
                        if start.to_comparison_time() > now {
                            return false;
                        }
                    }

                    // 3. Must not be blocked by incomplete dependencies
                    if self.is_blocked(t) {
                        return false;
                    }
                }
                // ----------------------------------------

                if let Some(mins) = t.estimated_duration {
                    if let Some(min) = options.min_duration
                        && mins < min
                    {
                        return false;
                    }
                    if let Some(max) = options.max_duration
                        && mins > max
                    {
                        return false;
                    }
                } else if !options.include_unset_duration {
                    return false;
                }

                if !options.selected_categories.is_empty() {
                    let filter_uncategorized =
                        options.selected_categories.contains(UNCATEGORIZED_ID);
                    let check_match = |task_cat: &str, selected: &str| -> bool {
                        if task_cat == selected {
                            return true;
                        }
                        if let Some(stripped) = task_cat.strip_prefix(selected) {
                            return stripped.starts_with(':');
                        }
                        false
                    };

                    if options.match_all_categories {
                        for sel in options.selected_categories {
                            if sel == UNCATEGORIZED_ID {
                                if !t.categories.is_empty() {
                                    return false;
                                }
                            } else {
                                let mut has_cat_or_child = false;
                                for task_cat in &t.categories {
                                    if check_match(task_cat, sel) {
                                        has_cat_or_child = true;
                                        break;
                                    }
                                }
                                if !has_cat_or_child {
                                    return false;
                                }
                            }
                        }
                    } else {
                        let mut hit = false;
                        if filter_uncategorized && t.categories.is_empty() {
                            hit = true;
                        } else {
                            for sel in options.selected_categories {
                                if sel != UNCATEGORIZED_ID {
                                    for task_cat in &t.categories {
                                        if check_match(task_cat, sel) {
                                            hit = true;
                                            break;
                                        }
                                    }
                                }
                                if hit {
                                    break;
                                }
                            }
                        }
                        if !hit {
                            return false;
                        }
                    }
                }

                // --- CHANGED: Location Filtering (Hierarchy Support) ---
                if !options.selected_locations.is_empty() {
                    if let Some(loc) = &t.location {
                        let mut hit = false;
                        for sel in options.selected_locations {
                            // Match exact string OR hierarchical child (starts with "sel:")
                            if loc == sel
                                || (loc.starts_with(sel) && loc.chars().nth(sel.len()) == Some(':'))
                            {
                                hit = true;
                                break;
                            }
                        }
                        if !hit {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }

                // NOTE: We do NOT filter by text search here yet.
                // We keep all tasks that satisfy the "properties" filters.
                true
            })
            .collect();

        // Pass 2: Apply Search with Hierarchy Expansion
        let final_tasks = if options.search_term.is_empty() {
            visible_candidates
        } else {
            // Build parent->children index for the visible set to allow efficient traversal
            let mut children_map: HashMap<String, Vec<String>> = HashMap::new();
            for t in &visible_candidates {
                if let Some(p) = &t.parent_uid {
                    children_map
                        .entry(p.clone())
                        .or_default()
                        .push(t.uid.clone());
                }
            }

            let mut matched_uids = HashSet::new();
            let mut queue = Vec::new();

            // Find Explicit Matches
            for t in &visible_candidates {
                if t.matches_search_term(options.search_term) && matched_uids.insert(t.uid.clone())
                {
                    queue.push(t.uid.clone());
                }
            }

            // Expand to Descendants (Implicit Matches)
            // If parent matched, include all children recursively
            let mut idx = 0;
            while idx < queue.len() {
                let current_uid = queue[idx].clone();
                idx += 1;

                if let Some(children) = children_map.get(&current_uid) {
                    for child_uid in children {
                        // Add child if not already added
                        if matched_uids.insert(child_uid.clone()) {
                            queue.push(child_uid.clone());
                        }
                    }
                }
            }

            // Filter the candidate list to only include matched UIDs
            visible_candidates
                .into_iter()
                .filter(|t| matched_uids.contains(&t.uid))
                .collect()
        };

        Task::organize_hierarchy(
            final_tasks,
            options.cutoff_date,
            options.urgent_days,
            options.urgent_prio,
        )
    }

    pub fn is_task_done(&self, uid: &str) -> Option<bool> {
        if let Some(href) = self.index.get(uid)
            && let Some(tasks) = self.calendars.get(href)
            && let Some(t) = tasks.iter().find(|t| t.uid == uid)
        {
            return Some(t.status.is_done());
        }
        None
    }

    pub fn get_task_status(&self, uid: &str) -> Option<bool> {
        self.is_task_done(uid)
    }

    pub fn is_blocked(&self, task: &Task) -> bool {
        if task.dependencies.is_empty() {
            return false;
        }
        for dep_uid in &task.dependencies {
            if let Some(is_done) = self.is_task_done(dep_uid)
                && !is_done
            {
                return true;
            }
        }
        false
    }

    pub fn get_summary(&self, uid: &str) -> Option<String> {
        if let Some(href) = self.index.get(uid)
            && let Some(tasks) = self.calendars.get(href)
            && let Some(t) = tasks.iter().find(|t| t.uid == uid)
        {
            return Some(t.summary.clone());
        }
        None
    }

    /// Get all tasks that have a related_to link to the given task
    /// Get all tasks that have a related_to link to the given task
    /// Uses the reverse index for O(1) lookup instead of O(N) scan
    pub fn get_tasks_related_to(&self, uid: &str) -> Vec<(String, String)> {
        if let Some(source_uids) = self.related_from_index.get(uid) {
            source_uids
                .iter()
                .filter_map(|source_uid| {
                    self.get_summary(source_uid)
                        .map(|summary| (source_uid.clone(), summary))
                })
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Rebuild the reverse relation index from scratch
    /// This is O(N) but only runs on data load/sync, not on every render
    pub fn rebuild_relation_index(&mut self) {
        self.related_from_index.clear();

        // Collect all relationships first to avoid borrow checker issues
        let mut relationships = Vec::new();
        for tasks in self.calendars.values() {
            for task in tasks {
                for target in &task.related_to {
                    relationships.push((target.clone(), task.uid.clone()));
                }
            }
        }

        // Build the reverse index
        for (target, source) in relationships {
            self.related_from_index
                .entry(target)
                .or_default()
                .push(source);
        }
    }
}
