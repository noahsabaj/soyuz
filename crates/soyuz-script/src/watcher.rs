//! File watcher for hot reloading Rhai scripts
//!
//! Watches script files for changes and notifies when they should be re-evaluated.

use anyhow::{Result, anyhow};
use notify::RecursiveMode;
use notify_debouncer_mini::{DebouncedEvent, new_debouncer};
use parking_lot::Mutex;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

/// Event emitted when a watched file changes
#[derive(Debug, Clone)]
pub enum WatchEvent {
    /// A watched file was modified
    Modified(PathBuf),
    /// A watched file was created
    Created(PathBuf),
    /// A watched file was deleted
    Deleted(PathBuf),
    /// An error occurred while watching
    Error(String),
}

/// Watches script files for changes
pub struct ScriptWatcher {
    /// The debouncer that handles file watching
    _debouncer: notify_debouncer_mini::Debouncer<notify::RecommendedWatcher>,
    /// Receiver for watch events
    receiver: Receiver<WatchEvent>,
    /// Paths being watched
    watched_paths: Arc<Mutex<Vec<PathBuf>>>,
}

impl ScriptWatcher {
    /// Create a new script watcher
    ///
    /// # Arguments
    ///
    /// * `debounce_ms` - Debounce duration in milliseconds (default: 100)
    pub fn new(debounce_ms: Option<u64>) -> Result<Self> {
        let (tx, rx) = channel();
        let watched_paths = Arc::new(Mutex::new(Vec::new()));
        let watched_paths_clone = watched_paths.clone();

        let debounce_duration = Duration::from_millis(debounce_ms.unwrap_or(100));

        let debouncer = new_debouncer(
            debounce_duration,
            move |result: Result<Vec<DebouncedEvent>, notify::Error>| {
                match result {
                    Ok(events) => {
                        for event in events {
                            let path = event.path.clone();
                            let watched = watched_paths_clone.lock();

                            // Check if this path is one we're watching
                            let is_watched =
                                watched.iter().any(|p| path.starts_with(p) || path == *p);

                            if is_watched {
                                // Check if it's a .rhai file or the exact watched path
                                let is_rhai = path.extension().is_some_and(|e| e == "rhai");
                                let is_exact = watched.contains(&path);

                                if is_rhai || is_exact {
                                    // All debounced events are treated as modifications
                                    let _ = tx.send(WatchEvent::Modified(path));
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(WatchEvent::Error(format!("Watch error: {:?}", e)));
                    }
                }
            },
        )
        .map_err(|e| anyhow!("Failed to create file watcher: {:?}", e))?;

        Ok(Self {
            _debouncer: debouncer,
            receiver: rx,
            watched_paths,
        })
    }

    /// Watch a file or directory for changes
    pub fn watch(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref().to_path_buf();
        let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());

        // Add to watched paths
        {
            let mut watched = self.watched_paths.lock();
            if !watched.contains(&canonical) {
                watched.push(canonical.clone());
            }
        }

        // Watch with the debouncer
        self._debouncer
            .watcher()
            .watch(&canonical, RecursiveMode::NonRecursive)
            .map_err(|e| anyhow!("Failed to watch path {}: {}", canonical.display(), e))?;

        tracing::info!("Watching: {}", canonical.display());
        Ok(())
    }

    /// Stop watching a path
    pub fn unwatch(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());

        // Remove from watched paths
        {
            let mut watched = self.watched_paths.lock();
            watched.retain(|p| p != &canonical);
        }

        self._debouncer
            .watcher()
            .unwatch(&canonical)
            .map_err(|e| anyhow!("Failed to unwatch path {}: {}", canonical.display(), e))?;

        Ok(())
    }

    /// Try to receive a watch event (non-blocking)
    pub fn try_recv(&self) -> Option<WatchEvent> {
        self.receiver.try_recv().ok()
    }

    /// Receive a watch event (blocking)
    pub fn recv(&self) -> Option<WatchEvent> {
        self.receiver.recv().ok()
    }

    /// Receive a watch event with timeout
    pub fn recv_timeout(&self, timeout: Duration) -> Option<WatchEvent> {
        self.receiver.recv_timeout(timeout).ok()
    }

    /// Check if there are pending events
    pub fn has_events(&self) -> bool {
        // This is a bit hacky but works for checking if there are events
        !self.receiver.try_iter().collect::<Vec<_>>().is_empty()
    }

    /// Get all pending events
    pub fn drain_events(&self) -> Vec<WatchEvent> {
        self.receiver.try_iter().collect()
    }
}

/// A simple callback-based watcher for integration with event loops
pub struct CallbackWatcher {
    watcher: ScriptWatcher,
    callback: Box<dyn Fn(WatchEvent) + Send>,
}

impl CallbackWatcher {
    /// Create a new callback watcher
    pub fn new<F>(callback: F) -> Result<Self>
    where
        F: Fn(WatchEvent) + Send + 'static,
    {
        Ok(Self {
            watcher: ScriptWatcher::new(None)?,
            callback: Box::new(callback),
        })
    }

    /// Watch a file or directory
    pub fn watch(&mut self, path: impl AsRef<Path>) -> Result<()> {
        self.watcher.watch(path)
    }

    /// Poll for events and call the callback
    pub fn poll(&self) {
        while let Some(event) = self.watcher.try_recv() {
            (self.callback)(event);
        }
    }
}

/// Watch state for use with async contexts or polling
#[derive(Debug, Clone)]
pub struct WatchState {
    /// The path being watched
    pub path: PathBuf,
    /// Whether the file has been modified since last check
    pub modified: bool,
    /// Last error if any
    pub last_error: Option<String>,
}

impl WatchState {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            modified: false,
            last_error: None,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_watcher_creation() {
        let watcher = ScriptWatcher::new(None);
        assert!(watcher.is_ok());
    }

    #[test]
    fn test_watch_nonexistent() {
        let mut watcher = ScriptWatcher::new(None).unwrap();
        // Watching a nonexistent file should fail
        let result = watcher.watch("/nonexistent/path/test.rhai");
        assert!(result.is_err());
    }
}
