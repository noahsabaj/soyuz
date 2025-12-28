//! Export functionality for meshes and textures

mod gltf_export;
mod obj;

use crate::Result;
use crate::material::MeshWithMaterial;
use crate::mesh::Mesh;
use std::path::Path;

pub use gltf_export::{
    GltfExportOptions, export_gltf, export_gltf_with_material, export_gltf_with_options,
};
pub use obj::export_obj;

/// Supported export formats
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExportFormat {
    Obj,
    Gltf,
    #[default]
    Glb,
}

impl ExportFormat {
    /// Detect format from file extension
    pub fn from_extension(path: &Path) -> Option<Self> {
        match path.extension()?.to_str()? {
            "obj" => Some(Self::Obj),
            "gltf" => Some(Self::Gltf),
            "glb" => Some(Self::Glb),
            _ => None,
        }
    }

    /// Get the file extension for this format
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Glb => "glb",
            Self::Gltf => "gltf",
            Self::Obj => "obj",
        }
    }

    /// Get a human-readable name for this format
    pub fn name(&self) -> &'static str {
        match self {
            Self::Glb => "GLB (Binary)",
            Self::Gltf => "GLTF (JSON)",
            Self::Obj => "OBJ",
        }
    }
}

/// Export options
#[derive(Debug, Clone)]
pub struct ExportOptions {
    /// Embed textures in the output file (for GLTF/GLB)
    pub embed_textures: bool,
    /// Generate LOD levels
    pub generate_lod: bool,
    /// Apply compression where possible
    pub compress: bool,
    /// Texture resolution for materials
    pub texture_size: u32,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            embed_textures: true,
            generate_lod: false,
            compress: false,
            texture_size: 1024,
        }
    }
}

/// Extension trait for exporting meshes
pub trait MeshExport {
    /// Export mesh to file, auto-detecting format from extension
    fn export<P: AsRef<Path>>(&self, path: P) -> Result<()>;

    /// Export mesh to OBJ format
    fn export_obj<P: AsRef<Path>>(&self, path: P) -> Result<()>;

    /// Export mesh to GLTF format
    fn export_gltf<P: AsRef<Path>>(&self, path: P) -> Result<()>;
}

impl MeshExport for Mesh {
    fn export<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path = path.as_ref();
        match ExportFormat::from_extension(path) {
            Some(ExportFormat::Obj) => self.export_obj(path),
            Some(ExportFormat::Gltf) | Some(ExportFormat::Glb) => self.export_gltf(path),
            None => Err(crate::Error::Export(format!(
                "Unknown file extension: {}",
                path.display()
            ))),
        }
    }

    fn export_obj<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        export_obj(self, path.as_ref())
    }

    fn export_gltf<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        export_gltf(self, path.as_ref())
    }
}

impl MeshExport for MeshWithMaterial {
    fn export<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path = path.as_ref();
        match ExportFormat::from_extension(path) {
            Some(ExportFormat::Obj) => self.export_obj(path),
            Some(ExportFormat::Gltf) | Some(ExportFormat::Glb) => self.export_gltf(path),
            None => Err(crate::Error::Export(format!(
                "Unknown file extension: {}",
                path.display()
            ))),
        }
    }

    fn export_obj<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        // OBJ doesn't support materials directly, export mesh only
        export_obj(&self.mesh, path.as_ref())
    }

    fn export_gltf<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        export_gltf_with_material(self, path.as_ref())
    }
}
