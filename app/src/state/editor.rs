//! Editor types for tabs and panes
//!
//! Contains the core data structures for the editor layout:
//! - `EditorTab`: A single editor tab with content, path, and history
//! - `EditorPane`: A recursive tree structure for split pane layouts
//! - Type aliases and enums for tab/pane identification

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::undo::UndoHistory;

/// Unique identifier for editor tabs
pub type TabId = u64;

/// The kind of tab - determines rendering and behavior
#[derive(Clone, PartialEq, Default)]
pub enum TabKind {
    /// Regular file or untitled document
    #[default]
    File,
    /// Settings panel (singleton)
    Settings,
    /// Embedded interactive preview (singleton)
    Preview,
    /// Embedded export panel (singleton)
    Export,
    /// Embedded markdown documentation (singleton per container)
    Markdown(MarkdownDoc),
}

/// Top-level embedded markdown tab containers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkdownContainer {
    /// General documentation tab.
    Documentation,
    /// Cookbook tab.
    Cookbook,
    /// Project README tab.
    Readme,
}

/// State for an embedded markdown tab.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MarkdownDoc {
    /// Top-level tab container.
    pub container: MarkdownContainer,
    /// Selected generated document id inside the container.
    pub selected_id: &'static str,
}

impl MarkdownDoc {
    /// Documentation tab.
    pub const DOCS_INDEX: Self = Self {
        container: MarkdownContainer::Documentation,
        selected_id: "index",
    };
    /// Cookbook tab.
    pub const COOKBOOK: Self = Self {
        container: MarkdownContainer::Cookbook,
        selected_id: "cookbook/recipes-overview",
    };
    /// Project README tab.
    pub const README: Self = Self {
        container: MarkdownContainer::Readme,
        selected_id: "readme",
    };

    /// Get display name for the tab
    pub fn display_name(self) -> &'static str {
        match self.container {
            MarkdownContainer::Documentation => "Documentation",
            MarkdownContainer::Cookbook => "Cookbook",
            MarkdownContainer::Readme => {
                crate::docs_generated::doc_title(self.selected_id).unwrap_or("README")
            }
        }
    }

    /// Check whether the generated document belongs in this tab container.
    pub fn can_select(self, doc_id: &str) -> bool {
        crate::docs_generated::doc_by_id(doc_id).is_some_and(|doc| match self.container {
            MarkdownContainer::Documentation => !matches!(doc.section, "Cookbook" | "Project"),
            MarkdownContainer::Cookbook => doc.section == "Cookbook",
            MarkdownContainer::Readme => doc.id == Self::README.selected_id,
        })
    }

    /// Return a copy with a new selected generated document if it belongs here.
    pub fn with_selected_id(self, doc_id: &'static str) -> Self {
        if self.can_select(doc_id) {
            Self {
                selected_id: doc_id,
                ..self
            }
        } else {
            self
        }
    }

    /// Check whether two markdown tabs belong to the same top-level container.
    pub fn same_container(self, other: Self) -> bool {
        self.container == other.container
    }
}

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

/// A single editor tab
#[derive(Clone, PartialEq)]
pub struct EditorTab {
    /// Unique identifier
    pub id: TabId,
    /// The kind of tab (File or Settings)
    pub kind: TabKind,
    /// File path (None if untitled or Settings)
    pub path: Option<PathBuf>,
    /// Content of the file (empty for Settings tabs)
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
    /// Create a new blank tab (empty content)
    pub fn new_blank(id: TabId) -> Self {
        Self {
            id,
            kind: TabKind::File,
            path: None,
            content: String::new(),
            is_dirty: false,
            cursor_line: 1,
            cursor_col: 1,
            history: UndoHistory::default(),
        }
    }

    /// Create a Settings tab
    pub fn new_settings(id: TabId) -> Self {
        Self {
            id,
            kind: TabKind::Settings,
            path: None,
            content: String::new(),
            is_dirty: false,
            cursor_line: 1,
            cursor_col: 1,
            history: UndoHistory::default(),
        }
    }

    /// Create a Preview tab
    pub fn new_preview(id: TabId, source_code: String) -> Self {
        Self {
            id,
            kind: TabKind::Preview,
            path: None,
            content: source_code,
            is_dirty: false,
            cursor_line: 1,
            cursor_col: 1,
            history: UndoHistory::default(),
        }
    }

    /// Create an Export tab
    pub fn new_export(id: TabId, source_code: String) -> Self {
        Self {
            id,
            kind: TabKind::Export,
            path: None,
            content: source_code,
            is_dirty: false,
            cursor_line: 1,
            cursor_col: 1,
            history: UndoHistory::default(),
        }
    }

    /// Create a Markdown documentation tab
    pub fn new_markdown(id: TabId, doc: MarkdownDoc) -> Self {
        Self {
            id,
            kind: TabKind::Markdown(doc),
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
            kind: TabKind::File,
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
            kind: TabKind::File,
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
        match &self.kind {
            TabKind::Settings => "Settings".to_string(),
            TabKind::Preview => "Preview".to_string(),
            TabKind::Export => "Export".to_string(),
            TabKind::Markdown(doc) => doc.display_name().to_string(),
            TabKind::File => self
                .path
                .as_ref()
                .and_then(|p| p.file_name())
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "Untitled".to_string()),
        }
    }

    /// Check if this is a Settings tab
    pub fn is_settings(&self) -> bool {
        self.kind == TabKind::Settings
    }

    /// Check if this is a Preview tab
    pub fn is_preview(&self) -> bool {
        self.kind == TabKind::Preview
    }

    /// Check if this is an Export tab
    pub fn is_export(&self) -> bool {
        self.kind == TabKind::Export
    }

    /// Check if this tab should be restored across sessions
    pub fn is_persistable(&self) -> bool {
        matches!(self.kind, TabKind::File)
    }

    /// Check if this is a markdown documentation tab
    pub fn is_markdown(&self) -> bool {
        matches!(self.kind, TabKind::Markdown(_))
    }

    /// Get the markdown doc type if this is a markdown tab
    pub fn markdown_doc(&self) -> Option<MarkdownDoc> {
        match self.kind {
            TabKind::Markdown(doc) => Some(doc),
            _ => None,
        }
    }

    /// Select a generated markdown document inside this tab.
    pub fn select_markdown_doc(&mut self, doc_id: &'static str) {
        if let TabKind::Markdown(doc) = &mut self.kind {
            *doc = doc.with_selected_id(doc_id);
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
        // Start with no tabs - shows welcome screen (VSCode behavior)
        EditorPane::TabGroup {
            id: 1,
            tabs: Vec::new(),
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
            EditorPane::Split { first, second, .. } => first
                .find_pane(pane_id)
                .or_else(|| second.find_pane(pane_id)),
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
            EditorPane::Split { first, second, .. } => first
                .find_tab_by_path(path)
                .or_else(|| second.find_tab_by_path(path)),
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
            EditorPane::Split { first, second, .. } => first
                .find_pane_containing_tab(tab_id)
                .or_else(|| second.find_pane_containing_tab(tab_id)),
        }
    }

    /// Find a tab by ID (recursive)
    pub fn find_tab(&self, tab_id: TabId) -> Option<&EditorTab> {
        match self {
            EditorPane::TabGroup { tabs, .. } => tabs.iter().find(|tab| tab.id == tab_id),
            EditorPane::Split { first, second, .. } => {
                first.find_tab(tab_id).or_else(|| second.find_tab(tab_id))
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

    /// Find the Settings tab if it exists (returns pane_id, tab_id)
    pub fn find_settings_tab(&self) -> Option<(PaneId, TabId)> {
        match self {
            EditorPane::TabGroup { id, tabs, .. } => {
                for tab in tabs {
                    if tab.is_settings() {
                        return Some((*id, tab.id));
                    }
                }
                None
            }
            EditorPane::Split { first, second, .. } => first
                .find_settings_tab()
                .or_else(|| second.find_settings_tab()),
        }
    }

    /// Find the Preview tab if it exists (returns pane_id, tab_id)
    pub fn find_preview_tab(&self) -> Option<(PaneId, TabId)> {
        match self {
            EditorPane::TabGroup { id, tabs, .. } => {
                for tab in tabs {
                    if tab.is_preview() {
                        return Some((*id, tab.id));
                    }
                }
                None
            }
            EditorPane::Split { first, second, .. } => first
                .find_preview_tab()
                .or_else(|| second.find_preview_tab()),
        }
    }

    /// Find the Export tab if it exists (returns pane_id, tab_id)
    pub fn find_export_tab(&self) -> Option<(PaneId, TabId)> {
        match self {
            EditorPane::TabGroup { id, tabs, .. } => {
                for tab in tabs {
                    if tab.is_export() {
                        return Some((*id, tab.id));
                    }
                }
                None
            }
            EditorPane::Split { first, second, .. } => {
                first.find_export_tab().or_else(|| second.find_export_tab())
            }
        }
    }

    /// Find a markdown documentation tab by container (returns pane_id, tab_id)
    pub fn find_markdown_tab(&self, doc: MarkdownDoc) -> Option<(PaneId, TabId)> {
        match self {
            EditorPane::TabGroup { id, tabs, .. } => {
                for tab in tabs {
                    if tab
                        .markdown_doc()
                        .is_some_and(|existing| existing.same_container(doc))
                    {
                        return Some((*id, tab.id));
                    }
                }
                None
            }
            EditorPane::Split { first, second, .. } => first
                .find_markdown_tab(doc)
                .or_else(|| second.find_markdown_tab(doc)),
        }
    }
}
