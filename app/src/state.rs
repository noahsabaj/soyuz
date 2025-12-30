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

/// Direction of a pane split
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

/// A pane in the editor layout - either a single tab group or a split
#[derive(Clone, PartialEq)]
pub enum EditorPane {
    /// A single tab group with tabs
    TabGroup {
        /// Unique ID for this pane
        id: PaneId,
        tabs: Vec<EditorTab>,
        active_tab_idx: usize,
    },
    /// A split between two panes
    Split {
        direction: SplitDirection,
        first: Box<EditorPane>,
        second: Box<EditorPane>,
        /// Ratio of first pane (0.0 to 1.0)
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
    /// Get the pane ID (for TabGroups only)
    pub fn id(&self) -> Option<PaneId> {
        match self {
            EditorPane::TabGroup { id, .. } => Some(*id),
            EditorPane::Split { .. } => None,
        }
    }

    /// Get the active tab in this pane (if it's a tab group)
    pub fn active_tab(&self) -> Option<&EditorTab> {
        match self {
            EditorPane::TabGroup {
                tabs,
                active_tab_idx,
                ..
            } => tabs.get(*active_tab_idx),
            EditorPane::Split { .. } => None,
        }
    }

    /// Get mutable active tab
    pub fn active_tab_mut(&mut self) -> Option<&mut EditorTab> {
        match self {
            EditorPane::TabGroup {
                tabs,
                active_tab_idx,
                ..
            } => tabs.get_mut(*active_tab_idx),
            EditorPane::Split { .. } => None,
        }
    }

    /// Find a pane by ID and return a reference
    pub fn find_pane(&self, pane_id: PaneId) -> Option<&EditorPane> {
        match self {
            EditorPane::TabGroup { id, .. } if *id == pane_id => Some(self),
            EditorPane::TabGroup { .. } => None,
            EditorPane::Split { first, second, .. } => first
                .find_pane(pane_id)
                .or_else(|| second.find_pane(pane_id)),
        }
    }

    /// Find a pane by ID and return a mutable reference
    pub fn find_pane_mut(&mut self, pane_id: PaneId) -> Option<&mut EditorPane> {
        match self {
            EditorPane::TabGroup { id, .. } if *id == pane_id => Some(self),
            EditorPane::TabGroup { .. } => None,
            EditorPane::Split { first, second, .. } => {
                if first.find_pane(pane_id).is_some() {
                    first.find_pane_mut(pane_id)
                } else {
                    second.find_pane_mut(pane_id)
                }
            }
        }
    }

    /// Get all pane IDs
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

    /// Find a tab by its file path, returns (pane_id, tab_index)
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
            EditorPane::Split { first, second, .. } => first
                .find_tab_by_path(path)
                .or_else(|| second.find_tab_by_path(path)),
        }
    }

    /// Find which pane contains a tab by ID
    pub fn find_pane_containing_tab(&self, tab_id: TabId) -> Option<PaneId> {
        match self {
            EditorPane::TabGroup { id, tabs, .. } => {
                if tabs.iter().any(|t| t.id == tab_id) {
                    Some(*id)
                } else {
                    None
                }
            }
            EditorPane::Split { first, second, .. } => first
                .find_pane_containing_tab(tab_id)
                .or_else(|| second.find_pane_containing_tab(tab_id)),
        }
    }

    /// Collect all tabs from the pane tree
    pub fn collect_tabs(&self) -> Vec<&EditorTab> {
        let mut tabs = Vec::new();
        self.collect_tabs_into(&mut tabs);
        tabs
    }

    fn collect_tabs_into<'a>(&'a self, tabs: &mut Vec<&'a EditorTab>) {
        match self {
            EditorPane::TabGroup {
                tabs: pane_tabs, ..
            } => {
                tabs.extend(pane_tabs.iter());
            }
            EditorPane::Split { first, second, .. } => {
                first.collect_tabs_into(tabs);
                second.collect_tabs_into(tabs);
            }
        }
    }

    /// Split this pane at a specific location
    pub fn split_at(
        self,
        target_id: PaneId,
        direction: SplitDirection,
        new_pane_id: PaneId,
        new_tab: EditorTab,
    ) -> Self {
        match self {
            EditorPane::TabGroup {
                id,
                tabs,
                active_tab_idx,
            } if id == target_id => {
                // This is the pane to split
                let original_pane = EditorPane::TabGroup {
                    id,
                    tabs,
                    active_tab_idx,
                };
                let new_pane = EditorPane::TabGroup {
                    id: new_pane_id,
                    tabs: vec![new_tab],
                    active_tab_idx: 0,
                };
                EditorPane::Split {
                    direction,
                    first: Box::new(original_pane),
                    second: Box::new(new_pane),
                    ratio: 0.5,
                }
            }
            EditorPane::TabGroup { .. } => self,
            EditorPane::Split {
                direction: dir,
                first,
                second,
                ratio,
            } => EditorPane::Split {
                direction: dir,
                first: Box::new(first.split_at(target_id, direction, new_pane_id, new_tab.clone())),
                second: Box::new(second.split_at(target_id, direction, new_pane_id, new_tab)),
                ratio,
            },
        }
    }

    /// Remove a pane, collapsing the split
    pub fn remove_pane(self, target_id: PaneId) -> Self {
        match self {
            EditorPane::TabGroup { .. } => self,
            EditorPane::Split {
                direction,
                first,
                second,
                ratio,
            } => {
                // Check if one of the children is the target
                if let Some(first_id) = first.id() {
                    if first_id == target_id {
                        return *second;
                    }
                }
                if let Some(second_id) = second.id() {
                    if second_id == target_id {
                        return *first;
                    }
                }
                // Recurse
                EditorPane::Split {
                    direction,
                    first: Box::new(first.remove_pane(target_id)),
                    second: Box::new(second.remove_pane(target_id)),
                    ratio,
                }
            }
        }
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
    /// Current working directory for asset browser
    pub working_dir: PathBuf,
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
        let working_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

        Self {
            editor_pane: EditorPane::default(),
            next_tab_id: 2,  // 1 is used by the default tab
            next_pane_id: 2, // 1 is used by the default pane
            focused_pane_id: 1,
            working_dir,
            is_previewing: false,
            has_error: false,
            error_message: None,
            export_settings: ExportSettings::default(),
            preview_state: Arc::new(Mutex::new(PreviewState::default())),
            preview_process: Arc::new(Mutex::new(None)),
        }
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

        if let Some(pane) = self.editor_pane.find_pane_mut(pane_id) {
            if let EditorPane::TabGroup {
                tabs,
                active_tab_idx,
                ..
            } = pane
            {
                tabs.push(tab);
                *active_tab_idx = tabs.len() - 1;
            }
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
            if let Some(pane) = self.editor_pane.find_pane_mut(found_pane_id) {
                if let EditorPane::TabGroup { active_tab_idx, .. } = pane {
                    *active_tab_idx = tab_idx;
                }
            }
            return;
        }

        // Create new tab in specified pane
        if let Some(pane) = self.editor_pane.find_pane_mut(pane_id) {
            if let EditorPane::TabGroup {
                tabs,
                active_tab_idx,
                ..
            } = pane
            {
                let tab = EditorTab::from_file(self.next_tab_id, path.clone(), content);
                self.next_tab_id += 1;
                tabs.push(tab);
                *active_tab_idx = tabs.len() - 1;
            }
        }

        // Update working directory
        if let Some(parent) = path.parent() {
            self.working_dir = parent.to_path_buf();
        }

        self.has_error = false;
        self.error_message = None;
    }

    /// Close a tab by ID in a specific pane
    pub fn close_tab_in_pane(&mut self, pane_id: PaneId, tab_id: TabId) -> bool {
        if let Some(pane) = self.editor_pane.find_pane_mut(pane_id) {
            if let EditorPane::TabGroup {
                tabs,
                active_tab_idx,
                ..
            } = pane
            {
                if let Some(idx) = tabs.iter().position(|t| t.id == tab_id) {
                    // Don't close if it's the last tab - create a new untitled instead
                    if tabs.len() == 1 {
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
        }
        true
    }

    /// Switch to a tab by ID in any pane (finds the pane containing the tab)
    pub fn switch_to_tab(&mut self, tab_id: TabId) {
        if let Some(pane_id) = self.editor_pane.find_pane_containing_tab(tab_id) {
            self.focused_pane_id = pane_id;
            if let Some(pane) = self.editor_pane.find_pane_mut(pane_id) {
                if let EditorPane::TabGroup {
                    tabs,
                    active_tab_idx,
                    ..
                } = pane
                {
                    if let Some(idx) = tabs.iter().position(|t| t.id == tab_id) {
                        *active_tab_idx = idx;
                    }
                }
            }
        }
    }

    /// Split the focused pane in a given direction
    #[allow(dead_code)]
    pub fn split_pane(&mut self, direction: SplitDirection) {
        self.split_pane_by_id(self.focused_pane_id, direction);
    }

    /// Split a specific pane
    pub fn split_pane_by_id(&mut self, pane_id: PaneId, direction: SplitDirection) {
        let new_pane_id = self.next_pane_id;
        self.next_pane_id += 1;

        // Create a new empty tab for the second pane
        let new_tab = EditorTab::new_untitled(self.next_tab_id);
        self.next_tab_id += 1;

        self.editor_pane = std::mem::take(&mut self.editor_pane).split_at(
            pane_id,
            direction,
            new_pane_id,
            new_tab,
        );

        // Focus the new pane
        self.focused_pane_id = new_pane_id;
    }

    /// Close a pane (unsplit) - moves tabs to sibling or removes empty
    pub fn close_pane(&mut self, pane_id: PaneId) {
        // Can't close if it's the only pane
        if self.editor_pane.all_pane_ids().len() <= 1 {
            return;
        }

        self.editor_pane = std::mem::take(&mut self.editor_pane).remove_pane(pane_id);

        // If focused pane was closed, focus the first available pane
        if self.editor_pane.find_pane(self.focused_pane_id).is_none() {
            if let Some(first_id) = self.editor_pane.all_pane_ids().first() {
                self.focused_pane_id = *first_id;
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
    /// Texture size for materials
    pub texture_size: u32,
    /// Whether to optimize mesh
    pub optimize: bool,
    /// Whether to generate LODs
    pub generate_lod: bool,
}

impl Default for ExportSettings {
    fn default() -> Self {
        Self {
            format: ExportFormat::Glb,
            resolution: 128,    // Middle of 16-256 slider range
            texture_size: 1024, // Middle of 256-4096 slider range
            optimize: false,
            generate_lod: false,
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
