use crate::model::Task;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Default)]
pub struct TaskStore {
    pub calendars: HashMap<String, Vec<Task>>,
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

    pub fn get_all_categories(&self) -> Vec<String> {
        let mut set = HashSet::new();
        for tasks in self.calendars.values() {
            for task in tasks {
                for cat in &task.categories {
                    set.insert(cat.clone());
                }
            }
        }
        let mut list: Vec<String> = set.into_iter().collect();
        list.sort();
        list
    }

    pub fn filter(
        &self,
        active_cal_href: Option<&str>,
        selected_categories: &HashSet<String>,
        match_all_categories: bool,
        search_term: &str,
        // NEW ARGS
        hide_completed_global: bool,
        hide_completed_in_tags: bool,
    ) -> Vec<Task> {
        let mut raw_tasks = Vec::new();
        let is_category_mode = active_cal_href.is_none();

        if let Some(href) = active_cal_href {
            if let Some(tasks) = self.calendars.get(href) {
                raw_tasks.extend(tasks.clone());
            }
        } else {
            for tasks in self.calendars.values() {
                raw_tasks.extend(tasks.clone());
            }
        }

        let filtered: Vec<Task> = raw_tasks
            .into_iter()
            .filter(|t| {
                // VISIBILITY FILTER
                if t.completed {
                    if hide_completed_global {
                        return false;
                    }
                    if is_category_mode && hide_completed_in_tags {
                        return false;
                    }
                }

                if !selected_categories.is_empty() {
                    if match_all_categories {
                        for sel in selected_categories {
                            if !t.categories.contains(sel) {
                                return false;
                            }
                        }
                    } else {
                        let mut hit = false;
                        for sel in selected_categories {
                            if t.categories.contains(sel) {
                                hit = true;
                                break;
                            }
                        }
                        if !hit {
                            return false;
                        }
                    }
                }

                if !search_term.is_empty() {
                    if !t
                        .summary
                        .to_lowercase()
                        .contains(&search_term.to_lowercase())
                    {
                        return false;
                    }
                }

                true
            })
            .collect();

        Task::organize_hierarchy(filtered)
    }
}
