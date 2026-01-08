//! Application state management
//!
//! This module contains the core application state and related types:
//! - `AppState`: The global application state
//! - `EditorTab`, `EditorPane`: Editor layout structures
//! - `UndoHistory`: Edit history management
//! - `ExportSettings`, `PreviewState`: Supporting state types

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

mod editor;
mod export;
mod preview;
mod terminal;
mod undo;

// Re-export all public types
pub use editor::{EditorPane, EditorTab, MarkdownDoc, PaneId, SplitDirection, TabId};
pub use export::{ExportFormat, ExportSettings};
pub use preview::PreviewState;
pub use terminal::{TerminalBuffer, TerminalEntry, TerminalFilter, TerminalLevel};
pub use undo::UndoHistory;

use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::warn;

use crate::settings::Settings;
use undo::line_col_to_offset;

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
    /// Error message if any (None = no error)
    pub error_message: Option<String>,
    /// Export settings
    pub export_settings: ExportSettings,
    /// Application settings
    pub settings: Settings,
    /// Shared state for preview communication
    pub preview_state: Arc<Mutex<PreviewState>>,
    /// Handle to the preview process (for stopping it)
    pub preview_process: Arc<Mutex<Option<std::process::Child>>>,
    /// Terminal output buffer (shared with tracing subscriber)
    pub terminal_buffer: TerminalBuffer,
    /// Whether the terminal panel is visible
    pub terminal_visible: bool,
    /// Terminal panel height in pixels (for resize persistence)
    pub terminal_height: f32,
    /// Terminal output filter settings
    pub terminal_filter: TerminalFilter,
}

impl AppState {
    /// Create a new AppState with default settings
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self::with_settings(Settings::default())
    }

    /// Create with loaded settings
    pub fn with_settings(settings: Settings) -> Self {
        Self {
            editor_pane: EditorPane::default(),
            next_tab_id: 2,
            next_pane_id: 2,
            focused_pane_id: 1,
            workspace: None,
            recent_files: Vec::new(),
            is_previewing: false,
            error_message: None,
            export_settings: ExportSettings::default(),
            settings,
            preview_state: Arc::new(Mutex::new(PreviewState::default())),
            preview_process: Arc::new(Mutex::new(None)),
            terminal_buffer: TerminalBuffer::new(),
            terminal_visible: false,
            terminal_height: 200.0,
            terminal_filter: TerminalFilter::default(),
        }
    }

    /// Check if there's an error in the script
    pub fn has_error(&self) -> bool {
        self.error_message.is_some()
    }

    // ========================================================================
    // Terminal Methods
    // ========================================================================

    /// Toggle terminal panel visibility
    pub fn toggle_terminal(&mut self) {
        self.terminal_visible = !self.terminal_visible;
    }

    /// Set terminal panel height (clamped between 100-500px)
    pub fn set_terminal_height(&mut self, height: f32) {
        self.terminal_height = height.clamp(100.0, 500.0);
    }

    /// Add a message to the terminal buffer
    pub fn terminal_log(&self, level: TerminalLevel, message: impl Into<String>) {
        self.terminal_buffer
            .push(TerminalEntry::new(level, message));
    }

    /// Clear the terminal buffer
    pub fn terminal_clear(&self) {
        self.terminal_buffer.clear();
    }

    /// Toggle a terminal filter level
    pub fn toggle_terminal_filter(&mut self, level: TerminalLevel) {
        self.terminal_filter.toggle(level);
    }

    // ========================================================================
    // Workspace Methods
    // ========================================================================

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
            if let Err(e) = process.kill() {
                warn!("Failed to kill preview process: {e}");
            }
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

        if let Some(EditorPane::TabGroup {
            tabs, active_tab_idx, ..
        }) = self.editor_pane.find_pane_mut(pane_id)
        {
            tabs.push(tab);
            *active_tab_idx = tabs.len() - 1;
        }

        self.error_message = None;
    }

    /// Open the Settings tab (singleton - focuses existing if already open)
    pub fn open_settings(&mut self) {
        // Check if Settings tab already exists anywhere
        if let Some((pane_id, tab_id)) = self.editor_pane.find_settings_tab() {
            // Focus the existing Settings tab
            self.focused_pane_id = pane_id;
            self.switch_to_tab(tab_id);
            return;
        }

        // Create a new Settings tab in the focused pane
        let tab = EditorTab::new_settings(self.next_tab_id);
        self.next_tab_id += 1;

        if let Some(EditorPane::TabGroup {
            tabs, active_tab_idx, ..
        }) = self.editor_pane.find_pane_mut(self.focused_pane_id)
        {
            tabs.push(tab);
            *active_tab_idx = tabs.len() - 1;
        }
    }

    /// Open a markdown documentation tab (singleton - focuses existing if already open)
    pub fn open_markdown(&mut self, doc: MarkdownDoc) {
        // Check if this markdown tab already exists anywhere
        if let Some((pane_id, tab_id)) = self.editor_pane.find_markdown_tab(doc) {
            // Focus the existing tab
            self.focused_pane_id = pane_id;
            self.switch_to_tab(tab_id);
            return;
        }

        // Create a new markdown tab in the focused pane
        let tab = EditorTab::new_markdown(self.next_tab_id, doc);
        self.next_tab_id += 1;

        if let Some(EditorPane::TabGroup {
            tabs, active_tab_idx, ..
        }) = self.editor_pane.find_pane_mut(self.focused_pane_id)
        {
            tabs.push(tab);
            *active_tab_idx = tabs.len() - 1;
        }
    }

    /// Open the Cookbook tab (convenience method)
    pub fn open_cookbook(&mut self) {
        self.open_markdown(MarkdownDoc::Cookbook);
    }

    /// Open the README tab (convenience method)
    pub fn open_readme(&mut self) {
        self.open_markdown(MarkdownDoc::Readme);
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
        if let Some(EditorPane::TabGroup {
            tabs, active_tab_idx, ..
        }) = self.editor_pane.find_pane_mut(pane_id)
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
    /// If closing the last tab in a split pane, close the entire pane
    /// If closing the last tab in the root pane, show empty welcome screen (VSCode behavior)
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
        } else if let Some(EditorPane::TabGroup {
            tabs, active_tab_idx, ..
        }) = self.editor_pane.find_pane_mut(pane_id)
        {
            if let Some(idx) = tabs.iter().position(|t| t.id == tab_id) {
                tabs.remove(idx);
                // Adjust active_tab_idx if needed
                if tabs.is_empty() {
                    *active_tab_idx = 0;
                } else if *active_tab_idx >= tabs.len() {
                    *active_tab_idx = tabs.len() - 1;
                } else if *active_tab_idx > idx {
                    *active_tab_idx -= 1;
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
            if let Some(EditorPane::TabGroup {
                tabs, active_tab_idx, ..
            }) = self.editor_pane.find_pane_mut(pane_id)
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
        let Some(source_pane_id) = self.editor_pane.find_pane_containing_tab(tab_id) else {
            return;
        };

        // Check if moving within the same pane (reorder)
        if source_pane_id == target_pane_id {
            // Find current index and reorder
            if let Some(EditorPane::TabGroup { tabs, .. }) =
                self.editor_pane.find_pane(source_pane_id)
            {
                if let Some(old_idx) = tabs.iter().position(|t| t.id == tab_id) {
                    self.reorder_tab(source_pane_id, old_idx, target_index);
                }
            }
            return;
        }

        // Remove tab from source pane
        let tab = {
            let Some(pane) = self.editor_pane.find_pane_mut(source_pane_id) else {
                return;
            };
            if let EditorPane::TabGroup {
                tabs, active_tab_idx, ..
            } = pane
            {
                let Some(idx) = tabs.iter().position(|t| t.id == tab_id) else {
                    return;
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
            if let Some(EditorPane::TabGroup { tabs, .. }) =
                self.editor_pane.find_pane(source_pane_id)
            {
                tabs.is_empty()
            } else {
                false
            }
        };
        let is_root = self.editor_pane.is_single_pane();

        // Insert tab into target pane
        if let Some(EditorPane::TabGroup {
            tabs, active_tab_idx, ..
        }) = self.editor_pane.find_pane_mut(target_pane_id)
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

        if let Some(EditorPane::TabGroup {
            tabs, active_tab_idx, ..
        }) = self.editor_pane.find_pane_mut(pane_id)
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
            let Some(pane) = self.editor_pane.find_pane(pane_id) else {
                return;
            };

            match pane {
                EditorPane::TabGroup {
                    tabs, active_tab_idx, ..
                } => tabs.get(*active_tab_idx).map(|tab| EditorTab {
                    id: self.next_tab_id,
                    kind: tab.kind.clone(),
                    path: tab.path.clone(),
                    content: tab.content.clone(),
                    is_dirty: tab.is_dirty,
                    cursor_line: 1,
                    cursor_col: 1,
                    history: UndoHistory::default(),
                }),
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
            EditorPane::TabGroup {
                id,
                tabs,
                active_tab_idx,
            } if id == target_id => {
                // Found the target - create a split
                let original = EditorPane::TabGroup {
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
                    first: Box::new(Self::create_split_at(
                        *first,
                        target_id,
                        direction,
                        new_tab.clone(),
                        new_pane_id,
                    )),
                    second: Box::new(Self::create_split_at(
                        *second, target_id, direction, new_tab, new_pane_id,
                    )),
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

        self.editor_pane = Self::collapse_pane(std::mem::take(&mut self.editor_pane), pane_id);

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
            EditorPane::Split {
                first,
                second,
                direction,
                ratio,
            } => {
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
        self.editor_pane =
            Self::update_split_ratio(std::mem::take(&mut self.editor_pane), pane_id, ratio);
    }

    /// Helper: recursively find and update split ratio (standalone)
    fn update_split_ratio(pane: EditorPane, target_id: PaneId, new_ratio: f32) -> EditorPane {
        match pane {
            EditorPane::TabGroup { .. } => pane,
            EditorPane::Split {
                direction,
                first,
                second,
                ratio,
            } => {
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
                self.error_message = None;
            }
            Err(e) => {
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
