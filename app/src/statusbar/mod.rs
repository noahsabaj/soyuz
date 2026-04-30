//! Bottom status bar component

use crate::state::AppStore;
use dioxus::prelude::*;

/// Application version from Cargo.toml
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Bottom status bar showing application state
#[component]
pub fn StatusBar() -> Element {
    let state = use_context::<AppStore>();

    let (status_text, status_class, cursor_info, has_unsaved) = {
        let s = state.read();
        let (status, class) = if s.has_error() {
            ("Error in script", "status-error")
        } else if s.is_previewing {
            ("Preview running", "status-running")
        } else {
            ("Ready", "status-ready")
        };
        let (line, col) = s.cursor_position();
        let cursor = format!("Line {}, Col {}", line, col);
        let unsaved = s.has_unsaved_changes();
        (status, class, cursor, unsaved)
    };

    rsx! {
        div { class: "bar",
            span { class: "item {status_class}", "{status_text}" }
            span { class: "item", "{cursor_info}" }
            if has_unsaved {
                span { class: "item unsaved", "Unsaved changes" }
            }
            span { class: "item version", "v{VERSION}" }
        }
    }
}
