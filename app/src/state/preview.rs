//! Preview window state
//!
//! Manages the state shared between the main application and the preview window.

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
