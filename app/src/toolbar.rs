//! Top toolbar component with file operations and window controls

// Separate if statements are clearer for async dialog handling
#![allow(clippy::collapsible_if)]
// map_or_else is less readable for UI state
#![allow(clippy::map_unwrap_or)]
// Borrowed format strings are valid
#![allow(clippy::needless_borrows_for_generic_args)]

use crate::command_palette::PaletteState;
use crate::preview::spawn_preview;
use crate::state::AppState;
use dioxus::prelude::*;

/// Application logo in the toolbar
#[component]
fn AppLogo() -> Element {
    rsx! {
        div {
            class: "app-logo",
            onmousedown: |e| e.stop_propagation(),
            img {
                src: asset!("/assets/icons/icon-32.png"),
                alt: "Soyuz Studio",
                width: "20",
                height: "20"
            }
        }
    }
}

/// Top toolbar with file operations and window controls
#[component]
pub fn Toolbar() -> Element {
    let state = use_context::<Signal<AppState>>();
    let window = dioxus::desktop::use_window();

    // Clone window for each closure that needs it
    let window_drag = window.clone();
    let window_dblclick = window.clone();
    let window_min = window.clone();
    let window_max = window.clone();
    let window_close = window.clone();

    rsx! {
        div {
            class: "titlebar",
            onmousedown: move |_| { window_drag.drag(); },
            ondoubleclick: move |_| { window_dblclick.set_maximized(!window_dblclick.is_maximized()); },

            // Left side: Logo, file operations and preview controls
            div { class: "titlebar-left",
                AppLogo {}
                FileOperations { state }
                PreviewControls { state }
            }

            // Center: Search bar (fills available space, centers content)
            div { class: "titlebar-center",
                WindowTitle { state }
            }

            // Right side: Window controls
            div { class: "titlebar-right window-controls",
                button {
                    class: "window-button minimize",
                    title: "Minimize",
                    onclick: move |_| window_min.set_minimized(true),
                    onmousedown: |e| e.stop_propagation(),
                    // Minimize icon: horizontal line
                    svg {
                        width: "10",
                        height: "10",
                        view_box: "0 0 10 10",
                        path {
                            d: "M0 5L10 5",
                            stroke: "currentColor",
                            stroke_width: "1.2"
                        }
                    }
                }
                button {
                    class: "window-button maximize",
                    title: "Maximize",
                    onclick: {
                        let window_max = window_max.clone();
                        move |_| window_max.set_maximized(!window_max.is_maximized())
                    },
                    onmousedown: |e| e.stop_propagation(),
                    // Maximize icon: square outline
                    svg {
                        width: "10",
                        height: "10",
                        view_box: "0 0 10 10",
                        rect {
                            x: "0.5",
                            y: "0.5",
                            width: "9",
                            height: "9",
                            fill: "none",
                            stroke: "currentColor",
                            stroke_width: "1.2"
                        }
                    }
                }
                button {
                    class: "window-button close",
                    title: "Close",
                    onclick: move |_| window_close.close(),
                    onmousedown: |e| e.stop_propagation(),
                    // Close icon: X shape
                    svg {
                        width: "10",
                        height: "10",
                        view_box: "0 0 10 10",
                        path {
                            d: "M0 0L10 10M10 0L0 10",
                            stroke: "currentColor",
                            stroke_width: "1.2"
                        }
                    }
                }
            }
        }
    }
}

/// File operation buttons (New, Open, Save, etc.)
#[component]
fn FileOperations(state: Signal<AppState>) -> Element {
    let mut state = state;
    let has_workspace = state.read().has_workspace();

    rsx! {
        div { class: "toolbar-group",
            ToolbarButton {
                title: "New file (Ctrl+N)",
                label: "New",
                onclick: move |_| { state.write().new_tab(); }
            }
            ToolbarButton {
                title: "Open file (Ctrl+O)",
                label: "Open",
                onclick: move |_| {
                    spawn(async move {
                        if let Some(path) = rfd::AsyncFileDialog::new()
                            .add_filter("Rhai Scripts", &["rhai"])
                            .pick_file()
                            .await
                        {
                            if let Ok(content) = tokio::fs::read_to_string(path.path()).await {
                                state.write().open_file(path.path().to_path_buf(), content);
                            }
                        }
                    });
                }
            }
            ToolbarButton {
                title: "Save file (Ctrl+S)",
                label: "Save",
                onclick: move |_| { save_current_file(state); }
            }
        }
        div { class: "toolbar-group",
            ToolbarButton {
                title: "Open a new window",
                label: "New Window",
                onclick: move |_| { spawn_new_window(); }
            }
            if has_workspace {
                ToolbarButton {
                    title: "Close the current folder",
                    label: "Close Folder",
                    onclick: move |_| { state.write().close_folder(); }
                }
            }
        }
    }
}

/// Spawn a new Soyuz Studio window (fresh session)
fn spawn_new_window() {
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::process::Command::new(exe)
            .arg("--fresh")
            .spawn();
    }
}

/// Preview and export controls
#[component]
fn PreviewControls(state: Signal<AppState>) -> Element {
    let mut state = state;
    let is_previewing = state.read().is_previewing;

    rsx! {
        div { class: "toolbar-group",
            if is_previewing {
                ToolbarButton {
                    title: "Stop preview",
                    label: "Stop",
                    class: "stop",
                    onclick: move |_| { state.write().stop_preview(); }
                }
            } else {
                ToolbarButton {
                    title: "Run preview (Ctrl+Enter)",
                    label: "Preview",
                    onclick: move |_| { spawn_preview(state); }
                }
            }
            ToolbarButton {
                title: "Export mesh",
                label: "Export",
                onclick: move |_| { crate::export::open_export_window(state); }
            }
        }
    }
}

/// Search bar in toolbar - opens command palette when clicked
#[component]
fn WindowTitle(state: Signal<AppState>) -> Element {
    let mut palette = use_context::<Signal<PaletteState>>();

    // Get workspace name for display
    let workspace_name = state
        .read()
        .workspace
        .as_ref()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "Soyuz Studio".to_string());

    let open_palette = move |_| {
        palette.write().visible = true;
        palette.write().query.clear();
    };

    rsx! {
        div {
            class: "toolbar-search-bar",
            onclick: open_palette,
            onmousedown: |e| e.stop_propagation(), // Don't drag window

            span { class: "search-icon", "" }
            span { class: "search-placeholder", "{workspace_name}" }
        }
    }
}

/// Reusable toolbar button component
#[component]
fn ToolbarButton(
    title: &'static str,
    label: &'static str,
    onclick: EventHandler<MouseEvent>,
    #[props(default = "")] class: &'static str,
) -> Element {
    let button_class = if class.is_empty() {
        "toolbar-button".to_string()
    } else {
        format!("toolbar-button {}", class)
    };

    rsx! {
        button {
            class: "{button_class}",
            title: "{title}",
            onclick: move |evt| onclick.call(evt),
            onmousedown: |e| e.stop_propagation(),
            "{label}"
        }
    }
}

/// Save the current file (or show save dialog for untitled)
fn save_current_file(mut state: Signal<AppState>) {
    let current_file = state.read().current_file();
    let code = state.read().code();

    if let Some(path) = current_file {
        spawn(async move {
            if tokio::fs::write(&path, &code).await.is_ok() {
                state.write().mark_saved(None);
            }
        });
    } else {
        spawn(async move {
            if let Some(path) = rfd::AsyncFileDialog::new()
                .add_filter("Rhai Scripts", &["rhai"])
                .set_file_name("untitled.rhai")
                .save_file()
                .await
            {
                let code = state.read().code();
                if tokio::fs::write(path.path(), &code).await.is_ok() {
                    state.write().mark_saved(Some(path.path().to_path_buf()));
                }
            }
        });
    }
}

