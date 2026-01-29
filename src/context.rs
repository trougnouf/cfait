// File: ./src/context.rs
//! Application context abstraction for filesystem paths.
//!
//! This module provides an `AppContext` trait that encapsulates how the
//! application determines its data/config/cache directories. Two concrete
//! implementations are provided:
//!
//! - `StandardContext`: Uses `directories::ProjectDirs` and optionally an
//!   override root (useful for Android or CLI overrides).
//! - `TestContext`: Creates a temporary directory for isolated tests and
//!   cleans it up when dropped.
//!
//! The goal is to remove global singletons and environment-variable hacks and
//! allow dependency injection of path resolution into components that perform
//! disk I/O.

use anyhow::{Context, Result};
use directories::ProjectDirs;
use std::path::PathBuf;
use std::sync::Arc;

/// Defines the file system context for the application.
///
/// The trait is object-safe so callers can hold `Arc<dyn AppContext>`.
pub trait AppContext: Send + Sync {
    fn get_data_dir(&self) -> Result<PathBuf>;
    fn get_config_dir(&self) -> Result<PathBuf>;
    fn get_cache_dir(&self) -> Result<PathBuf>;

    // Convenience methods

    /// Returns the full path to the canonical config file.
    fn get_config_file_path(&self) -> Result<PathBuf> {
        Ok(self.get_config_dir()?.join("config.toml"))
    }

    /// Path to the journal (if resolvable).
    fn get_journal_path(&self) -> Option<PathBuf> {
        self.get_data_dir().ok().map(|p| p.join("journal.json"))
    }

    /// Path to the primary local task file (if resolvable).
    fn get_local_task_path(&self) -> Option<PathBuf> {
        self.get_data_dir().ok().map(|p| p.join("local.json"))
    }

    /// Path to the alarm index file (if resolvable).
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
        if let Ok(test_dir) = std::env::var("CFAIT_TEST_DIR") {
            let p = PathBuf::from(test_dir).join("data");
            if !p.exists() {
                std::fs::create_dir_all(&p)?;
            }
            return Ok(p);
        }

        if let Some(root) = &self.override_root {
            return Self::ensure_exists(root.join("data"));
        }
        let proj = Self::get_proj_dirs().ok_or_else(|| anyhow::anyhow!("No home directory"))?;
        Self::ensure_exists(proj.data_dir().to_path_buf())
    }

    fn get_config_dir(&self) -> Result<PathBuf> {
        if let Ok(test_dir) = std::env::var("CFAIT_TEST_DIR") {
            let p = PathBuf::from(test_dir).join("config");
            if !p.exists() {
                std::fs::create_dir_all(&p)?;
            }
            return Ok(p);
        }

        if let Some(root) = &self.override_root {
            return Self::ensure_exists(root.join("config"));
        }
        let proj = Self::get_proj_dirs().ok_or_else(|| anyhow::anyhow!("No home directory"))?;
        Self::ensure_exists(proj.config_dir().to_path_buf())
    }

    fn get_cache_dir(&self) -> Result<PathBuf> {
        if let Ok(test_dir) = std::env::var("CFAIT_TEST_DIR") {
            let p = PathBuf::from(test_dir).join("cache");
            if !p.exists() {
                std::fs::create_dir_all(&p)?;
            }
            return Ok(p);
        }

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

// Compatibility helpers for incremental migration
//
// Many call sites are being updated to accept an `&impl AppContext` or
// `Arc<dyn AppContext>`. While migrating, some modules may still call into
// functions that expect global paths. These lightweight wrappers provide an
// easy-to-use default (desktop) context backed by `StandardContext::new(None)`
// so older call sites can be migrated incrementally.
//
// NOTE: Prefer dependency-injecting an `Arc<dyn AppContext>` into structs and
// functions; these helpers are only for compatibility.

/// Returns a fresh `StandardContext` using the default platform locations.
pub fn default_context() -> StandardContext {
    StandardContext::new(None)
}

/// Returns an `Arc` boxed context using standard platform locations.
pub fn default_shared_context() -> SharedContext {
    Arc::new(StandardContext::new(None))
}

/// Convenience wrapper: returns the data directory using a default context.
pub fn get_data_dir() -> Result<PathBuf> {
    default_context().get_data_dir()
}

/// Convenience wrapper: returns the config directory using a default context.
pub fn get_config_dir() -> Result<PathBuf> {
    default_context().get_config_dir()
}

/// Convenience wrapper: returns the cache directory using a default context.
pub fn get_cache_dir() -> Result<PathBuf> {
    default_context().get_cache_dir()
}

/// Convenience wrapper: returns the canonical config file path using a default context.
pub fn get_config_file_path() -> Result<PathBuf> {
    default_context().get_config_file_path()
}

/// Convenience wrapper: returns the journal path (if resolvable) using a default context.
pub fn get_journal_path() -> Option<PathBuf> {
    default_context().get_journal_path()
}

/// Convenience wrapper: returns the local task path (if resolvable) using a default context.
pub fn get_local_task_path() -> Option<PathBuf> {
    default_context().get_local_task_path()
}

/// Convenience wrapper: returns the alarm index path (if resolvable) using a default context.
pub fn get_alarm_index_path() -> Option<PathBuf> {
    default_context().get_alarm_index_path()
}

// Convenience alias for users who want to store the context in an Arc.
pub type SharedContext = Arc<dyn AppContext>;
