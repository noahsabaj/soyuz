//! Bottom status bar component

use crate::state::AppState;
use dioxus::prelude::*;

/// Application version from Cargo.toml
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Bottom status bar showing application state
#[component]
pub fn StatusBar() -> Element {
    let state = use_context::<Signal<AppState>>();

    let (status_text, cursor_info, has_unsaved) = {
        let s = state.read();
        let status = if s.has_error {
            "Error in script"
        } else if s.is_previewing {
            "Preview running"
        } else {
            "Ready"
        };
        let (line, col) = s.cursor_position();
        let cursor = format!("Line {}, Col {}", line, col);
        let unsaved = s.has_unsaved_changes();
        (status, cursor, unsaved)
    };

    rsx! {
        div { class: "status-bar",
            span { class: "status-item", "{status_text}" }
            span { class: "status-item", "{cursor_info}" }
            if has_unsaved {
                span { class: "status-item unsaved", "Unsaved changes" }
            }
            span { class: "status-item version", "v{VERSION}" }
        }
    }
}
