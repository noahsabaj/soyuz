//! Rendering tools for the MCP server
//!
//! Provides tools for rendering the current scene to images.

use schemars::JsonSchema;
use serde::Deserialize;

fn default_angle() -> String {
    "isometric".to_string()
}

fn default_angles() -> String {
    "all".to_string()
}

fn default_size() -> u32 {
    512
}

/// Request for rendering a preview image
#[derive(Debug, Deserialize, JsonSchema)]
pub struct RenderPreviewRequest {
    /// Camera viewing angle.
    /// Options: "front", "back", "left", "right", "top", "bottom", "isometric" (default)
    #[serde(default = "default_angle")]
    pub angle: String,

    /// Image width in pixels (default: 512)
    #[serde(default = "default_size")]
    pub width: u32,

    /// Image height in pixels (default: 512)
    #[serde(default = "default_size")]
    pub height: u32,
}

/// Request for rendering multiple preview images at different angles
#[derive(Debug, Deserialize, JsonSchema)]
pub struct RenderPreviewsRequest {
    /// Comma-separated list of camera angles to render.
    /// Options: "front", "back", "left", "right", "top", "bottom", "isometric"
    /// Use "all" to render all 7 angles at once.
    /// Example: "front, right, isometric" or "all"
    #[serde(default = "default_angles")]
    pub angles: String,

    /// Image width in pixels for each render (default: 512)
    #[serde(default = "default_size")]
    pub width: u32,

    /// Image height in pixels for each render (default: 512)
    #[serde(default = "default_size")]
    pub height: u32,
}

impl RenderPreviewsRequest {
    /// Parse the angles string into a list of angle names
    pub fn parse_angles(&self) -> Vec<&'static str> {
        let input = self.angles.trim().to_lowercase();

        if input == "all" {
            return vec!["front", "back", "left", "right", "top", "bottom", "isometric"];
        }

        input
            .split(',')
            .map(|s| s.trim())
            .filter_map(|s| match s {
                "front" => Some("front"),
                "back" => Some("back"),
                "left" => Some("left"),
                "right" => Some("right"),
                "top" => Some("top"),
                "bottom" => Some("bottom"),
                "isometric" | "iso" => Some("isometric"),
                _ => None,
            })
            .collect()
    }
}
