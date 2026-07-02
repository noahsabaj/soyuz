//! Export functionality for meshes

mod gltf_export;
mod obj;
mod stl;

use crate::Result;
use crate::mesh::Mesh;
use serde::{Deserialize, Serialize};
use std::path::Path;

pub use gltf_export::export_gltf;
pub use obj::export_obj;
pub use stl::export_stl;

/// Supported export formats
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Obj,
    Gltf,
    #[default]
    Glb,
    /// STL format for 3D printing
    Stl,
}

impl ExportFormat {
    /// Detect format from file extension
    pub fn from_extension(path: &Path) -> Option<Self> {
        match path.extension()?.to_str()? {
            "obj" => Some(Self::Obj),
            "gltf" => Some(Self::Gltf),
            "glb" => Some(Self::Glb),
            "stl" => Some(Self::Stl),
            _ => None,
        }
    }

    /// Parse format from a string extension name (case-insensitive)
    pub fn from_str_ext(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "obj" => Some(Self::Obj),
            "gltf" => Some(Self::Gltf),
            "glb" => Some(Self::Glb),
            "stl" => Some(Self::Stl),
            _ => None,
        }
    }

    /// Get the file extension for this format
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Glb => "glb",
            Self::Gltf => "gltf",
            Self::Obj => "obj",
            Self::Stl => "stl",
        }
    }

    /// Get a human-readable name for this format
    pub fn name(&self) -> &'static str {
        match self {
            Self::Glb => "GLB (Binary)",
            Self::Gltf => "GLTF (JSON)",
            Self::Obj => "OBJ",
            Self::Stl => "STL",
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

    /// Export mesh to STL format (binary)
    fn export_stl<P: AsRef<Path>>(&self, path: P) -> Result<()>;
}

impl MeshExport for Mesh {
    fn export<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path = path.as_ref();
        match ExportFormat::from_extension(path) {
            Some(ExportFormat::Obj) => self.export_obj(path),
            Some(ExportFormat::Gltf | ExportFormat::Glb) => self.export_gltf(path),
            Some(ExportFormat::Stl) => self.export_stl(path),
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

    fn export_stl<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        export_stl(self, path.as_ref())
    }
}
