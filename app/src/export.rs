//! Export window for mesh generation and export
//!
//! Opens a separate OS window for configuring and executing mesh exports.

// Closure is required for Dioxus signals
#![allow(clippy::redundant_closure)]
// Borrowed format strings are valid for file dialogs
#![allow(clippy::needless_borrows_for_generic_args)]

use crate::state::{AppState, ExportFormat, ExportSettings, TerminalLevel};
use dioxus::desktop::{window, Config, LogicalSize, WindowBuilder};
use dioxus::prelude::*;
use std::path::PathBuf;
use tracing::warn;

/// Open the export window
pub fn open_export_window(state: Signal<AppState>) {

    // Get initial values from state for the new window
    let initial_state = state.read();
    let last_export_dir = initial_state.export_settings.last_export_dir.clone();
    let current_file = initial_state.current_file();
    let close_after_export = initial_state.export_settings.close_after_export;
    let format = initial_state.export_settings.format;
    let resolution = initial_state.export_settings.resolution;
    let optimize = initial_state.export_settings.optimize;
    let code = initial_state.code();
    drop(initial_state);

    // Compute default path
    let default_path = compute_default_path(last_export_dir.as_ref(), current_file.as_ref());

    // Compute default filename
    let default_filename = compute_default_filename(current_file.as_ref(), format);

    // Create the export window component with captured values
    let dom = VirtualDom::new_with_props(
        ExportWindow,
        ExportWindowProps {
            main_state: state,
            initial_path: default_path,
            initial_filename: default_filename,
            initial_format: format,
            initial_resolution: resolution,
            initial_optimize: optimize,
            initial_close_after: close_after_export,
            initial_code: code,
        },
    );

    let window_builder = WindowBuilder::new()
        .with_title("Export - Soyuz Studio")
        .with_inner_size(LogicalSize::new(420.0, 520.0))
        .with_resizable(true);

    let config = Config::new()
        .with_window(window_builder)
        .with_menu(None::<dioxus::desktop::muda::Menu>);

    window().new_window(dom, config);
}

/// Compute the default export path
fn compute_default_path(last_export_dir: Option<&PathBuf>, current_file: Option<&PathBuf>) -> PathBuf {
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
    let stem = current_file
        .and_then(|p| p.file_stem())
        .map_or_else(|| "untitled".to_string(), |s| s.to_string_lossy().to_string());

    format!("{}.{}", stem, format.extension())
}

/// Props for the export window
#[derive(Clone, PartialEq, Props)]
struct ExportWindowProps {
    main_state: Signal<AppState>,
    initial_path: PathBuf,
    initial_filename: String,
    initial_format: ExportFormat,
    initial_resolution: u32,
    initial_optimize: bool,
    initial_close_after: bool,
    initial_code: String,
}

/// The export window component
#[component]
fn ExportWindow(props: ExportWindowProps) -> Element {
    // Local state for the export window
    let mut export_path = use_signal(|| props.initial_path.clone());
    let mut filename = use_signal(|| props.initial_filename.clone());
    let mut format = use_signal(|| props.initial_format);
    let mut resolution = use_signal(|| props.initial_resolution);
    let mut optimize = use_signal(|| props.initial_optimize);
    let mut close_after_export = use_signal(|| props.initial_close_after);
    let mut is_exporting = use_signal(|| false);
    let mut status_message = use_signal(|| None::<String>);
    let code = use_signal(|| props.initial_code.clone());
    let mut main_state = props.main_state;

    // Resize window when STL is selected (to accommodate the info message)
    use_effect(move || {
        let current_format = *format.read();
        let height = if current_format == ExportFormat::Stl {
            580.0 // Taller to fit the STL info message
        } else {
            520.0 // Standard height
        };
        window().set_inner_size(LogicalSize::new(420.0, height));
    });

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
    let do_export = move |action: ExportAction| {
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
            close_after_export: *close_after_export.read(),
        };
        let export_code = code.read().clone();
        let should_close = *close_after_export.read();

        // Clone path for use after spawn_blocking
        let path_for_state = path.clone();
        let full_path_for_action = full_path.clone();

        spawn(async move {
            is_exporting.set(true);
            status_message.set(Some("Generating mesh...".to_string()));

            // Log to terminal
            main_state.read().terminal_log(
                TerminalLevel::Info,
                format!("Exporting to {}...", name),
            );

            let result = tokio::task::spawn_blocking(move || {
                export_mesh(&export_code, &full_path, &settings)
            })
            .await;

            match result {
                Ok(Ok(info)) => {
                    // Log success to terminal
                    main_state.read().terminal_log(
                        TerminalLevel::Info,
                        format!("Export complete: {}", info),
                    );
                    status_message.set(Some(format!("Exported: {}", info)));

                    // Update main state with last export directory
                    main_state.write().export_settings.last_export_dir = Some(path_for_state.clone());
                    main_state.write().export_settings.close_after_export = should_close;
                    main_state.write().export_settings.format = export_format;
                    main_state.write().export_settings.resolution = export_resolution;
                    main_state.write().export_settings.optimize = export_optimize;

                    // Handle post-export action
                    match action {
                        ExportAction::Export => {
                            if should_close {
                                window().close();
                            }
                        }
                        ExportAction::ExportAndOpenFolder => {
                            open_folder(&path_for_state);
                            if should_close {
                                window().close();
                            }
                        }
                        ExportAction::ExportAndOpenFile => {
                            open_file(&full_path_for_action);
                            if should_close {
                                window().close();
                            }
                        }
                    }
                }
                Ok(Err(e)) => {
                    // Log error to terminal
                    main_state.read().terminal_log(
                        TerminalLevel::Error,
                        format!("Export failed: {}", e),
                    );
                    status_message.set(Some(format!("Error: {}", e)));
                }
                Err(e) => {
                    // Log error to terminal
                    main_state.read().terminal_log(
                        TerminalLevel::Error,
                        format!("Export task failed: {}", e),
                    );
                    status_message.set(Some(format!("Error: {}", e)));
                }
            }

            is_exporting.set(false);
        });
    };

    // Check if STL format (no material support)
    let is_stl = *format.read() == ExportFormat::Stl;

    rsx! {
        style { {include_str!("../assets/theme.css")} }
        style { {include_str!("../assets/export-window.css")} }

        div { class: "export-window",
            // Save Location
            div { class: "export-section",
                label { class: "export-section-label", "Save Location" }
                div { class: "export-path-row",
                    input {
                        r#type: "text",
                        class: "export-path-input",
                        value: "{export_path.read().display()}",
                        oninput: move |evt| {
                            export_path.set(PathBuf::from(evt.value()));
                        }
                    }
                    button {
                        class: "export-browse-btn",
                        onclick: browse_folder,
                        "..."
                    }
                }
            }

            // Filename
            div { class: "export-section",
                label { class: "export-section-label", "Filename" }
                input {
                    r#type: "text",
                    class: "export-filename-input",
                    value: "{filename}",
                    oninput: move |evt| {
                        filename.set(evt.value());
                    }
                }
            }

            // Format
            div { class: "export-section",
                label { class: "export-section-label", "Format" }
                div { class: "export-format-buttons",
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
                div { class: "export-info-message",
                    "STL format is optimized for 3D printing. Materials and textures are not supported."
                }
            }

            // Mesh Resolution
            div { class: "export-section",
                label { class: "export-section-label",
                    "Mesh Resolution: {resolution}"
                }
                input {
                    r#type: "range",
                    class: "export-slider",
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
            div { class: "export-section",
                label { class: "export-section-label", "Options" }

                div { class: "export-option",
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

                div { class: "export-option",
                    input {
                        r#type: "checkbox",
                        id: "close-after",
                        checked: *close_after_export.read(),
                        onchange: move |evt| {
                            close_after_export.set(evt.checked());
                        }
                    }
                    label { r#for: "close-after", "Close after export" }
                }
            }

            // Export buttons
            div { class: "export-actions",
                button {
                    class: "export-btn-primary",
                    disabled: *is_exporting.read(),
                    onclick: move |_| do_export(ExportAction::Export),
                    if *is_exporting.read() { "Exporting..." } else { "Export" }
                }
                button {
                    class: "export-btn-secondary",
                    disabled: *is_exporting.read(),
                    onclick: move |_| do_export(ExportAction::ExportAndOpenFolder),
                    "& Open Folder"
                }
                button {
                    class: "export-btn-secondary",
                    disabled: *is_exporting.read(),
                    onclick: move |_| do_export(ExportAction::ExportAndOpenFile),
                    "& Open"
                }
            }

            // Status
            div { class: "export-status",
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

    rsx! {
        button {
            class: if is_selected { "export-format-btn active" } else { "export-format-btn" },
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
