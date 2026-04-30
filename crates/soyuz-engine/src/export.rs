//! Mesh export functionality for the Soyuz engine
//!
//! Provides functions to convert SDF scenes to polygon meshes and export
//! them to various 3D file formats (GLB, glTF, OBJ, STL).

use crate::scene::Scene;
use anyhow::Result;
use soyuz_core::export::{ExportFormat, MeshExport};
use soyuz_core::mesh::{Mesh, MeshConfig, OptimizeConfig, SdfToMesh};
use soyuz_core::sdf::Sdf;
use soyuz_script::CpuSdf;
use std::path::PathBuf;

/// Options for mesh export
#[derive(Debug, Clone)]
pub struct ExportOptions {
    /// Output file path
    pub path: PathBuf,

    /// Export format (if None, inferred from path extension)
    pub format: Option<ExportFormat>,

    /// Mesh resolution (higher = more detail, slower)
    pub resolution: u32,

    /// Whether to optimize the mesh (remove duplicates, etc.)
    pub optimize: bool,
}

impl ExportOptions {
    /// Create export options for a given path
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            format: None,
            resolution: 64,
            optimize: true,
        }
    }

    /// Set the export format explicitly
    pub fn with_format(mut self, format: ExportFormat) -> Self {
        self.format = Some(format);
        self
    }

    /// Set the mesh resolution
    pub fn with_resolution(mut self, resolution: u32) -> Self {
        self.resolution = resolution;
        self
    }

    /// Set whether to optimize the mesh
    pub fn with_optimize(mut self, optimize: bool) -> Self {
        self.optimize = optimize;
        self
    }

    /// Get the effective format (explicit or inferred from path)
    pub fn effective_format(&self) -> Option<ExportFormat> {
        self.format
            .or_else(|| ExportFormat::from_extension(&self.path))
    }
}

/// Result of a successful export operation
#[derive(Debug, Clone)]
pub struct ExportResult {
    /// Path where the file was written
    pub path: PathBuf,

    /// Format used for export
    pub format: ExportFormat,

    /// Number of vertices in the mesh
    pub vertex_count: usize,

    /// Number of triangles in the mesh
    pub triangle_count: usize,
}

impl std::fmt::Display for ExportResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Exported {} ({} vertices, {} triangles)",
            self.path.display(),
            self.vertex_count,
            self.triangle_count
        )
    }
}

/// Generate a mesh from a scene without exporting to file
pub fn generate_mesh_from_scene(scene: &Scene, config: MeshConfig) -> Result<Mesh> {
    // Create CPU-evaluable SDF from the scene's SDF op
    let cpu_sdf = CpuSdf::new(scene.sdf.clone());

    // Generate mesh using marching cubes
    let mesh = cpu_sdf.to_mesh(config)?;

    Ok(mesh)
}

/// Export a scene to a mesh file
pub fn export_scene(scene: &Scene, options: &ExportOptions) -> Result<ExportResult> {
    // Determine format
    let format = options
        .effective_format()
        .ok_or_else(|| anyhow::anyhow!("Cannot determine export format from path"))?;

    // Create CPU-evaluable SDF
    let cpu_sdf = CpuSdf::new(scene.sdf.clone());

    // Use the SDF's bounds for mesh generation
    let bounds = cpu_sdf.bounds();

    // Configure mesh generation
    let config = MeshConfig::default()
        .with_resolution(options.resolution)
        .with_bounds(bounds);

    // Generate mesh
    let mut mesh = cpu_sdf.to_mesh(config)?;

    // Optimize if requested
    if options.optimize {
        mesh.optimize(&OptimizeConfig::default());
    }

    let vertex_count = mesh.vertex_count();
    let triangle_count = mesh.triangle_count();

    // Ensure the file has the correct extension
    let mut output_path = options.path.clone();
    if output_path.extension().is_none() {
        output_path.set_extension(format.extension());
    }

    // Export to file
    mesh.export(&output_path)?;

    Ok(ExportResult {
        path: output_path,
        format,
        vertex_count,
        triangle_count,
    })
}

/// Export a scene with simple parameters (convenience function)
pub fn quick_export(
    scene: &Scene,
    path: impl Into<PathBuf>,
    resolution: u32,
) -> Result<ExportResult> {
    export_scene(scene, &ExportOptions::new(path).with_resolution(resolution))
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use soyuz_sdf::SdfOp;
    use std::path::Path;

    #[test]
    fn test_format_extension() {
        assert_eq!(ExportFormat::Glb.extension(), "glb");
        assert_eq!(ExportFormat::Gltf.extension(), "gltf");
        assert_eq!(ExportFormat::Obj.extension(), "obj");
        assert_eq!(ExportFormat::Stl.extension(), "stl");
    }

    #[test]
    fn test_format_from_str_ext() {
        assert_eq!(ExportFormat::from_str_ext("glb"), Some(ExportFormat::Glb));
        assert_eq!(ExportFormat::from_str_ext("GLB"), Some(ExportFormat::Glb));
        assert_eq!(ExportFormat::from_str_ext("obj"), Some(ExportFormat::Obj));
        assert_eq!(ExportFormat::from_str_ext("xyz"), None);
    }

    #[test]
    fn test_format_from_extension() {
        assert_eq!(
            ExportFormat::from_extension(Path::new("model.glb")),
            Some(ExportFormat::Glb)
        );
        assert_eq!(
            ExportFormat::from_extension(Path::new("/path/to/mesh.stl")),
            Some(ExportFormat::Stl)
        );
        assert_eq!(ExportFormat::from_extension(Path::new("noext")), None);
    }

    #[test]
    fn test_export_options_builder() {
        let opts = ExportOptions::new("model.glb")
            .with_format(ExportFormat::Glb)
            .with_resolution(128)
            .with_optimize(false);

        assert_eq!(opts.path, PathBuf::from("model.glb"));
        assert_eq!(opts.format, Some(ExportFormat::Glb));
        assert_eq!(opts.resolution, 128);
        assert!(!opts.optimize);
    }

    #[test]
    fn test_generate_mesh() {
        let scene = Scene::new(
            SdfOp::Sphere { radius: 0.5 },
            soyuz_sdf::Environment::default(),
        );

        let config = MeshConfig::default().with_resolution(16);
        let mesh = generate_mesh_from_scene(&scene, config);

        assert!(mesh.is_ok());
        let mesh = mesh.expect("Failed to generate mesh");
        assert!(mesh.vertex_count() > 0);
        assert!(mesh.triangle_count() > 0);
    }
}
