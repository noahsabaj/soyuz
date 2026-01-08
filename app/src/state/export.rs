//! Export settings for mesh generation
//!
//! Contains configuration for exporting SDF shapes to various 3D formats.

use std::path::PathBuf;

// Re-export ExportFormat from soyuz-core for convenience
pub use soyuz_core::export::ExportFormat;

/// Export settings for mesh generation
#[derive(Clone)]
pub struct ExportSettings {
    /// Output format
    pub format: ExportFormat,
    /// Mesh resolution
    pub resolution: u32,
    /// Whether to optimize mesh
    pub optimize: bool,
    /// Last used export directory (remembered across sessions)
    pub last_export_dir: Option<PathBuf>,
    /// Whether to close the export window after exporting
    pub close_after_export: bool,
}

impl Default for ExportSettings {
    fn default() -> Self {
        Self {
            format: ExportFormat::Glb,
            resolution: 128,
            optimize: false,
            last_export_dir: None,
            close_after_export: true,
        }
    }
}
