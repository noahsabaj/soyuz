//! Application state management
//!
//! This module contains the core application state and related types:
//! - `AppState`: The global application state
//! - `EditorTab`, `EditorPane`: Editor layout structures
//! - `UndoHistory`: Edit history management
//! - `ExportSettings`: Supporting state types

// Separate if statements are clearer for pane traversal logic
#![allow(clippy::collapsible_if)]
// Collapsible match patterns are less readable for pane operations
#![allow(clippy::collapsible_match)]
// clone_from() adds noise for simple string assignments
#![allow(clippy::assigning_clones)]
// map_or is less readable for optional values
#![allow(clippy::map_unwrap_or)]
// Owned PathBuf is intentional for storage
#![allow(clippy::needless_pass_by_value)]

mod editor;
mod export;
mod terminal;
mod undo;

// Re-export all public types
pub use editor::{
    EditorPane, EditorTab, MarkdownContainer, MarkdownDoc, PaneId, SplitDirection, TabId,
};
pub use export::{ExportFormat, ExportSettings};
pub use terminal::{TerminalBuffer, TerminalEntry, TerminalFilter, TerminalLevel};
pub use undo::UndoHistory;

use std::path::PathBuf;

use crate::services::AppServices;
use crate::settings::Settings;
use undo::line_col_to_offset;

/// Reactive application store type.
pub type AppStore = dioxus_stores::Store<AppState>;

/// Modal dialog currently shown above the workbench.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AppDialog {
    /// Product information dialog.
    About,
}

/// Global application state
#[derive(Clone, dioxus_stores::Store)]
pub struct AppState {
    /// Editor pane layout
    pub editor_pane: EditorPane,
    /// Next tab ID to assign
    pub next_tab_id: TabId,
    /// Next pane ID to assign
    pub next_pane_id: PaneId,
    /// Currently focused pane ID
    pub focused_pane_id: PaneId,
    /// Most recent real source tab used by Preview/Export tool tabs
    pub last_source_tab_id: Option<TabId>,
    /// Current workspace folder (None = no folder opened)
    pub workspace: Option<PathBuf>,
    /// Recently opened files (most recent first)
    pub recent_files: Vec<PathBuf>,
    /// Whether preview window is open
    pub is_previewing: bool,
    /// Whether the open preview is stale relative to the editor content
    pub preview_dirty: bool,
    /// Error message if any (None = no error)
    pub error_message: Option<String>,
    /// Export settings
    pub export_settings: ExportSettings,
    /// Application settings
    pub settings: Settings,
    /// Whether the terminal panel is visible
    pub terminal_visible: bool,
    /// Terminal panel height in pixels (for resize persistence)
    pub terminal_height: f32,
    /// Terminal output filter settings
    pub terminal_filter: TerminalFilter,
    /// Modal dialog currently shown above the workbench.
    pub active_dialog: Option<AppDialog>,
    /// Newer release discovered by the startup update check, if any.
    ///
    /// Populated once, asynchronously, at startup by `crate::updater`. `None`
    /// means either up to date or the check has not completed / failed — a
    /// failed check is intentionally indistinguishable so it stays silent.
    pub available_update: Option<crate::updater::UpdateInfo>,
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
            last_source_tab_id: None,
            workspace: None,
            recent_files: Vec::new(),
            is_previewing: false,
            preview_dirty: false,
            error_message: None,
            export_settings: ExportSettings::default(),
            settings,
            terminal_visible: false,
            terminal_height: 200.0,
            terminal_filter: TerminalFilter::default(),
            active_dialog: None,
            available_update: None,
        }
    }

    /// Check if there's an error in the script
    // Retained as public API; the status bar now reads `error_message` via a
    // store field selector for fine-grained reactivity (F73).
    #[allow(dead_code)]
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

    // NOTE: `set_terminal_height` and `toggle_terminal_filter` were removed — the
    // UI now writes these through scoped `dioxus-stores` field selectors
    // (`state.terminal_height().set(..)`, `state.terminal_filter().write().toggle(..)`)
    // so a change doesn't invalidate the whole store (F73).

    /// Open the About dialog.
    pub fn open_about(&mut self) {
        self.active_dialog = Some(AppDialog::About);
    }

    /// Check whether the About dialog is open.
    pub fn is_about_open(&self) -> bool {
        self.active_dialog == Some(AppDialog::About)
    }

    /// Close the About dialog.
    pub fn close_about(&mut self) {
        if self.is_about_open() {
            self.active_dialog = None;
        }
    }

    // ========================================================================
    // Workspace Methods
    // ========================================================================

    /// Check if a workspace folder is currently open
    // Retained as public API; the toolbar now reads `workspace` via a store field
    // selector for fine-grained reactivity (F73).
    #[allow(dead_code)]
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
    pub fn stop_preview(&mut self, services: &AppServices) {
        services.stop_preview_process();
        self.is_previewing = false;
    }

    /// Get the currently active code (from the active tab)
    pub fn code(&self) -> String {
        self.active_tab()
            .map(|t| t.content.clone())
            .unwrap_or_default()
    }

    /// Get the source code Preview/Export should operate on.
    pub fn source_code(&self) -> String {
        self.source_tab()
            .or_else(|| self.tool_snapshot_tab())
            .map(|tab| tab.content.clone())
            .unwrap_or_default()
    }

    /// Get the current file path (from the active tab)
    pub fn current_file(&self) -> Option<PathBuf> {
        self.active_tab().and_then(|t| t.path.clone())
    }

    /// Get the file path for the source script used by Preview/Export.
    pub fn source_file(&self) -> Option<PathBuf> {
        self.source_tab().and_then(|tab| tab.path.clone())
    }

    /// Get cursor position from active tab
    // Retained as public API; the status bar now derives the cursor from the
    // `editor_pane`/`focused_pane_id` field selectors directly (F73).
    #[allow(dead_code)]
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

    /// Get the source tab Preview/Export should use.
    fn source_tab(&self) -> Option<&EditorTab> {
        if let Some(tab) = self.active_tab()
            && tab.is_persistable()
        {
            return Some(tab);
        }

        if let Some(tab_id) = self.last_source_tab_id
            && let Some(tab) = self.editor_pane.find_tab(tab_id)
            && tab.is_persistable()
        {
            return Some(tab);
        }

        self.all_tabs().into_iter().find(|tab| tab.is_persistable())
    }

    /// Get a Preview/Export snapshot when no real source tab remains open.
    fn tool_snapshot_tab(&self) -> Option<&EditorTab> {
        if let Some(tab) = self.active_tab()
            && (tab.is_preview() || tab.is_export())
            && !tab.content.trim().is_empty()
        {
            return Some(tab);
        }

        self.all_tabs()
            .into_iter()
            .find(|tab| (tab.is_preview() || tab.is_export()) && !tab.content.trim().is_empty())
    }

    fn remember_active_source_tab(&mut self) {
        if let Some(tab_id) = self
            .active_tab()
            .filter(|tab| tab.is_persistable())
            .map(|tab| tab.id)
        {
            self.last_source_tab_id = Some(tab_id);
        }
    }

    fn repair_last_source_tab(&mut self) {
        if let Some(tab_id) = self.last_source_tab_id
            && self
                .editor_pane
                .find_tab(tab_id)
                .is_some_and(|tab| tab.is_persistable())
        {
            return;
        }

        self.last_source_tab_id = self
            .active_tab()
            .filter(|tab| tab.is_persistable())
            .map(|tab| tab.id)
            .or_else(|| {
                self.all_tabs()
                    .into_iter()
                    .find(|tab| tab.is_persistable())
                    .map(|tab| tab.id)
            });
    }

    /// Focus a specific pane
    pub fn focus_pane(&mut self, pane_id: PaneId) {
        if self.editor_pane.find_pane(pane_id).is_some() {
            self.focused_pane_id = pane_id;
            self.remember_active_source_tab();
        }
    }

    /// Create a new untitled tab in the focused pane
    pub fn new_tab(&mut self) {
        self.new_tab_in_pane(self.focused_pane_id);
    }

    /// Create a new blank tab in a specific pane
    pub fn new_tab_in_pane(&mut self, pane_id: PaneId) {
        let tab_id = self.next_tab_id;
        let tab = EditorTab::new_blank(tab_id);
        self.next_tab_id += 1;

        if let Some(EditorPane::TabGroup {
            tabs,
            active_tab_idx,
            ..
        }) = self.editor_pane.find_pane_mut(pane_id)
        {
            tabs.push(tab);
            *active_tab_idx = tabs.len() - 1;
            self.last_source_tab_id = Some(tab_id);
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
            tabs,
            active_tab_idx,
            ..
        }) = self.editor_pane.find_pane_mut(self.focused_pane_id)
        {
            tabs.push(tab);
            *active_tab_idx = tabs.len() - 1;
        }
    }

    /// Open the Preview tab (singleton - focuses existing if already open)
    pub fn open_preview_tab(&mut self) {
        let source_code = self.source_code();
        if let Some((pane_id, tab_id)) = self.editor_pane.find_preview_tab() {
            self.focused_pane_id = pane_id;
            self.switch_to_tab(tab_id);
            if let Some(tab) = self.active_tab_mut() {
                tab.content = source_code;
            }
            return;
        }

        let tab = EditorTab::new_preview(self.next_tab_id, source_code);
        self.next_tab_id += 1;

        if let Some(EditorPane::TabGroup {
            tabs,
            active_tab_idx,
            ..
        }) = self.editor_pane.find_pane_mut(self.focused_pane_id)
        {
            tabs.push(tab);
            *active_tab_idx = tabs.len() - 1;
        }
    }

    /// Open the Export tab (singleton - focuses existing if already open)
    pub fn open_export_tab(&mut self) {
        let source_code = self.source_code();
        if let Some((pane_id, tab_id)) = self.editor_pane.find_export_tab() {
            self.focused_pane_id = pane_id;
            self.switch_to_tab(tab_id);
            if let Some(tab) = self.active_tab_mut() {
                tab.content = source_code;
            }
            return;
        }

        let tab = EditorTab::new_export(self.next_tab_id, source_code);
        self.next_tab_id += 1;

        if let Some(EditorPane::TabGroup {
            tabs,
            active_tab_idx,
            ..
        }) = self.editor_pane.find_pane_mut(self.focused_pane_id)
        {
            tabs.push(tab);
            *active_tab_idx = tabs.len() - 1;
        }
    }

    /// Open a markdown documentation tab (singleton - focuses existing if already open)
    pub fn open_markdown(&mut self, doc: MarkdownDoc) {
        if let Some((pane_id, tab_id)) = self.editor_pane.find_markdown_tab(doc) {
            self.focused_pane_id = pane_id;
            self.switch_to_tab(tab_id);
            return;
        }

        let tab = EditorTab::new_markdown(self.next_tab_id, doc);
        self.next_tab_id += 1;

        if let Some(EditorPane::TabGroup {
            tabs,
            active_tab_idx,
            ..
        }) = self.editor_pane.find_pane_mut(self.focused_pane_id)
        {
            tabs.push(tab);
            *active_tab_idx = tabs.len() - 1;
        }
    }

    /// Open the Cookbook tab (convenience method)
    pub fn open_cookbook(&mut self) {
        self.open_markdown(MarkdownDoc::COOKBOOK);
    }

    /// Open the README tab (convenience method)
    pub fn open_readme(&mut self) {
        self.open_markdown(MarkdownDoc::README);
    }

    /// Open the generated documentation browser.
    pub fn open_documentation(&mut self) {
        self.open_markdown(MarkdownDoc::DOCS_INDEX);
    }

    /// Select a generated document inside the active markdown tab.
    pub fn select_active_markdown_doc(&mut self, doc_id: &'static str) {
        if let Some(tab) = self.active_tab_mut() {
            tab.select_markdown_doc(doc_id);
        }
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
            self.remember_active_source_tab();
            return;
        }

        // Create new tab in specified pane
        if let Some(EditorPane::TabGroup {
            tabs,
            active_tab_idx,
            ..
        }) = self.editor_pane.find_pane_mut(pane_id)
        {
            let tab_id = self.next_tab_id;
            let tab = EditorTab::from_file(tab_id, path.clone(), content);
            self.next_tab_id += 1;
            tabs.push(tab);
            *active_tab_idx = tabs.len() - 1;
            self.last_source_tab_id = Some(tab_id);
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
        let limit = self.settings.recent_files_limit.max(1);
        // Remove if already present
        self.recent_files.retain(|p| p != &path);
        // Add to front
        self.recent_files.insert(0, path);
        // Keep only up to the configured limit
        self.recent_files.truncate(limit);
    }

    /// Close a tab by ID in a specific pane
    /// If closing the last tab in a split pane, close the entire pane
    /// If closing the last tab in the root pane, show empty welcome screen (VSCode behavior)
    pub fn close_tab_in_pane(&mut self, pane_id: PaneId, tab_id: TabId) -> bool {
        let closing_last_source = self.last_source_tab_id == Some(tab_id);

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
            tabs,
            active_tab_idx,
            ..
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
        if closing_last_source {
            self.repair_last_source_tab();
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
                tabs,
                active_tab_idx,
                ..
            }) = self.editor_pane.find_pane_mut(pane_id)
            {
                if let Some(idx) = tabs.iter().position(|t| t.id == tab_id) {
                    *active_tab_idx = idx;
                }
            }
            self.remember_active_source_tab();
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
                tabs,
                active_tab_idx,
                ..
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
        let moved_tab_is_source = tab.is_persistable();
        let moved_tab_id = tab.id;
        if let Some(EditorPane::TabGroup {
            tabs,
            active_tab_idx,
            ..
        }) = self.editor_pane.find_pane_mut(target_pane_id)
        {
            let insert_idx = target_index.min(tabs.len());
            tabs.insert(insert_idx, tab);
            *active_tab_idx = insert_idx;
        }

        // Focus target pane
        self.focused_pane_id = target_pane_id;
        if moved_tab_is_source {
            self.last_source_tab_id = Some(moved_tab_id);
        }

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
            tabs,
            active_tab_idx,
            ..
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
                    tabs,
                    active_tab_idx,
                    ..
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
        let new_tab_id = new_tab.id;
        let new_tab_is_source = new_tab.is_persistable();
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
        if new_tab_is_source {
            self.last_source_tab_id = Some(new_tab_id);
        }
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
                        *second,
                        target_id,
                        direction,
                        new_tab,
                        new_pane_id,
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
        self.repair_last_source_tab();
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

    /// Update the code in the active tab (records to undo history).
    ///
    /// The editor's per-keystroke path now writes through the scoped
    /// `EditorPane::set_active_code` selector for fine-grained reactivity (F73);
    /// this whole-`AppState` convenience is retained for the workflow tests.
    #[allow(dead_code)]
    pub fn set_code(&mut self, code: String) {
        let mut edited_source_tab_id = None;
        let mut changed = false;
        let undo_limit = self.settings.undo_history_limit;

        if let Some(tab) = self.active_tab_mut() {
            if tab.content != code {
                // Record the old state to history before changing
                let old_content = tab.content.clone();
                // Convert current line/col to byte offset for undo
                let old_cursor = line_col_to_offset(&old_content, tab.cursor_line, tab.cursor_col);
                tab.history
                    .record_edit(&old_content, old_cursor, undo_limit);

                tab.content = code;
                tab.is_dirty = true;
                changed = true;
                if tab.is_persistable() {
                    edited_source_tab_id = Some(tab.id);
                }
            }
        }

        if changed {
            self.preview_dirty = true;
        }
        if let Some(tab_id) = edited_source_tab_id {
            self.last_source_tab_id = Some(tab_id);
        }
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
                    return Some((snapshot.content, snapshot.cursor_pos));
                }
            }
        }
        None
    }

    /// Mark active tab as saved
    pub fn mark_saved(&mut self, path: Option<PathBuf>) {
        let mut saved_source_tab_id = None;

        if let Some(tab) = self.active_tab_mut() {
            tab.is_dirty = false;
            if let Some(p) = path {
                tab.path = Some(p);
            }
            if tab.is_persistable() {
                saved_source_tab_id = Some(tab.id);
            }
        }

        if let Some(tab_id) = saved_source_tab_id {
            self.last_source_tab_id = Some(tab_id);
        }
    }

    /// Set cursor position in active tab
    pub fn set_cursor(&mut self, line: usize, col: usize) {
        if let Some(tab) = self.active_tab_mut() {
            tab.cursor_line = line;
            tab.cursor_col = col;
        }
    }

    /// Get all tabs (flattened from the pane tree)
    pub fn all_tabs(&self) -> Vec<&EditorTab> {
        self.editor_pane.collect_tabs()
    }

    /// Check if any tab has unsaved changes.
    ///
    /// Gates the autosave loop's periodic session checkpoint. (The status bar
    /// derives dirtiness from the `editor_pane` field selector directly, F73.)
    pub fn has_unsaved_changes(&self) -> bool {
        self.all_tabs().iter().any(|t| t.is_dirty)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn active_tab(state: &AppState) -> &EditorTab {
        let Some(tab) = state.active_tab() else {
            panic!("expected an active tab");
        };
        tab
    }

    #[test]
    fn preview_and_export_use_last_source_tab_when_tool_tab_is_focused() {
        let source_path = PathBuf::from("/tmp/barrel.rhai");
        let source = "sphere(1.0)";
        let mut state = AppState::new();

        state.open_file(source_path.clone(), source.to_string());
        let source_tab_id = active_tab(&state).id;

        state.open_preview_tab();
        assert!(active_tab(&state).is_preview());
        assert_eq!(state.source_code(), source);
        assert_eq!(state.source_file(), Some(source_path.clone()));

        state.open_settings();
        assert!(active_tab(&state).is_settings());
        assert_eq!(state.source_code(), source);
        assert_eq!(state.source_file(), Some(source_path));

        state.open_export_tab();
        assert!(active_tab(&state).is_export());
        assert_eq!(active_tab(&state).content, source);

        state.switch_to_tab(source_tab_id);
        state.set_code("cube(1.0)".to_string());
        state.open_preview_tab();
        assert_eq!(active_tab(&state).content, "cube(1.0)");
        assert_eq!(state.source_code(), "cube(1.0)");
    }

    #[test]
    fn preview_snapshot_remains_runnable_after_source_tab_closes() {
        let source = "sphere(2.0)";
        let mut state = AppState::new();

        state.open_file(PathBuf::from("/tmp/snapshot.rhai"), source.to_string());
        let source_tab_id = active_tab(&state).id;
        state.open_preview_tab();

        state.close_tab_in_pane(1, source_tab_id);

        assert!(active_tab(&state).is_preview());
        assert_eq!(state.source_file(), None);
        assert_eq!(state.source_code(), source);
    }

    #[test]
    fn about_dialog_visibility_opens_and_closes() {
        let mut state = AppState::new();

        assert!(!state.is_about_open());
        state.open_about();
        assert!(state.is_about_open());
        state.close_about();
        assert!(!state.is_about_open());
    }

    #[test]
    fn markdown_help_docs_are_three_container_tabs_with_local_sidebar_state() {
        let mut state = AppState::new();

        state.open_cookbook();
        let cookbook_tab_id = active_tab(&state).id;
        assert_eq!(active_tab(&state).display_name(), "Cookbook");
        assert_eq!(
            active_tab(&state).markdown_doc(),
            Some(MarkdownDoc::COOKBOOK)
        );

        state.select_active_markdown_doc("cookbook/patterns");
        assert_eq!(active_tab(&state).id, cookbook_tab_id);
        assert_eq!(state.all_tabs().len(), 1);
        assert_eq!(
            active_tab(&state).markdown_doc(),
            Some(MarkdownDoc {
                selected_id: "cookbook/patterns",
                ..MarkdownDoc::COOKBOOK
            })
        );

        state.select_active_markdown_doc("getting-started/installation");
        assert_eq!(
            active_tab(&state).markdown_doc(),
            Some(MarkdownDoc {
                selected_id: "cookbook/patterns",
                ..MarkdownDoc::COOKBOOK
            })
        );

        state.open_readme();
        let readme_tab_id = active_tab(&state).id;
        assert_ne!(readme_tab_id, cookbook_tab_id);
        assert_eq!(active_tab(&state).display_name(), "README");
        assert_eq!(active_tab(&state).markdown_doc(), Some(MarkdownDoc::README));
        assert_eq!(state.all_tabs().len(), 2);

        state.select_active_markdown_doc("cookbook/tips");
        assert_eq!(active_tab(&state).markdown_doc(), Some(MarkdownDoc::README));

        state.open_cookbook();
        assert_eq!(active_tab(&state).id, cookbook_tab_id);
        assert_eq!(active_tab(&state).display_name(), "Cookbook");
        assert_eq!(
            active_tab(&state).markdown_doc(),
            Some(MarkdownDoc {
                selected_id: "cookbook/patterns",
                ..MarkdownDoc::COOKBOOK
            })
        );
        assert_eq!(state.all_tabs().len(), 2);

        state.open_documentation();
        let docs_tab_id = active_tab(&state).id;
        assert_eq!(active_tab(&state).display_name(), "Documentation");
        assert_eq!(
            active_tab(&state).markdown_doc(),
            Some(MarkdownDoc::DOCS_INDEX)
        );
        assert_eq!(state.all_tabs().len(), 3);

        state.select_active_markdown_doc("getting-started/installation");
        assert_eq!(active_tab(&state).id, docs_tab_id);
        assert_eq!(state.all_tabs().len(), 3);
        assert_eq!(
            active_tab(&state).markdown_doc(),
            Some(MarkdownDoc {
                selected_id: "getting-started/installation",
                ..MarkdownDoc::DOCS_INDEX
            })
        );

        state.select_active_markdown_doc("cookbook/tips");
        assert_eq!(
            active_tab(&state).markdown_doc(),
            Some(MarkdownDoc {
                selected_id: "getting-started/installation",
                ..MarkdownDoc::DOCS_INDEX
            })
        );

        state.open_documentation();
        assert_eq!(active_tab(&state).id, docs_tab_id);
        assert_eq!(state.all_tabs().len(), 3);
    }
}
