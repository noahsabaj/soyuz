//! Recursive pane rendering for split editor views

// Syntax highlighter has complex state machine logic
#![allow(clippy::too_many_lines)]
// map_or is less readable for optional values
#![allow(clippy::map_unwrap_or)]
// Borrowed format strings for computed class names
#![allow(clippy::needless_borrows_for_generic_args)]

use crate::js_interop::{self, position_to_line_col};
use crate::markdown_panel::MarkdownPanel;
use crate::services::AppServices;
use crate::settings_panel::SettingsPanel;
use crate::state::{
    AppStateStoreExt, AppStore, EditorPane, EditorTab, PaneId, SplitDirection, TabId,
};
use dioxus::prelude::*;

/// State for tab drag-and-drop operations (shared via context)
#[derive(Clone, Copy, Default, PartialEq)]
pub struct TabDragState {
    /// Source tab being dragged: (pane_id, tab_id, tab_index)
    pub source: Option<(PaneId, TabId, usize)>,
    /// Current drop target: (pane_id, insert_index, is_content_area)
    pub target: Option<(PaneId, usize, bool)>,
}

/// A tab whose close is awaiting confirmation because it has unsaved changes (F71).
/// Shared via context so every close entry point (X button, middle-click, Ctrl+W
/// in the editor, and the global Ctrl+W handler) can route through one dialog.
#[derive(Clone, Copy, PartialEq)]
pub struct PendingCloseTab {
    pub pane_id: PaneId,
    pub tab_id: TabId,
}

/// Request closing a tab. Stops the preview first when closing the Preview tab,
/// then closes immediately - unless the tab has unsaved changes, in which case it
/// defers to a confirmation dialog so edits are never silently discarded (F71).
pub fn request_close_tab(
    mut state: AppStore,
    services: &AppServices,
    mut pending_close: Signal<Option<PendingCloseTab>>,
    pane_id: PaneId,
    tab_id: TabId,
) {
    let (is_dirty, is_preview) = state
        .read()
        .editor_pane
        .find_tab(tab_id)
        .map(|tab| (tab.is_dirty, tab.is_preview()))
        .unwrap_or((false, false));

    if is_preview {
        crate::preview::stop_preview(state, services);
    }

    if is_dirty {
        pending_close.set(Some(PendingCloseTab { pane_id, tab_id }));
    } else {
        state.write().close_tab_in_pane(pane_id, tab_id);
    }
}

/// Confirmation dialog shown when closing a tab that has unsaved changes (F71).
#[component]
pub fn CloseTabConfirmDialog() -> Element {
    let mut state = use_context::<AppStore>();
    let mut pending_close = use_context::<Signal<Option<PendingCloseTab>>>();

    let Some(pending) = *pending_close.read() else {
        return rsx! {};
    };

    let name = state
        .read()
        .editor_pane
        .find_tab(pending.tab_id)
        .map(EditorTab::display_name)
        .unwrap_or_default();

    rsx! {
        crate::components::ConfirmDialog {
            title: "Unsaved changes",
            message: format!("Close \"{name}\" without saving? Your changes will be lost."),
            confirm_label: "Close Without Saving",
            destructive: true,
            on_confirm: move |_| {
                pending_close.set(None);
                state.write().close_tab_in_pane(pending.pane_id, pending.tab_id);
            },
            on_cancel: move |_| pending_close.set(None),
        }
    }
}

/// Welcome screen shown when no tabs are open (VSCode-style)
#[component]
fn WelcomeScreen() -> Element {
    rsx! {
        div { class: "welcome-screen",
            // Logo (grayed out like VSCode)
            div { class: "welcome-logo",
                // Using the Soyuz icon character or a simple placeholder
                div { class: "welcome-logo-icon", "S" }
            }

            // Keyboard shortcuts
            div { class: "welcome-shortcuts",
                div { class: "welcome-shortcut",
                    span { class: "welcome-shortcut-label", "New File" }
                    span { class: "welcome-shortcut-keys",
                        kbd { "Ctrl" }
                        span { class: "welcome-shortcut-plus", "+" }
                        kbd { "N" }
                    }
                }
                div { class: "welcome-shortcut",
                    span { class: "welcome-shortcut-label", "Open File" }
                    span { class: "welcome-shortcut-keys",
                        kbd { "Ctrl" }
                        span { class: "welcome-shortcut-plus", "+" }
                        kbd { "O" }
                    }
                }
                div { class: "welcome-shortcut",
                    span { class: "welcome-shortcut-label", "Command Palette" }
                    span { class: "welcome-shortcut-keys",
                        kbd { "Ctrl" }
                        span { class: "welcome-shortcut-plus", "+" }
                        kbd { "P" }
                    }
                }
            }
        }
    }
}

/// Render the entire pane tree recursively
#[component]
pub fn PaneTree() -> Element {
    let state = use_context::<AppStore>();

    // Memoize the pane tree clone. Subscribing through the `editor_pane` store
    // selector (not the whole store) means this only re-runs when the pane tree
    // actually changes, not on unrelated state changes.
    let pane = use_memo(move || state.editor_pane().read().clone());

    // Provide drag state context for all child components
    let _drag_state: Signal<TabDragState> =
        use_context_provider(|| Signal::new(TabDragState::default()));

    rsx! {
        div { class: "pane-tree",
            PaneView { pane: pane() }
        }
    }
}

/// Component to render the editor pane (recursive)
#[component]
fn PaneView(pane: EditorPane) -> Element {
    match pane {
        EditorPane::TabGroup {
            id,
            tabs,
            active_tab_idx,
        } => {
            rsx! {
                TabGroupPane {
                    pane_id: id,
                    tabs,
                    active_tab_idx,
                }
            }
        }
        EditorPane::Split {
            direction,
            first,
            second,
            ratio,
        } => {
            rsx! {
                SplitPane {
                    direction,
                    first: *first,
                    second: *second,
                    ratio,
                }
            }
        }
    }
}

/// Resize state stored during drag operation
#[derive(Clone, Copy, Default)]
struct ResizeState {
    active: bool,
    start_mouse_pos: f64, // Mouse position when drag started
    start_ratio: f32,     // Ratio when drag started
    container_width: f64, // Estimated container width
}

/// A split container with two child panes and a resizable handle
#[component]
fn SplitPane(
    direction: SplitDirection,
    first: EditorPane,
    second: EditorPane,
    ratio: f32,
) -> Element {
    let mut state = use_context::<AppStore>();
    let mut resize_state = use_signal(ResizeState::default);

    // Generate a stable ID for this split container based on first pane's ID
    let target_pane_id = first.all_pane_ids().first().copied().unwrap_or(1);
    let container_id = format!("split-{}", target_pane_id);

    let container_class = match direction {
        SplitDirection::Vertical => "split-container split-vertical",
        SplitDirection::Horizontal => "split-container split-horizontal",
    };

    let handle_class = match direction {
        SplitDirection::Vertical => "split-handle split-handle-vertical",
        SplitDirection::Horizontal => "split-handle split-handle-horizontal",
    };

    // Calculate flex values based on ratio
    let first_flex = ratio;
    let second_flex = 1.0 - ratio;

    // Cursor style for the resize overlay
    let overlay_cursor = match direction {
        SplitDirection::Vertical => "col-resize",
        SplitDirection::Horizontal => "row-resize",
    };

    rsx! {
        div {
            id: "{container_id}",
            class: container_class,

            // First pane
            div {
                class: "split-pane",
                style: "flex: {first_flex};",
                PaneView { pane: first }
            }

            // Resizable handle
            div {
                class: handle_class,
                onmousedown: move |evt| {
                    evt.prevent_default();
                    let start_pos = match direction {
                        SplitDirection::Vertical => evt.client_coordinates().x,
                        SplitDirection::Horizontal => evt.client_coordinates().y,
                    };
                    // Estimate container size from click position and current ratio
                    // For vertical: click_x = explorer_width + container_width * ratio
                    // For horizontal: click_y = menu_height + container_height * ratio
                    let (offset, min_size) = match direction {
                        SplitDirection::Vertical => (220.0, 400.0),   // Explorer panel width
                        SplitDirection::Horizontal => (60.0, 200.0), // Menu/title bar height
                    };
                    let container_size = if ratio > 0.01 {
                        (start_pos - offset) / ratio as f64
                    } else {
                        800.0 // Fallback
                    };
                    resize_state.set(ResizeState {
                        active: true,
                        start_mouse_pos: start_pos,
                        start_ratio: ratio,
                        container_width: container_size.max(min_size),
                    });
                },
            }

            // Second pane
            div {
                class: "split-pane",
                style: "flex: {second_flex};",
                PaneView { pane: second }
            }

            // Invisible overlay during resize - captures all mouse events
            if resize_state.read().active {
                div {
                    class: "resize-overlay",
                    style: "position: fixed; top: 0; left: 0; right: 0; bottom: 0; z-index: 9999; cursor: {overlay_cursor};",
                    onmousemove: move |evt| {
                        let rs = *resize_state.read();
                        if rs.active {
                            let current_pos = match direction {
                                SplitDirection::Vertical => evt.client_coordinates().x,
                                SplitDirection::Horizontal => evt.client_coordinates().y,
                            };
                            // Delta in pixels from start position
                            let delta_px = current_pos - rs.start_mouse_pos;
                            // Convert to ratio change using estimated container size
                            let delta_ratio = (delta_px / rs.container_width) as f32;
                            let new_ratio = (rs.start_ratio + delta_ratio).clamp(0.1, 0.9);
                            state.write().set_split_ratio(target_pane_id, new_ratio);
                        }
                    },
                    onmouseup: move |_| {
                        resize_state.set(ResizeState::default());
                    },
                }
            }
        }
    }
}

/// A single tab group pane with tabs and editor
#[component]
fn TabGroupPane(pane_id: PaneId, tabs: Vec<EditorTab>, active_tab_idx: usize) -> Element {
    let mut state = use_context::<AppStore>();
    let mut drag_state = use_context::<Signal<TabDragState>>();
    let tabs_len = tabs.len();

    let is_focused = *state.focused_pane_id().read() == pane_id;

    // If no tabs, show welcome screen (VSCode behavior)
    if tabs.is_empty() {
        let pane_class = if is_focused {
            "editor-pane focused"
        } else {
            "editor-pane"
        };
        return rsx! {
            div {
                class: pane_class,
                onclick: move |_| { state.write().focus_pane(pane_id); },

                // Empty tab bar (just shows the + button)
                TabBar {
                    pane_id,
                    tabs: Vec::new(),
                    active_tab_id: 0,
                    is_focused,
                }

                // Welcome screen instead of editor
                WelcomeScreen {}
            }
        };
    }

    let active_tab = tabs.get(active_tab_idx);
    let code = active_tab.map(|t| t.content.clone()).unwrap_or_default();
    let active_tab_id = active_tab.map(|t| t.id).unwrap_or(0);
    let is_settings_tab = active_tab.map(|t| t.is_settings()).unwrap_or(false);
    let is_preview_tab = active_tab.map(|t| t.is_preview()).unwrap_or(false);
    let is_export_tab = active_tab.map(|t| t.is_export()).unwrap_or(false);
    let markdown_doc = active_tab.and_then(|t| t.markdown_doc());

    // Check if editor content is a drop target
    let is_content_drop_target = drag_state.read().target
        .map(|t| t.0 == pane_id && t.2)  // t.2 = is_content_area
        .unwrap_or(false);

    // Memoize syntax highlighting - only recalculate when code changes (skip for Settings tab)
    let code_for_highlight = code.clone();
    let highlighted_html = use_memo(use_reactive!(|code_for_highlight| {
        highlight_rhai(&code_for_highlight)
    }));

    let pane_class = if is_focused {
        "editor-pane focused"
    } else {
        "editor-pane"
    };

    let content_wrapper_class = if is_content_drop_target {
        "editor-content-wrapper drop-target"
    } else {
        "editor-content-wrapper"
    };

    rsx! {
        div {
            class: pane_class,
            onclick: move |_| { state.write().focus_pane(pane_id); },

            // Tab bar
            TabBar {
                pane_id,
                tabs: tabs.clone(),
                active_tab_id,
                is_focused,
            }

            // Editor content wrapper (drop zone for content area)
            div {
                class: content_wrapper_class,
                ondragover: move |evt| {
                    evt.prevent_default();
                    drag_state.write().target = Some((pane_id, tabs_len, true)); // is_content_area = true
                },
                ondragleave: move |_| {
                    // Only clear if this was a content area target
                    let current = drag_state.read().target;
                    if current.map(|t| t.0 == pane_id && t.2).unwrap_or(false) {
                        drag_state.write().target = None;
                    }
                },
                ondrop: move |evt| {
                    evt.prevent_default();
                    let ds = *drag_state.read();
                    if let Some((_, src_tab_id, _)) = ds.source {
                        state.write().move_tab(src_tab_id, pane_id, tabs_len); // Append at end
                    }
                    drag_state.set(TabDragState::default());
                },

                // Render panel based on tab type. Each embedded panel gets its own
                // ErrorBoundary so a panic/`Err` inside one panel renders an inline
                // fallback instead of collapsing the whole pane tree. (The editor
                // path is covered by the outer boundary in main.rs.)
                if is_settings_tab {
                    ErrorBoundary {
                        handle_error: |error| rsx! {
                            crate::PanelError {
                                panel_name: "Settings".to_string(),
                                error_msg: format!("{error:?}"),
                            }
                        },
                        SettingsPanel {}
                    }
                } else if is_preview_tab {
                    ErrorBoundary {
                        handle_error: |error| rsx! {
                            crate::PanelError {
                                panel_name: "Preview".to_string(),
                                error_msg: format!("{error:?}"),
                            }
                        },
                        crate::preview::PreviewPanel {}
                    }
                } else if is_export_tab {
                    ErrorBoundary {
                        handle_error: |error| rsx! {
                            crate::PanelError {
                                panel_name: "Export".to_string(),
                                error_msg: format!("{error:?}"),
                            }
                        },
                        crate::export::ExportPanel {}
                    }
                } else if let Some(doc) = markdown_doc {
                    ErrorBoundary {
                        handle_error: |error| rsx! {
                            crate::PanelError {
                                panel_name: "Documentation".to_string(),
                                error_msg: format!("{error:?}"),
                            }
                        },
                        MarkdownPanel { doc }
                    }
                } else {
                    EditorArea {
                        pane_id,
                        code,
                        active_tab_id,
                        highlighted_html,
                    }
                }
            }
        }
    }
}

/// Tab bar with tabs and action buttons
#[component]
fn TabBar(pane_id: PaneId, tabs: Vec<EditorTab>, active_tab_id: u64, is_focused: bool) -> Element {
    let mut state = use_context::<AppStore>();
    let services = use_context::<AppServices>();
    let mut drag_state = use_context::<Signal<TabDragState>>();
    let pending_close = use_context::<Signal<Option<PendingCloseTab>>>();
    let tabs_len = tabs.len();

    rsx! {
        div {
            class: "editor-tabs",
            // Drop zone for end of tab bar (when not over a specific tab)
            ondragover: move |evt| {
                evt.prevent_default();
                // Only set target to end if not already over a specific tab
                let current_target = drag_state.read().target;
                if current_target.map(|t| t.0 != pane_id).unwrap_or(true) {
                    drag_state.write().target = Some((pane_id, tabs_len, false));
                }
            },
            ondrop: move |evt| {
                evt.prevent_default();
                let ds = *drag_state.read();
                if let Some((_, src_tab_id, _)) = ds.source {
                    let target_idx = ds.target.map(|t| t.1).unwrap_or(tabs_len);
                    state.write().move_tab(src_tab_id, pane_id, target_idx);
                }
                drag_state.set(TabDragState::default());
            },

            for (tab_index, tab) in tabs.iter().enumerate() {
                {
                    let tab_id = tab.id;
                    let name = tab.display_name();
                    let is_dirty = tab.is_dirty;
                    let is_active = tab_id == active_tab_id;
                    let is_settings = tab.is_settings();
                    let is_preview = tab.is_preview();
                    let is_export = tab.is_export();
                    let is_markdown = tab.is_markdown();
                    let services_for_middle = services.clone();
                    let services_for_switch = services.clone();
                    let services_for_close = services.clone();

                    // Determine CSS classes based on drag state
                    let ds = drag_state.read();
                    let is_dragging = ds.source.map(|s| s.1 == tab_id).unwrap_or(false);
                    let is_drop_target = ds.target.map(|t| t.0 == pane_id && t.1 == tab_index && !t.2).unwrap_or(false);
                    let is_drop_after = ds.target.map(|t| t.0 == pane_id && t.1 == tab_index + 1 && !t.2).unwrap_or(false);

                    let mut class = String::from("editor-tab");
                    if is_active { class.push_str(" active"); }
                    if is_dragging { class.push_str(" dragging"); }
                    if is_drop_target { class.push_str(" drop-before"); }
                    if is_drop_after { class.push_str(" drop-after"); }

                    rsx! {
                        div {
                            key: "{tab_id}",
                            class: "{class}",
                            draggable: "true",

                            // Start drag
                            ondragstart: move |_| {
                                drag_state.write().source = Some((pane_id, tab_id, tab_index));
                            },

                            // End drag (cleanup)
                            ondragend: move |_| {
                                drag_state.set(TabDragState::default());
                            },

                            // Drag over this tab - set as drop target
                            ondragover: move |evt| {
                                evt.prevent_default();
                                evt.stop_propagation();
                                drag_state.write().target = Some((pane_id, tab_index, false));
                            },

                            // Drag left this tab
                            ondragleave: move |_| {
                                // Only clear if this was the target
                                let current = drag_state.read().target;
                                if current.map(|t| t.0 == pane_id && t.1 == tab_index && !t.2).unwrap_or(false) {
                                    drag_state.write().target = None;
                                }
                            },

                            // Drop on this tab
                            ondrop: move |evt| {
                                evt.prevent_default();
                                evt.stop_propagation();
                                let ds = *drag_state.read();
                                if let Some((_, src_tab_id, _)) = ds.source {
                                    state.write().move_tab(src_tab_id, pane_id, tab_index);
                                }
                                drag_state.set(TabDragState::default());
                            },

                            // Handle clicks: left-click to switch, middle-click to close
                            onmousedown: move |evt| {
                                // Middle button (button index 1)
                                if evt.trigger_button() == Some(dioxus_elements::input_data::MouseButton::Auxiliary) {
                                    evt.stop_propagation();
                                    // F71: confirm before discarding unsaved edits.
                                    request_close_tab(state, &services_for_middle, pending_close, pane_id, tab_id);
                                }
                            },

                            // Left-click to switch tab
                            onclick: move |_| {
                                if !is_preview {
                                    crate::preview::stop_preview(state, &services_for_switch);
                                }
                                state.write().switch_to_tab(tab_id);
                            },

                            span { class: "tab-name",
                                // Gear icon for Settings tab
                                if is_settings {
                                    span {
                                        class: "tab-icon settings-icon",
                                        dangerous_inner_html: include_str!("../../assets/gear.svg")
                                    }
                                }
                                if is_preview {
                                    span { class: "tab-icon preview-icon", "P" }
                                }
                                if is_export {
                                    span { class: "tab-icon export-icon", "E" }
                                }
                                // Book icon for markdown tabs (Cookbook, README, etc.)
                                if is_markdown {
                                    span {
                                        class: "tab-icon markdown-icon",
                                        dangerous_inner_html: include_str!("../../assets/book.svg")
                                    }
                                }
                                if is_dirty {
                                    span { class: "dirty-indicator", "*" }
                                }
                                "{name}"
                            }
                            button {
                                class: "tab-close",
                                onclick: move |evt| {
                                    evt.stop_propagation();
                                    // F71: confirm before discarding unsaved edits.
                                    request_close_tab(state, &services_for_close, pending_close, pane_id, tab_id);
                                },
                                "x"
                            }
                        }
                    }
                }
            }
            // New tab button
            button {
                class: "new-tab",
                onclick: move |_| { state.write().new_tab_in_pane(pane_id); },
                "+"
            }

            // Split action buttons (right side)
            div { class: "tab-actions",
                // Vertical split (side-by-side)
                button {
                    class: "tab-action-btn",
                    title: "Split Right",
                    onclick: move |_| {
                        state.write().split_pane(pane_id, SplitDirection::Vertical);
                    },
                    // Two vertical rectangles icon
                    svg {
                        width: "14",
                        height: "14",
                        view_box: "0 0 14 14",
                        fill: "none",
                        stroke: "currentColor",
                        stroke_width: "1.2",
                        rect { x: "1", y: "1", width: "5", height: "12", rx: "1" }
                        rect { x: "8", y: "1", width: "5", height: "12", rx: "1" }
                    }
                }
                // Horizontal split (top/bottom)
                button {
                    class: "tab-action-btn",
                    title: "Split Down",
                    onclick: move |_| {
                        state.write().split_pane(pane_id, SplitDirection::Horizontal);
                    },
                    // Two horizontal rectangles icon
                    svg {
                        width: "14",
                        height: "14",
                        view_box: "0 0 14 14",
                        fill: "none",
                        stroke: "currentColor",
                        stroke_width: "1.2",
                        rect { x: "1", y: "1", width: "12", height: "5", rx: "1" }
                        rect { x: "1", y: "8", width: "12", height: "5", rx: "1" }
                    }
                }
            }
        }
    }
}

/// Editor area with line numbers, syntax highlighting, and textarea
#[component]
fn EditorArea(
    pane_id: PaneId,
    code: String,
    active_tab_id: u64,
    highlighted_html: String,
) -> Element {
    let mut state = use_context::<AppStore>();
    let services = use_context::<AppServices>();
    let pending_close = use_context::<Signal<Option<PendingCloseTab>>>();
    let editor_id = format!("editor-{}", pane_id);

    // F61/F73: read editor display settings through a field selector so only
    // changes to `settings` (not every keystroke elsewhere) re-render here, then
    // apply them to the editor's rendering.
    let (font_family, font_size, tab_size, word_wrap, line_numbers) = {
        let settings = state.settings();
        let s = settings.read();
        (
            s.font_family.clone(),
            s.font_size,
            s.tab_size,
            s.word_wrap,
            s.line_numbers,
        )
    };
    let white_space = if word_wrap { "pre-wrap" } else { "pre" };
    // Shared text metrics keep the gutter, syntax overlay and textarea aligned.
    let text_style =
        format!("font-family: {font_family}; font-size: {font_size}px; tab-size: {tab_size};");
    let editor_style = format!("{text_style} white-space: {white_space};");

    rsx! {
        div { class: "editor-content",
            // Line numbers (hidden when the line_numbers setting is off)
            if line_numbers {
                div {
                    id: "line-numbers-{pane_id}",
                    class: "line-numbers",
                    style: "{text_style}",
                    LineNumbers { code: code.clone() }
                }
            }
            // Code area
            div {
                id: "code-area-{pane_id}",
                class: "code-area",
                pre {
                    id: "syntax-{pane_id}",
                    class: "syntax-highlight",
                    style: "{editor_style}",
                    dangerous_inner_html: "{highlighted_html}"
                }
                textarea {
                    id: "{editor_id}",
                    class: "code-input",
                    style: "{editor_style}",
                    // Data attribute for JS scroll sync (CSS module class names are hashed)
                    "data-editor-pane": "{pane_id}",
                    spellcheck: false,
                    value: "{code}",
                    onfocus: move |_| { state.write().focus_pane(pane_id); },
                    // Scroll sync handled by native JS in main.rs (continuous sync)
                    // scrollend fires once when scrolling stops - useful for state updates
                    onscrollend: move |_| {
                        // Could save scroll position to state here for restoration
                        // Currently a no-op placeholder for future scroll position persistence
                    },
                    oninput: {
                        let editor_id = editor_id.clone();
                        move |evt| {
                            let new_code = evt.value().clone();
                            // Scope the per-keystroke write to the `editor_pane`
                            // store node (and the two affected fields) instead of
                            // a whole-AppState root write, so a keystroke no longer
                            // re-renders settings/terminal/workspace subscribers.
                            let focused = *state.focused_pane_id().peek();
                            let undo_limit = state.settings().peek().undo_history_limit;
                            let outcome = state.editor_pane().write().set_active_code(
                                focused,
                                new_code.clone(),
                                undo_limit,
                            );
                            if outcome.changed {
                                state.preview_dirty().set(true);
                            }
                            if let Some(id) = outcome.edited_source_tab_id {
                                state.last_source_tab_id().set(Some(id));
                            }
                            update_cursor_position(state, &editor_id, &new_code);
                        }
                    },
                    onkeyup: {
                        let editor_id = editor_id.clone();
                        let code = code.clone();
                        move |_| { update_cursor_position(state, &editor_id, &code); }
                    },
                    onclick: {
                        let editor_id = editor_id.clone();
                        let code = code.clone();
                        move |_| { update_cursor_position(state, &editor_id, &code); }
                    },
                    onkeydown: {
                        let services = services.clone();
                        move |evt| {
                            handle_editor_keydown(
                                state,
                                &services,
                                pending_close,
                                pane_id,
                                active_tab_id,
                                tab_size,
                                &evt,
                            );
                        }
                    },
                }
            }
        }
    }
}

/// Update cursor position from the DOM
fn update_cursor_position(state: AppStore, editor_id: &str, code: &str) {
    let editor_id = editor_id.to_string();
    let code = code.to_string();
    spawn(async move {
        if let Some(pos) = js_interop::get_cursor_position(&editor_id).await {
            let (line, col) = position_to_line_col(&code, pos);
            // Scoped write: cursor lives in the editor_pane node.
            let focused = *state.focused_pane_id().peek();
            state
                .editor_pane()
                .write()
                .set_active_cursor(focused, line, col);
        }
    });
}

/// Handle keyboard shortcuts in the editor.
///
/// Shortcuts handled here call `stop_propagation()` so the global handler in
/// `main.rs` does not run them a second time when focus is in the editor.
fn handle_editor_keydown(
    mut state: AppStore,
    services: &AppServices,
    pending_close: Signal<Option<PendingCloseTab>>,
    pane_id: PaneId,
    active_tab_id: u64,
    tab_size: u32,
    evt: &KeyboardEvent,
) {
    let key = evt.key();
    // Accept Cmd (meta) in addition to Ctrl so shortcuts also work on macOS.
    let ctrl = evt.modifiers().ctrl() || evt.modifiers().meta();
    let shift = evt.modifiers().shift();
    // Lowercase character keys so Caps Lock / Shift do not break matching.
    let key_char = match &key {
        Key::Character(s) => Some(s.to_lowercase()),
        _ => None,
    };
    let is_char = |c: &str| key_char.as_deref() == Some(c);

    // Ctrl+Enter: Run preview
    if ctrl && key == Key::Enter {
        evt.prevent_default();
        evt.stop_propagation();
        crate::preview::open_docked_preview(state, services.clone());
        return;
    }

    // Ctrl+Z: Undo / Ctrl+Shift+Z: Redo
    if ctrl && is_char("z") {
        evt.prevent_default();
        evt.stop_propagation();
        let restored = if shift {
            state.write().redo()
        } else {
            state.write().undo()
        };
        if let Some((new_content, cursor_pos)) = restored {
            spawn(async move {
                js_interop::set_editor_content(pane_id, &new_content, cursor_pos).await;
            });
        }
        return;
    }

    // Ctrl+N: New tab
    if ctrl && !shift && is_char("n") {
        evt.prevent_default();
        evt.stop_propagation();
        state.write().new_tab_in_pane(pane_id);
        return;
    }

    // Ctrl+W: Close tab. Stops the preview when closing the Preview tab and, via
    // request_close_tab, confirms before discarding unsaved edits (F71).
    if ctrl && !shift && is_char("w") {
        evt.prevent_default();
        evt.stop_propagation();
        request_close_tab(state, services, pending_close, pane_id, active_tab_id);
        return;
    }

    // Tab: Insert `tab_size` spaces or indent selection (F61)
    if key == Key::Tab && !shift {
        evt.prevent_default();
        evt.stop_propagation();
        spawn(async move {
            js_interop::insert_indent(pane_id, tab_size).await;
        });
    }
}

/// Render line numbers for the editor
#[component]
fn LineNumbers(code: String) -> Element {
    let line_count = if code.is_empty() {
        1
    } else if code.ends_with('\n') {
        code.lines().count() + 1
    } else {
        code.lines().count()
    };

    rsx! {
        for i in 1..=line_count {
            div { key: "{i}", class: "line-number", "{i}" }
        }
    }
}

/// Simple Rhai syntax highlighting
fn highlight_rhai(code: &str) -> String {
    let mut result = String::with_capacity(code.len() * 2);

    let keywords = [
        "let", "const", "fn", "if", "else", "while", "for", "in", "loop", "break", "continue",
        "return", "true", "false", "null",
    ];

    let builtins = [
        "sphere",
        "cube",
        "box3",
        "cylinder",
        "capsule",
        "torus",
        "cone",
        "plane",
        "ellipsoid",
        "octahedron",
        "hex_prism",
        "tri_prism",
        "rounded_box",
        "mandelbulb",
        "menger",
        "union",
        "subtract",
        "intersect",
        "smooth_union",
        "smooth_subtract",
        "smooth_intersect",
        "translate",
        "translate_x",
        "translate_y",
        "translate_z",
        "rotate",
        "rotate_x",
        "rotate_y",
        "rotate_z",
        "scale",
        "scale_xyz",
        "mirror",
        "twist",
        "bend",
        "taper",
        "hollow",
        "shell",
        "onion",
        "round",
        "elongate",
        "repeat",
        "repeat_limited",
        "repeat_polar",
        "ground_plane",
    ];

    for line in code.lines() {
        let chars: Vec<char> = line.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            let c = chars[i];

            if c == '/' && i + 1 < chars.len() && chars[i + 1] == '/' {
                let comment: String = chars[i..].iter().collect();
                result.push_str(&format!(
                    "<span class=\"hl-comment\">{}</span>",
                    html_escape(&comment)
                ));
                break;
            }

            if c == '"' {
                let mut end = i + 1;
                while end < chars.len() && chars[end] != '"' {
                    if chars[end] == '\\' && end + 1 < chars.len() {
                        end += 1;
                    }
                    end += 1;
                }
                if end < chars.len() {
                    end += 1;
                }
                let string: String = chars[i..end].iter().collect();
                result.push_str(&format!(
                    "<span class=\"hl-string\">{}</span>",
                    html_escape(&string)
                ));
                i = end;
                continue;
            }

            if c.is_ascii_digit()
                || (c == '-' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit())
            {
                let mut end = i;
                if c == '-' {
                    end += 1;
                }
                while end < chars.len() && (chars[end].is_ascii_digit() || chars[end] == '.') {
                    end += 1;
                }
                let number: String = chars[i..end].iter().collect();
                result.push_str(&format!(
                    "<span class=\"hl-number\">{}</span>",
                    html_escape(&number)
                ));
                i = end;
                continue;
            }

            if c.is_alphabetic() || c == '_' {
                let mut end = i;
                while end < chars.len() && (chars[end].is_alphanumeric() || chars[end] == '_') {
                    end += 1;
                }
                let word: String = chars[i..end].iter().collect();

                if keywords.contains(&word.as_str()) {
                    result.push_str(&format!(
                        "<span class=\"hl-keyword\">{}</span>",
                        html_escape(&word)
                    ));
                } else if builtins.contains(&word.as_str()) {
                    result.push_str(&format!(
                        "<span class=\"hl-builtin\">{}</span>",
                        html_escape(&word)
                    ));
                } else {
                    result.push_str(&html_escape(&word));
                }
                i = end;
                continue;
            }

            result.push_str(&html_escape(&c.to_string()));
            i += 1;
        }
        result.push('\n');
    }
    result
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
