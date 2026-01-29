// File: ./src/context.rs
/*! Application context abstraction for filesystem paths.

This module provides an `AppContext` trait that encapsulates how the
application determines its data/config/cache directories. Two concrete
implementations are provided:

- `StandardContext`: Uses `directories::ProjectDirs` and optionally an
  override root (useful for Android or CLI overrides).
- `TestContext`: Creates a temporary directory for isolated tests and
  cleans it up when dropped.

This file intentionally does NOT provide any global or environment-var
based helpers. Consumers must explicitly pass an `Arc<dyn AppContext>`
or `&dyn AppContext` to any code that performs filesystem IO. This
removes hidden global state and makes tests and multi-tenant usage safe.
*/

use anyhow::{Context, Result};
use directories::ProjectDirs;
use std::path::PathBuf;

/// Defines the file system context for the application.
///
/// The trait is object-safe so callers can hold `Arc<dyn AppContext>`.
pub trait AppContext: Send + Sync + std::fmt::Debug {
    fn get_data_dir(&self) -> Result<PathBuf>;
    fn get_config_dir(&self) -> Result<PathBuf>;
    fn get_cache_dir(&self) -> Result<PathBuf>;

    fn get_config_file_path(&self) -> Result<PathBuf> {
        Ok(self.get_config_dir()?.join("config.toml"))
    }

    fn get_journal_path(&self) -> Option<PathBuf> {
        self.get_data_dir().ok().map(|p| p.join("journal.json"))
    }

    fn get_local_task_path(&self) -> Option<PathBuf> {
        self.get_data_dir().ok().map(|p| p.join("local.json"))
    }

    fn get_alarm_index_path(&self) -> Option<PathBuf> {
        self.get_data_dir().ok().map(|p| p.join("alarm_index.json"))
    }
}

// --- Production Implementation ---

#[derive(Clone, Debug)]
pub struct StandardContext {
    override_root: Option<PathBuf>,
}

impl StandardContext {
    /// Create a new StandardContext.
    ///
    /// When `override_root` is `Some(path)`, all directories will be created
    /// under that root using `data`, `config`, and `cache` subdirectories.
    pub fn new(override_root: Option<PathBuf>) -> Self {
        Self { override_root }
    }

    fn ensure_exists(path: PathBuf) -> Result<PathBuf> {
        if !path.exists() {
            std::fs::create_dir_all(&path)
                .with_context(|| format!("Failed to create directory: {:?}", path))?;
        }
        Ok(path)
    }

    fn get_proj_dirs() -> Option<ProjectDirs> {
        // Try both historical vendor IDs to be tolerant of packaging differences.
        ProjectDirs::from("com", "cfait", "cfait")
            .or_else(|| ProjectDirs::from("com", "trougnouf", "cfait"))
    }
}

impl AppContext for StandardContext {
    fn get_data_dir(&self) -> Result<PathBuf> {
        if let Some(root) = &self.override_root {
            return Self::ensure_exists(root.join("data"));
        }
        let proj = Self::get_proj_dirs().ok_or_else(|| anyhow::anyhow!("No home directory"))?;
        Self::ensure_exists(proj.data_dir().to_path_buf())
    }

    fn get_config_dir(&self) -> Result<PathBuf> {
        if let Some(root) = &self.override_root {
            return Self::ensure_exists(root.join("config"));
        }
        let proj = Self::get_proj_dirs().ok_or_else(|| anyhow::anyhow!("No home directory"))?;
        Self::ensure_exists(proj.config_dir().to_path_buf())
    }

    fn get_cache_dir(&self) -> Result<PathBuf> {
        if let Some(root) = &self.override_root {
            return Self::ensure_exists(root.join("cache"));
        }
        let proj = Self::get_proj_dirs().ok_or_else(|| anyhow::anyhow!("No home directory"))?;
        Self::ensure_exists(proj.cache_dir().to_path_buf())
    }
}

// --- Test Implementation ---

#[derive(Clone, Debug)]
pub struct TestContext {
    pub root: PathBuf,
}

impl TestContext {
    /// Creates a new TestContext backed by a unique temporary directory.
    ///
    /// The directory is created immediately and removed when the `TestContext`
    /// is dropped.
    pub fn new() -> Self {
        // Generate a unique directory under the OS temp dir.
        let uuid = uuid::Uuid::new_v4();
        let root = std::env::temp_dir().join(format!("cfait_test_{}", uuid));
        // Best-effort create; tests will panic if this fails.
        std::fs::create_dir_all(&root).expect("failed to create TestContext temp dir");
        Self { root }
    }
}

impl Default for TestContext {
    fn default() -> Self {
        Self::new()
    }
}

impl AppContext for TestContext {
    fn get_data_dir(&self) -> Result<PathBuf> {
        let p = self.root.join("data");
        std::fs::create_dir_all(&p)?;
        Ok(p)
    }

    fn get_config_dir(&self) -> Result<PathBuf> {
        let p = self.root.join("config");
        std::fs::create_dir_all(&p)?;
        Ok(p)
    }

    fn get_cache_dir(&self) -> Result<PathBuf> {
        let p = self.root.join("cache");
        std::fs::create_dir_all(&p)?;
        Ok(p)
    }
}

impl Drop for TestContext {
    fn drop(&mut self) {
        // Best-effort cleanup; ignore errors.
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

// Convenience alias for users who want to store the context in an Arc.
pub type SharedContext = std::sync::Arc<dyn AppContext>;
