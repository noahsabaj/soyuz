//! Runtime services shared by Dioxus UI components.
//!
//! These handles are intentionally kept outside the reactive app store. They
//! represent process and thread-safe IO resources, not UI state.

use crate::state::{PreviewState, TerminalBuffer, TerminalEntry, TerminalLevel};
use parking_lot::Mutex;
use std::sync::Arc;
use tracing::warn;

/// Non-reactive application services used by the desktop UI.
#[derive(Clone)]
pub struct AppServices {
    terminal_buffer: TerminalBuffer,
    preview_state: Arc<Mutex<PreviewState>>,
    preview_process: Arc<Mutex<Option<std::process::Child>>>,
}

impl PartialEq for AppServices {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl AppServices {
    /// Create services backed by the tracing terminal buffer.
    pub fn new(terminal_buffer: TerminalBuffer) -> Self {
        Self {
            terminal_buffer,
            preview_state: Arc::new(Mutex::new(PreviewState::default())),
            preview_process: Arc::new(Mutex::new(None)),
        }
    }

    /// Shared preview process handle.
    pub fn preview_process(&self) -> Arc<Mutex<Option<std::process::Child>>> {
        self.preview_process.clone()
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
    }

    /// Publish a script to the preview process and request an update.
    pub fn publish_preview_script(&self, script: String) {
        let mut preview = self.preview_state.lock();
        preview.script = script;
        preview.needs_update = true;
        preview.should_open = true;
    }

    /// Mark the preview as stale after an editor change.
    pub fn mark_preview_dirty(&self) {
        self.preview_state.lock().needs_update = true;
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

    /// Snapshot terminal output for rendering.
    pub fn terminal_snapshot(&self) -> Vec<TerminalEntry> {
        self.terminal_buffer.snapshot()
    }
}
