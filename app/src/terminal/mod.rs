//! Terminal panel component for displaying application output
//!
//! Provides a VSCode-style bottom-docked panel with:
//! - Collapsible/expandable behavior
//! - Resizable via drag handle
//! - Clear button and output display
//! - Colored log levels with timestamps
//! - Filter dropdown for level-based filtering

pub mod layer;

use crate::services::AppServices;
use crate::state::{AppStateStoreExt, AppStore, TerminalEntry, TerminalLevel};
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
    let state = use_context::<AppStore>();
    let services = use_context::<AppServices>();
    let mut resize_state = use_signal(TerminalResizeState::default);

    // F72: mirror the (non-reactive) terminal buffer into a reactive signal so
    // new log lines re-render the panel live. A background task refreshes the
    // signal whenever the buffer wakes it. Interest is registered *before* each
    // snapshot (register-before-snapshot), so a push that races the snapshot is
    // captured by the next await instead of being lost — this removes the old
    // 250ms fallback poll entirely.
    let mut entries = use_signal(|| services.terminal_snapshot_with_ids());
    let services_for_task = services.clone();
    use_future(move || {
        let services = services_for_task.clone();
        async move {
            let notifier = services.terminal_notifier();
            loop {
                // Register with the notifier before snapshotting so a change
                // landing during/after the snapshot still completes this await.
                let notified = notifier.notified();
                tokio::pin!(notified);
                notified.as_mut().enable();
                entries.set(services.terminal_snapshot_with_ids());
                notified.await;
            }
        }
    });

    // F73: read each field through a store selector so the panel only re-renders
    // when that specific field changes, not on any unrelated root-state write.
    let visible = *state.terminal_visible().read();
    let height = *state.terminal_height().read();
    let filter = state.terminal_filter().read().clone();

    // Read the two time-display settings once here and pass them to each row as
    // props, instead of every row reading the store (up to 1000 reads/render).
    let (timezone_offset, use_24h) = {
        let settings = state.settings();
        let s = settings.read();
        (s.timezone_offset, s.time_format_24h)
    };

    // Filter entries based on current filter settings, keeping each entry's
    // stable id so list rows stay correctly associated after a front-trim.
    let filtered_entries: Vec<(u64, TerminalEntry)> = entries
        .read()
        .iter()
        .filter(|(_, e)| filter.should_show(e.level))
        .cloned()
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
                        // Write through the field selector so a drag re-renders
                        // only terminal-height subscribers, not the whole app.
                        // Clamp is preserved from the former `set_terminal_height`.
                        state.terminal_height().set(new_height.clamp(100.0, 500.0));
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
                for (id, entry) in filtered_entries.iter() {
                    TerminalEntryRow {
                        key: "{id}",
                        entry: entry.clone(),
                        timezone_offset,
                        use_24h,
                    }
                }
            }
        }
    }
}

/// Terminal header bar with title and actions
#[component]
fn TerminalHeader() -> Element {
    let mut state = use_context::<AppStore>();
    let services = use_context::<AppServices>();
    let mut filter_dropdown = use_signal(FilterDropdownState::default);

    // F73: read the filter through a field selector so header re-renders track
    // only `terminal_filter`, not unrelated root-state changes.
    let filter = state.terminal_filter().read().clone();
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
                                            state.terminal_filter().write().toggle(TerminalLevel::Info);
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
                                            state.terminal_filter().write().toggle(TerminalLevel::Warn);
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
                                            state.terminal_filter().write().toggle(TerminalLevel::Error);
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
                    onclick: move |_| { services.terminal_clear(); },
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
///
/// Time-display settings are passed in as props (read once in `TerminalPanel`)
/// so each row is a pure function of its props and never reads the store — with
/// up to 1000 rows that avoids up to 1000 store reads per render.
#[component]
fn TerminalEntryRow(entry: TerminalEntry, timezone_offset: i8, use_24h: bool) -> Element {
    // Format timestamp using the settings supplied by the parent.
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
