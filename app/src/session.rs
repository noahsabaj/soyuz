//! Session persistence - save and restore open tabs across sessions

use crate::state::{AppState, EditorPane, EditorTab, UndoHistory};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Serializable session state
#[derive(Serialize, Deserialize, Default)]
pub struct Session {
    /// Open tabs
    pub tabs: Vec<TabSession>,
    /// Active tab index
    pub active_tab_idx: usize,
    /// Working directory
    pub working_dir: Option<PathBuf>,
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

/// Convert app state to session for saving
pub fn state_to_session(state: &AppState) -> Session {
    let (tabs, active_tab_idx) = match &state.editor_pane {
        EditorPane::TabGroup {
            tabs,
            active_tab_idx,
            ..
        } => {
            let tab_sessions: Vec<TabSession> = tabs
                .iter()
                .map(|tab| {
                    TabSession {
                        path: tab.path.clone(),
                        // Only store content for untitled tabs or dirty tabs without a path
                        content: if tab.path.is_none() || tab.is_dirty {
                            Some(tab.content.clone())
                        } else {
                            None
                        },
                        is_dirty: tab.is_dirty,
                        history: Some(tab.history.clone()),
                    }
                })
                .collect();
            (tab_sessions, *active_tab_idx)
        }
        EditorPane::Split { .. } => (vec![], 0),
    };

    Session {
        tabs,
        active_tab_idx,
        working_dir: Some(state.working_dir.clone()),
    }
}

/// Restore app state from session
pub fn restore_session(state: &mut AppState, session: Session) {
    if session.tabs.is_empty() {
        return;
    }

    let mut tabs = Vec::new();
    let mut next_id = 1u64;

    for tab_session in session.tabs {
        let content = if let Some(path) = &tab_session.path {
            // Try to load file content
            if let Some(stored_content) = tab_session.content {
                // Use stored content if tab was dirty
                stored_content
            } else {
                // Load from disk
                std::fs::read_to_string(path).unwrap_or_else(|_| {
                    format!("// Error: Could not load file: {}", path.display())
                })
            }
        } else {
            // Untitled tab - use stored content
            tab_session.content.unwrap_or_default()
        };

        // Restore undo history if present
        let history = tab_session.history.unwrap_or_default();

        tabs.push(EditorTab::with_history(
            next_id,
            tab_session.path,
            content,
            tab_session.is_dirty,
            history,
        ));
        next_id += 1;
    }

    // Update state
    let active_idx = session.active_tab_idx.min(tabs.len().saturating_sub(1));
    state.editor_pane = EditorPane::TabGroup {
        id: 1, // Default pane ID
        tabs,
        active_tab_idx: active_idx,
    };
    state.next_tab_id = next_id;
    state.next_pane_id = 2; // Next available pane ID

    if let Some(wd) = session.working_dir {
        if wd.exists() {
            state.working_dir = wd;
        }
    }
}
