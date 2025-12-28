//! Export panel for mesh generation and export

use crate::state::{AppState, ExportFormat, ExportSettings};
use dioxus::prelude::*;

/// Export panel component
#[component]
pub fn ExportPanel() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let mut is_exporting = use_signal(|| false);
    let mut export_status = use_signal(|| String::new());

    rsx! {
        div { class: "export-container",
            div { class: "export-header",
                span { class: "export-title", "Export" }
            }

            div { class: "export-content",
                // Format selection
                div { class: "export-section",
                    label { class: "export-label", "Format" }
                    div { class: "export-format-buttons",
                        FormatButton {
                            format: ExportFormat::Glb,
                            current: state.read().export_settings.format,
                            on_select: move |f| {
                                state.write().export_settings.format = f;
                            }
                        }
                        FormatButton {
                            format: ExportFormat::Gltf,
                            current: state.read().export_settings.format,
                            on_select: move |f| {
                                state.write().export_settings.format = f;
                            }
                        }
                        FormatButton {
                            format: ExportFormat::Obj,
                            current: state.read().export_settings.format,
                            on_select: move |f| {
                                state.write().export_settings.format = f;
                            }
                        }
                    }
                }

                // Resolution slider
                div { class: "export-section",
                    label { class: "export-label",
                        "Mesh Resolution: {state.read().export_settings.resolution}"
                    }
                    input {
                        r#type: "range",
                        class: "export-slider",
                        min: "16",
                        max: "256",
                        step: "16",
                        value: "{state.read().export_settings.resolution}",
                        oninput: move |evt| {
                            if let Ok(val) = evt.value().parse::<u32>() {
                                state.write().export_settings.resolution = val;
                            }
                        }
                    }
                }

                // Texture size slider
                div { class: "export-section",
                    label { class: "export-label",
                        "Texture Size: {state.read().export_settings.texture_size}px"
                    }
                    input {
                        r#type: "range",
                        class: "export-slider",
                        min: "256",
                        max: "4096",
                        step: "256",
                        value: "{state.read().export_settings.texture_size}",
                        oninput: move |evt| {
                            if let Ok(val) = evt.value().parse::<u32>() {
                                state.write().export_settings.texture_size = val;
                            }
                        }
                    }
                }

                // Options
                div { class: "export-section",
                    label { class: "export-label", "Options" }

                    div { class: "export-option",
                        input {
                            r#type: "checkbox",
                            id: "optimize",
                            checked: state.read().export_settings.optimize,
                            onchange: move |evt| {
                                state.write().export_settings.optimize = evt.checked();
                            }
                        }
                        label { r#for: "optimize", "Optimize mesh" }
                    }

                    div { class: "export-option",
                        input {
                            r#type: "checkbox",
                            id: "lod",
                            checked: state.read().export_settings.generate_lod,
                            onchange: move |evt| {
                                state.write().export_settings.generate_lod = evt.checked();
                            }
                        }
                        label { r#for: "lod", "Generate LOD levels" }
                    }
                }

                // Export button
                div { class: "export-section",
                    button {
                        class: "export-button",
                        disabled: *is_exporting.read(),
                        onclick: move |_| {
                            let settings = state.read().export_settings.clone();
                            let code = state.read().code();
                            let format = settings.format;

                            spawn(async move {
                                // Show file dialog
                                if let Some(file) = rfd::AsyncFileDialog::new()
                                    .add_filter(format.name(), &[format.extension()])
                                    .set_file_name(&format!("export.{}", format.extension()))
                                    .save_file()
                                    .await
                                {
                                    is_exporting.set(true);
                                    export_status.set("Generating mesh...".to_string());

                                    let result = tokio::task::spawn_blocking(move || {
                                        export_mesh(&code, file.path(), &settings)
                                    }).await;

                                    match result {
                                        Ok(Ok(info)) => {
                                            export_status.set(format!("Exported: {}", info));
                                        }
                                        Ok(Err(e)) => {
                                            export_status.set(format!("Error: {}", e));
                                        }
                                        Err(e) => {
                                            export_status.set(format!("Error: {}", e));
                                        }
                                    }

                                    is_exporting.set(false);
                                }
                            });
                        },
                        if *is_exporting.read() {
                            "Exporting..."
                        } else {
                            "Export Mesh"
                        }
                    }
                }

                // Status
                if !export_status.read().is_empty() {
                    div { class: "export-status",
                        {export_status.read().clone()}
                    }
                }
            }

            // Quick info
            div { class: "export-info",
                h4 { "Tips" }
                ul {
                    li { "GLB: Best for games (single file)" }
                    li { "GLTF: For editing (separate files)" }
                    li { "OBJ: Universal compatibility" }
                    li { "Higher resolution = more detail" }
                }
            }
        }
    }
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
            class: if is_selected { "format-button selected" } else { "format-button" },
            onclick: move |_| on_select.call(format),
            {format.name()}
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
    use soyuz_core::export::MeshExport;
    use soyuz_core::mesh::{MeshConfig, OptimizeConfig, SdfToMesh};
    use soyuz_core::sdf::Sdf;
    use soyuz_script::ScriptEngine;

    // Evaluate script to get SDF
    let engine = ScriptEngine::new();
    let rhai_sdf = engine.eval_sdf(code)?;

    // Get the SdfOperation which implements the Sdf trait for CPU evaluation
    let sdf_op = &rhai_sdf.op;

    // Use the SDF's bounds for mesh generation, or default if unbounded
    let bounds = sdf_op.bounds();

    // Generate mesh using parallel marching cubes
    let config = MeshConfig::default()
        .with_resolution(settings.resolution)
        .with_bounds(bounds);

    let mut mesh = sdf_op.to_mesh(config)?;

    // Optimize if requested
    if settings.optimize {
        mesh.optimize(&OptimizeConfig::default());
    }

    let vertex_count = mesh.vertex_count();
    let triangle_count = mesh.triangle_count();

    // Export
    mesh.export(output_path)?;

    Ok(format!(
        "{} vertices, {} triangles",
        vertex_count, triangle_count
    ))
}
