//! Terminal panel component for displaying application output
//!
//! Provides a VSCode-style bottom-docked panel with:
//! - Collapsible/expandable behavior
//! - Resizable via drag handle
//! - Clear button and output display
//! - Colored log levels with timestamps
//! - Filter dropdown for level-based filtering

pub mod layer;

use crate::state::{AppState, TerminalEntry, TerminalLevel};
use dioxus::prelude::*;

/// Resize state for terminal panel drag operation
#[derive(Clone, Copy, Default)]
struct TerminalResizeState {
    active: bool,
    start_y: f64,
    start_height: f32,
}

/// Local state for filter dropdown visibility
#[derive(Clone, Copy, Default)]
struct FilterDropdownState {
    open: bool,
}

/// Terminal panel component
#[component]
pub fn TerminalPanel() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let mut resize_state = use_signal(TerminalResizeState::default);

    let visible = state.read().terminal_visible;
    let height = state.read().terminal_height;
    let filter = state.read().terminal_filter.clone();
    let entries = state.read().terminal_buffer.snapshot();

    // Filter entries based on current filter settings
    let filtered_entries: Vec<TerminalEntry> = entries
        .into_iter()
        .filter(|e| filter.should_show(e.level))
        .collect();

    // Don't render if collapsed
    if !visible {
        return rsx! {};
    }

    rsx! {
        // Resize handle at top of terminal
        div {
            class: "resize-handle",
            onmousedown: move |evt| {
                evt.prevent_default();
                resize_state.set(TerminalResizeState {
                    active: true,
                    start_y: evt.client_coordinates().y,
                    start_height: height,
                });
            },
        }

        // Resize overlay during drag
        if resize_state.read().active {
            div {
                class: "resize-overlay",
                onmousemove: move |evt| {
                    let rs = *resize_state.read();
                    if rs.active {
                        let delta = rs.start_y - evt.client_coordinates().y;
                        let new_height = rs.start_height + delta as f32;
                        state.write().set_terminal_height(new_height);
                    }
                },
                onmouseup: move |_| {
                    resize_state.set(TerminalResizeState::default());
                },
            }
        }

        // Terminal panel container
        div {
            class: "terminal-panel",
            style: "height: {height}px;",

            // Header bar
            TerminalHeader {}

            // Output content
            div {
                class: "content",
                for (idx, entry) in filtered_entries.iter().enumerate() {
                    TerminalEntryRow { key: "{idx}", entry: entry.clone() }
                }
            }
        }
    }
}

/// Terminal header bar with title and actions
#[component]
fn TerminalHeader() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let mut filter_dropdown = use_signal(FilterDropdownState::default);

    let filter = state.read().terminal_filter.clone();
    let dropdown_open = filter_dropdown.read().open;

    rsx! {
        div { class: "header",
            // Left: Title
            div { class: "header-left",
                span { class: "title", "OUTPUT" }
            }

            // Right: Action buttons
            div { class: "header-right",
                // Filter button with dropdown
                div { class: "filter-container",
                    button {
                        class: if dropdown_open { "action-btn active" } else { "action-btn" },
                        title: "Filter Output",
                        onclick: move |_| {
                            let current = filter_dropdown.read().open;
                            filter_dropdown.write().open = !current;
                        },
                        // Filter icon (funnel)
                        svg {
                            width: "14",
                            height: "14",
                            view_box: "0 0 14 14",
                            fill: "none",
                            stroke: "currentColor",
                            stroke_width: "1.2",
                            path { d: "M1 2h12l-4.5 5v4l-3 2v-6L1 2z" }
                        }
                    }

                    // Dropdown menu
                    if dropdown_open {
                        div { class: "filter-dropdown",
                            // Click outside to close
                            div {
                                class: "filter-backdrop",
                                onclick: move |_| {
                                    filter_dropdown.write().open = false;
                                }
                            }
                            div { class: "filter-menu",
                                // Info checkbox
                                label { class: "filter-item",
                                    input {
                                        r#type: "checkbox",
                                        checked: filter.show_info,
                                        onchange: move |_| {
                                            state.write().toggle_terminal_filter(TerminalLevel::Info);
                                        }
                                    }
                                    span { class: "filter-label info", "Info" }
                                }
                                // Warn checkbox
                                label { class: "filter-item",
                                    input {
                                        r#type: "checkbox",
                                        checked: filter.show_warn,
                                        onchange: move |_| {
                                            state.write().toggle_terminal_filter(TerminalLevel::Warn);
                                        }
                                    }
                                    span { class: "filter-label warn", "Warning" }
                                }
                                // Error checkbox
                                label { class: "filter-item",
                                    input {
                                        r#type: "checkbox",
                                        checked: filter.show_error,
                                        onchange: move |_| {
                                            state.write().toggle_terminal_filter(TerminalLevel::Error);
                                        }
                                    }
                                    span { class: "filter-label error", "Error" }
                                }
                            }
                        }
                    }
                }

                // Clear button
                button {
                    class: "action-btn",
                    title: "Clear Output",
                    onclick: move |_| { state.read().terminal_clear(); },
                    // Trash icon (inline SVG)
                    svg {
                        width: "14",
                        height: "14",
                        view_box: "0 0 14 14",
                        fill: "none",
                        stroke: "currentColor",
                        stroke_width: "1.2",
                        path { d: "M2 4h10M5 4V2h4v2M3 4v8a1 1 0 001 1h6a1 1 0 001-1V4" }
                    }
                }
                // Collapse button
                button {
                    class: "action-btn",
                    title: "Close Panel",
                    onclick: move |_| { state.write().toggle_terminal(); },
                    // Chevron down icon (inline SVG)
                    svg {
                        width: "14",
                        height: "14",
                        view_box: "0 0 14 14",
                        fill: "none",
                        stroke: "currentColor",
                        stroke_width: "1.5",
                        path { d: "M3 5l4 4 4-4" }
                    }
                }
            }
        }
    }
}

/// Single terminal entry row
#[component]
fn TerminalEntryRow(entry: TerminalEntry) -> Element {
    let state = use_context::<Signal<AppState>>();

    // Get time settings
    let timezone_offset = state.read().settings.timezone_offset;
    let use_24h = state.read().settings.time_format_24h;

    // Format timestamp using settings
    let timestamp = entry.format_timestamp(timezone_offset, use_24h);
    let level_prefix = entry.level.prefix();
    let message = &entry.message;

    // Map level to CSS class
    let level_class = match entry.level {
        TerminalLevel::Trace => "trace",
        TerminalLevel::Debug => "debug",
        TerminalLevel::Info => "info",
        TerminalLevel::Warn => "warn",
        TerminalLevel::Error => "error",
    };

    // Pre-compute combined entry class
    let entry_class = format!("entry {}", level_class);

    rsx! {
        div { class: "{entry_class}",
            span { class: "timestamp", "[{timestamp}]" }
            span { class: "level", "{level_prefix}" }
            span { class: "message", "{message}" }
        }
    }
}
