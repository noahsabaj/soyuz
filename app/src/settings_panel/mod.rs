//! Settings panel UI component
//!
//! Renders a VSCode-style settings panel with search, categories, and controls.

use dioxus::prelude::*;
use tracing::warn;

use crate::settings::{
    AutoSave, ControlType, SettingCategory, SettingsStoreExt, all_settings_meta, save_settings,
};
use crate::state::{AppStateStoreExt, AppStore};
use soyuz_core::export::ExportFormat;

/// Main settings panel component
#[component]
pub fn SettingsPanel() -> Element {
    let mut search_query = use_signal(String::new);

    // Get all settings metadata
    let all_meta = all_settings_meta();

    // Filter settings based on search query
    let query = search_query.read().to_lowercase();
    let filtered_meta: Vec<_> = if query.is_empty() {
        all_meta
    } else {
        all_meta
            .into_iter()
            .filter(|m| {
                m.label.to_lowercase().contains(&query)
                    || m.description.to_lowercase().contains(&query)
                    || m.id.to_lowercase().contains(&query)
            })
            .collect()
    };

    // Group by category
    let categories = SettingCategory::all();

    rsx! {
        div { class: "settings-panel-content",
            // Search bar
            div { class: "search",
                input {
                    class: "search-input",
                    r#type: "text",
                    placeholder: "Search settings...",
                    value: "{search_query}",
                    oninput: move |evt| search_query.set(evt.value()),
                }
            }

            // Settings content
            div { class: "content",
                // Show "no results" if search has no matches
                if filtered_meta.is_empty() && !query.is_empty() {
                    div { class: "no-results",
                        "No settings found for \"{query}\""
                    }
                }

                // Render each category
                for category in categories {
                    {
                        let category_settings: Vec<_> = filtered_meta
                            .iter()
                            .filter(|m| m.category == *category)
                            .collect();

                        if category_settings.is_empty() {
                            rsx! {}
                        } else {
                            rsx! {
                                div { class: "section",
                                    div { class: "section-header", "{category.label()}" }

                                    for setting in category_settings {
                                        SettingRow {
                                            key: "{setting.id}",
                                            id: setting.id,
                                            label: setting.label,
                                            description: setting.description,
                                            control_type: setting.control_type.clone(),
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Individual setting row with label, description, and control
#[component]
fn SettingRow(
    id: &'static str,
    label: &'static str,
    description: &'static str,
    control_type: ControlType,
) -> Element {
    use dioxus_primitives::label::Label;

    let state = use_context::<AppStore>();
    rsx! {
        div { class: "row",
            div { class: "info",
                // `Label`'s `html_for` ties the visible name to the control's `id`
                // (each control forwards the setting id to its focusable element).
                Label { class: "label", html_for: id, "{label}" }
                div { class: "description", "{description}" }
            }
            div { class: "control",
                {setting_control(id, label, &control_type, state)}
            }
        }
    }
}

/// Bind a single setting to its control in one place.
///
/// Each arm reads the current value through a field selector (fine-grained
/// reactivity: this row only re-renders when *its* field changes) and constructs
/// an `on_change` handler that writes back through the same selector, then
/// schedules a debounced off-thread save. This replaces the old pattern where
/// every control matched `id` twice — once to read and once to write.
#[allow(clippy::too_many_lines)] // One arm per setting; splitting hurts readability.
fn setting_control(
    id: &'static str,
    label: &'static str,
    control_type: &ControlType,
    state: AppStore,
) -> Element {
    // Control-shape parameters still come from the metadata (single source of truth).
    let (num_min, num_max) = match control_type {
        ControlType::Number { min, max } => (*min, *max),
        _ => (u32::MIN, u32::MAX),
    };
    let options = match control_type {
        ControlType::Dropdown { options } => options.clone(),
        _ => Vec::new(),
    };

    match id {
        // --- Text ---
        "font_family" => rsx! {
            TextControl {
                id,
                value: state.settings().font_family().cloned(),
                on_change: move |v: String| {
                    state.settings().font_family().set(v);
                    schedule_save(state);
                },
            }
        },

        // --- Slider ---
        "font_size" => rsx! {
            SliderControl {
                id,
                label,
                value: state.settings().font_size().cloned(),
                min: num_min,
                max: num_max,
                on_change: move |v: u32| {
                    state.settings().font_size().set(v);
                    schedule_save(state);
                },
            }
        },

        // --- Number ---
        "tab_size" => rsx! {
            NumberControl {
                id,
                value: state.settings().tab_size().cloned(),
                min: num_min,
                max: num_max,
                on_change: move |v: u32| {
                    state.settings().tab_size().set(v);
                    schedule_save(state);
                },
            }
        },
        "export_resolution" => rsx! {
            NumberControl {
                id,
                value: state.settings().export_resolution().cloned(),
                min: num_min,
                max: num_max,
                on_change: move |v: u32| {
                    state.settings().export_resolution().set(v);
                    schedule_save(state);
                },
            }
        },
        "recent_files_limit" => rsx! {
            NumberControl {
                id,
                value: state.settings().recent_files_limit().cloned() as u32,
                min: num_min,
                max: num_max,
                on_change: move |v: u32| {
                    state.settings().recent_files_limit().set(v as usize);
                    schedule_save(state);
                },
            }
        },
        "undo_history_limit" => rsx! {
            NumberControl {
                id,
                value: state.settings().undo_history_limit().cloned() as u32,
                min: num_min,
                max: num_max,
                on_change: move |v: u32| {
                    state.settings().undo_history_limit().set(v as usize);
                    schedule_save(state);
                },
            }
        },

        // --- Switch ---
        "word_wrap" => rsx! {
            SwitchControl {
                id,
                value: state.settings().word_wrap().cloned(),
                on_change: move |v: bool| {
                    state.settings().word_wrap().set(v);
                    schedule_save(state);
                },
            }
        },
        "line_numbers" => rsx! {
            SwitchControl {
                id,
                value: state.settings().line_numbers().cloned(),
                on_change: move |v: bool| {
                    state.settings().line_numbers().set(v);
                    schedule_save(state);
                },
            }
        },
        "export_optimize" => rsx! {
            SwitchControl {
                id,
                value: state.settings().export_optimize().cloned(),
                on_change: move |v: bool| {
                    state.settings().export_optimize().set(v);
                    schedule_save(state);
                },
            }
        },
        "restore_session" => rsx! {
            SwitchControl {
                id,
                value: state.settings().restore_session().cloned(),
                on_change: move |v: bool| {
                    state.settings().restore_session().set(v);
                    schedule_save(state);
                },
            }
        },
        "remember_workspace" => rsx! {
            SwitchControl {
                id,
                value: state.settings().remember_workspace().cloned(),
                on_change: move |v: bool| {
                    state.settings().remember_workspace().set(v);
                    schedule_save(state);
                },
            }
        },

        // --- Dropdown ---
        "theme" => rsx! {
            DropdownControl {
                id,
                value: state.settings().theme().cloned(),
                options,
                on_change: move |v: String| {
                    state.settings().theme().set(v);
                    schedule_save(state);
                },
            }
        },
        "auto_save" => {
            let value = match state.settings().auto_save().cloned() {
                AutoSave::Off => "off".to_string(),
                AutoSave::AfterDelay(secs) => secs.to_string(),
            };
            rsx! {
                DropdownControl {
                    id,
                    value,
                    options,
                    on_change: move |v: String| {
                        let parsed = if v == "off" {
                            AutoSave::Off
                        } else if let Ok(secs) = v.parse::<u32>() {
                            AutoSave::AfterDelay(secs)
                        } else {
                            AutoSave::Off
                        };
                        state.settings().auto_save().set(parsed);
                        schedule_save(state);
                    },
                }
            }
        }
        "export_format" => {
            let value = match state.settings().export_format().cloned() {
                ExportFormat::Glb => "glb",
                ExportFormat::Gltf => "gltf",
                ExportFormat::Obj => "obj",
                ExportFormat::Stl => "stl",
            }
            .to_string();
            rsx! {
                DropdownControl {
                    id,
                    value,
                    options,
                    on_change: move |v: String| {
                        let format = match v.as_str() {
                            "gltf" => ExportFormat::Gltf,
                            "obj" => ExportFormat::Obj,
                            "stl" => ExportFormat::Stl,
                            _ => ExportFormat::Glb,
                        };
                        state.settings().export_format().set(format);
                        schedule_save(state);
                    },
                }
            }
        }
        "timezone_offset" => {
            let value = state.settings().timezone_offset().cloned().to_string();
            rsx! {
                DropdownControl {
                    id,
                    value,
                    options,
                    on_change: move |v: String| {
                        if let Ok(offset) = v.parse::<i8>() {
                            state.settings().timezone_offset().set(offset.clamp(-12, 14));
                            schedule_save(state);
                        }
                    },
                }
            }
        }
        "time_format_24h" => {
            let value = state.settings().time_format_24h().cloned().to_string();
            rsx! {
                DropdownControl {
                    id,
                    value,
                    options,
                    on_change: move |v: String| {
                        state.settings().time_format_24h().set(v == "true");
                        schedule_save(state);
                    },
                }
            }
        }

        _ => rsx! {},
    }
}

/// Persist settings off the UI thread.
///
/// The change has already been applied to the store, so we snapshot the current
/// settings synchronously (never lost) and hand the blocking `save_settings` to a
/// blocking thread. `spawn_blocking` runs its closure to completion even if this
/// task is later dropped (e.g. the Settings tab closes), so the write always lands
/// — unlike a debounced timer, which could be cancelled mid-wait. This replaces
/// the old synchronous `save_settings` call that ran on the UI thread inside every
/// `onchange` handler.
fn schedule_save(state: AppStore) {
    let settings = state.peek().settings.clone();
    spawn(async move {
        match tokio::task::spawn_blocking(move || save_settings(&settings)).await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => warn!("Failed to save settings: {e}"),
            Err(e) => warn!("Settings save task failed: {e}"),
        }
    });
}

/// Text input control. Presentational: reads its value and reports changes
/// through props, so the caller owns the single read/write binding.
#[component]
fn TextControl(id: &'static str, value: String, on_change: EventHandler<String>) -> Element {
    rsx! {
        input {
            id,
            class: "input",
            r#type: "text",
            value: "{value}",
            onchange: move |evt| on_change.call(evt.value()),
        }
    }
}

/// Number input control (clamps to `[min, max]` before reporting).
#[component]
fn NumberControl(
    id: &'static str,
    value: u32,
    min: u32,
    max: u32,
    on_change: EventHandler<u32>,
) -> Element {
    rsx! {
        input {
            id,
            class: "input number",
            r#type: "number",
            min: "{min}",
            max: "{max}",
            value: "{value}",
            onchange: move |evt| {
                if let Ok(parsed) = evt.value().parse::<u32>() {
                    on_change.call(parsed.clamp(min, max));
                }
            },
        }
    }
}

/// Boolean toggle, built on the `dioxus_primitives` `Switch` (reads better than a
/// checkbox for on/off preferences). The store value is mirrored into a stable,
/// reactive signal so the primitive's controlled state tracks external changes.
#[component]
fn SwitchControl(id: &'static str, value: bool, on_change: EventHandler<bool>) -> Element {
    use dioxus_primitives::switch::{Switch, SwitchThumb};

    let checked = use_memo(use_reactive!(|value| Some(value)));

    rsx! {
        Switch {
            id,
            class: "settings-switch",
            checked,
            on_checked_change: move |v: bool| on_change.call(v),
            SwitchThumb { class: "settings-switch-thumb" }
        }
    }
}

/// Numeric control rendered as a `dioxus_primitives` `Slider` (used where a slider
/// reads better than a number field, e.g. font size). The primitive snaps to
/// `step` and clamps to `[min, max]`; the live value is shown alongside it.
#[component]
fn SliderControl(
    id: &'static str,
    label: &'static str,
    value: u32,
    min: u32,
    max: u32,
    on_change: EventHandler<u32>,
) -> Element {
    use dioxus_primitives::slider::{Slider, SliderRange, SliderThumb, SliderTrack};

    let current = use_memo(use_reactive!(|value| Some(value as f64)));

    rsx! {
        div { class: "settings-slider-wrap",
            Slider {
                class: "settings-slider",
                value: current,
                min: min as f64,
                max: max as f64,
                step: 1.0,
                label,
                on_value_change: move |v: f64| on_change.call(v as u32),
                SliderTrack { class: "settings-slider-track",
                    SliderRange { class: "settings-slider-range" }
                    SliderThumb { id, class: "settings-slider-thumb" }
                }
            }
            span { class: "settings-slider-value", "{value}" }
        }
    }
}

/// Enum/string choice, built on the `dioxus_primitives` `Select`. Reads the
/// current value and reports the chosen option through props. `text_value` carries
/// the human-readable label so the trigger shows it (the primitive otherwise falls
/// back to the programmatic option value).
#[component]
fn DropdownControl(
    id: &'static str,
    value: String,
    options: Vec<(&'static str, &'static str)>,
    on_change: EventHandler<String>,
) -> Element {
    use dioxus_primitives::select::{
        Select, SelectItemIndicator, SelectList, SelectOption, SelectTrigger, SelectValue,
    };

    let selected = use_memo(use_reactive!(|value| Some(value)));

    rsx! {
        Select::<String> {
            class: "settings-select",
            value: Some(ReadSignal::from(selected)),
            on_value_change: move |v: Option<String>| {
                if let Some(v) = v {
                    on_change.call(v);
                }
            },
            SelectTrigger { id, class: "settings-select-trigger",
                SelectValue {}
                svg {
                    class: "settings-select-icon",
                    width: "12",
                    height: "12",
                    view_box: "0 0 12 12",
                    path {
                        d: "M2 4l4 4 4-4",
                        fill: "none",
                        stroke: "currentColor",
                        stroke_width: "1.5",
                    }
                }
            }
            SelectList { class: "settings-select-list",
                for (index, (option_value, option_label)) in options.into_iter().enumerate() {
                    SelectOption::<String> {
                        key: "{option_value}",
                        index,
                        value: option_value,
                        text_value: option_label,
                        class: "settings-select-option",
                        "{option_label}"
                        SelectItemIndicator {
                            span { class: "settings-select-indicator", "✓" }
                        }
                    }
                }
            }
        }
    }
}
