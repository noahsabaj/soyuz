//! Settings panel UI component
//!
//! Renders a VSCode-style settings panel with search, categories, and controls.

use dioxus::prelude::*;
use tracing::warn;

use crate::settings::{
    all_settings_meta, save_settings, AutoSave, ControlType, SettingCategory,
};
use crate::state::AppState;
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
        div { class: "settings-panel",
            // Search bar
            div { class: "settings-search",
                input {
                    class: "settings-search-input",
                    r#type: "text",
                    placeholder: "Search settings...",
                    value: "{search_query}",
                    oninput: move |evt| search_query.set(evt.value()),
                }
            }

            // Settings content
            div { class: "settings-content",
                // Show "no results" if search has no matches
                if filtered_meta.is_empty() && !query.is_empty() {
                    div { class: "settings-no-results",
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
                                div { class: "settings-section",
                                    div { class: "settings-section-header", "{category.label()}" }

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
    rsx! {
        div { class: "setting-row",
            div { class: "setting-info",
                div { class: "setting-label", "{label}" }
                div { class: "setting-description", "{description}" }
            }
            div { class: "setting-control",
                match control_type {
                    ControlType::Text => {
                        rsx! {
                            TextControl { id }
                        }
                    }
                    ControlType::Number { min, max } => {
                        rsx! {
                            NumberControl { id, min, max }
                        }
                    }
                    ControlType::Checkbox => {
                        rsx! {
                            CheckboxControl { id }
                        }
                    }
                    ControlType::Dropdown { options } => {
                        rsx! {
                            DropdownControl { id, options }
                        }
                    }
                }
            }
        }
    }
}

/// Text input control
#[component]
fn TextControl(id: &'static str) -> Element {
    let mut state = use_context::<Signal<AppState>>();

    let value = {
        let s = state.read();
        match id {
            "font_family" => s.settings.font_family.clone(),
            _ => String::new(),
        }
    };

    rsx! {
        input {
            class: "settings-input",
            r#type: "text",
            value: "{value}",
            onchange: move |evt| {
                let new_value = evt.value();
                {
                    let mut s = state.write();
                    if id == "font_family" {
                        s.settings.font_family = new_value;
                    }
                }
                if let Err(e) = save_settings(&state.read().settings) {
                    warn!("Failed to save settings: {e}");
                }
            },
        }
    }
}

/// Number input control
#[component]
fn NumberControl(id: &'static str, min: u32, max: u32) -> Element {
    let mut state = use_context::<Signal<AppState>>();

    let value = {
        let s = state.read();
        match id {
            "font_size" => s.settings.font_size,
            "tab_size" => s.settings.tab_size,
            "export_resolution" => s.settings.export_resolution,
            "recent_files_limit" => s.settings.recent_files_limit as u32,
            "undo_history_limit" => s.settings.undo_history_limit as u32,
            _ => 0,
        }
    };

    rsx! {
        input {
            class: "settings-input settings-number",
            r#type: "number",
            min: "{min}",
            max: "{max}",
            value: "{value}",
            onchange: move |evt| {
                if let Ok(new_value) = evt.value().parse::<u32>() {
                    let clamped = new_value.clamp(min, max);
                    {
                        let mut s = state.write();
                        match id {
                            "font_size" => s.settings.font_size = clamped,
                            "tab_size" => s.settings.tab_size = clamped,
                            "export_resolution" => s.settings.export_resolution = clamped,
                            "recent_files_limit" => s.settings.recent_files_limit = clamped as usize,
                            "undo_history_limit" => s.settings.undo_history_limit = clamped as usize,
                            _ => {}
                        }
                    }
                    if let Err(e) = save_settings(&state.read().settings) {
                    warn!("Failed to save settings: {e}");
                }
                }
            },
        }
    }
}

/// Checkbox control
#[component]
fn CheckboxControl(id: &'static str) -> Element {
    let mut state = use_context::<Signal<AppState>>();

    let checked = {
        let s = state.read();
        match id {
            "word_wrap" => s.settings.word_wrap,
            "line_numbers" => s.settings.line_numbers,
            "export_optimize" => s.settings.export_optimize,
            "export_close_after" => s.settings.export_close_after,
            "restore_session" => s.settings.restore_session,
            "remember_workspace" => s.settings.remember_workspace,
            _ => false,
        }
    };

    rsx! {
        input {
            class: "settings-checkbox",
            r#type: "checkbox",
            checked: "{checked}",
            onchange: move |evt| {
                let new_value = evt.checked();
                {
                    let mut s = state.write();
                    match id {
                        "word_wrap" => s.settings.word_wrap = new_value,
                        "line_numbers" => s.settings.line_numbers = new_value,
                        "export_optimize" => s.settings.export_optimize = new_value,
                        "export_close_after" => s.settings.export_close_after = new_value,
                        "restore_session" => s.settings.restore_session = new_value,
                        "remember_workspace" => s.settings.remember_workspace = new_value,
                        _ => {}
                    }
                }
                if let Err(e) = save_settings(&state.read().settings) {
                    warn!("Failed to save settings: {e}");
                }
            },
        }
    }
}

/// Dropdown control
#[component]
fn DropdownControl(id: &'static str, options: Vec<(&'static str, &'static str)>) -> Element {
    let mut state = use_context::<Signal<AppState>>();

    let current_value = {
        let s = state.read();
        match id {
            "theme" => s.settings.theme.clone(),
            "auto_save" => match &s.settings.auto_save {
                AutoSave::Off => "off".to_string(),
                AutoSave::AfterDelay(secs) => secs.to_string(),
            },
            "export_format" => match s.settings.export_format {
                ExportFormat::Glb => "glb".to_string(),
                ExportFormat::Gltf => "gltf".to_string(),
                ExportFormat::Obj => "obj".to_string(),
                ExportFormat::Stl => "stl".to_string(),
            },
            "timezone_offset" => s.settings.timezone_offset.to_string(),
            "time_format_24h" => s.settings.time_format_24h.to_string(),
            _ => String::new(),
        }
    };

    rsx! {
        select {
            class: "settings-select",
            value: "{current_value}",
            onchange: move |evt| {
                let new_value = evt.value();
                {
                    let mut s = state.write();
                    match id {
                        "theme" => s.settings.theme = new_value,
                        "auto_save" => {
                            s.settings.auto_save = if new_value == "off" {
                                AutoSave::Off
                            } else if let Ok(secs) = new_value.parse::<u32>() {
                                AutoSave::AfterDelay(secs)
                            } else {
                                AutoSave::Off
                            };
                        }
                        "export_format" => {
                            s.settings.export_format = match new_value.as_str() {
                                "gltf" => ExportFormat::Gltf,
                                "obj" => ExportFormat::Obj,
                                "stl" => ExportFormat::Stl,
                                _ => ExportFormat::Glb,
                            };
                        }
                        "timezone_offset" => {
                            if let Ok(offset) = new_value.parse::<i8>() {
                                s.settings.timezone_offset = offset.clamp(-12, 14);
                            }
                        }
                        "time_format_24h" => {
                            s.settings.time_format_24h = new_value == "true";
                        }
                        _ => {}
                    }
                }
                if let Err(e) = save_settings(&state.read().settings) {
                    warn!("Failed to save settings: {e}");
                }
            },

            for (value, label) in options {
                option {
                    value: "{value}",
                    selected: current_value == value,
                    "{label}"
                }
            }
        }
    }
}
