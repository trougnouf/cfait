// File: ./src/storage.rs
use crate::model::Task;
use crate::paths::AppPaths;
use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};

// --- Android Specific Imports ---
#[cfg(target_os = "android")]
use std::collections::HashMap;
#[cfg(target_os = "android")]
use std::sync::{Arc, Mutex, OnceLock};

// --- Desktop Specific Imports ---
#[cfg(not(target_os = "android"))]
use fs2::FileExt;

// Constants for identification
pub const LOCAL_CALENDAR_HREF: &str = "local://default";
pub const LOCAL_CALENDAR_NAME: &str = "Local";

// --- Android Global Lock Map ---
#[cfg(target_os = "android")]
static ANDROID_FILE_LOCKS: OnceLock<Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>> = OnceLock::new();

pub struct LocalStorage;

impl LocalStorage {
    pub fn get_path() -> Option<PathBuf> {
        AppPaths::get_local_task_path()
    }

    /// Helper to get a sidecar lock file path (Desktop only)
    #[cfg(not(target_os = "android"))]
    fn get_lock_path(file_path: &Path) -> PathBuf {
        let mut lock_path = file_path.to_path_buf();
        if let Some(ext) = lock_path.extension() {
            let mut new_ext = ext.to_os_string();
            new_ext.push(".lock");
            lock_path.set_extension(new_ext);
        } else {
            lock_path.set_extension("lock");
        }
        lock_path
    }

    // --- DESKTOP IMPLEMENTATION (fs2) ---
    #[cfg(not(target_os = "android"))]
    pub fn with_lock<F, T>(file_path: &Path, f: F) -> Result<T>
    where
        F: FnOnce() -> Result<T>,
    {
        let lock_path = Self::get_lock_path(file_path);
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)?;

        file.lock_exclusive()?;
        let result = f();
        file.unlock()?;
        result
    }

    // --- ANDROID IMPLEMENTATION (In-Memory Mutex) ---
    #[cfg(target_os = "android")]
    pub fn with_lock<F, T>(file_path: &Path, f: F) -> Result<T>
    where
        F: FnOnce() -> Result<T>,
    {
        // Get the global map
        let map_mutex = ANDROID_FILE_LOCKS.get_or_init(|| Mutex::new(HashMap::new()));

        // Canonicalize to avoid race conditions via symlinks or relative paths
        let key = file_path.canonicalize().unwrap_or(file_path.to_path_buf());

        // Get or create the mutex specifically for this file path
        let file_mutex = {
            let mut map = map_mutex.lock().unwrap();
            map.entry(key)
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .clone()
        };

        // Lock this specific file
        let _guard = file_mutex.lock().unwrap();

        // Perform operation
        f()
    }

    /// Atomic write: Write to .tmp file then rename
    pub fn atomic_write<P: AsRef<Path>, C: AsRef<[u8]>>(path: P, contents: C) -> Result<()> {
        let path = path.as_ref();
        let tmp_path = path.with_extension("tmp");
        fs::write(&tmp_path, contents)?;
        fs::rename(tmp_path, path)?;
        Ok(())
    }

    pub fn save(tasks: &[Task]) -> Result<()> {
        if let Some(path) = Self::get_path() {
            Self::with_lock(&path, || {
                let json = serde_json::to_string_pretty(tasks)?;
                Self::atomic_write(&path, json)?;
                Ok(())
            })?;
        }
        Ok(())
    }

    pub fn load() -> Result<Vec<Task>> {
        if let Some(path) = Self::get_path() {
            if !path.exists() {
                return Ok(vec![]);
            }
            return Self::with_lock(&path, || {
                let json = fs::read_to_string(&path)?;
                let tasks = serde_json::from_str::<Vec<Task>>(&json)?;
                Ok(tasks)
            });
        }
        Ok(vec![])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_atomic_write_and_load() {
        let temp_dir = std::env::temp_dir().join("cfait_test_storage");
        let _ = fs::create_dir_all(&temp_dir);
        let file_path = temp_dir.join("test.json");

        // Fix: Explicit type annotation
        let tasks: Vec<Task> = vec![];

        LocalStorage::atomic_write(&file_path, serde_json::to_string(&tasks).unwrap()).unwrap();

        let loaded: Vec<Task> =
            serde_json::from_str(&fs::read_to_string(&file_path).unwrap()).unwrap();
        assert_eq!(loaded.len(), 0);

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_locking_concurrency() {
        let temp_dir = std::env::temp_dir().join("cfait_test_lock");
        let _ = fs::create_dir_all(&temp_dir);
        let file_path = temp_dir.join("lock_test.txt");
        let path_ref = Arc::new(file_path.clone());

        let _ = fs::write(&file_path, "0");

        let mut handles = vec![];
        for _ in 0..10 {
            let p = path_ref.clone();
            handles.push(thread::spawn(move || {
                LocalStorage::with_lock(&p, || {
                    let content = fs::read_to_string(&*p).unwrap();
                    let num: i32 = content.parse().unwrap();
                    std::thread::sleep(std::time::Duration::from_millis(10));
                    fs::write(&*p, (num + 1).to_string()).unwrap();
                    Ok(())
                })
                .unwrap();
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "10");

        let _ = fs::remove_dir_all(temp_dir);
    }
}
