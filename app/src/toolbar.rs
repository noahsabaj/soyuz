//! Top toolbar component with file operations and window controls

// Separate if statements are clearer for async dialog handling
#![allow(clippy::collapsible_if)]
// map_or_else is less readable for UI state
#![allow(clippy::map_unwrap_or)]
// Borrowed format strings are valid
#![allow(clippy::needless_borrows_for_generic_args)]

use crate::preview::spawn_preview;
use crate::state::AppState;
use dioxus::prelude::*;

/// Top toolbar with file operations and window controls
#[component]
pub fn Toolbar() -> Element {
    let state = use_context::<Signal<AppState>>();
    let window = dioxus::desktop::use_window();

    // Clone window for each closure that needs it
    let window_drag = window.clone();
    let window_min = window.clone();
    let window_max = window.clone();
    let window_close = window.clone();

    rsx! {
        div {
            class: "titlebar",
            onmousedown: move |_| { window_drag.drag(); },

            // File operations
            FileOperations { state }

            // Preview and export
            PreviewControls { state }

            // Title
            WindowTitle { state }

            // Window controls
            div { class: "window-controls",
                button {
                    class: "window-button minimize",
                    title: "Minimize",
                    onclick: move |_| window_min.set_minimized(true),
                    onmousedown: |e| e.stop_propagation(),
                    "-"
                }
                button {
                    class: "window-button maximize",
                    title: "Maximize",
                    onclick: {
                        let window_max = window_max.clone();
                        move |_| window_max.set_maximized(!window_max.is_maximized())
                    },
                    onmousedown: |e| e.stop_propagation(),
                    "[]"
                }
                button {
                    class: "window-button close",
                    title: "Close",
                    onclick: move |_| window_close.close(),
                    onmousedown: |e| e.stop_propagation(),
                    "x"
                }
            }
        }
    }
}

/// File operation buttons (New, Open, Save)
#[component]
fn FileOperations(state: Signal<AppState>) -> Element {
    let mut state = state;

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
                onclick: move |_| { trigger_export(state); }
            }
        }
    }
}

/// Window title showing current file
#[component]
fn WindowTitle(state: Signal<AppState>) -> Element {
    let title = {
        let state_read = state.read();
        state_read
            .active_tab()
            .map(|t| {
                let name = t.display_name();
                if t.is_dirty {
                    format!("{} *", name)
                } else {
                    name
                }
            })
            .unwrap_or_else(|| "Untitled".to_string())
    };

    rsx! {
        div { class: "toolbar-title",
            "Soyuz Studio - {title}"
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

/// Trigger mesh export with file dialog
fn trigger_export(state: Signal<AppState>) {
    let settings = state.read().export_settings.clone();
    let code = state.read().code();
    let format = settings.format;

    spawn(async move {
        if let Some(file) = rfd::AsyncFileDialog::new()
            .add_filter(format.name(), &[format.extension()])
            .set_file_name(&format!("export.{}", format.extension()))
            .save_file()
            .await
        {
            let result = tokio::task::spawn_blocking(move || {
                crate::export::export_mesh(&code, file.path(), &settings)
            })
            .await;

            match result {
                Ok(Ok(info)) => {
                    tracing::info!("Export successful: {}", info);
                }
                Ok(Err(e)) => {
                    tracing::error!("Export failed: {}", e);
                }
                Err(e) => {
                    tracing::error!("Export task failed: {}", e);
                }
            }
        }
    });
}
