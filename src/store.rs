// File: ./src/store.rs
use crate::model::Task;
use chrono::{DateTime, Utc};
use std::collections::{HashMap, HashSet};

// Special ID for the "Uncategorized" pseudo-tag
pub const UNCATEGORIZED_ID: &str = ":::uncategorized:::";

#[derive(Debug, Clone, Default)]
pub struct TaskStore {
    pub calendars: HashMap<String, Vec<Task>>,
}

pub struct FilterOptions<'a> {
    pub active_cal_href: Option<&'a str>,
    pub hidden_calendars: &'a std::collections::HashSet<String>,
    pub selected_categories: &'a HashSet<String>,
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
        self.calendars.insert(calendar_href, tasks);
    }

    pub fn clear(&mut self) {
        self.calendars.clear();
    }

    /// Returns a list of (Category Name, Active Task Count)
    pub fn get_all_categories(
        &self,
        _hide_completed: bool, // Param kept for API compat, but logic is improved below
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
                        present_tags.insert(cat.clone());
                        if is_active {
                            *active_counts.entry(cat.clone()).or_insert(0) += 1;
                        }
                    }
                }
            }
        }

        let mut result = Vec::new();

        // Process standard tags
        for tag in present_tags {
            let count = *active_counts.get(&tag).unwrap_or(&0);

            // Logic:
            // 1. If hide_fully_completed_tags is TRUE: Only show if count > 0 or it's selected.
            // 2. If hide_fully_completed_tags is FALSE: Show it because it exists (even if count is 0).
            let should_show = if hide_fully_completed_tags {
                count > 0 || forced_includes.contains(&tag)
            } else {
                true
            };

            if should_show {
                result.push((tag, count));
            }
        }

        // Process Uncategorized
        // Same logic: If hiding completed tags, only show Uncategorized if it has active tasks or is selected.
        let show_uncategorized = if hide_fully_completed_tags {
            has_uncategorized_active || forced_includes.contains(UNCATEGORIZED_ID)
        } else {
            has_uncategorized_any || forced_includes.contains(UNCATEGORIZED_ID)
        };

        if show_uncategorized {
            let count = if has_uncategorized_active {
                // We need to actually count them if we haven't tracked exact numbers above
                // To save a second loop, let's assume we want exact numbers:
                self.count_uncategorized_active(hidden_calendars)
            } else {
                0
            };
            result.push((UNCATEGORIZED_ID.to_string(), count));
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
            // If explicit calendar selected, ignore hidden list (unless it matches)
            if !options.hidden_calendars.contains(href)
                && let Some(tasks) = self.calendars.get(href)
            {
                raw_tasks.extend(tasks.clone());
            }
        } else {
            // "All Tasks" view: Skip hidden calendars
            for (href, tasks) in &self.calendars {
                if !options.hidden_calendars.contains(href) {
                    raw_tasks.extend(tasks.clone());
                }
            }
        }

        let filtered: Vec<Task> = raw_tasks
            .into_iter()
            .filter(|t| {
                // Pre-check for any status-related filter in the search term
                let search_lower = options.search_term.to_lowercase();
                let has_status_filter = search_lower.contains("is:done")
                    || search_lower.contains("is:active")
                    || search_lower.contains("is:ongoing");

                // Apply global hide setting ONLY if there's no overriding status filter in the search
                if !has_status_filter && t.status.is_done() && options.hide_completed_global {
                    return false;
                }

                // Duration Filter (UI Sliders)
                match t.estimated_duration {
                    Some(mins) => {
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
                    }
                    None => {
                        if !options.include_unset_duration {
                            return false;
                        }
                    }
                }

                // Category Filter
                if !options.selected_categories.is_empty() {
                    let filter_uncategorized =
                        options.selected_categories.contains(UNCATEGORIZED_ID);

                    if options.match_all_categories {
                        for sel in options.selected_categories {
                            if sel == UNCATEGORIZED_ID {
                                if !t.categories.is_empty() {
                                    return false;
                                }
                            } else if !t.categories.contains(sel) {
                                return false;
                            }
                        }
                    } else {
                        let mut hit = false;
                        if filter_uncategorized && t.categories.is_empty() {
                            hit = true;
                        } else {
                            for sel in options.selected_categories {
                                if sel != UNCATEGORIZED_ID && t.categories.contains(sel) {
                                    hit = true;
                                    break;
                                }
                            }
                        }
                        if !hit {
                            return false;
                        }
                    }
                }

                // Advanced Search Parsing (Delegated to Model)
                if !options.search_term.is_empty() {
                    return t.matches_search_term(options.search_term);
                }

                true
            })
            .collect();

        Task::organize_hierarchy(filtered, options.cutoff_date)
    }

    pub fn is_task_done(&self, uid: &str) -> Option<bool> {
        for tasks in self.calendars.values() {
            if let Some(t) = tasks.iter().find(|t| t.uid == uid) {
                return Some(t.status.is_done());
            }
        }
        None
    }
    // Backward compat helper
    pub fn get_task_status(&self, uid: &str) -> Option<bool> {
        self.is_task_done(uid)
    }

    pub fn is_blocked(&self, task: &Task) -> bool {
        if task.dependencies.is_empty() {
            return false;
        }
        for dep_uid in &task.dependencies {
            // Blocked if the dependency exists and is NOT done
            if let Some(is_done) = self.is_task_done(dep_uid)
                && !is_done
            {
                return true;
            }
        }
        false
    }

    pub fn get_summary(&self, uid: &str) -> Option<String> {
        for tasks in self.calendars.values() {
            if let Some(t) = tasks.iter().find(|t| t.uid == uid) {
                return Some(t.summary.clone());
            }
        }
        None
    }
}
