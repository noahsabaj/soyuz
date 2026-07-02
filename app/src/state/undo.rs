//! Undo/redo history management
//!
//! Provides edit history tracking with snapshot-based undo/redo operations
//! and automatic grouping of rapid consecutive edits.

use serde::{Deserialize, Serialize};
use std::time::Instant;

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
    ///
    /// `limit` is the maximum number of undo steps to keep (from settings).
    pub fn record_edit(&mut self, old_content: &str, old_cursor: usize, limit: usize) {
        // Don't record if we're in an undo/redo operation
        if self.in_undo_redo {
            return;
        }

        // Guard against a pathological 0 limit which would discard all history.
        let limit = limit.max(1);

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
            while self.undo_stack.len() > limit {
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

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    /// Record an edit as the start of a new group, regardless of wall clock:
    /// `finish_undo_redo` resets the grouping timer.
    fn record_new_group(history: &mut UndoHistory, old_content: &str, limit: usize) {
        history.finish_undo_redo();
        history.record_edit(old_content, 0, limit);
    }

    #[test]
    fn line_col_to_offset_maps_lines_and_clamps() {
        let text = "let a = 1;\nlet b = 2;\n";
        assert_eq!(line_col_to_offset(text, 1, 1), 0);
        assert_eq!(line_col_to_offset(text, 2, 1), 11);
        assert_eq!(line_col_to_offset(text, 2, 5), 15);
        // Column past the end of the text clamps to the text length.
        assert_eq!(line_col_to_offset(text, 2, 999), text.len());
        // Line past the end of the text clamps to the text length.
        assert_eq!(line_col_to_offset(text, 99, 1), text.len());
    }

    #[test]
    fn rapid_edits_group_into_one_undo_step() {
        let mut history = UndoHistory::default();
        history.record_edit("v1", 0, 100);
        // Immediately after: well inside the grouping window.
        history.record_edit("v2", 1, 100);
        history.record_edit("v3", 2, 100);
        assert_eq!(history.undo_stack.len(), 1);
        assert_eq!(history.undo_stack[0].content, "v1");
    }

    #[test]
    fn edits_past_the_grouping_window_start_a_new_step() {
        let mut history = UndoHistory::default();
        history.record_edit("v1", 0, 100);
        // The window is EDIT_GROUP_MS of real time; sleep just past it.
        std::thread::sleep(std::time::Duration::from_millis(EDIT_GROUP_MS as u64 + 100));
        history.record_edit("v2", 1, 100);
        assert_eq!(history.undo_stack.len(), 2);
        assert_eq!(history.undo_stack[1].content, "v2");
    }

    #[test]
    fn history_trims_oldest_beyond_limit() {
        let mut history = UndoHistory::default();
        for content in ["v1", "v2", "v3", "v4", "v5"] {
            record_new_group(&mut history, content, 3);
        }
        let contents: Vec<_> = history
            .undo_stack
            .iter()
            .map(|s| s.content.as_str())
            .collect();
        assert_eq!(contents, ["v3", "v4", "v5"]);

        // Undo lands on the newest retained snapshot.
        let restored = history.undo("v6", 0).expect("undo should pop");
        assert_eq!(restored.content, "v5");
    }

    #[test]
    fn zero_limit_is_guarded_to_keep_one_step() {
        let mut history = UndoHistory::default();
        record_new_group(&mut history, "v1", 0);
        record_new_group(&mut history, "v2", 0);
        assert_eq!(history.undo_stack.len(), 1);
        assert_eq!(history.undo_stack[0].content, "v2");
    }

    #[test]
    fn undo_redo_transfer_round_trips_content_and_cursor() {
        let mut history = UndoHistory::default();
        history.record_edit("v1", 3, 100);

        // Undo: current state moves to the redo stack.
        let restored = history.undo("v2", 7).expect("undo should pop");
        assert_eq!((restored.content.as_str(), restored.cursor_pos), ("v1", 3));
        assert_eq!(history.redo_stack.len(), 1);
        history.finish_undo_redo();

        // Redo: round-trips back, restoring content and cursor.
        let redone = history.redo("v1", 3).expect("redo should pop");
        assert_eq!((redone.content.as_str(), redone.cursor_pos), ("v2", 7));
        assert_eq!(history.undo_stack.len(), 1);
        history.finish_undo_redo();
    }

    #[test]
    fn new_edit_clears_redo_and_undo_ignores_in_flight_recording() {
        let mut history = UndoHistory::default();
        history.record_edit("v1", 0, 100);
        let _ = history.undo("v2", 0).expect("undo should pop");

        // While an undo is being applied, recording is suppressed.
        history.record_edit("applying-undo", 0, 100);
        assert!(history.undo_stack.is_empty());
        history.finish_undo_redo();

        // A fresh edit clears the redo stack.
        assert_eq!(history.redo_stack.len(), 1);
        history.record_edit("v1", 0, 100);
        assert!(history.redo_stack.is_empty());
    }
}
