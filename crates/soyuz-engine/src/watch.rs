//! File watching utilities for hot-reload functionality
//!
//! This module re-exports and wraps the file watching functionality from
//! soyuz-script for use with the Engine API.

// Re-export the watcher types from soyuz-script
pub use soyuz_script::{ScriptWatcher, WatchEvent};

use anyhow::Result;
use std::path::Path;

/// Create a new script watcher with default settings
pub fn create_watcher() -> Result<ScriptWatcher> {
    ScriptWatcher::new(None)
}

/// Create a new script watcher with custom debounce duration
pub fn create_watcher_with_debounce(debounce_ms: u64) -> Result<ScriptWatcher> {
    ScriptWatcher::new(Some(debounce_ms))
}

/// Convenience function to start watching a single file
pub fn watch_file(path: impl AsRef<Path>) -> Result<ScriptWatcher> {
    let mut watcher = create_watcher()?;
    watcher.watch(path)?;
    Ok(watcher)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_watcher() {
        let watcher = create_watcher();
        assert!(watcher.is_ok());
    }

    #[test]
    fn test_create_watcher_with_debounce() {
        let watcher = create_watcher_with_debounce(200);
        assert!(watcher.is_ok());
    }
}
