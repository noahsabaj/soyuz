//! Undo/redo history management
//!
//! Provides edit history tracking with snapshot-based undo/redo operations
//! and automatic grouping of rapid consecutive edits.

use serde::{Deserialize, Serialize};
use std::time::Instant;

/// Maximum number of undo steps to keep per tab
const MAX_UNDO_HISTORY: usize = 100;

/// Time window in milliseconds for grouping consecutive edits
const EDIT_GROUP_MS: u128 = 500;

/// Convert line and column (1-indexed) to byte offset
pub fn line_col_to_offset(text: &str, line: usize, col: usize) -> usize {
    let mut current_line = 1;
    let mut offset = 0;

    for (idx, ch) in text.char_indices() {
        if current_line == line {
            // We're on the target line, count columns
            let line_start = idx;
            let target_offset = line_start + col.saturating_sub(1);
            return target_offset.min(text.len());
        }
        if ch == '\n' {
            current_line += 1;
        }
        offset = idx + ch.len_utf8();
    }

    // If we didn't find the line, return end of text
    offset.min(text.len())
}

/// A snapshot of editor content for undo/redo
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct EditSnapshot {
    /// The content at this point
    pub content: String,
    /// Cursor position (byte offset)
    pub cursor_pos: usize,
}

/// Undo/redo history for a tab
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct UndoHistory {
    /// Stack of previous states (for undo)
    pub undo_stack: Vec<EditSnapshot>,
    /// Stack of undone states (for redo)
    pub redo_stack: Vec<EditSnapshot>,
    /// Timestamp of last edit (for grouping) - not serialized
    #[serde(skip)]
    last_edit_time: Option<Instant>,
    /// Whether we're in the middle of an undo/redo operation - not serialized
    #[serde(skip)]
    in_undo_redo: bool,
}

impl PartialEq for UndoHistory {
    fn eq(&self, other: &Self) -> bool {
        // Only compare stacks, not timing info
        self.undo_stack == other.undo_stack && self.redo_stack == other.redo_stack
    }
}

impl UndoHistory {
    /// Record a new edit, potentially grouping with previous edit
    pub fn record_edit(&mut self, old_content: &str, old_cursor: usize) {
        // Don't record if we're in an undo/redo operation
        if self.in_undo_redo {
            return;
        }

        let now = Instant::now();
        let should_group = self
            .last_edit_time
            .map(|t| now.duration_since(t).as_millis() < EDIT_GROUP_MS)
            .unwrap_or(false);

        if !should_group {
            // Start a new undo group - save the old state
            self.undo_stack.push(EditSnapshot {
                content: old_content.to_string(),
                cursor_pos: old_cursor,
            });

            // Trim history if too long
            while self.undo_stack.len() > MAX_UNDO_HISTORY {
                self.undo_stack.remove(0);
            }

            // Clear redo stack on new edit
            self.redo_stack.clear();
        }

        self.last_edit_time = Some(now);
    }

    /// Undo the last edit, returns the state to restore (if any)
    pub fn undo(&mut self, current_content: &str, current_cursor: usize) -> Option<EditSnapshot> {
        if let Some(snapshot) = self.undo_stack.pop() {
            // Save current state to redo stack
            self.redo_stack.push(EditSnapshot {
                content: current_content.to_string(),
                cursor_pos: current_cursor,
            });
            self.in_undo_redo = true;
            Some(snapshot)
        } else {
            None
        }
    }

    /// Redo the last undone edit, returns the state to restore (if any)
    pub fn redo(&mut self, current_content: &str, current_cursor: usize) -> Option<EditSnapshot> {
        if let Some(snapshot) = self.redo_stack.pop() {
            // Save current state to undo stack
            self.undo_stack.push(EditSnapshot {
                content: current_content.to_string(),
                cursor_pos: current_cursor,
            });
            self.in_undo_redo = true;
            Some(snapshot)
        } else {
            None
        }
    }

    /// Mark that an undo/redo operation is complete
    pub fn finish_undo_redo(&mut self) {
        self.in_undo_redo = false;
        self.last_edit_time = None; // Reset grouping timer
    }
}
