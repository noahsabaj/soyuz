//! Session persistence - save and restore open tabs across sessions

// Separate if statements are clearer for path validation
#![allow(clippy::collapsible_if)]

use crate::state::{AppState, EditorPane, EditorTab, PaneId, SplitDirection, UndoHistory};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Load tab content from stored content or file path
///
/// Priority:
/// 1. Use stored content if available
/// 2. Read from file path if stored content is missing
/// 3. Return default (empty string) if neither is available
fn load_tab_content(path: Option<&PathBuf>, stored_content: Option<&String>) -> String {
    match (path, stored_content) {
        // Stored content takes priority
        (_, Some(content)) => content.clone(),
        // No stored content, try to read from file
        (Some(p), None) => std::fs::read_to_string(p)
            .unwrap_or_else(|_| format!("// Error: Could not load file: {}", p.display())),
        // No path and no stored content
        (None, None) => String::new(),
    }
}

/// Serializable pane state (recursive tree structure)
#[derive(Serialize, Deserialize)]
pub enum PaneSession {
    TabGroup {
        id: u64,
        tabs: Vec<TabSession>,
        active_tab_idx: usize,
    },
    Split {
        direction: SplitDirection,
        first: Box<PaneSession>,
        second: Box<PaneSession>,
        ratio: f32,
    },
}

/// Serializable session state
#[derive(Serialize, Deserialize, Default)]
pub struct Session {
    /// Pane layout (new: full pane tree)
    #[serde(default)]
    pub pane_layout: Option<PaneSession>,
    /// Focused pane ID
    #[serde(default = "default_focused_pane")]
    pub focused_pane_id: u64,
    /// Legacy: flat tabs list (for backward compatibility)
    #[serde(default)]
    pub tabs: Vec<TabSession>,
    /// Legacy: active tab index
    #[serde(default)]
    pub active_tab_idx: usize,
    /// Workspace folder (None = no folder opened)
    pub workspace: Option<PathBuf>,
    /// Last used export directory
    #[serde(default)]
    pub last_export_dir: Option<PathBuf>,
    /// Whether to close export window after exporting
    #[serde(default = "default_close_after_export")]
    pub close_after_export: bool,
}

fn default_close_after_export() -> bool {
    true
}

fn default_focused_pane() -> u64 {
    1
}

/// Serializable tab state
#[derive(Serialize, Deserialize)]
pub struct TabSession {
    /// File path (None for untitled)
    pub path: Option<PathBuf>,
    /// Content (only stored for untitled tabs or dirty tabs)
    pub content: Option<String>,
    /// Whether the tab has unsaved changes
    pub is_dirty: bool,
    /// Undo/redo history
    #[serde(default)]
    pub history: Option<UndoHistory>,
}

impl Session {
    /// Get the session file path
    fn session_path() -> Option<PathBuf> {
        dirs::config_dir().map(|p| p.join("soyuz").join("session.json"))
    }

    /// Load session from disk
    pub fn load() -> Option<Session> {
        let path = Self::session_path()?;

        if !path.exists() {
            return None;
        }

        let content = std::fs::read_to_string(&path).ok()?;
        serde_json::from_str(&content).ok()
    }

    /// Save session to disk
    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::session_path()
            .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?;

        // Create directory if needed
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content)?;

        Ok(())
    }
}

/// Convert EditorPane to PaneSession (recursive)
/// Note: Settings tabs are skipped - they should not be persisted
fn pane_to_session(pane: &EditorPane) -> PaneSession {
    match pane {
        EditorPane::TabGroup { id, tabs, active_tab_idx } => {
            // Filter out Settings tabs - they shouldn't be persisted
            let file_tabs: Vec<_> = tabs.iter()
                .filter(|tab| !tab.is_settings())
                .collect();

            // Adjust active_tab_idx to account for filtered tabs
            let adjusted_active_idx = if file_tabs.is_empty() {
                0
            } else {
                // Find the new index of the previously active tab
                let active_tab_id = tabs.get(*active_tab_idx).map(|t| t.id);
                file_tabs.iter()
                    .position(|t| Some(t.id) == active_tab_id)
                    .unwrap_or(0)
            };

            PaneSession::TabGroup {
                id: *id,
                tabs: file_tabs.iter().map(|tab| TabSession {
                    path: tab.path.clone(),
                    content: if tab.path.is_none() || tab.is_dirty {
                        Some(tab.content.clone())
                    } else {
                        None
                    },
                    is_dirty: tab.is_dirty,
                    history: Some(tab.history.clone()),
                }).collect(),
                active_tab_idx: adjusted_active_idx,
            }
        }
        EditorPane::Split { direction, first, second, ratio } => {
            PaneSession::Split {
                direction: *direction,
                first: Box::new(pane_to_session(first)),
                second: Box::new(pane_to_session(second)),
                ratio: *ratio,
            }
        }
    }
}

/// Convert app state to session for saving
pub fn state_to_session(state: &AppState) -> Session {
    Session {
        pane_layout: Some(pane_to_session(&state.editor_pane)),
        focused_pane_id: state.focused_pane_id,
        tabs: Vec::new(), // Legacy field, empty for new sessions
        active_tab_idx: 0,
        workspace: state.workspace.clone(),
        last_export_dir: state.export_settings.last_export_dir.clone(),
        close_after_export: state.export_settings.close_after_export,
    }
}

/// Convert PaneSession to EditorPane (recursive)
fn session_to_pane(session: &PaneSession, next_tab_id: &mut u64, max_pane_id: &mut PaneId) -> EditorPane {
    match session {
        PaneSession::TabGroup { id, tabs, active_tab_idx } => {
            // Track max pane ID
            if *id > *max_pane_id {
                *max_pane_id = *id;
            }

            let mut restored_tabs = Vec::new();
            for tab_session in tabs {
                let content = load_tab_content(
                    tab_session.path.as_ref(),
                    tab_session.content.as_ref(),
                );

                let history = tab_session.history.clone().unwrap_or_default();
                restored_tabs.push(EditorTab::with_history(
                    *next_tab_id,
                    tab_session.path.clone(),
                    content,
                    tab_session.is_dirty,
                    history,
                ));
                *next_tab_id += 1;
            }

            EditorPane::TabGroup {
                id: *id,
                tabs: restored_tabs,
                active_tab_idx: *active_tab_idx,
            }
        }
        PaneSession::Split { direction, first, second, ratio } => {
            EditorPane::Split {
                direction: *direction,
                first: Box::new(session_to_pane(first, next_tab_id, max_pane_id)),
                second: Box::new(session_to_pane(second, next_tab_id, max_pane_id)),
                ratio: *ratio,
            }
        }
    }
}

/// Restore app state from session
pub fn restore_session(state: &mut AppState, session: Session) {
    let mut next_tab_id = 1u64;
    let mut max_pane_id = 0u64;

    // Try new pane_layout first, fall back to legacy tabs
    if let Some(pane_layout) = session.pane_layout {
        state.editor_pane = session_to_pane(&pane_layout, &mut next_tab_id, &mut max_pane_id);
    } else if !session.tabs.is_empty() {
        // Legacy: restore from flat tabs list
        let mut tabs = Vec::new();
        for tab_session in session.tabs {
            let content = load_tab_content(
                tab_session.path.as_ref(),
                tab_session.content.as_ref(),
            );

            let history = tab_session.history.unwrap_or_default();
            tabs.push(EditorTab::with_history(
                next_tab_id,
                tab_session.path,
                content,
                tab_session.is_dirty,
                history,
            ));
            next_tab_id += 1;
        }

        let active_idx = session.active_tab_idx.min(tabs.len().saturating_sub(1));
        state.editor_pane = EditorPane::TabGroup {
            id: 1,
            tabs,
            active_tab_idx: active_idx,
        };
        max_pane_id = 1;
    } else {
        return;
    }

    state.next_tab_id = next_tab_id;
    state.next_pane_id = max_pane_id + 1;

    // Restore focused pane (validate it exists)
    if state.editor_pane.find_pane(session.focused_pane_id).is_some() {
        state.focused_pane_id = session.focused_pane_id;
    } else if let Some(first_id) = state.editor_pane.all_pane_ids().first() {
        state.focused_pane_id = *first_id;
    }

    // Restore workspace only if the folder still exists
    state.workspace = session.workspace.filter(|p| p.exists());

    // Restore export settings
    state.export_settings.last_export_dir = session.last_export_dir.filter(|p| p.exists());
    state.export_settings.close_after_export = session.close_after_export;
}
