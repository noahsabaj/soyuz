//! Export panel for mesh generation and export
//!
//! Renders export controls inside a docked editor tab.

// Closure is required for Dioxus signals
#![allow(clippy::redundant_closure)]
// Borrowed format strings are valid for file dialogs
#![allow(clippy::needless_borrows_for_generic_args)]

use crate::components::ConfirmDialog;
use crate::services::AppServices;
use crate::state::{AppStateStoreExt, AppStore, ExportFormat, ExportSettings, TerminalLevel};
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
    // Seed the local signals once with `.peek()` so the panel does not subscribe
    // to (and re-render on) unrelated store changes just to read initial defaults.
    let initial_state = state.peek();
    let initial_source_file = initial_state.source_file();
    let initial_path = compute_default_path(
        initial_state.export_settings.last_export_dir.as_ref(),
        initial_source_file.as_ref(),
    );
    let initial_format = initial_state.export_settings.format;
    let initial_filename = compute_default_filename(initial_source_file.as_ref(), initial_format);
    let initial_resolution = initial_state.export_settings.resolution;
    let initial_optimize = initial_state.export_settings.optimize;
    drop(initial_state);

    // Local state for the export panel.
    let mut export_path = use_signal(|| initial_path.clone());
    let mut filename = use_signal(|| initial_filename.clone());
    let mut format = use_signal(|| initial_format);
    let mut resolution = use_signal(|| initial_resolution);
    let mut optimize = use_signal(|| initial_optimize);
    // Single source of truth for export progress (replaces the old in-flight bool
    // plus a separate status string that had to be kept in sync).
    let export_status = use_signal(|| ExportStatus::Idle);
    // F62: an export awaiting overwrite confirmation (target file already exists).
    let pending_overwrite = use_signal(|| None::<ExportAction>);
    let main_state = state;
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

    // Performs the actual export. `export_status` is set to `Exporting` synchronously
    // *before* spawning so a second click is suppressed and the buttons disable
    // immediately (F62).
    let run_export = std::rc::Rc::new(move |action: ExportAction| {
        // The synchronous `.set()` below takes `&mut self`. Copy the Copy-able
        // signal handle into a local binding so this closure stays `Fn` and remains
        // callable through the shared `Rc`.
        let mut export_status = export_status;

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
        };
        // F57: export the CURRENT editor code, read live from app state at click
        // time, not the snapshot captured when the panel mounted.
        let export_code = main_state.peek().source_code();

        // Clone path for use after spawn_blocking
        let path_for_state = path.clone();
        let full_path_for_action = full_path.clone();
        let services = services.clone();

        // F62: mark the export in-flight synchronously, before the async task starts,
        // so a second click is suppressed and the buttons disable immediately.
        export_status.set(ExportStatus::Exporting);

        spawn(async move {
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

                    // Persist the used export settings in one scoped write instead
                    // of four whole-store invalidations.
                    {
                        let mut export_settings_store = main_state.export_settings();
                        let mut export_settings = export_settings_store.write();
                        export_settings.last_export_dir = Some(path_for_state.clone());
                        export_settings.format = export_format;
                        export_settings.resolution = export_resolution;
                        export_settings.optimize = export_optimize;
                    }

                    export_status.set(ExportStatus::Done(info));

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
                    export_status.set(ExportStatus::Error(e.to_string()));
                }
                Err(e) => {
                    // Log error to terminal
                    services
                        .terminal_log(TerminalLevel::Error, format!("Export task failed: {}", e));
                    export_status.set(ExportStatus::Error(e.to_string()));
                }
            }
        });
    });

    // Click handler: ignore clicks while an export is in flight, and confirm
    // before overwriting an existing file (F62).
    let do_export = std::rc::Rc::new({
        let run_export = run_export.clone();
        move |action: ExportAction| {
            // Local mutable copy so the `.set()` below keeps this closure `Fn`.
            let mut pending_overwrite = pending_overwrite;
            // F62: ignore clicks while an export is already in flight.
            if matches!(*export_status.peek(), ExportStatus::Exporting) {
                return;
            }
            let full_path = export_path.read().join(&*filename.read());
            if full_path.exists() {
                pending_overwrite.set(Some(action));
                return;
            }
            run_export(action);
        }
    });

    // Check if STL format (no material support)
    let is_stl = *format.read() == ExportFormat::Stl;
    // Whether an export is currently running (drives the disabled/label state).
    let exporting = matches!(*export_status.read(), ExportStatus::Exporting);

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
                    disabled: exporting,
                    onclick: {
                        let do_export = do_export.clone();
                        move |_| do_export(ExportAction::Export)
                    },
                    if exporting { "Exporting..." } else { "Export" }
                }
                button {
                    class: "btn-secondary",
                    disabled: exporting,
                    onclick: {
                        let do_export = do_export.clone();
                        move |_| do_export(ExportAction::ExportAndOpenFolder)
                    },
                    "& Open Folder"
                }
                button {
                    class: "btn-secondary",
                    disabled: exporting,
                    onclick: {
                        let do_export = do_export.clone();
                        move |_| do_export(ExportAction::ExportAndOpenFile)
                    },
                    "& Open"
                }
            }

            // Status
            div { class: "status",
                match &*export_status.read() {
                    ExportStatus::Idle => rsx! { "Ready" },
                    ExportStatus::Exporting => rsx! { "Generating mesh..." },
                    ExportStatus::Done(info) => rsx! { "Exported: {info}" },
                    ExportStatus::Error(e) => rsx! { "Error: {e}" },
                }
            }

            // F62: overwrite confirmation dialog (shared component).
            if let Some(action) = *pending_overwrite.read() {
                {
                    let full_path = export_path.read().join(&*filename.read());
                    let name = full_path.file_name().map_or_else(
                        || full_path.display().to_string(),
                        |n| n.to_string_lossy().to_string(),
                    );
                    let run_export = run_export.clone();
                    rsx! {
                        ConfirmDialog {
                            title: "Overwrite?",
                            message: format!("A file named \"{name}\" already exists. Overwrite it?"),
                            confirm_label: "Overwrite",
                            destructive: false,
                            on_confirm: move |()| {
                                let mut pending_overwrite = pending_overwrite;
                                pending_overwrite.set(None);
                                run_export(action);
                            },
                            on_cancel: move |()| {
                                let mut pending_overwrite = pending_overwrite;
                                pending_overwrite.set(None);
                            },
                        }
                    }
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

/// Progress of an export, modeled as one signal so the panel does not have to
/// keep a separate in-flight flag and status string in sync.
#[derive(Clone, PartialEq)]
enum ExportStatus {
    /// No export has run yet.
    Idle,
    /// An export is in flight (set synchronously before the task spawns; F62).
    Exporting,
    /// The last export succeeded; carries the summary line.
    Done(String),
    /// The last export failed; carries the error text.
    Error(String),
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
