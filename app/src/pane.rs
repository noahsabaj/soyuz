//! Recursive pane rendering for split editor views

use crate::js_interop::{self, position_to_line_col};
use crate::state::{AppState, EditorPane, EditorTab, PaneId, SplitDirection};
use dioxus::prelude::*;

/// Render the entire pane tree recursively
#[component]
pub fn PaneTree() -> Element {
    let state = use_context::<Signal<AppState>>();
    let pane = state.read().editor_pane.clone();

    rsx! {
        div { class: "pane-tree",
            PaneView { pane }
        }
    }
}

/// Recursive component to render a pane (either a TabGroup or a Split)
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
            let class = match direction {
                SplitDirection::Horizontal => "split-container horizontal",
                SplitDirection::Vertical => "split-container vertical",
            };
            let first_style = format!("flex: {};", ratio);
            let second_style = format!("flex: {};", 1.0 - ratio);

            rsx! {
                div { class: "{class}",
                    div {
                        class: "split-pane",
                        style: "{first_style}",
                        PaneView { pane: *first }
                    }
                    div { class: "split-handle" }
                    div {
                        class: "split-pane",
                        style: "{second_style}",
                        PaneView { pane: *second }
                    }
                }
            }
        }
    }
}

/// A single tab group pane with tabs and editor
#[component]
fn TabGroupPane(pane_id: PaneId, tabs: Vec<EditorTab>, active_tab_idx: usize) -> Element {
    let mut state = use_context::<Signal<AppState>>();

    let is_focused = state.read().focused_pane_id == pane_id;
    let active_tab = tabs.get(active_tab_idx);
    let code = active_tab.map(|t| t.content.clone()).unwrap_or_default();
    let active_tab_id = active_tab.map(|t| t.id).unwrap_or(0);

    let highlighted_html = highlight_rhai(&code);

    rsx! {
        div {
            class: if is_focused { "editor-pane focused" } else { "editor-pane" },
            onclick: move |_| { state.write().focus_pane(pane_id); },

            // Tab bar
            TabBar {
                pane_id,
                tabs: tabs.clone(),
                active_tab_id,
                is_focused,
            }

            // Editor content
            EditorArea {
                pane_id,
                code,
                active_tab_id,
                highlighted_html,
            }
        }
    }
}

/// Tab bar with tabs and action buttons
#[component]
fn TabBar(pane_id: PaneId, tabs: Vec<EditorTab>, active_tab_id: u64, is_focused: bool) -> Element {
    let mut state = use_context::<Signal<AppState>>();

    rsx! {
        div { class: "editor-tabs",
            for tab in tabs.iter() {
                {
                    let tab_id = tab.id;
                    let name = tab.display_name();
                    let is_dirty = tab.is_dirty;
                    let is_active = tab_id == active_tab_id;

                    rsx! {
                        div {
                            key: "{tab_id}",
                            class: if is_active { "editor-tab active" } else { "editor-tab" },
                            onclick: move |_| { state.write().switch_to_tab(tab_id); },
                            span { class: "tab-name",
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
            // Split buttons (only show if focused)
            if is_focused {
                div { class: "tab-actions",
                    button {
                        class: "tab-action-btn",
                        title: "Split vertically",
                        onclick: move |_| {
                            state.write().split_pane_by_id(pane_id, SplitDirection::Vertical);
                        },
                        "|"
                    }
                    button {
                        class: "tab-action-btn",
                        title: "Split horizontally",
                        onclick: move |_| {
                            state.write().split_pane_by_id(pane_id, SplitDirection::Horizontal);
                        },
                        "-"
                    }
                    // Close pane button (only if more than one pane)
                    if state.read().editor_pane.all_pane_ids().len() > 1 {
                        button {
                            class: "tab-action-btn",
                            title: "Close this pane",
                            onclick: move |_| { state.write().close_pane(pane_id); },
                            "x"
                        }
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

    // Ctrl+\: Split pane
    if evt.modifiers().ctrl() && evt.key() == Key::Character("\\".to_string()) {
        evt.prevent_default();
        if evt.modifiers().shift() {
            state
                .write()
                .split_pane_by_id(pane_id, SplitDirection::Horizontal);
        } else {
            state
                .write()
                .split_pane_by_id(pane_id, SplitDirection::Vertical);
        }
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
