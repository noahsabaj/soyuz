//! Session persistence - save and restore open tabs across sessions

// Separate if statements are clearer for path validation
#![allow(clippy::collapsible_if)]

use crate::state::{AppState, EditorPane, EditorTab, PaneId, SplitDirection, UndoHistory};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Load tab content from stored content or file path.
///
/// Priority:
/// 1. Use stored content if available
/// 2. Read from file path if stored content is missing
/// 3. Return an empty string when there is no path and no stored content
///
/// Returns `None` when a file-backed tab has no stored content and the file can
/// no longer be read (deleted / permission denied / non-UTF8). The caller must
/// skip such tabs: restoring them with placeholder content would let a
/// subsequent save overwrite the real file with that placeholder (F55).
fn load_tab_content(path: Option<&PathBuf>, stored_content: Option<&String>) -> Option<String> {
    match (path, stored_content) {
        // Stored content takes priority
        (_, Some(content)) => Some(content.clone()),
        // No stored content, try to read from file
        (Some(p), None) => match std::fs::read_to_string(p) {
            Ok(content) => Some(content),
            Err(e) => {
                tracing::warn!(
                    "Skipping restore of tab; could not load file {}: {e}",
                    p.display()
                );
                None
            }
        },
        // No path and no stored content
        (None, None) => Some(String::new()),
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

    /// Resolve the destination path, atomic temp path, and serialized JSON for a
    /// save. Shared by the sync and async save paths so the atomic-write scheme
    /// (write temp, rename over target) is defined once.
    fn prepare_save(&self) -> anyhow::Result<(PathBuf, PathBuf, String)> {
        let path = Self::session_path()
            .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?;
        let content = serde_json::to_string_pretty(self)?;
        let tmp_path = path.with_file_name(format!("session.json.{}.tmp", std::process::id()));
        Ok((path, tmp_path, content))
    }

    /// Save session to disk synchronously.
    ///
    /// Retained for the shutdown path, where blocking is acceptable and an async
    /// runtime may not be available. The periodic autosave should prefer
    /// [`Session::save_async`] so it never blocks the desktop executor.
    pub fn save(&self) -> anyhow::Result<()> {
        let (path, tmp_path, content) = self.prepare_save()?;

        // Create directory if needed
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Write atomically: write to a temp file in the same directory, then rename
        // over the target so a crash mid-write cannot corrupt the session file.
        std::fs::write(&tmp_path, content)?;
        std::fs::rename(&tmp_path, &path)?;

        Ok(())
    }

    /// Save session to disk without blocking the async executor.
    ///
    /// Serialization happens on the caller's task (cheap) and every filesystem
    /// operation goes through `tokio::fs`, so the desktop UI thread is never
    /// stalled on synchronous IO. The session-autosave `use_future` in `main.rs`
    /// calls this on the interval configured by the auto-save setting, gated on
    /// a dirty check.
    pub async fn save_async(&self) -> anyhow::Result<()> {
        let (path, tmp_path, content) = self.prepare_save()?;

        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Same atomic write+rename as `save()`, but via non-blocking tokio::fs.
        tokio::fs::write(&tmp_path, content).await?;
        tokio::fs::rename(&tmp_path, &path).await?;

        Ok(())
    }
}

/// Convert EditorPane to PaneSession (recursive)
/// Note: transient workspace tabs are skipped - they should not be persisted
fn pane_to_session(pane: &EditorPane) -> PaneSession {
    match pane {
        EditorPane::TabGroup {
            id,
            tabs,
            active_tab_idx,
        } => {
            // Filter out Settings/Preview/Export tabs - they shouldn't be persisted
            let file_tabs: Vec<_> = tabs.iter().filter(|tab| tab.is_persistable()).collect();

            // Adjust active_tab_idx to account for filtered tabs
            let adjusted_active_idx = if file_tabs.is_empty() {
                0
            } else {
                // Find the new index of the previously active tab
                let active_tab_id = tabs.get(*active_tab_idx).map(|t| t.id);
                file_tabs
                    .iter()
                    .position(|t| Some(t.id) == active_tab_id)
                    .unwrap_or(0)
            };

            PaneSession::TabGroup {
                id: *id,
                tabs: file_tabs
                    .iter()
                    .map(|tab| TabSession {
                        path: tab.path.clone(),
                        content: if tab.path.is_none() || tab.is_dirty {
                            Some(tab.content.clone())
                        } else {
                            None
                        },
                        is_dirty: tab.is_dirty,
                        history: Some(tab.history.clone()),
                    })
                    .collect(),
                active_tab_idx: adjusted_active_idx,
            }
        }
        EditorPane::Split {
            direction,
            first,
            second,
            ratio,
        } => PaneSession::Split {
            direction: *direction,
            first: Box::new(pane_to_session(first)),
            second: Box::new(pane_to_session(second)),
            ratio: *ratio,
        },
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
    }
}

/// Convert PaneSession to EditorPane (recursive)
fn session_to_pane(
    session: &PaneSession,
    next_tab_id: &mut u64,
    max_pane_id: &mut PaneId,
) -> EditorPane {
    match session {
        PaneSession::TabGroup {
            id,
            tabs,
            active_tab_idx,
        } => {
            // Track max pane ID
            if *id > *max_pane_id {
                *max_pane_id = *id;
            }

            let mut restored_tabs = Vec::new();
            let mut adjusted_active_idx = *active_tab_idx;
            for (orig_idx, tab_session) in tabs.iter().enumerate() {
                let Some(content) =
                    load_tab_content(tab_session.path.as_ref(), tab_session.content.as_ref())
                else {
                    // F55: file is gone/unreadable and there is no saved content.
                    // Skip the tab entirely so a later save cannot clobber the
                    // real file with placeholder content.
                    if orig_idx < *active_tab_idx {
                        adjusted_active_idx = adjusted_active_idx.saturating_sub(1);
                    }
                    continue;
                };

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

            let active_tab_idx = if restored_tabs.is_empty() {
                0
            } else {
                adjusted_active_idx.min(restored_tabs.len() - 1)
            };

            EditorPane::TabGroup {
                id: *id,
                tabs: restored_tabs,
                active_tab_idx,
            }
        }
        PaneSession::Split {
            direction,
            first,
            second,
            ratio,
        } => EditorPane::Split {
            direction: *direction,
            first: Box::new(session_to_pane(first, next_tab_id, max_pane_id)),
            second: Box::new(session_to_pane(second, next_tab_id, max_pane_id)),
            ratio: *ratio,
        },
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
        let legacy_active = session.active_tab_idx;
        let mut adjusted_active = legacy_active;
        for (orig_idx, tab_session) in session.tabs.into_iter().enumerate() {
            let Some(content) =
                load_tab_content(tab_session.path.as_ref(), tab_session.content.as_ref())
            else {
                // F55: skip tabs whose backing file can no longer be read.
                if orig_idx < legacy_active {
                    adjusted_active = adjusted_active.saturating_sub(1);
                }
                continue;
            };

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

        let active_idx = adjusted_active.min(tabs.len().saturating_sub(1));
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
    if state
        .editor_pane
        .find_pane(session.focused_pane_id)
        .is_some()
    {
        state.focused_pane_id = session.focused_pane_id;
    } else if let Some(first_id) = state.editor_pane.all_pane_ids().first() {
        state.focused_pane_id = *first_id;
    }

    // Restore workspace only if remembering is enabled and the folder still exists
    state.workspace = if state.settings.remember_workspace {
        session.workspace.filter(|p| p.exists())
    } else {
        None
    };

    // Restore export settings
    state.export_settings.last_export_dir = session.last_export_dir.filter(|p| p.exists());
    state.last_source_tab_id = state
        .active_tab()
        .filter(|tab| tab.is_persistable())
        .map(|tab| tab.id)
        .or_else(|| {
            state
                .all_tabs()
                .into_iter()
                .find(|tab| tab.is_persistable())
                .map(|tab| tab.id)
        });
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn temp_file(name: &str, content: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("soyuz_session_test_{}_{name}", std::process::id()));
        std::fs::write(&path, content).expect("test file should be writable");
        path
    }

    fn tab(path: Option<PathBuf>, content: Option<&str>, is_dirty: bool) -> TabSession {
        TabSession {
            path,
            content: content.map(String::from),
            is_dirty,
            history: None,
        }
    }

    fn group(id: u64, tabs: Vec<TabSession>, active_tab_idx: usize) -> PaneSession {
        PaneSession::TabGroup {
            id,
            tabs,
            active_tab_idx,
        }
    }

    fn session_with_layout(layout: PaneSession) -> Session {
        Session {
            pane_layout: Some(layout),
            focused_pane_id: 1,
            ..Session::default()
        }
    }

    fn source_tab(id: u64, path: Option<PathBuf>, content: &str, is_dirty: bool) -> EditorTab {
        EditorTab::with_history(
            id,
            path,
            content.to_string(),
            is_dirty,
            UndoHistory::default(),
        )
    }

    #[test]
    fn restore_skips_unreadable_tabs_and_keeps_the_active_tab() {
        // F55: a file-backed tab whose file is gone must be skipped (not
        // restored with placeholder content), and the active index must follow
        // the surviving tabs.
        let readable = temp_file("f55_a.rhai", "sphere(0.5)");
        let missing = std::env::temp_dir().join("soyuz_session_test_missing_f55.rhai");
        let session = session_with_layout(group(
            1,
            vec![
                tab(Some(readable.clone()), None, false),
                tab(Some(missing), None, false),
                tab(None, Some("cube(1.0)"), false),
            ],
            2,
        ));

        let mut state = AppState::new();
        restore_session(&mut state, session);

        let EditorPane::TabGroup {
            tabs,
            active_tab_idx,
            ..
        } = &state.editor_pane
        else {
            panic!("expected a tab group");
        };
        assert_eq!(tabs.len(), 2);
        assert_eq!(tabs[0].content, "sphere(0.5)");
        assert_eq!(tabs[1].content, "cube(1.0)");
        // The previously-active tab (index 2) survives as index 1.
        assert_eq!(*active_tab_idx, 1);
        assert_eq!(state.next_tab_id, 3);
        std::fs::remove_file(readable).ok();
    }

    #[test]
    fn restore_with_every_tab_unreadable_yields_an_empty_group() {
        let missing_a = std::env::temp_dir().join("soyuz_session_test_missing_a.rhai");
        let missing_b = std::env::temp_dir().join("soyuz_session_test_missing_b.rhai");
        let session = session_with_layout(group(
            1,
            vec![
                tab(Some(missing_a), None, false),
                tab(Some(missing_b), None, false),
            ],
            1,
        ));

        let mut state = AppState::new();
        restore_session(&mut state, session);

        let EditorPane::TabGroup {
            tabs,
            active_tab_idx,
            ..
        } = &state.editor_pane
        else {
            panic!("expected a tab group");
        };
        assert!(tabs.is_empty());
        assert_eq!(*active_tab_idx, 0);
        assert_eq!(state.next_tab_id, 1);
    }

    #[test]
    fn legacy_flat_tabs_migrate_into_a_pane_tree() {
        let session = Session {
            pane_layout: None,
            focused_pane_id: 1,
            tabs: vec![tab(None, Some("a"), false), tab(None, Some("b"), true)],
            active_tab_idx: 1,
            ..Session::default()
        };

        let mut state = AppState::new();
        restore_session(&mut state, session);

        let EditorPane::TabGroup {
            id,
            tabs,
            active_tab_idx,
        } = &state.editor_pane
        else {
            panic!("expected a tab group");
        };
        assert_eq!(*id, 1);
        assert_eq!(tabs.len(), 2);
        assert_eq!(tabs[0].content, "a");
        assert_eq!(tabs[1].content, "b");
        assert!(tabs[1].is_dirty);
        assert_eq!(*active_tab_idx, 1);
        assert_eq!(state.next_tab_id, 3);
        assert_eq!(state.next_pane_id, 2);
    }

    #[test]
    fn pane_tree_round_trips_through_serialized_session() {
        let mut state = AppState::new();
        state.editor_pane = EditorPane::Split {
            direction: SplitDirection::Vertical,
            first: Box::new(EditorPane::TabGroup {
                id: 1,
                tabs: vec![source_tab(1, None, "first pane", false)],
                active_tab_idx: 0,
            }),
            second: Box::new(EditorPane::TabGroup {
                id: 2,
                tabs: vec![source_tab(2, None, "second pane", true)],
                active_tab_idx: 0,
            }),
            ratio: 0.6,
        };
        state.focused_pane_id = 2;

        // Full round trip through the on-disk representation.
        let json = serde_json::to_string(&state_to_session(&state)).expect("serialize");
        let session: Session = serde_json::from_str(&json).expect("deserialize");

        let mut restored = AppState::new();
        restore_session(&mut restored, session);

        let EditorPane::Split {
            direction,
            first,
            second,
            ratio,
        } = &restored.editor_pane
        else {
            panic!("expected a split");
        };
        assert!(matches!(direction, SplitDirection::Vertical));
        assert!((ratio - 0.6).abs() < f32::EPSILON);
        let EditorPane::TabGroup {
            id: first_id,
            tabs: first_tabs,
            ..
        } = first.as_ref()
        else {
            panic!("expected a tab group");
        };
        let EditorPane::TabGroup {
            id: second_id,
            tabs: second_tabs,
            ..
        } = second.as_ref()
        else {
            panic!("expected a tab group");
        };
        assert_eq!((*first_id, *second_id), (1, 2));
        assert_eq!(first_tabs[0].content, "first pane");
        assert_eq!(second_tabs[0].content, "second pane");
        assert!(second_tabs[0].is_dirty);
        assert_eq!(restored.focused_pane_id, 2);
        assert_eq!(restored.next_pane_id, 3);
        assert_eq!(restored.next_tab_id, 3);
    }

    #[test]
    fn tool_tabs_are_not_persisted() {
        let mut state = AppState::new();
        state.editor_pane = EditorPane::TabGroup {
            id: 1,
            tabs: vec![source_tab(1, None, "source", false)],
            active_tab_idx: 0,
        };
        state.focused_pane_id = 1;
        state.open_preview_tab();

        let session = state_to_session(&state);
        let Some(PaneSession::TabGroup {
            tabs,
            active_tab_idx,
            ..
        }) = session.pane_layout
        else {
            panic!("expected a tab group");
        };
        // Only the source tab persists; the preview tool tab is filtered out
        // and the active index is remapped onto the surviving tabs.
        assert_eq!(tabs.len(), 1);
        assert_eq!(tabs[0].content.as_deref(), Some("source"));
        assert_eq!(active_tab_idx, 0);
    }

    #[test]
    fn only_dirty_or_untitled_tabs_persist_content() {
        let file = temp_file("content_policy.rhai", "on disk");
        let mut state = AppState::new();
        state.editor_pane = EditorPane::TabGroup {
            id: 1,
            tabs: vec![
                source_tab(1, Some(file.clone()), "on disk", false),
                source_tab(2, Some(file.clone()), "edited", true),
                source_tab(3, None, "untitled", false),
            ],
            active_tab_idx: 0,
        };

        let session = state_to_session(&state);
        let Some(PaneSession::TabGroup { tabs, .. }) = session.pane_layout else {
            panic!("expected a tab group");
        };
        // Clean file-backed tabs reload from disk; dirty and untitled tabs
        // must carry their content.
        assert_eq!(tabs[0].content, None);
        assert_eq!(tabs[1].content.as_deref(), Some("edited"));
        assert_eq!(tabs[2].content.as_deref(), Some("untitled"));
        std::fs::remove_file(file).ok();
    }

    #[test]
    fn restore_filters_missing_workspace_and_export_dir() {
        let existing_dir =
            std::env::temp_dir().join(format!("soyuz_session_test_dir_{}", std::process::id()));
        std::fs::create_dir_all(&existing_dir).expect("test dir should be creatable");
        let missing_dir = std::env::temp_dir().join("soyuz_session_test_never_exists_dir");

        let mut session = session_with_layout(group(1, vec![tab(None, Some("x"), false)], 0));
        session.workspace = Some(existing_dir.clone());
        session.last_export_dir = Some(existing_dir.clone());
        let mut state = AppState::new();
        state.settings.remember_workspace = true;
        restore_session(&mut state, session);
        assert_eq!(state.workspace.as_deref(), Some(existing_dir.as_path()));
        assert_eq!(
            state.export_settings.last_export_dir.as_deref(),
            Some(existing_dir.as_path())
        );

        let mut session = session_with_layout(group(1, vec![tab(None, Some("x"), false)], 0));
        session.workspace = Some(missing_dir.clone());
        session.last_export_dir = Some(missing_dir);
        let mut state = AppState::new();
        state.settings.remember_workspace = true;
        restore_session(&mut state, session);
        assert!(state.workspace.is_none());
        assert!(state.export_settings.last_export_dir.is_none());

        std::fs::remove_dir_all(existing_dir).ok();
    }

    #[test]
    fn session_json_with_fields_from_older_versions_still_loads() {
        // Sessions written by v0.7.x carried `close_after_export`; the field is
        // gone but old files must still deserialize.
        let json = r#"{
            "pane_layout": null,
            "focused_pane_id": 1,
            "tabs": [],
            "active_tab_idx": 0,
            "workspace": null,
            "last_export_dir": null,
            "close_after_export": true
        }"#;
        let session: Session = serde_json::from_str(json).expect("old session JSON should load");
        assert!(session.pane_layout.is_none());
    }
}
