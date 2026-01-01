//! Application state management

// Separate if statements are clearer for pane traversal logic
#![allow(clippy::collapsible_if)]
// Collapsible match patterns are less readable for pane operations
#![allow(clippy::collapsible_match)]
// clone_from() adds noise for simple string assignments
#![allow(clippy::assigning_clones)]
// Matching over () is explicit but not more readable here
#![allow(clippy::ignored_unit_patterns)]
// map_or is less readable for optional values
#![allow(clippy::map_unwrap_or)]
// Owned PathBuf is intentional for storage
#![allow(clippy::needless_pass_by_value)]

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

// Re-export ExportFormat from soyuz-core
pub use soyuz_core::export::ExportFormat;

/// Unique identifier for editor tabs
pub type TabId = u64;

/// Unique identifier for panes
pub type PaneId = u64;

/// Direction of a split pane
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum SplitDirection {
    /// Side-by-side (left/right columns)
    Vertical,
    /// Top/bottom (rows)
    Horizontal,
}

/// Maximum number of undo steps to keep per tab
const MAX_UNDO_HISTORY: usize = 100;

/// Time window in milliseconds for grouping consecutive edits
const EDIT_GROUP_MS: u128 = 500;

/// Convert line and column (1-indexed) to byte offset
fn line_col_to_offset(text: &str, line: usize, col: usize) -> usize {
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

/// A single editor tab
#[derive(Clone, PartialEq)]
pub struct EditorTab {
    /// Unique identifier
    pub id: TabId,
    /// File path (None if untitled)
    pub path: Option<PathBuf>,
    /// Content of the file
    pub content: String,
    /// Whether the content has been modified since last save
    pub is_dirty: bool,
    /// Cursor line position
    pub cursor_line: usize,
    /// Cursor column position
    pub cursor_col: usize,
    /// Undo/redo history
    pub history: UndoHistory,
}

impl EditorTab {
    /// Create a new untitled tab with default example script
    pub fn new_untitled(id: TabId) -> Self {
        Self {
            id,
            path: None,
            content: DEFAULT_SCRIPT.to_string(),
            is_dirty: false,
            cursor_line: 1,
            cursor_col: 1,
            history: UndoHistory::default(),
        }
    }

    /// Create a new blank tab (empty content)
    pub fn new_blank(id: TabId) -> Self {
        Self {
            id,
            path: None,
            content: String::new(),
            is_dirty: false,
            cursor_line: 1,
            cursor_col: 1,
            history: UndoHistory::default(),
        }
    }

    /// Create a tab from a file
    pub fn from_file(id: TabId, path: PathBuf, content: String) -> Self {
        Self {
            id,
            path: Some(path),
            content,
            is_dirty: false,
            cursor_line: 1,
            cursor_col: 1,
            history: UndoHistory::default(),
        }
    }

    /// Create a tab with existing history (for session restore)
    pub fn with_history(
        id: TabId,
        path: Option<PathBuf>,
        content: String,
        is_dirty: bool,
        history: UndoHistory,
    ) -> Self {
        Self {
            id,
            path,
            content,
            is_dirty,
            cursor_line: 1,
            cursor_col: 1,
            history,
        }
    }

    /// Get display name for the tab
    pub fn display_name(&self) -> String {
        match &self.path {
            Some(path) => path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "Untitled".to_string()),
            None => "Untitled".to_string(),
        }
    }
}

/// A pane in the editor layout (recursive tree structure)
#[derive(Clone, PartialEq)]
pub enum EditorPane {
    /// A single tab group with tabs
    TabGroup {
        /// Unique ID for this pane
        id: PaneId,
        tabs: Vec<EditorTab>,
        active_tab_idx: usize,
    },
    /// A split container with two child panes
    Split {
        /// Direction of the split
        direction: SplitDirection,
        /// First child (left for vertical, top for horizontal)
        first: Box<EditorPane>,
        /// Second child (right for vertical, bottom for horizontal)
        second: Box<EditorPane>,
        /// Proportion of space for first pane (0.0 to 1.0, default 0.5)
        ratio: f32,
    },
}

impl Default for EditorPane {
    fn default() -> Self {
        EditorPane::TabGroup {
            id: 1,
            tabs: vec![EditorTab::new_untitled(1)],
            active_tab_idx: 0,
        }
    }
}

impl EditorPane {
    /// Get the active tab in the first TabGroup found (for single pane operations)
    pub fn active_tab(&self) -> Option<&EditorTab> {
        match self {
            EditorPane::TabGroup {
                tabs,
                active_tab_idx,
                ..
            } => tabs.get(*active_tab_idx),
            EditorPane::Split { first, .. } => first.active_tab(),
        }
    }

    /// Get mutable active tab in the first TabGroup found
    pub fn active_tab_mut(&mut self) -> Option<&mut EditorTab> {
        match self {
            EditorPane::TabGroup {
                tabs,
                active_tab_idx,
                ..
            } => tabs.get_mut(*active_tab_idx),
            EditorPane::Split { first, .. } => first.active_tab_mut(),
        }
    }

    /// Find a pane by ID and return a reference (recursive)
    pub fn find_pane(&self, pane_id: PaneId) -> Option<&EditorPane> {
        match self {
            EditorPane::TabGroup { id, .. } if *id == pane_id => Some(self),
            EditorPane::TabGroup { .. } => None,
            EditorPane::Split { first, second, .. } => {
                first.find_pane(pane_id).or_else(|| second.find_pane(pane_id))
            }
        }
    }

    /// Find a pane by ID and return a mutable reference (recursive)
    pub fn find_pane_mut(&mut self, pane_id: PaneId) -> Option<&mut EditorPane> {
        match self {
            EditorPane::TabGroup { id, .. } if *id == pane_id => Some(self),
            EditorPane::TabGroup { .. } => None,
            EditorPane::Split { first, second, .. } => {
                // Check first child, then second
                if first.find_pane(pane_id).is_some() {
                    first.find_pane_mut(pane_id)
                } else {
                    second.find_pane_mut(pane_id)
                }
            }
        }
    }

    /// Find a tab by its file path, returns (pane_id, tab_index) (recursive)
    pub fn find_tab_by_path(&self, path: &PathBuf) -> Option<(PaneId, usize)> {
        match self {
            EditorPane::TabGroup { id, tabs, .. } => {
                for (idx, tab) in tabs.iter().enumerate() {
                    if tab.path.as_ref() == Some(path) {
                        return Some((*id, idx));
                    }
                }
                None
            }
            EditorPane::Split { first, second, .. } => {
                first.find_tab_by_path(path).or_else(|| second.find_tab_by_path(path))
            }
        }
    }

    /// Find which pane contains a tab by ID (recursive)
    pub fn find_pane_containing_tab(&self, tab_id: TabId) -> Option<PaneId> {
        match self {
            EditorPane::TabGroup { id, tabs, .. } => {
                if tabs.iter().any(|t| t.id == tab_id) {
                    Some(*id)
                } else {
                    None
                }
            }
            EditorPane::Split { first, second, .. } => {
                first.find_pane_containing_tab(tab_id)
                    .or_else(|| second.find_pane_containing_tab(tab_id))
            }
        }
    }

    /// Collect all tabs from the pane tree (recursive)
    pub fn collect_tabs(&self) -> Vec<&EditorTab> {
        match self {
            EditorPane::TabGroup { tabs, .. } => tabs.iter().collect(),
            EditorPane::Split { first, second, .. } => {
                let mut result = first.collect_tabs();
                result.extend(second.collect_tabs());
                result
            }
        }
    }

    /// Get all TabGroup pane IDs in the tree (recursive)
    pub fn all_pane_ids(&self) -> Vec<PaneId> {
        match self {
            EditorPane::TabGroup { id, .. } => vec![*id],
            EditorPane::Split { first, second, .. } => {
                let mut ids = first.all_pane_ids();
                ids.extend(second.all_pane_ids());
                ids
            }
        }
    }

    /// Check if this is a root pane (not inside a split) - helper for tree context
    pub fn is_single_pane(&self) -> bool {
        matches!(self, EditorPane::TabGroup { .. })
    }
}

/// Global application state
#[derive(Clone)]
pub struct AppState {
    /// Editor pane layout
    pub editor_pane: EditorPane,
    /// Next tab ID to assign
    pub next_tab_id: TabId,
    /// Next pane ID to assign
    pub next_pane_id: PaneId,
    /// Currently focused pane ID
    pub focused_pane_id: PaneId,
    /// Current workspace folder (None = no folder opened)
    pub workspace: Option<PathBuf>,
    /// Recently opened files (most recent first)
    pub recent_files: Vec<PathBuf>,
    /// Whether preview window is open
    pub is_previewing: bool,
    /// Whether there's an error in the script
    pub has_error: bool,
    /// Error message if any
    pub error_message: Option<String>,
    /// Export settings
    pub export_settings: ExportSettings,
    /// Shared state for preview communication
    pub preview_state: Arc<Mutex<PreviewState>>,
    /// Handle to the preview process (for stopping it)
    pub preview_process: Arc<Mutex<Option<PreviewProcess>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            editor_pane: EditorPane::default(),
            next_tab_id: 2,  // 1 is used by the default tab
            next_pane_id: 2, // 1 is used by the default pane
            focused_pane_id: 1,
            workspace: None, // Start with no folder opened
            recent_files: Vec::new(),
            is_previewing: false,
            has_error: false,
            error_message: None,
            export_settings: ExportSettings::default(),
            preview_state: Arc::new(Mutex::new(PreviewState::default())),
            preview_process: Arc::new(Mutex::new(None)),
        }
    }

    /// Check if a workspace folder is currently open
    pub fn has_workspace(&self) -> bool {
        self.workspace.is_some()
    }

    /// Open a folder as the workspace
    pub fn open_folder(&mut self, path: PathBuf) {
        self.workspace = Some(path);
    }

    /// Close the current workspace folder
    pub fn close_folder(&mut self) {
        self.workspace = None;
    }

    /// Stop the preview process if running
    pub fn stop_preview(&mut self) {
        if let Some(ref mut process) = *self.preview_process.lock() {
            let _ = process.kill();
        }
        *self.preview_process.lock() = None;
        self.is_previewing = false;
    }

    /// Get the currently active code (from the active tab)
    pub fn code(&self) -> String {
        self.active_tab()
            .map(|t| t.content.clone())
            .unwrap_or_default()
    }

    /// Get the current file path (from the active tab)
    pub fn current_file(&self) -> Option<PathBuf> {
        self.active_tab().and_then(|t| t.path.clone())
    }

    /// Get cursor position from active tab
    pub fn cursor_position(&self) -> (usize, usize) {
        self.active_tab()
            .map(|t| (t.cursor_line, t.cursor_col))
            .unwrap_or((1, 1))
    }

    /// Get the active tab from the focused pane
    pub fn active_tab(&self) -> Option<&EditorTab> {
        self.editor_pane
            .find_pane(self.focused_pane_id)
            .and_then(|pane| pane.active_tab())
    }

    /// Get mutable active tab from the focused pane
    pub fn active_tab_mut(&mut self) -> Option<&mut EditorTab> {
        let focused = self.focused_pane_id;
        self.editor_pane
            .find_pane_mut(focused)
            .and_then(|pane| pane.active_tab_mut())
    }

    /// Focus a specific pane
    pub fn focus_pane(&mut self, pane_id: PaneId) {
        if self.editor_pane.find_pane(pane_id).is_some() {
            self.focused_pane_id = pane_id;
        }
    }

    /// Create a new untitled tab in the focused pane
    pub fn new_tab(&mut self) {
        self.new_tab_in_pane(self.focused_pane_id);
    }

    /// Create a new blank tab in a specific pane
    pub fn new_tab_in_pane(&mut self, pane_id: PaneId) {
        let tab = EditorTab::new_blank(self.next_tab_id);
        self.next_tab_id += 1;

        if let Some(EditorPane::TabGroup { tabs, active_tab_idx, .. }) =
            self.editor_pane.find_pane_mut(pane_id)
        {
            tabs.push(tab);
            *active_tab_idx = tabs.len() - 1;
        }

        self.has_error = false;
        self.error_message = None;
    }

    /// Open a file in a new tab (or focus existing tab if already open)
    pub fn open_file(&mut self, path: PathBuf, content: String) {
        self.open_file_in_pane(self.focused_pane_id, path, content);
    }

    /// Open a file in a specific pane
    pub fn open_file_in_pane(&mut self, pane_id: PaneId, path: PathBuf, content: String) {
        // Check if file is already open in any pane
        if let Some((found_pane_id, tab_idx)) = self.editor_pane.find_tab_by_path(&path) {
            self.focused_pane_id = found_pane_id;
            if let Some(EditorPane::TabGroup { active_tab_idx, .. }) =
                self.editor_pane.find_pane_mut(found_pane_id)
            {
                *active_tab_idx = tab_idx;
            }
            return;
        }

        // Create new tab in specified pane
        if let Some(EditorPane::TabGroup { tabs, active_tab_idx, .. }) =
            self.editor_pane.find_pane_mut(pane_id)
        {
            let tab = EditorTab::from_file(self.next_tab_id, path.clone(), content);
            self.next_tab_id += 1;
            tabs.push(tab);
            *active_tab_idx = tabs.len() - 1;
        }

        // Set workspace to file's parent directory if no workspace is open
        if self.workspace.is_none() {
            if let Some(parent) = path.parent() {
                self.workspace = Some(parent.to_path_buf());
            }
        }

        // Add to recent files (move to front if already present)
        self.add_to_recent_files(path);

        self.has_error = false;
        self.error_message = None;
    }

    /// Add a file to the recent files list
    pub fn add_to_recent_files(&mut self, path: PathBuf) {
        // Remove if already present
        self.recent_files.retain(|p| p != &path);
        // Add to front
        self.recent_files.insert(0, path);
        // Keep only last 20 files
        self.recent_files.truncate(20);
    }

    /// Close a tab by ID in a specific pane
    /// If closing the last tab: in root pane, create untitled; in split pane, close the pane
    pub fn close_tab_in_pane(&mut self, pane_id: PaneId, tab_id: TabId) -> bool {
        // Check if this is the last tab
        let is_last_tab = {
            if let Some(EditorPane::TabGroup { tabs, .. }) = self.editor_pane.find_pane(pane_id) {
                tabs.len() == 1 && tabs.iter().any(|t| t.id == tab_id)
            } else {
                false
            }
        };

        // Check if this is the root (only) pane
        let is_root = self.editor_pane.is_single_pane();

        if is_last_tab && !is_root {
            // In a split: close the entire pane
            self.close_pane(pane_id);
        } else if let Some(EditorPane::TabGroup { tabs, active_tab_idx, .. }) =
            self.editor_pane.find_pane_mut(pane_id)
        {
            if let Some(idx) = tabs.iter().position(|t| t.id == tab_id) {
                if tabs.len() == 1 {
                    // Root pane: replace with untitled
                    tabs[0] = EditorTab::new_untitled(self.next_tab_id);
                    self.next_tab_id += 1;
                } else {
                    tabs.remove(idx);
                    if *active_tab_idx >= tabs.len() {
                        *active_tab_idx = tabs.len().saturating_sub(1);
                    } else if *active_tab_idx > idx {
                        *active_tab_idx -= 1;
                    }
                }
            }
        }
        true
    }

    /// Close all tabs associated with a deleted file or directory
    /// For files: closes tabs with matching path
    /// For directories: closes tabs with paths inside the directory
    pub fn close_tabs_for_deleted_path(&mut self, deleted_path: &PathBuf, is_dir: bool) {
        // Collect tab IDs to close (we can't close while iterating)
        let tabs_to_close: Vec<(PaneId, TabId)> = self
            .editor_pane
            .all_pane_ids()
            .into_iter()
            .flat_map(|pane_id| {
                if let Some(EditorPane::TabGroup { tabs, .. }) = self.editor_pane.find_pane(pane_id)
                {
                    tabs.iter()
                        .filter_map(|tab| {
                            tab.path.as_ref().and_then(|tab_path| {
                                let should_close = if is_dir {
                                    // For directories, close any tab with a path inside the directory
                                    tab_path.starts_with(deleted_path)
                                } else {
                                    // For files, close tabs with exact path match
                                    tab_path == deleted_path
                                };
                                if should_close {
                                    Some((pane_id, tab.id))
                                } else {
                                    None
                                }
                            })
                        })
                        .collect::<Vec<_>>()
                } else {
                    vec![]
                }
            })
            .collect();

        // Close each affected tab
        for (pane_id, tab_id) in tabs_to_close {
            self.close_tab_in_pane(pane_id, tab_id);
        }
    }

    /// Switch to a tab by ID in any pane (finds the pane containing the tab)
    pub fn switch_to_tab(&mut self, tab_id: TabId) {
        if let Some(pane_id) = self.editor_pane.find_pane_containing_tab(tab_id) {
            self.focused_pane_id = pane_id;
            if let Some(EditorPane::TabGroup { tabs, active_tab_idx, .. }) =
                self.editor_pane.find_pane_mut(pane_id)
            {
                if let Some(idx) = tabs.iter().position(|t| t.id == tab_id) {
                    *active_tab_idx = idx;
                }
            }
        }
    }

    /// Move a tab from its current pane to a target pane at a specific index
    pub fn move_tab(&mut self, tab_id: TabId, target_pane_id: PaneId, target_index: usize) {
        // Find source pane
        let source_pane_id = match self.editor_pane.find_pane_containing_tab(tab_id) {
            Some(id) => id,
            None => return,
        };

        // Check if moving within the same pane (reorder)
        if source_pane_id == target_pane_id {
            // Find current index and reorder
            if let Some(EditorPane::TabGroup { tabs, .. }) = self.editor_pane.find_pane(source_pane_id) {
                if let Some(old_idx) = tabs.iter().position(|t| t.id == tab_id) {
                    self.reorder_tab(source_pane_id, old_idx, target_index);
                }
            }
            return;
        }

        // Remove tab from source pane
        let tab = {
            let pane = match self.editor_pane.find_pane_mut(source_pane_id) {
                Some(p) => p,
                None => return,
            };
            if let EditorPane::TabGroup { tabs, active_tab_idx, .. } = pane {
                let idx = match tabs.iter().position(|t| t.id == tab_id) {
                    Some(i) => i,
                    None => return,
                };
                let tab = tabs.remove(idx);
                // Adjust active_tab_idx
                if *active_tab_idx >= tabs.len() && !tabs.is_empty() {
                    *active_tab_idx = tabs.len() - 1;
                } else if *active_tab_idx > idx && *active_tab_idx > 0 {
                    *active_tab_idx -= 1;
                }
                tab
            } else {
                return;
            }
        };

        // Check if source pane is now empty (not root)
        let source_empty = {
            if let Some(EditorPane::TabGroup { tabs, .. }) = self.editor_pane.find_pane(source_pane_id) {
                tabs.is_empty()
            } else {
                false
            }
        };
        let is_root = self.editor_pane.is_single_pane();

        // Insert tab into target pane
        if let Some(EditorPane::TabGroup { tabs, active_tab_idx, .. }) =
            self.editor_pane.find_pane_mut(target_pane_id)
        {
            let insert_idx = target_index.min(tabs.len());
            tabs.insert(insert_idx, tab);
            *active_tab_idx = insert_idx;
        }

        // Focus target pane
        self.focused_pane_id = target_pane_id;

        // Collapse source pane if empty and not root
        if source_empty && !is_root {
            self.close_pane(source_pane_id);
        }
    }

    /// Reorder tabs within the same pane
    pub fn reorder_tab(&mut self, pane_id: PaneId, old_index: usize, new_index: usize) {
        if old_index == new_index {
            return;
        }

        if let Some(EditorPane::TabGroup { tabs, active_tab_idx, .. }) =
            self.editor_pane.find_pane_mut(pane_id)
        {
            if old_index >= tabs.len() || new_index > tabs.len() {
                return;
            }

            let tab = tabs.remove(old_index);
            // Adjust new_index if removing shifted things
            let insert_idx = if new_index > old_index {
                (new_index - 1).min(tabs.len())
            } else {
                new_index.min(tabs.len())
            };
            tabs.insert(insert_idx, tab);

            // Update active_tab_idx to follow the moved tab if it was active
            if *active_tab_idx == old_index {
                *active_tab_idx = insert_idx;
            } else if old_index < *active_tab_idx && insert_idx >= *active_tab_idx {
                *active_tab_idx = active_tab_idx.saturating_sub(1);
            } else if old_index > *active_tab_idx && insert_idx <= *active_tab_idx {
                *active_tab_idx = (*active_tab_idx + 1).min(tabs.len() - 1);
            }
        }
    }

    /// Split the specified pane in the given direction, cloning the current file
    pub fn split_pane(&mut self, pane_id: PaneId, direction: SplitDirection) {
        // Get the active tab content to clone into the new pane
        let cloned_tab = {
            let pane = match self.editor_pane.find_pane(pane_id) {
                Some(p) => p,
                None => return,
            };

            match pane {
                EditorPane::TabGroup { tabs, active_tab_idx, .. } => {
                    tabs.get(*active_tab_idx).map(|tab| EditorTab {
                        id: self.next_tab_id,
                        path: tab.path.clone(),
                        content: tab.content.clone(),
                        is_dirty: tab.is_dirty,
                        cursor_line: 1,
                        cursor_col: 1,
                        history: UndoHistory::default(),
                    })
                }
                EditorPane::Split { .. } => return, // Can't split a Split directly
            }
        };

        let new_tab = cloned_tab.unwrap_or_else(|| EditorTab::new_blank(self.next_tab_id));
        self.next_tab_id += 1;

        let new_pane_id = self.next_pane_id;
        self.next_pane_id += 1;

        // Replace the target pane with a split containing original + new pane
        self.editor_pane = Self::create_split_at(
            std::mem::take(&mut self.editor_pane),
            pane_id,
            direction,
            new_tab,
            new_pane_id,
        );

        // Focus the new pane
        self.focused_pane_id = new_pane_id;
    }

    /// Helper: recursively find and split a pane (standalone function)
    fn create_split_at(
        pane: EditorPane,
        target_id: PaneId,
        direction: SplitDirection,
        new_tab: EditorTab,
        new_pane_id: PaneId,
    ) -> EditorPane {
        match pane {
            EditorPane::TabGroup { id, tabs, active_tab_idx } if id == target_id => {
                // Found the target - create a split
                let original = EditorPane::TabGroup { id, tabs, active_tab_idx };
                let new_pane = EditorPane::TabGroup {
                    id: new_pane_id,
                    tabs: vec![new_tab],
                    active_tab_idx: 0,
                };
                EditorPane::Split {
                    direction,
                    first: Box::new(original),
                    second: Box::new(new_pane),
                    ratio: 0.5,
                }
            }
            EditorPane::TabGroup { .. } => pane, // Not the target, return unchanged
            EditorPane::Split {
                direction: d,
                first,
                second,
                ratio,
            } => {
                // Recurse into children
                EditorPane::Split {
                    direction: d,
                    first: Box::new(Self::create_split_at(*first, target_id, direction, new_tab.clone(), new_pane_id)),
                    second: Box::new(Self::create_split_at(*second, target_id, direction, new_tab, new_pane_id)),
                    ratio,
                }
            }
        }
    }

    /// Close a pane and collapse its parent split
    pub fn close_pane(&mut self, pane_id: PaneId) {
        // Can't close if it's the only pane
        if self.editor_pane.all_pane_ids().len() <= 1 {
            return;
        }

        self.editor_pane = Self::collapse_pane(
            std::mem::take(&mut self.editor_pane),
            pane_id,
        );

        // If focused pane was closed, focus another pane
        if self.editor_pane.find_pane(self.focused_pane_id).is_none() {
            if let Some(first_id) = self.editor_pane.all_pane_ids().first() {
                self.focused_pane_id = *first_id;
            }
        }
    }

    /// Helper: recursively remove a pane and collapse its parent split (standalone)
    fn collapse_pane(pane: EditorPane, target_id: PaneId) -> EditorPane {
        match pane {
            EditorPane::TabGroup { .. } => pane, // Can't collapse a TabGroup
            EditorPane::Split { first, second, direction, ratio } => {
                // Check if first child is the target
                if let EditorPane::TabGroup { id, .. } = first.as_ref() {
                    if *id == target_id {
                        return *second; // Promote second child
                    }
                }
                // Check if second child is the target
                if let EditorPane::TabGroup { id, .. } = second.as_ref() {
                    if *id == target_id {
                        return *first; // Promote first child
                    }
                }
                // Recurse into children
                EditorPane::Split {
                    direction,
                    first: Box::new(Self::collapse_pane(*first, target_id)),
                    second: Box::new(Self::collapse_pane(*second, target_id)),
                    ratio,
                }
            }
        }
    }

    /// Set the split ratio for resizing (finds the split containing the pane)
    pub fn set_split_ratio(&mut self, pane_id: PaneId, new_ratio: f32) {
        let ratio = new_ratio.clamp(0.1, 0.9);
        self.editor_pane = Self::update_split_ratio(
            std::mem::take(&mut self.editor_pane),
            pane_id,
            ratio,
        );
    }

    /// Helper: recursively find and update split ratio (standalone)
    fn update_split_ratio(pane: EditorPane, target_id: PaneId, new_ratio: f32) -> EditorPane {
        match pane {
            EditorPane::TabGroup { .. } => pane,
            EditorPane::Split { direction, first, second, ratio } => {
                // Only update THIS split's ratio if target_id matches first child's first pane
                // This ensures we only resize the exact split, not ancestors
                let is_target_split = first.all_pane_ids().first() == Some(&target_id);

                if is_target_split {
                    // This is the exact split being resized
                    EditorPane::Split {
                        direction,
                        first,
                        second,
                        ratio: new_ratio,
                    }
                } else {
                    // Recurse into children without changing this split's ratio
                    EditorPane::Split {
                        direction,
                        first: Box::new(Self::update_split_ratio(*first, target_id, new_ratio)),
                        second: Box::new(Self::update_split_ratio(*second, target_id, new_ratio)),
                        ratio,
                    }
                }
            }
        }
    }

    /// Update the code in the active tab (records to undo history)
    pub fn set_code(&mut self, code: String) {
        if let Some(tab) = self.active_tab_mut() {
            if tab.content != code {
                // Record the old state to history before changing
                let old_content = tab.content.clone();
                // Convert current line/col to byte offset for undo
                let old_cursor = line_col_to_offset(&old_content, tab.cursor_line, tab.cursor_col);
                tab.history.record_edit(&old_content, old_cursor);

                tab.content = code;
                tab.is_dirty = true;
            }
        }
        self.preview_state.lock().needs_update = true;
    }

    /// Undo the last edit in the active tab, returns (content, cursor_position) if successful
    pub fn undo(&mut self) -> Option<(String, usize)> {
        let focused = self.focused_pane_id;
        if let Some(pane) = self.editor_pane.find_pane_mut(focused) {
            if let Some(tab) = pane.active_tab_mut() {
                let current_content = tab.content.clone();
                let current_cursor =
                    line_col_to_offset(&current_content, tab.cursor_line, tab.cursor_col);

                if let Some(snapshot) = tab.history.undo(&current_content, current_cursor) {
                    tab.content = snapshot.content.clone();
                    tab.is_dirty = true;
                    tab.history.finish_undo_redo();
                    self.preview_state.lock().needs_update = true;
                    return Some((snapshot.content, snapshot.cursor_pos));
                }
            }
        }
        None
    }

    /// Redo the last undone edit in the active tab, returns (content, cursor_position) if successful
    pub fn redo(&mut self) -> Option<(String, usize)> {
        let focused = self.focused_pane_id;
        if let Some(pane) = self.editor_pane.find_pane_mut(focused) {
            if let Some(tab) = pane.active_tab_mut() {
                let current_content = tab.content.clone();
                let current_cursor =
                    line_col_to_offset(&current_content, tab.cursor_line, tab.cursor_col);

                if let Some(snapshot) = tab.history.redo(&current_content, current_cursor) {
                    tab.content = snapshot.content.clone();
                    tab.is_dirty = true;
                    tab.history.finish_undo_redo();
                    self.preview_state.lock().needs_update = true;
                    return Some((snapshot.content, snapshot.cursor_pos));
                }
            }
        }
        None
    }

    /// Mark active tab as saved
    pub fn mark_saved(&mut self, path: Option<PathBuf>) {
        if let Some(tab) = self.active_tab_mut() {
            tab.is_dirty = false;
            if let Some(p) = path {
                tab.path = Some(p);
            }
        }
    }

    /// Set cursor position in active tab
    pub fn set_cursor(&mut self, line: usize, col: usize) {
        if let Some(tab) = self.active_tab_mut() {
            tab.cursor_line = line;
            tab.cursor_col = col;
        }
    }

    /// Run the preview
    pub fn run_preview(&mut self) {
        self.is_previewing = true;

        // Update preview state
        {
            let mut preview = self.preview_state.lock();
            preview.script = self.code();
            preview.needs_update = true;
            preview.should_open = true;
        }

        // Validate the script
        let engine = soyuz_script::ScriptEngine::new();
        match engine.compile(&self.code()) {
            Ok(_) => {
                self.has_error = false;
                self.error_message = None;
            }
            Err(e) => {
                self.has_error = true;
                self.error_message = Some(e.to_string());
            }
        }
    }

    /// Get all tabs (flattened from the pane tree)
    pub fn all_tabs(&self) -> Vec<&EditorTab> {
        self.editor_pane.collect_tabs()
    }

    /// Check if any tab has unsaved changes
    pub fn has_unsaved_changes(&self) -> bool {
        self.all_tabs().iter().any(|t| t.is_dirty)
    }
}

/// State shared with the preview window
#[derive(Default)]
pub struct PreviewState {
    /// Current script to render
    pub script: String,
    /// Whether the script has changed
    pub needs_update: bool,
    /// Whether to open the preview window
    pub should_open: bool,
}

/// Handle to the preview process for stopping it
pub struct PreviewProcess {
    pub child: std::process::Child,
}

impl PreviewProcess {
    pub fn new(child: std::process::Child) -> Self {
        Self { child }
    }

    pub fn kill(&mut self) -> std::io::Result<()> {
        self.child.kill()
    }
}

/// Export settings
#[derive(Clone)]
pub struct ExportSettings {
    /// Output format
    pub format: ExportFormat,
    /// Mesh resolution
    pub resolution: u32,
    /// Texture size for materials (reserved for future use)
    #[allow(dead_code)]
    pub texture_size: u32,
    /// Whether to optimize mesh
    pub optimize: bool,
    /// Whether to generate LODs (reserved for future use)
    #[allow(dead_code)]
    pub generate_lod: bool,
    /// Last used export directory (remembered across sessions)
    pub last_export_dir: Option<PathBuf>,
    /// Whether to close the export window after exporting
    pub close_after_export: bool,
}

impl Default for ExportSettings {
    fn default() -> Self {
        Self {
            format: ExportFormat::Glb,
            resolution: 128,    // Middle of 16-256 slider range
            texture_size: 1024, // Middle of 256-4096 slider range
            optimize: false,
            generate_lod: false,
            last_export_dir: None,
            close_after_export: true,
        }
    }
}

const DEFAULT_SCRIPT: &str = r#"// Welcome to Soyuz Studio!
// Write your SDF script here and click Preview to see it.

// Example: A simple barrel shape
let body = cylinder(0.5, 1.2);
let band_top = torus(0.5, 0.08).translate_y(0.5);
let band_bottom = torus(0.5, 0.08).translate_y(-0.5);

body
    .smooth_union(band_top, 0.05)
    .smooth_union(band_bottom, 0.05)
    .hollow(0.05)
"#;
