// File: src/store.rs
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
    }

    pub fn add_task(&mut self, task: Task) {
        let href = task.calendar_href.clone();
        self.index.insert(task.uid.clone(), href.clone());
        self.calendars.entry(href).or_default().push(task);
    }

    /// Updates an existing task or adds it if missing.
    /// Maintains index and persists to cache.
    pub fn update_or_add_task(&mut self, task: Task) {
        let href = task.calendar_href.clone();

        // Ensure index is up to date
        self.index.insert(task.uid.clone(), href.clone());

        let list = self.calendars.entry(href.clone()).or_default();

        if let Some(idx) = list.iter().position(|t| t.uid == task.uid) {
            list[idx] = task;
        } else {
            list.push(task);
        }

        // Persist logic
        if href == crate::storage::LOCAL_CALENDAR_HREF {
            let _ = LocalStorage::save(list);
        } else {
            let (_, token) = Cache::load(&href).unwrap_or((vec![], None));
            let _ = Cache::save(&href, list, token);
        }
    }

    pub fn clear(&mut self) {
        self.calendars.clear();
        self.index.clear();
    }

    // --- Core Logic Helpers ---

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
                // Unchecking: Back to NeedsAction, Clear Progress
                task.status = TaskStatus::NeedsAction;
                task.percent_complete = None;
            } else {
                // Checking: Completed, 100% Progress
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

    // --- PAUSE / STOP / START HELPERS ---

    pub fn set_status_in_process(&mut self, uid: &str) -> Option<Task> {
        if let Some((task, _)) = self.get_task_mut(uid) {
            task.status = TaskStatus::InProcess;
            // Optionally ensure percent_complete is > 0 to imply started, but not strictly required
            return Some(task.clone());
        }
        None
    }

    pub fn pause_task(&mut self, uid: &str) -> Option<Task> {
        if let Some((task, _)) = self.get_task_mut(uid) {
            task.status = TaskStatus::NeedsAction;
            // To be "Paused", percent must be > 0.
            // If it's currently 0 or missing, set it to 50% as a default "in progress" marker.
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
            task.percent_complete = None; // Explicitly clear progress to un-pause
            return Some(task.clone());
        }
        None
    }

    // ------------------------------------

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

            // Persist the change to the correct storage (cache or local)
            if href == crate::storage::LOCAL_CALENDAR_HREF {
                let _ = LocalStorage::save(tasks);
            } else {
                let (_, token) = Cache::load(&href).unwrap_or((vec![], None));
                let _ = Cache::save(&href, tasks, token);
            }
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

    pub fn move_task(&mut self, uid: &str, target_href: String) -> Option<Task> {
        if let Some((mut task, old_href)) = self.delete_task(uid) {
            if old_href == target_href {
                self.add_task(task); // Put it back
                return None;
            }

            task.calendar_href = target_href.clone();
            self.add_task(task.clone());

            // Persist the change to the NEW calendar's storage
            if target_href == crate::storage::LOCAL_CALENDAR_HREF {
                if let Some(local_tasks) = self.calendars.get(&target_href) {
                    let _ = LocalStorage::save(local_tasks);
                }
            } else if let Some(target_list) = self.calendars.get(&target_href) {
                let (_, token) = Cache::load(&target_href).unwrap_or((vec![], None));
                let _ = Cache::save(&target_href, target_list, token);
            }

            return Some(task);
        }
        None
    }

    // --- FIX: Intelligent Alias Application ---
    pub fn apply_alias_retroactively(
        &mut self,
        alias_key: &str,
        raw_values: &[String],
    ) -> Vec<Task> {
        let mut uids_to_update = Vec::new();
        let alias_prefix = format!("{}:", alias_key);

        // 1. Identify tasks that match the alias (or are sub-tags of it)
        for tasks in self.calendars.values() {
            for task in tasks {
                let has_alias_or_child = task
                    .categories
                    .iter()
                    .any(|cat| cat == alias_key || cat.starts_with(&alias_prefix));

                if has_alias_or_child {
                    // Check if we actually need to change anything to avoid spurious updates
                    let mut needs_update = false;
                    for val in raw_values {
                        // Check logic mirrors the update logic below
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

        // 2. Modify
        let mut modified_tasks = Vec::new();
        for uid in uids_to_update {
            if let Some((task, _)) = self.get_task_mut(&uid) {
                for val in raw_values {
                    if let Some(tag) = val.strip_prefix('#') {
                        // FIX: Strip quotes and hash before storing
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
                    } else if let Some(_d) = val.strip_prefix('~') {
                        // Basic duration handling for aliases if needed
                    } else {
                        // Fallback: If no sigil, treat as tag to be safe (legacy support)
                        if !task.categories.contains(val) {
                            task.categories.push(val.clone());
                        }
                    }
                }

                task.categories.sort();
                task.categories.dedup();
                modified_tasks.push(task.clone());
            }
        }

        modified_tasks
    }

    // --- Read/Filter Logic ---

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
                        // Handle hierarchy: gaming:coop -> gaming, gaming:coop
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

    // --- NEW: Location Aggregation ---
    pub fn get_all_locations(
        &self,
        hide_completed: bool,
        hidden_calendars: &HashSet<String>,
    ) -> Vec<(String, usize)> {
        let mut counts = HashMap::new();

        for (href, tasks) in &self.calendars {
            if hidden_calendars.contains(href) {
                continue;
            }
            for task in tasks {
                if hide_completed && task.status.is_done() {
                    continue;
                }
                if let Some(loc) = &task.location {
                    *counts.entry(loc.clone()).or_insert(0) += 1;
                }
            }
        }

        let mut result: Vec<_> = counts.into_iter().collect();
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

        let filtered: Vec<Task> = raw_tasks
            .into_iter()
            .filter(|t| {
                let search_lower = options.search_term.to_lowercase();
                let has_status_filter = search_lower.contains("is:done")
                    || search_lower.contains("is:active")
                    || search_lower.contains("is:ongoing");

                if !has_status_filter && t.status.is_done() && options.hide_completed_global {
                    return false;
                }

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

                // --- NEW: Location Filtering ---
                if !options.selected_locations.is_empty() {
                    if let Some(loc) = &t.location {
                        if !options.selected_locations.contains(loc) {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }

                if !options.search_term.is_empty() {
                    return t.matches_search_term(options.search_term);
                }
                true
            })
            .collect();

        Task::organize_hierarchy(filtered, options.cutoff_date)
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
}
