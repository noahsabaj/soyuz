//! Runtime services shared by Dioxus UI components.
//!
//! These handles are intentionally kept outside the reactive app store. They
//! represent process and thread-safe IO resources, not UI state.

use crate::state::{TerminalBuffer, TerminalEntry, TerminalLevel};
use parking_lot::Mutex;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::Notify;
use tracing::warn;

/// Non-reactive application services used by the desktop UI.
#[derive(Clone)]
pub struct AppServices {
    terminal_buffer: TerminalBuffer,
    preview_process: Arc<Mutex<Option<std::process::Child>>>,
    /// Monotonic counter identifying the current preview. Bumped on each spawn so
    /// a superseded wait-loop can detect it is stale and stop touching state (F53).
    preview_generation: Arc<AtomicU64>,
}

impl PartialEq for AppServices {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl Drop for AppServices {
    fn drop(&mut self) {
        // `AppServices` is cloned into many UI closures and `Drop` runs for every
        // clone, so only stop the preview when this is the last owner of the shared
        // process handle. Otherwise a transient clone drop would kill a running
        // preview. The explicit window-close path remains the primary cleanup.
        if Arc::strong_count(&self.preview_process) == 1 {
            self.stop_preview_process();
        }
    }
}

impl AppServices {
    /// Create services backed by the tracing terminal buffer.
    pub fn new(terminal_buffer: TerminalBuffer) -> Self {
        Self {
            terminal_buffer,
            preview_process: Arc::new(Mutex::new(None)),
            preview_generation: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Shared preview process handle.
    pub fn preview_process(&self) -> Arc<Mutex<Option<std::process::Child>>> {
        self.preview_process.clone()
    }

    /// Bump the preview generation and return the new value. Each preview spawn
    /// calls this so a previous `wait_for_process_exit` loop can recognise it was
    /// superseded and exit without clobbering the newer preview's state (F53).
    pub fn bump_preview_generation(&self) -> u64 {
        self.preview_generation.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// The current preview generation.
    pub fn preview_generation(&self) -> u64 {
        self.preview_generation.load(Ordering::SeqCst)
    }

    /// Replace the preview process handle.
    pub fn set_preview_process(&self, process: std::process::Child) {
        *self.preview_process.lock() = Some(process);
    }

    /// Stop the preview process if one is running.
    pub fn stop_preview_process(&self) {
        if let Some(ref mut process) = *self.preview_process.lock()
            && let Err(e) = process.kill()
        {
            warn!("Failed to kill preview process: {e}");
        }
        *self.preview_process.lock() = None;

        // Best-effort cleanup of the per-process temp script file.
        let _ = std::fs::remove_file(crate::preview::preview_temp_path());
    }

    /// Add a message to the terminal output.
    pub fn terminal_log(&self, level: TerminalLevel, message: impl Into<String>) {
        self.terminal_buffer
            .push(TerminalEntry::new(level, message));
    }

    /// Clear the terminal output.
    pub fn terminal_clear(&self) {
        self.terminal_buffer.clear();
    }

    /// Snapshot terminal output with stable per-entry ids for rendering (F72).
    pub fn terminal_snapshot_with_ids(&self) -> Vec<(u64, TerminalEntry)> {
        self.terminal_buffer.snapshot_with_ids()
    }

    /// Handle to await the next terminal output change (F72).
    pub fn terminal_notifier(&self) -> Arc<Notify> {
        self.terminal_buffer.notifier()
    }
}
