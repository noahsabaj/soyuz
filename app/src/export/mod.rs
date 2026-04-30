//! Export panel for mesh generation and export
//!
//! Renders export controls inside a docked editor tab.

// Closure is required for Dioxus signals
#![allow(clippy::redundant_closure)]
// Borrowed format strings are valid for file dialogs
#![allow(clippy::needless_borrows_for_generic_args)]

use crate::services::AppServices;
use crate::state::{AppStore, ExportFormat, ExportSettings, TerminalLevel};
use dioxus::prelude::*;
use std::path::PathBuf;
use tracing::warn;

/// Open the docked export panel.
pub fn open_export_panel(mut state: AppStore) {
    state.write().open_export_tab();
}

/// Compute the default export path
fn compute_default_path(
    last_export_dir: Option<&PathBuf>,
    current_file: Option<&PathBuf>,
) -> PathBuf {
    // 1. Check last_export_dir
    if let Some(dir) = last_export_dir
        && dir.exists()
    {
        return dir.clone();
    }

    // 2. Fall back to script's directory
    if let Some(path) = current_file
        && let Some(parent) = path.parent()
    {
        return parent.to_path_buf();
    }

    // 3. Fall back to home/documents directory
    dirs::document_dir().unwrap_or_else(|| PathBuf::from("."))
}

/// Compute the default filename (without path)
fn compute_default_filename(current_file: Option<&PathBuf>, format: ExportFormat) -> String {
    let stem = current_file.and_then(|p| p.file_stem()).map_or_else(
        || "untitled".to_string(),
        |s| s.to_string_lossy().to_string(),
    );

    format!("{}.{}", stem, format.extension())
}

/// The export panel component.
#[component]
pub fn ExportPanel() -> Element {
    let state = use_context::<AppStore>();
    let initial_state = state.read();
    let initial_source_file = initial_state.source_file();
    let initial_path = compute_default_path(
        initial_state.export_settings.last_export_dir.as_ref(),
        initial_source_file.as_ref(),
    );
    let initial_format = initial_state.export_settings.format;
    let initial_filename = compute_default_filename(initial_source_file.as_ref(), initial_format);
    let initial_resolution = initial_state.export_settings.resolution;
    let initial_optimize = initial_state.export_settings.optimize;
    let initial_code = initial_state.source_code();
    drop(initial_state);

    // Local state for the export panel.
    let mut export_path = use_signal(|| initial_path.clone());
    let mut filename = use_signal(|| initial_filename.clone());
    let mut format = use_signal(|| initial_format);
    let mut resolution = use_signal(|| initial_resolution);
    let mut optimize = use_signal(|| initial_optimize);
    let mut is_exporting = use_signal(|| false);
    let mut status_message = use_signal(|| None::<String>);
    let code = use_signal(|| initial_code.clone());
    let mut main_state = state;
    let services = use_context::<AppServices>();

    // Handler for format change - updates filename extension
    let on_format_change = move |new_format: ExportFormat| {
        let mut name = filename.read().clone();

        // Remove old extension
        if let Some(dot_idx) = name.rfind('.') {
            name.truncate(dot_idx);
        }

        // Add new extension
        name.push('.');
        name.push_str(new_format.extension());

        filename.set(name);
        format.set(new_format);
    };

    // Browse for folder
    let browse_folder = move |_| {
        let current_dir = export_path.read().clone();
        spawn(async move {
            if let Some(folder) = rfd::AsyncFileDialog::new()
                .set_directory(current_dir)
                .pick_folder()
                .await
            {
                export_path.set(folder.path().to_path_buf());
            }
        });
    };

    // Export handler
    let do_export = std::rc::Rc::new(move |action: ExportAction| {
        let path = export_path.read().clone();
        let name = filename.read().clone();
        let full_path = path.join(&name);
        let export_format = *format.read();
        let export_resolution = *resolution.read();
        let export_optimize = *optimize.read();
        let settings = ExportSettings {
            format: export_format,
            resolution: export_resolution,
            optimize: export_optimize,
            last_export_dir: Some(path.clone()),
            close_after_export: false,
        };
        let export_code = code.read().clone();

        // Clone path for use after spawn_blocking
        let path_for_state = path.clone();
        let full_path_for_action = full_path.clone();
        let services = services.clone();

        spawn(async move {
            is_exporting.set(true);
            status_message.set(Some("Generating mesh...".to_string()));

            // Log to terminal
            services.terminal_log(TerminalLevel::Info, format!("Exporting to {}...", name));

            let result = tokio::task::spawn_blocking(move || {
                export_mesh(&export_code, &full_path, &settings)
            })
            .await;

            match result {
                Ok(Ok(info)) => {
                    // Log success to terminal
                    services
                        .terminal_log(TerminalLevel::Info, format!("Export complete: {}", info));
                    status_message.set(Some(format!("Exported: {}", info)));

                    // Update main state with last export directory
                    main_state.write().export_settings.last_export_dir =
                        Some(path_for_state.clone());
                    main_state.write().export_settings.format = export_format;
                    main_state.write().export_settings.resolution = export_resolution;
                    main_state.write().export_settings.optimize = export_optimize;

                    // Handle post-export action
                    match action {
                        ExportAction::Export => {
                            // Docked export stays open for repeated exports.
                        }
                        ExportAction::ExportAndOpenFolder => {
                            open_folder(&path_for_state);
                        }
                        ExportAction::ExportAndOpenFile => {
                            open_file(&full_path_for_action);
                        }
                    }
                }
                Ok(Err(e)) => {
                    // Log error to terminal
                    services.terminal_log(TerminalLevel::Error, format!("Export failed: {}", e));
                    status_message.set(Some(format!("Error: {}", e)));
                }
                Err(e) => {
                    // Log error to terminal
                    services
                        .terminal_log(TerminalLevel::Error, format!("Export task failed: {}", e));
                    status_message.set(Some(format!("Error: {}", e)));
                }
            }

            is_exporting.set(false);
        });
    });

    // Check if STL format (no material support)
    let is_stl = *format.read() == ExportFormat::Stl;

    rsx! {
        div { class: "export-panel window",
            // Save Location
            div { class: "section",
                label { class: "section-label", "Save Location" }
                div { class: "path-row",
                    input {
                        r#type: "text",
                        class: "path-input",
                        value: "{export_path.read().display()}",
                        oninput: move |evt| {
                            export_path.set(PathBuf::from(evt.value()));
                        }
                    }
                    button {
                        class: "browse-btn",
                        onclick: browse_folder,
                        "..."
                    }
                }
            }

            // Filename
            div { class: "section",
                label { class: "section-label", "Filename" }
                input {
                    r#type: "text",
                    class: "filename-input",
                    value: "{filename}",
                    oninput: move |evt| {
                        filename.set(evt.value());
                    }
                }
            }

            // Format
            div { class: "section",
                label { class: "section-label", "Format" }
                div { class: "format-buttons",
                    FormatButton {
                        format: ExportFormat::Glb,
                        current: *format.read(),
                        on_select: on_format_change
                    }
                    FormatButton {
                        format: ExportFormat::Gltf,
                        current: *format.read(),
                        on_select: on_format_change
                    }
                    FormatButton {
                        format: ExportFormat::Obj,
                        current: *format.read(),
                        on_select: on_format_change
                    }
                    FormatButton {
                        format: ExportFormat::Stl,
                        current: *format.read(),
                        on_select: on_format_change
                    }
                }
            }

            // STL info message
            if is_stl {
                div { class: "info-message",
                    "STL format is optimized for 3D printing. Materials and textures are not supported."
                }
            }

            // Mesh Resolution
            div { class: "section",
                label { class: "section-label",
                    "Mesh Resolution: {resolution}"
                }
                input {
                    r#type: "range",
                    class: "slider",
                    min: "16",
                    max: "256",
                    step: "16",
                    value: "{resolution}",
                    oninput: move |evt| {
                        if let Ok(val) = evt.value().parse::<u32>() {
                            resolution.set(val);
                        }
                    }
                }
            }

            // Options
            div { class: "section",
                label { class: "section-label", "Options" }

                div { class: "option",
                    input {
                        r#type: "checkbox",
                        id: "optimize",
                        checked: *optimize.read(),
                        onchange: move |evt| {
                            optimize.set(evt.checked());
                        }
                    }
                    label { r#for: "optimize", "Optimize mesh" }
                }

            }

            // Export buttons
            div { class: "actions",
                button {
                    class: "btn-primary",
                    disabled: *is_exporting.read(),
                    onclick: {
                        let do_export = do_export.clone();
                        move |_| do_export(ExportAction::Export)
                    },
                    if *is_exporting.read() { "Exporting..." } else { "Export" }
                }
                button {
                    class: "btn-secondary",
                    disabled: *is_exporting.read(),
                    onclick: {
                        let do_export = do_export.clone();
                        move |_| do_export(ExportAction::ExportAndOpenFolder)
                    },
                    "& Open Folder"
                }
                button {
                    class: "btn-secondary",
                    disabled: *is_exporting.read(),
                    onclick: {
                        let do_export = do_export.clone();
                        move |_| do_export(ExportAction::ExportAndOpenFile)
                    },
                    "& Open"
                }
            }

            // Status
            div { class: "status",
                if let Some(msg) = status_message.read().as_ref() {
                    "{msg}"
                } else {
                    "Ready"
                }
            }
        }
    }
}

/// Export action type
#[derive(Clone, Copy)]
enum ExportAction {
    Export,
    ExportAndOpenFolder,
    ExportAndOpenFile,
}

#[component]
fn FormatButton(
    format: ExportFormat,
    current: ExportFormat,
    on_select: EventHandler<ExportFormat>,
) -> Element {
    let is_selected = format == current;
    let class = if is_selected {
        "format-btn active"
    } else {
        "format-btn"
    };

    rsx! {
        button {
            class: class,
            onclick: move |_| on_select.call(format),
            {format.extension().to_uppercase()}
        }
    }
}

/// Open a folder in the system file manager
fn open_folder(path: &std::path::Path) {
    #[cfg(target_os = "linux")]
    {
        if let Err(e) = std::process::Command::new("xdg-open").arg(path).spawn() {
            warn!("Failed to open folder with xdg-open: {e}");
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Err(e) = std::process::Command::new("explorer").arg(path).spawn() {
            warn!("Failed to open folder with explorer: {e}");
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Err(e) = std::process::Command::new("open").arg(path).spawn() {
            warn!("Failed to open folder: {e}");
        }
    }
}

/// Open a file with the default application
fn open_file(path: &std::path::Path) {
    #[cfg(target_os = "linux")]
    {
        if let Err(e) = std::process::Command::new("xdg-open").arg(path).spawn() {
            warn!("Failed to open file with xdg-open: {e}");
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Err(e) = std::process::Command::new("cmd")
            .args(["/C", "start", "", &path.to_string_lossy()])
            .spawn()
        {
            warn!("Failed to open file with cmd: {e}");
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Err(e) = std::process::Command::new("open").arg(path).spawn() {
            warn!("Failed to open file: {e}");
        }
    }
}

/// Export mesh from script
///
/// Evaluates the script to get an SDF, then uses parallel marching cubes
/// (via Rayon) to generate a mesh for export.
pub fn export_mesh(
    code: &str,
    output_path: &std::path::Path,
    settings: &ExportSettings,
) -> anyhow::Result<String> {
    use soyuz_engine::{Engine, ExportOptions};

    // Create engine and run script
    let mut engine = Engine::new();
    engine.run_script(code)?;

    // Export using Engine API
    let options = ExportOptions::new(output_path)
        .with_resolution(settings.resolution)
        .with_optimize(settings.optimize);

    let result = engine.export(&options)?;

    Ok(format!(
        "{} vertices, {} triangles",
        result.vertex_count, result.triangle_count
    ))
}
