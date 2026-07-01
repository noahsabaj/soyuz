//! Application settings management
//!
//! Handles loading, saving, and managing user preferences that persist across sessions.
//! Settings are stored in `{config_dir}/soyuz/settings.json`.

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use soyuz_core::export::ExportFormat;
use std::fs;
use std::path::PathBuf;

const DEFAULT_THEME: &str = "soyuz-graphite";
const LEGACY_DARK_THEME: &str = "Dark";

/// Current settings schema version (bump when adding migrations).
const SETTINGS_VERSION: u32 = 1;

/// Auto-save behavior for the editor
#[derive(Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum AutoSave {
    /// Auto-save disabled
    #[default]
    Off,
    /// Auto-save after a delay (in seconds)
    AfterDelay(u32),
}

impl AutoSave {
    /// Auto-save interval in seconds, or `None` when auto-save is disabled.
    ///
    /// Drives the cadence of `main.rs`'s periodic session autosave loop.
    pub fn interval_secs(&self) -> Option<u32> {
        match self {
            AutoSave::Off => None,
            AutoSave::AfterDelay(secs) => Some(*secs),
        }
    }
}

/// Application settings that persist across sessions
#[allow(clippy::struct_excessive_bools)]
// Config structs naturally have many bool fields
// `dioxus_stores::Store` generates per-field selectors (`state.settings().font_size()`,
// etc.) so the settings UI can read/write one field at a time with fine-grained
// reactivity instead of reading and writing the whole store.
#[derive(Clone, PartialEq, Serialize, Deserialize, dioxus_stores::Store)]
// `#[serde(default)]` at the container level means any missing field falls back to
// its individual default instead of resetting every setting, so older config files
// keep their values when new fields are added.
#[serde(default)]
pub struct Settings {
    /// Settings schema version, for future migrations
    pub version: u32,

    // Editor settings
    /// Font family for the editor
    pub font_family: String,
    /// Font size in pixels
    pub font_size: u32,
    /// Number of spaces per tab
    pub tab_size: u32,
    /// Whether to wrap long lines
    pub word_wrap: bool,
    /// Whether to show line numbers
    pub line_numbers: bool,
    /// Auto-save behavior
    pub auto_save: AutoSave,

    // Theme settings
    /// Current theme name
    pub theme: String,

    // Export defaults
    /// Default export format
    pub export_format: ExportFormat,
    /// Default mesh resolution
    pub export_resolution: u32,
    /// Whether to optimize mesh by default
    pub export_optimize: bool,
    /// Whether to close export window after exporting
    pub export_close_after: bool,

    // Application settings
    /// Whether to restore session on startup
    pub restore_session: bool,
    /// Whether to remember last workspace
    pub remember_workspace: bool,
    /// Maximum number of recent files to keep
    pub recent_files_limit: usize,
    /// Maximum number of undo steps per tab
    pub undo_history_limit: usize,

    // Time display settings
    /// Timezone offset from UTC in hours (-12 to +14)
    pub timezone_offset: i8,
    /// Whether to use 24-hour time format (false = 12-hour with AM/PM)
    pub time_format_24h: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: SETTINGS_VERSION,

            // Editor defaults
            font_family: "monospace".to_string(),
            font_size: 14,
            tab_size: 4,
            word_wrap: false,
            line_numbers: true,
            auto_save: AutoSave::Off,

            // Theme defaults
            theme: DEFAULT_THEME.to_string(),

            // Export defaults
            export_format: ExportFormat::Glb,
            export_resolution: 128,
            export_optimize: false,
            export_close_after: true,

            // Application defaults
            restore_session: true,
            remember_workspace: true,
            recent_files_limit: 20,
            undo_history_limit: 100,

            // Time display defaults
            timezone_offset: 0,    // UTC
            time_format_24h: true, // 24-hour format
        }
    }
}

/// Get the path to the settings file
fn settings_path() -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("soyuz").join("settings.json"))
}

/// Load settings from disk, returning defaults if file doesn't exist or is invalid
pub fn load_settings() -> Settings {
    let Some(path) = settings_path() else {
        return Settings::default();
    };

    if !path.exists() {
        return Settings::default();
    }

    match fs::read_to_string(&path) {
        Ok(contents) => normalize_settings(serde_json::from_str(&contents).unwrap_or_default()),
        Err(_) => Settings::default(),
    }
}

fn normalize_settings(mut settings: Settings) -> Settings {
    if settings.theme == LEGACY_DARK_THEME {
        settings.theme = DEFAULT_THEME.to_string();
    }
    settings
}

/// Monotonic sequence for unique settings temp-file names (see `save_settings`).
static SAVE_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Save settings to disk
pub fn save_settings(settings: &Settings) -> Result<()> {
    let Some(path) = settings_path() else {
        bail!("Could not determine config directory");
    };

    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("Failed to create config directory")?;
    }

    // Serialize and write
    let json = serde_json::to_string_pretty(settings).context("Failed to serialize settings")?;

    // Write atomically: write to a unique temp file in the same directory, then
    // rename over the target so a crash mid-write cannot corrupt the settings file.
    // The pid guards against other app instances; the per-call sequence guards this
    // process's own saves, which now run off the UI thread and can therefore overlap
    // (the old synchronous saves were serialized by the UI thread and shared a path).
    let seq = SAVE_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp_path = path.with_file_name(format!("settings.json.{}.{}.tmp", std::process::id(), seq));
    fs::write(&tmp_path, json).context("Failed to write settings file")?;
    fs::rename(&tmp_path, &path).context("Failed to replace settings file")?;

    Ok(())
}

/// Setting metadata for the UI
pub struct SettingMeta {
    /// Unique identifier for the setting
    pub id: &'static str,
    /// Display label
    pub label: &'static str,
    /// Description shown below the setting
    pub description: &'static str,
    /// Category this setting belongs to
    pub category: SettingCategory,
    /// Type of control to render
    pub control_type: ControlType,
}

/// Categories for organizing settings in the UI
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SettingCategory {
    Editor,
    Theme,
    Export,
    Application,
}

impl SettingCategory {
    /// Get the display name for the category
    pub fn label(self) -> &'static str {
        match self {
            Self::Editor => "Editor",
            Self::Theme => "Theme",
            Self::Export => "Export",
            Self::Application => "Application",
        }
    }

    /// Get all categories in display order
    pub fn all() -> &'static [SettingCategory] {
        &[Self::Editor, Self::Theme, Self::Export, Self::Application]
    }
}

/// Type of control to render for a setting
#[derive(Clone, PartialEq)]
pub enum ControlType {
    Text,
    Number {
        min: u32,
        max: u32,
    },
    Checkbox,
    Dropdown {
        options: Vec<(&'static str, &'static str)>,
    },
}

/// Get metadata for all settings (used to build the settings UI)
#[allow(clippy::too_many_lines)] // Data definition, cannot be meaningfully split
pub fn all_settings_meta() -> Vec<SettingMeta> {
    vec![
        // Editor settings
        SettingMeta {
            id: "font_family",
            label: "Font Family",
            description: "Font family for the code editor",
            category: SettingCategory::Editor,
            control_type: ControlType::Text,
        },
        SettingMeta {
            id: "font_size",
            label: "Font Size",
            description: "Font size in pixels",
            category: SettingCategory::Editor,
            control_type: ControlType::Number { min: 8, max: 32 },
        },
        SettingMeta {
            id: "tab_size",
            label: "Tab Size",
            description: "Number of spaces per tab",
            category: SettingCategory::Editor,
            control_type: ControlType::Number { min: 1, max: 8 },
        },
        SettingMeta {
            id: "word_wrap",
            label: "Word Wrap",
            description: "Wrap long lines to fit the editor width",
            category: SettingCategory::Editor,
            control_type: ControlType::Checkbox,
        },
        SettingMeta {
            id: "line_numbers",
            label: "Line Numbers",
            description: "Show line numbers in the editor gutter",
            category: SettingCategory::Editor,
            control_type: ControlType::Checkbox,
        },
        SettingMeta {
            id: "auto_save",
            label: "Auto Save",
            description: "Automatically save files after a delay",
            category: SettingCategory::Editor,
            control_type: ControlType::Dropdown {
                options: vec![
                    ("off", "Off"),
                    ("5", "After 5 seconds"),
                    ("10", "After 10 seconds"),
                    ("30", "After 30 seconds"),
                    ("60", "After 1 minute"),
                ],
            },
        },
        // Theme settings
        SettingMeta {
            id: "theme",
            label: "Color Theme",
            description: "Color theme for the application",
            category: SettingCategory::Theme,
            control_type: ControlType::Dropdown {
                options: vec![(DEFAULT_THEME, "Soyuz Graphite")],
            },
        },
        // Export settings
        SettingMeta {
            id: "export_format",
            label: "Default Format",
            description: "Default export format for 3D models",
            category: SettingCategory::Export,
            control_type: ControlType::Dropdown {
                options: vec![
                    ("glb", "GLB (Binary glTF)"),
                    ("gltf", "glTF (JSON + Binary)"),
                    ("obj", "OBJ (Wavefront)"),
                    ("stl", "STL (Stereolithography)"),
                ],
            },
        },
        SettingMeta {
            id: "export_resolution",
            label: "Default Resolution",
            description: "Default mesh resolution (16-256)",
            category: SettingCategory::Export,
            control_type: ControlType::Number { min: 16, max: 256 },
        },
        SettingMeta {
            id: "export_optimize",
            label: "Optimize Mesh",
            description: "Optimize mesh geometry by default",
            category: SettingCategory::Export,
            control_type: ControlType::Checkbox,
        },
        SettingMeta {
            id: "export_close_after",
            label: "Close After Export",
            description: "Close the export dialog after successful export",
            category: SettingCategory::Export,
            control_type: ControlType::Checkbox,
        },
        // Application settings
        SettingMeta {
            id: "restore_session",
            label: "Restore Session",
            description: "Restore open tabs and layout on startup",
            category: SettingCategory::Application,
            control_type: ControlType::Checkbox,
        },
        SettingMeta {
            id: "remember_workspace",
            label: "Remember Workspace",
            description: "Reopen the last workspace folder on startup",
            category: SettingCategory::Application,
            control_type: ControlType::Checkbox,
        },
        SettingMeta {
            id: "recent_files_limit",
            label: "Recent Files Limit",
            description: "Maximum number of recent files to remember",
            category: SettingCategory::Application,
            control_type: ControlType::Number { min: 5, max: 50 },
        },
        SettingMeta {
            id: "undo_history_limit",
            label: "Undo History Limit",
            description: "Maximum number of undo steps per tab",
            category: SettingCategory::Application,
            control_type: ControlType::Number { min: 10, max: 500 },
        },
        SettingMeta {
            id: "timezone_offset",
            label: "Timezone",
            description: "Timezone offset from UTC for timestamps",
            category: SettingCategory::Application,
            control_type: ControlType::Dropdown {
                options: vec![
                    ("-12", "UTC-12"),
                    ("-11", "UTC-11"),
                    ("-10", "UTC-10 (Hawaii)"),
                    ("-9", "UTC-9 (Alaska)"),
                    ("-8", "UTC-8 (Pacific)"),
                    ("-7", "UTC-7 (Mountain)"),
                    ("-6", "UTC-6 (Central)"),
                    ("-5", "UTC-5 (Eastern)"),
                    ("-4", "UTC-4 (Atlantic)"),
                    ("-3", "UTC-3"),
                    ("-2", "UTC-2"),
                    ("-1", "UTC-1"),
                    ("0", "UTC+0 (GMT)"),
                    ("1", "UTC+1 (CET)"),
                    ("2", "UTC+2 (EET)"),
                    ("3", "UTC+3 (Moscow)"),
                    ("4", "UTC+4"),
                    ("5", "UTC+5"),
                    ("6", "UTC+6"),
                    ("7", "UTC+7"),
                    ("8", "UTC+8 (China)"),
                    ("9", "UTC+9 (Japan)"),
                    ("10", "UTC+10 (Sydney)"),
                    ("11", "UTC+11"),
                    ("12", "UTC+12 (NZ)"),
                    ("13", "UTC+13"),
                    ("14", "UTC+14"),
                ],
            },
        },
        SettingMeta {
            id: "time_format_24h",
            label: "Time Format",
            description: "Display format for timestamps in the terminal",
            category: SettingCategory::Application,
            control_type: ControlType::Dropdown {
                options: vec![
                    ("true", "24-hour (14:30:00)"),
                    ("false", "12-hour (2:30:00 PM)"),
                ],
            },
        },
    ]
}
