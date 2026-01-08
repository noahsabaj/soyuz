//! Soyuz MCP Server - Model Context Protocol server for procedural 3D asset generation
//!
//! This crate provides an MCP server that exposes Soyuz's SDF-based 3D asset
//! generation capabilities to AI agents. Agents can:
//!
//! - Execute Rhai scripts to create 3D scenes
//! - Render preview images from various camera angles
//! - Export meshes to GLB, glTF, OBJ, or STL formats
//! - Discover available primitives, operations, and transforms
//!
//! ## Workflow
//!
//! 1. `run_script` - Execute a Rhai script to create/update the scene
//! 2. `render_preview` - See what the scene looks like
//! 3. Iterate on the script based on visual feedback
//! 4. `export_mesh` - Export the final result as a 3D file

pub mod camera;
pub mod state;
pub mod tools;

use base64::Engine as _;
use rmcp::{
    ErrorData as McpError,
    ServerHandler,
    handler::server::tool::ToolRouter,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, Content, Implementation, ProtocolVersion, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router,
};
use serde_json::json;
use soyuz_engine::ExportFormat;

use crate::camera::CameraAngle;
use crate::state::SoyuzState;
use crate::tools::{
    discovery::{self, GetDocsRequest},
    export::ExportMeshRequest,
    render::{RenderPreviewRequest, RenderPreviewsRequest},
    script::{CompileScriptRequest, RunScriptRequest},
};

// Re-export for binary
pub use rmcp;
pub use state::SoyuzState as State;

/// The Soyuz MCP service
///
/// Implements the MCP ServerHandler to expose Soyuz functionality as MCP tools.
/// The SoyuzState uses a channel-based architecture to handle the non-Send Rhai engine,
/// so SoyuzMcpService is naturally Send + Sync.
#[derive(Clone)]
pub struct SoyuzMcpService {
    state: SoyuzState,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl SoyuzMcpService {
    /// Create a new MCP service with the given state
    pub fn new(state: SoyuzState) -> Self {
        Self {
            state,
            tool_router: Self::tool_router(),
        }
    }

    // ========================================================================
    // Script Execution Tools
    // ========================================================================

    #[tool(description = "Execute a Rhai script to create or update the current 3D scene. The script must return an SDF (Signed Distance Field) as its final expression (no trailing semicolon). Returns scene information on success or an error message.")]
    async fn run_script(
        &self,
        params: Parameters<RunScriptRequest>,
    ) -> Result<CallToolResult, McpError> {
        let request = params.0;
        match self.state.run_script(&request.code).await {
            Ok(info) => Ok(CallToolResult::success(vec![Content::text(info.to_string())])),
            Err(e) => Ok(CallToolResult::success(vec![Content::text(format!(
                "Script error: {}",
                e
            ))])),
        }
    }

    #[tool(description = "Check if a Rhai script is syntactically valid without executing it. Returns success if valid, or a compilation error message.")]
    async fn compile_script(
        &self,
        params: Parameters<CompileScriptRequest>,
    ) -> Result<CallToolResult, McpError> {
        let request = params.0;
        match self.state.compile_script(&request.code).await {
            Ok(()) => Ok(CallToolResult::success(vec![Content::text(
                "Script is valid",
            )])),
            Err(e) => Ok(CallToolResult::success(vec![Content::text(format!(
                "Compilation error: {}",
                e
            ))])),
        }
    }

    // ========================================================================
    // Rendering Tools
    // ========================================================================

    #[tool(description = "Render the current scene as a PNG image. Returns a base64-encoded image that can be viewed to inspect the 3D model. Use different angles to see the model from various viewpoints.")]
    async fn render_preview(
        &self,
        params: Parameters<RenderPreviewRequest>,
    ) -> Result<CallToolResult, McpError> {
        let request = params.0;
        let angle = CameraAngle::parse(&request.angle).unwrap_or_default();

        match self.state.render(angle, request.width, request.height).await {
            Ok(png_bytes) => {
                let b64 = base64::engine::general_purpose::STANDARD.encode(&png_bytes);
                Ok(CallToolResult::success(vec![Content::image(
                    b64,
                    "image/png",
                )]))
            }
            Err(e) => Ok(CallToolResult::success(vec![Content::text(format!(
                "Render error: {}",
                e
            ))])),
        }
    }

    #[tool(description = "Render the current scene from multiple angles at once. Returns multiple PNG images. Use comma-separated angle names (e.g., \"front, right, isometric\") or \"all\" for all 7 standard angles.")]
    async fn render_previews(
        &self,
        params: Parameters<RenderPreviewsRequest>,
    ) -> Result<CallToolResult, McpError> {
        let request = params.0;
        let angles = request.parse_angles();

        if angles.is_empty() {
            return Ok(CallToolResult::success(vec![Content::text(
                "No valid angles specified. Use comma-separated names: front, back, left, right, top, bottom, isometric. Or use \"all\" for all angles.",
            )]));
        }

        let mut contents = Vec::with_capacity(angles.len() * 2);

        for angle_name in angles {
            let angle = CameraAngle::parse(angle_name).unwrap_or_default();

            match self.state.render(angle, request.width, request.height).await {
                Ok(png_bytes) => {
                    contents.push(Content::text(format!("[{}]", angle_name)));
                    let b64 = base64::engine::general_purpose::STANDARD.encode(&png_bytes);
                    contents.push(Content::image(b64, "image/png"));
                }
                Err(e) => {
                    contents.push(Content::text(format!("[{}] Error: {}", angle_name, e)));
                }
            }
        }

        Ok(CallToolResult::success(contents))
    }

    // ========================================================================
    // Export Tools
    // ========================================================================

    #[tool(description = "Export the current scene as a 3D mesh file. Returns base64-encoded file data. Supported formats: glb (binary glTF, recommended), gltf, obj, stl.")]
    async fn export_mesh(
        &self,
        params: Parameters<ExportMeshRequest>,
    ) -> Result<CallToolResult, McpError> {
        let request = params.0;
        let format = match request.format.to_lowercase().as_str() {
            "glb" => ExportFormat::Glb,
            "gltf" => ExportFormat::Gltf,
            "obj" => ExportFormat::Obj,
            "stl" => ExportFormat::Stl,
            _ => {
                return Ok(CallToolResult::success(vec![Content::text(format!(
                    "Unknown format '{}'. Valid options: glb, gltf, obj, stl",
                    request.format
                ))]));
            }
        };

        match self.state.export_mesh(format, request.resolution, request.optimize).await {
            Ok(info) => {
                let b64 = base64::engine::general_purpose::STANDARD.encode(&info.bytes);
                let summary = info.to_string();

                // Return metadata and base64 data as text (MCP doesn't have blob content type)
                Ok(CallToolResult::success(vec![
                    Content::text(format!(
                        "{}\n\nBase64 data ({} bytes encoded):\n{}",
                        summary,
                        b64.len(),
                        b64
                    )),
                ]))
            }
            Err(e) => Ok(CallToolResult::success(vec![Content::text(format!(
                "Export error: {}",
                e
            ))])),
        }
    }

    #[tool(description = "Get the WGSL shader code generated from the current scene's SDF. Useful for understanding the GPU implementation or for custom rendering.")]
    async fn get_wgsl(&self) -> Result<CallToolResult, McpError> {
        match self.state.get_wgsl().await {
            Ok(wgsl) => Ok(CallToolResult::success(vec![Content::text(wgsl)])),
            Err(e) => Ok(CallToolResult::success(vec![Content::text(format!(
                "Error: {}",
                e
            ))])),
        }
    }

    // ========================================================================
    // Discovery Tools
    // ========================================================================

    #[tool(description = "List all available SDF primitive shapes (sphere, cube, cylinder, etc.) with their signatures and descriptions.")]
    async fn list_primitives(&self) -> Result<CallToolResult, McpError> {
        let primitives = discovery::list_primitives();
        let json = serde_json::to_string_pretty(&primitives).unwrap_or_default();
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "List all available boolean operations (union, subtract, intersect, smooth variants) with their signatures and descriptions.")]
    async fn list_operations(&self) -> Result<CallToolResult, McpError> {
        let operations = discovery::list_operations();
        let json = serde_json::to_string_pretty(&operations).unwrap_or_default();
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "List all available transform operations (translate, rotate, scale, mirror) with their signatures and descriptions.")]
    async fn list_transforms(&self) -> Result<CallToolResult, McpError> {
        let transforms = discovery::list_transforms();
        let json = serde_json::to_string_pretty(&transforms).unwrap_or_default();
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "List all available modifiers (shell, round, twist, bend, repeat) with their signatures and descriptions.")]
    async fn list_modifiers(&self) -> Result<CallToolResult, McpError> {
        let modifiers = discovery::list_modifiers();
        let json = serde_json::to_string_pretty(&modifiers).unwrap_or_default();
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "List all available environment and lighting functions (set_sun_direction, env_preset, etc.) with their signatures and descriptions.")]
    async fn list_environment(&self) -> Result<CallToolResult, McpError> {
        let environment = discovery::list_environment();
        let json = serde_json::to_string_pretty(&environment).unwrap_or_default();
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "List all available math helper functions (deg, rad, PI, TAU) with their signatures and descriptions.")]
    async fn list_math(&self) -> Result<CallToolResult, McpError> {
        let math = discovery::list_math();
        let json = serde_json::to_string_pretty(&math).unwrap_or_default();
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "List ALL available Soyuz functions in one call. Returns complete documentation for primitives, operations, transforms, modifiers, environment, and math functions. Use this for comprehensive discovery.")]
    async fn list_all(&self) -> Result<CallToolResult, McpError> {
        let all_docs = json!({
            "primitives": discovery::list_primitives(),
            "operations": discovery::list_operations(),
            "transforms": discovery::list_transforms(),
            "modifiers": discovery::list_modifiers(),
            "environment": discovery::list_environment(),
            "math": discovery::list_math(),
        });
        let json = serde_json::to_string_pretty(&all_docs).unwrap_or_default();
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Get detailed documentation for a specific Soyuz function by name.")]
    async fn get_docs(
        &self,
        params: Parameters<GetDocsRequest>,
    ) -> Result<CallToolResult, McpError> {
        let request = params.0;
        match discovery::get_docs(&request.function_name) {
            Some(info) => {
                let json = serde_json::to_string_pretty(&info).unwrap_or_default();
                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
            None => Ok(CallToolResult::success(vec![Content::text(format!(
                "Function '{}' not found. Use list_primitives, list_operations, list_transforms, or list_modifiers to see available functions.",
                request.function_name
            ))])),
        }
    }

    // ========================================================================
    // Scene Management Tools
    // ========================================================================

    #[tool(description = "Get information about the current scene (bounds, environment settings, etc.).")]
    async fn get_scene_info(&self) -> Result<CallToolResult, McpError> {
        let result = self.state.scene_info().await;

        if result.loaded {
            let json = json!({
                "loaded": true,
                "bounds": {
                    "min": result.bounds_min,
                    "max": result.bounds_max,
                    "size": result.bounds_size
                },
                "environment": result.environment
            });
            Ok(CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&json).unwrap_or_default(),
            )]))
        } else {
            Ok(CallToolResult::success(vec![Content::text(
                "No scene loaded. Use run_script to create a scene.",
            )]))
        }
    }

    #[tool(description = "Clear the current scene and reset to empty state.")]
    async fn clear_scene(&self) -> Result<CallToolResult, McpError> {
        self.state.clear_scene().await;
        Ok(CallToolResult::success(vec![Content::text("Scene cleared")]))
    }
}

#[tool_handler]
impl ServerHandler for SoyuzMcpService {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "soyuz-mcp".to_string(),
                title: Some("Soyuz 3D Asset Generator".to_string()),
                version: env!("CARGO_PKG_VERSION").to_string(),
                icons: None,
                website_url: Some("https://github.com/noahsabaj/soyuz".to_string()),
            },
            instructions: Some(
                "Soyuz MCP server for procedural 3D asset generation using SDFs (Signed Distance Fields). \
                 \n\nWorkflow:\n\
                 1. run_script() - Execute Rhai script to create a 3D scene\n\
                 2. render_preview() - See what the scene looks like\n\
                 3. Iterate on the script based on visual feedback\n\
                 4. export_mesh() - Export the final result as a 3D file\n\n\
                 Use list_all() to discover all available functions in one call.\n\
                 Use get_docs(function_name) for detailed documentation on any function."
                    .to_string(),
            ),
        }
    }
}
