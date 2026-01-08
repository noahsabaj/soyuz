//! Recursive pane rendering for split editor views

// Syntax highlighter has complex state machine logic
#![allow(clippy::too_many_lines)]
// map_or is less readable for optional values
#![allow(clippy::map_unwrap_or)]

use crate::cookbook_panel::CookbookPanel;
use crate::js_interop::{self, position_to_line_col};
use crate::settings_panel::SettingsPanel;
use crate::state::{AppState, EditorPane, EditorTab, PaneId, SplitDirection, TabId};
use dioxus::prelude::*;

/// State for tab drag-and-drop operations (shared via context)
#[derive(Clone, Copy, Default, PartialEq)]
pub struct TabDragState {
    /// Source tab being dragged: (pane_id, tab_id, tab_index)
    pub source: Option<(PaneId, TabId, usize)>,
    /// Current drop target: (pane_id, insert_index, is_content_area)
    pub target: Option<(PaneId, usize, bool)>,
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
    let state = use_context::<Signal<AppState>>();

    // Memoize the pane tree clone to avoid cloning on every render
    // Only re-clones when the pane structure actually changes
    let pane = use_memo(move || state.read().editor_pane.clone());

    // Provide drag state context for all child components
    let _drag_state: Signal<TabDragState> = use_context_provider(|| Signal::new(TabDragState::default()));

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
    start_mouse_pos: f64,   // Mouse position when drag started
    start_ratio: f32,       // Ratio when drag started
    container_width: f64,   // Estimated container width
}

/// A split container with two child panes and a resizable handle
#[component]
fn SplitPane(direction: SplitDirection, first: EditorPane, second: EditorPane, ratio: f32) -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let mut resize_state = use_signal(ResizeState::default);

    // Generate a stable ID for this split container based on first pane's ID
    let target_pane_id = first.all_pane_ids().first().copied().unwrap_or(1);
    let container_id = format!("split-{}", target_pane_id);

    let container_class = match direction {
        SplitDirection::Vertical => "split-container vertical",
        SplitDirection::Horizontal => "split-container horizontal",
    };

    let handle_class = match direction {
        SplitDirection::Vertical => "split-handle vertical",
        SplitDirection::Horizontal => "split-handle horizontal",
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
            class: "{container_class}",

            // First pane
            div {
                class: "split-pane",
                style: "flex: {first_flex};",
                PaneView { pane: first }
            }

            // Resizable handle
            div {
                class: "{handle_class}",
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
    let mut state = use_context::<Signal<AppState>>();
    let mut drag_state = use_context::<Signal<TabDragState>>();
    let tabs_len = tabs.len();

    let is_focused = state.read().focused_pane_id == pane_id;

    // If no tabs, show welcome screen (VSCode behavior)
    if tabs.is_empty() {
        let pane_class = if is_focused { "editor-pane focused" } else { "editor-pane" };
        return rsx! {
            div {
                class: "{pane_class}",
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
    let is_cookbook_tab = active_tab.map(|t| t.is_cookbook()).unwrap_or(false);

    // Check if editor content is a drop target
    let is_content_drop_target = drag_state.read().target
        .map(|t| t.0 == pane_id && t.2)  // t.2 = is_content_area
        .unwrap_or(false);

    // Memoize syntax highlighting - only recalculate when code changes (skip for Settings tab)
    let code_for_highlight = code.clone();
    let highlighted_html = use_memo(use_reactive!(|code_for_highlight| {
        highlight_rhai(&code_for_highlight)
    }));

    let pane_class = if is_focused { "editor-pane focused" } else { "editor-pane" };

    rsx! {
        div {
            class: "{pane_class}",
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
                class: if is_content_drop_target { "editor-content-wrapper drop-target" } else { "editor-content-wrapper" },
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

                // Render panel based on tab type
                if is_settings_tab {
                    SettingsPanel {}
                } else if is_cookbook_tab {
                    CookbookPanel {}
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
    let mut state = use_context::<Signal<AppState>>();
    let mut drag_state = use_context::<Signal<TabDragState>>();
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
                    let is_cookbook = tab.is_cookbook();

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
                    if is_settings { class.push_str(" settings-tab"); }
                    if is_cookbook { class.push_str(" cookbook-tab"); }

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
                                    state.write().close_tab_in_pane(pane_id, tab_id);
                                }
                            },

                            // Left-click to switch tab
                            onclick: move |_| { state.write().switch_to_tab(tab_id); },

                            span { class: "tab-name",
                                // Gear icon for Settings tab
                                if is_settings {
                                    span {
                                        class: "tab-icon settings-icon",
                                        dangerous_inner_html: include_str!("../assets/gear.svg")
                                    }
                                }
                                // Book icon for Cookbook tab
                                if is_cookbook {
                                    span {
                                        class: "tab-icon cookbook-icon",
                                        dangerous_inner_html: include_str!("../assets/book.svg")
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
                                    state.write().close_tab_in_pane(pane_id, tab_id);
                                },
                                "x"
                            }
                        }
                    }
                }
            }
            // New tab button
            button {
                class: "editor-tab new-tab",
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
    let mut state = use_context::<Signal<AppState>>();
    let editor_id = format!("editor-{}", pane_id);

    rsx! {
        div { class: "editor-content",
            // Line numbers
            div {
                id: "line-numbers-{pane_id}",
                class: "line-numbers",
                { render_line_numbers(&code) }
            }
            // Code area
            div {
                id: "code-area-{pane_id}",
                class: "code-area",
                pre {
                    id: "syntax-{pane_id}",
                    class: "syntax-highlight",
                    dangerous_inner_html: "{highlighted_html}"
                }
                textarea {
                    id: "{editor_id}",
                    class: "code-input",
                    spellcheck: false,
                    value: "{code}",
                    onfocus: move |_| { state.write().focus_pane(pane_id); },
                    // Scroll sync handled by native JS in main.rs (no async overhead)
                    oninput: {
                        let editor_id = editor_id.clone();
                        move |evt| {
                            let new_code = evt.value().clone();
                            state.write().set_code(new_code.clone());
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
                        move |evt| {
                            handle_editor_keydown(state, pane_id, active_tab_id, &evt);
                        }
                    },
                }
            }
        }
    }
}

/// Update cursor position from the DOM
fn update_cursor_position(mut state: Signal<AppState>, editor_id: &str, code: &str) {
    let editor_id = editor_id.to_string();
    let code = code.to_string();
    spawn(async move {
        if let Some(pos) = js_interop::get_cursor_position(&editor_id).await {
            let (line, col) = position_to_line_col(&code, pos);
            state.write().set_cursor(line, col);
        }
    });
}

/// Handle keyboard shortcuts in the editor
fn handle_editor_keydown(
    mut state: Signal<AppState>,
    pane_id: PaneId,
    active_tab_id: u64,
    evt: &KeyboardEvent,
) {
    // Ctrl+Enter: Run preview
    if evt.modifiers().ctrl() && evt.key() == Key::Enter {
        state.write().run_preview();
    }

    // Ctrl+Z: Undo
    if evt.modifiers().ctrl()
        && !evt.modifiers().shift()
        && evt.key() == Key::Character("z".to_string())
    {
        evt.prevent_default();
        if let Some((new_content, cursor_pos)) = state.write().undo() {
            spawn(async move {
                js_interop::set_editor_content(pane_id, &new_content, cursor_pos).await;
            });
        }
    }

    // Ctrl+Shift+Z: Redo
    if evt.modifiers().ctrl()
        && evt.modifiers().shift()
        && evt.key() == Key::Character("Z".to_string())
    {
        evt.prevent_default();
        if let Some((new_content, cursor_pos)) = state.write().redo() {
            spawn(async move {
                js_interop::set_editor_content(pane_id, &new_content, cursor_pos).await;
            });
        }
    }

    // Ctrl+N: New tab
    if evt.modifiers().ctrl() && evt.key() == Key::Character("n".to_string()) {
        evt.prevent_default();
        state.write().new_tab_in_pane(pane_id);
    }

    // Ctrl+W: Close tab
    if evt.modifiers().ctrl() && evt.key() == Key::Character("w".to_string()) {
        evt.prevent_default();
        state.write().close_tab_in_pane(pane_id, active_tab_id);
    }

    // Tab: Insert 4 spaces or indent selection
    if evt.key() == Key::Tab && !evt.modifiers().shift() {
        evt.prevent_default();
        spawn(async move {
            js_interop::insert_indent(pane_id).await;
        });
    }
}

/// Render line numbers for the editor
fn render_line_numbers(code: &str) -> Element {
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
