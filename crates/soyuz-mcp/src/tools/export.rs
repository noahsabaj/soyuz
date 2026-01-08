//! Export tools for the MCP server
//!
//! Provides tools for exporting scenes to mesh files and shader code.

use schemars::JsonSchema;
use serde::Deserialize;

fn default_format() -> String {
    "glb".to_string()
}

fn default_resolution() -> u32 {
    64
}

fn default_optimize() -> bool {
    true
}

/// Request for exporting the scene as a mesh
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ExportMeshRequest {
    /// Output format: "glb", "gltf", "obj", or "stl" (default: "glb")
    #[serde(default = "default_format")]
    pub format: String,

    /// Mesh resolution - higher values produce more detailed meshes but take longer.
    /// Typical values: 32 (fast/low), 64 (default), 128 (high), 256 (very high)
    #[serde(default = "default_resolution")]
    pub resolution: u32,

    /// Whether to optimize the mesh by removing duplicate vertices (default: true)
    #[serde(default = "default_optimize")]
    pub optimize: bool,
}
