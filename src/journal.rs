// Implements the offline action journal for syncing changes.
use crate::model::Task;
use crate::paths::AppPaths;
use crate::storage::LocalStorage;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Action {
    Create(Task),
    Update(Task),
    Delete(Task),
    Move(Task, String),
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Journal {
    pub queue: Vec<Action>,
}

impl Journal {
    pub fn get_path() -> Option<PathBuf> {
        AppPaths::get_journal_path()
    }

    fn load_internal(path: &PathBuf) -> Self {
        if path.exists()
            && let Ok(content) = fs::read_to_string(path)
            && let Ok(journal) = serde_json::from_str(&content)
        {
            return journal;
        }
        Self::default()
    }

    pub fn load() -> Self {
        if let Some(path) = Self::get_path() {
            if !path.exists() {
                return Self::default();
            }
            return LocalStorage::with_lock(&path, || Ok(Self::load_internal(&path)))
                .unwrap_or_default();
        }
        Self::default()
    }

    pub fn modify<F>(f: F) -> Result<()>
    where
        F: FnOnce(&mut Vec<Action>),
    {
        if let Some(path) = Self::get_path() {
            LocalStorage::with_lock(&path, || {
                let mut journal = Self::load_internal(&path);
                f(&mut journal.queue);
                let json = serde_json::to_string_pretty(&journal)?;
                LocalStorage::atomic_write(&path, json)?;
                Ok(())
            })?;
        }
        Ok(())
    }

    pub fn push(action: Action) -> Result<()> {
        Self::modify(|queue| queue.push(action))
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Squashes redundant actions for the same task.
    /// E.g. [Create(A), Update(A'), Update(A'')] -> [Create(A'')]
    /// This fixes the "Stuck Loading" issue caused by alias update explosions.
    pub fn compact(&mut self) {
        let mut uid_map: HashMap<String, usize> = HashMap::new();
        let mut compacted: Vec<Option<Action>> = Vec::new();

        for action in self.queue.drain(..) {
            let uid = match &action {
                Action::Create(t) => t.uid.clone(),
                Action::Update(t) => t.uid.clone(),
                Action::Delete(t) => t.uid.clone(),
                Action::Move(t, _) => t.uid.clone(),
            };

            let mut merged = false;
            if let Some(&idx) = uid_map.get(&uid)
                && let Some(prev) = &compacted[idx]
            {
                match (prev, &action) {
                    (Action::Create(_), Action::Update(t)) => {
                        // Upgrade the Create to include the updates
                        compacted[idx] = Some(Action::Create(t.clone()));
                        merged = true;
                    }
                    (Action::Update(_), Action::Update(t)) => {
                        // Replace old update with new update (Last write wins)
                        compacted[idx] = Some(Action::Update(t.clone()));
                        merged = true;
                    }
                    (Action::Create(_), Action::Delete(_)) => {
                        // Created then Deleted -> Cancel out entirely
                        compacted[idx] = None;
                        uid_map.remove(&uid);
                        merged = true;
                    }
                    (Action::Update(_), Action::Delete(t)) => {
                        // Updated then Deleted -> Just Delete
                        compacted[idx] = Some(Action::Delete(t.clone()));
                        merged = true;
                    }
                    _ => {}
                }
            }

            if !merged {
                compacted.push(Some(action));
                uid_map.insert(uid, compacted.len() - 1);
            }
        }

        self.queue = compacted.into_iter().flatten().collect();
    }

    /// Applies pending journal actions to a list of tasks.
    /// Also performs garbage collection on invalid "Ghost" tasks.
    pub fn apply_to_tasks(tasks: &mut Vec<Task>, calendar_href: &str) {
        let journal = Self::load();

        // 1. Identify valid pending creations to protect them from pruning
        let mut pending_uids = HashSet::new();
        for action in &journal.queue {
            match action {
                Action::Create(t) | Action::Update(t) => {
                    if t.calendar_href == calendar_href {
                        pending_uids.insert(t.uid.clone());
                    }
                }
                Action::Move(t, target) => {
                    if target == calendar_href {
                        pending_uids.insert(t.uid.clone());
                    }
                }
                _ => {}
            }
        }

        // 2. Ghost Pruning: Remove tasks with no ETag that are NOT in the journal.
        // FIX: Explicitly skip pruning for ALL Local Calendars, as local tasks never have ETags.
        if !calendar_href.starts_with("local://") {
            tasks.retain(|t| !t.etag.is_empty() || pending_uids.contains(&t.uid));
        }

        if journal.is_empty() {
            return;
        }

        // 3. Apply Actions (Last-Write-Wins via HashMap)
        let mut task_map: HashMap<String, Task> =
            tasks.drain(..).map(|t| (t.uid.clone(), t)).collect();

        for action in journal.queue {
            match action {
                Action::Create(t) => {
                    if t.calendar_href == calendar_href {
                        task_map.insert(t.uid.clone(), t);
                    }
                }
                Action::Update(t) => {
                    if t.calendar_href == calendar_href {
                        task_map.insert(t.uid.clone(), t);
                    }
                }
                Action::Delete(t) => {
                    if t.calendar_href == calendar_href {
                        task_map.remove(&t.uid);
                    }
                }
                Action::Move(t, new_href) => {
                    if t.calendar_href == calendar_href {
                        task_map.remove(&t.uid);
                    } else if new_href == calendar_href {
                        let mut moved_task = t;
                        moved_task.calendar_href = new_href;
                        task_map.insert(moved_task.uid.clone(), moved_task);
                    }
                }
            }
        }
        *tasks = task_map.into_values().collect();
    }
}
