//! Bottom status bar component

use crate::state::{AppStateStoreExt, AppStore};
use dioxus::prelude::*;

/// Application version from Cargo.toml
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Bottom status bar showing application state
#[component]
pub fn StatusBar() -> Element {
    let state = use_context::<AppStore>();

    // F73: subscribe only to the specific store fields the status bar uses (via
    // field selectors) so unrelated changes - terminal height, settings, export
    // settings, dialog state - no longer re-render the status bar. `is_some()` on
    // the Option store tracks shallowly: it only re-renders when the error toggles
    // present/absent, not when the message text itself changes.
    let has_error = state.error_message().is_some();
    let is_previewing = *state.is_previewing().read();

    let (status_text, status_class) = if has_error {
        ("Error in script", "status-error")
    } else if is_previewing {
        ("Preview running", "status-running")
    } else {
        ("Ready", "status-ready")
    };

    // Cursor position and the unsaved indicator derive from the pane tree and the
    // focused pane, so they subscribe to those fields (and legitimately update on
    // each keystroke), but not to the rest of the store.
    let focused = *state.focused_pane_id().read();
    let pane = state.editor_pane();
    let pane_ref = pane.read();
    let (line, col) = pane_ref
        .find_pane(focused)
        .and_then(|p| p.active_tab())
        .map_or((1, 1), |t| (t.cursor_line, t.cursor_col));
    drop(pane_ref);
    let cursor_info = format!("Line {}, Col {}", line, col);

    // The unsaved-changes flag walks the whole pane tree; memoize it so the walk
    // reruns only when the pane tree actually changes, not on every status-bar
    // re-render (e.g. when the preview or error state toggles).
    let has_unsaved = use_memo(move || {
        state
            .editor_pane()
            .read()
            .collect_tabs()
            .iter()
            .any(|t| t.is_dirty)
    });

    rsx! {
        div { class: "bar",
            span { class: "item {status_class}", "{status_text}" }
            span { class: "item", "{cursor_info}" }
            if *has_unsaved.read() {
                span { class: "item unsaved", "Unsaved changes" }
            }
            span { class: "item version", "v{VERSION}" }
        }
    }
}
