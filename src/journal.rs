// File: src/journal.rs
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
        // These are failed creations that shouldn't exist anymore.
        tasks.retain(|t| !t.etag.is_empty() || pending_uids.contains(&t.uid));

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
                        // Source: Remove
                        task_map.remove(&t.uid);
                    } else if new_href == calendar_href {
                        // Dest: Insert (Update href)
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
