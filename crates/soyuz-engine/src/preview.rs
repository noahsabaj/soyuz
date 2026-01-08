//! Preview window management for the Soyuz engine
//!
//! Provides functions to open a real-time preview window for visualizing
//! SDF scenes. The preview uses GPU raymarching for interactive rendering.

use crate::scene::Scene;
use anyhow::Result;
use soyuz_render::{WindowConfig, run_preview_with_sdf};

/// Options for opening a preview window
#[derive(Debug, Clone)]
pub struct PreviewOptions {
    /// Window title
    pub title: String,

    /// Window width in pixels
    pub width: u32,

    /// Window height in pixels
    pub height: u32,
}

impl Default for PreviewOptions {
    fn default() -> Self {
        Self {
            title: "Soyuz Preview".to_string(),
            width: 1280,
            height: 720,
        }
    }
}

impl PreviewOptions {
    /// Create options with a custom title
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// Create options with custom dimensions
    pub fn with_size(mut self, width: u32, height: u32) -> Self {
        self.width = width;
        self.height = height;
        self
    }
}

impl From<PreviewOptions> for WindowConfig {
    fn from(opts: PreviewOptions) -> Self {
        WindowConfig {
            title: opts.title,
            width: opts.width,
            height: opts.height,
        }
    }
}

/// Run a preview window with an optional scene
///
/// If no scene is provided, shows the default scene (sphere).
/// This function blocks until the preview window is closed.
pub fn run_preview(scene: Option<&Scene>, options: PreviewOptions) -> Result<()> {
    let config: WindowConfig = options.into();
    let sdf = scene.map(|s| s.sdf.clone());

    run_preview_with_sdf(config, sdf)
}

/// Preview controls help text
pub fn preview_help() -> &'static str {
    soyuz_render::controls_help()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preview_options_default() {
        let opts = PreviewOptions::default();
        assert_eq!(opts.title, "Soyuz Preview");
        assert_eq!(opts.width, 1280);
        assert_eq!(opts.height, 720);
    }

    #[test]
    fn test_preview_options_builder() {
        let opts = PreviewOptions::default()
            .with_title("My Preview")
            .with_size(800, 600);

        assert_eq!(opts.title, "My Preview");
        assert_eq!(opts.width, 800);
        assert_eq!(opts.height, 600);
    }

    #[test]
    fn test_convert_to_window_config() {
        let opts = PreviewOptions {
            title: "Test".to_string(),
            width: 640,
            height: 480,
        };

        let config: WindowConfig = opts.into();
        assert_eq!(config.title, "Test");
        assert_eq!(config.width, 640);
        assert_eq!(config.height, 480);
    }
}
