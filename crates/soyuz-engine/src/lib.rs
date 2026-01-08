//! Soyuz Engine - Unified runtime for scripting, rendering, and export
//!
//! The Engine is a thin orchestration layer that coordinates soyuz-script,
//! soyuz-render, and soyuz-core to provide a unified API for all Soyuz operations.
//!
//! ## Example
//!
//! ```ignore
//! use soyuz_engine::Engine;
//!
//! let mut engine = Engine::new();
//!
//! // Load and run a script
//! engine.run_script("sphere(0.5)")?;
//!
//! // Access the scene
//! if let Some(scene) = engine.scene() {
//!     println!("SDF loaded, env: {:?}", scene.environment);
//! }
//!
//! // Open preview window
//! engine.preview(PreviewOptions::default())?;
//!
//! // Export to file
//! engine.export(ExportOptions {
//!     path: "model.glb".into(),
//!     format: ExportFormat::Glb,
//!     resolution: 128,
//!     optimize: true,
//! })?;
//! ```

pub mod export;
pub mod preview;
pub mod scene;

#[cfg(feature = "file-watcher")]
pub mod watch;

use anyhow::Result;
use scene::Scene;
use soyuz_script::ScriptEngine;
use std::path::Path;

// Re-export commonly used types from dependencies
pub use soyuz_core::export::MeshExport;
pub use soyuz_core::mesh::{Mesh, MeshConfig, OptimizeConfig, SdfToMesh};
pub use soyuz_render::{Camera, WindowConfig, run_preview_with_sdf};
pub use soyuz_script::{CpuSdf, SceneResult};
pub use soyuz_sdf::{Environment, SdfOp};

// Re-export our own types
pub use export::{ExportFormat, ExportOptions, ExportResult};
pub use preview::PreviewOptions;
pub use scene::SceneError;

#[cfg(feature = "file-watcher")]
pub use soyuz_script::{ScriptWatcher, WatchEvent};

/// The main Soyuz engine
///
/// Provides a unified interface for:
/// - Script execution (load, run, compile)
/// - Scene access (SDF + environment)
/// - Preview window management
/// - Mesh export to various formats
/// - File watching for hot reload
pub struct Engine {
    /// The underlying Rhai script executor
    scripting: ScriptEngine,

    /// The currently loaded scene (SDF + environment)
    current_scene: Option<Scene>,

    /// File watcher for hot reload
    #[cfg(feature = "file-watcher")]
    watcher: Option<ScriptWatcher>,
}

impl Engine {
    /// Create a new engine instance
    pub fn new() -> Self {
        Self {
            scripting: ScriptEngine::new(),
            current_scene: None,
            #[cfg(feature = "file-watcher")]
            watcher: None,
        }
    }

    // ========================================================================
    // Script Operations
    // ========================================================================

    /// Load and execute a script from a file path
    ///
    /// The script is evaluated and the resulting scene (SDF + environment)
    /// is stored in the engine. Returns a reference to the loaded scene.
    pub fn load_script(&mut self, path: &Path) -> Result<&Scene> {
        let scene_result = self.scripting.eval_scene_file(path)?;

        self.current_scene = Some(Scene {
            sdf: scene_result.sdf,
            environment: scene_result.environment,
            source_path: Some(path.to_path_buf()),
        });

        self.current_scene
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Scene was not stored"))
    }

    /// Execute a script from a string
    ///
    /// The script is evaluated and the resulting scene (SDF + environment)
    /// is stored in the engine. Returns a reference to the loaded scene.
    pub fn run_script(&mut self, code: &str) -> Result<&Scene> {
        let scene_result = self.scripting.eval_scene(code)?;

        self.current_scene = Some(Scene {
            sdf: scene_result.sdf,
            environment: scene_result.environment,
            source_path: None,
        });

        self.current_scene
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Scene was not stored"))
    }

    /// Compile a script to check for syntax errors without executing
    pub fn compile(&self, code: &str) -> Result<()> {
        self.scripting.compile(code)
    }

    // ========================================================================
    // Scene Access
    // ========================================================================

    /// Get the currently loaded scene (if any)
    pub fn scene(&self) -> Option<&Scene> {
        self.current_scene.as_ref()
    }

    /// Get a mutable reference to the current scene
    pub fn scene_mut(&mut self) -> Option<&mut Scene> {
        self.current_scene.as_mut()
    }

    /// Get the current SDF (if a scene is loaded)
    pub fn sdf(&self) -> Option<&SdfOp> {
        self.current_scene.as_ref().map(|s| &s.sdf)
    }

    /// Get the current environment (if a scene is loaded)
    pub fn environment(&self) -> Option<&Environment> {
        self.current_scene.as_ref().map(|s| &s.environment)
    }

    /// Check if a scene is currently loaded
    pub fn has_scene(&self) -> bool {
        self.current_scene.is_some()
    }

    /// Clear the current scene
    pub fn clear_scene(&mut self) {
        self.current_scene = None;
    }

    // ========================================================================
    // Preview
    // ========================================================================

    /// Open a preview window with the current scene
    ///
    /// This blocks until the preview window is closed.
    pub fn preview(&self, options: PreviewOptions) -> Result<()> {
        preview::run_preview(self.current_scene.as_ref(), options)
    }

    // ========================================================================
    // Export
    // ========================================================================

    /// Export the current scene to a mesh file
    ///
    /// Returns information about the exported mesh.
    pub fn export(&self, options: &ExportOptions) -> Result<ExportResult> {
        let scene = self
            .current_scene
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No scene loaded"))?;

        export::export_scene(scene, options)
    }

    /// Generate a mesh from the current scene without saving to file
    ///
    /// Useful for further processing or custom export formats.
    pub fn generate_mesh(&self, config: MeshConfig) -> Result<Mesh> {
        let scene = self
            .current_scene
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No scene loaded"))?;

        export::generate_mesh_from_scene(scene, config)
    }

    // ========================================================================
    // File Watching (only with file-watcher feature)
    // ========================================================================

    /// Start watching a file or directory for changes
    #[cfg(feature = "file-watcher")]
    pub fn watch(&mut self, path: &Path) -> Result<()> {
        // Create watcher if not already created
        if self.watcher.is_none() {
            self.watcher = Some(ScriptWatcher::new(None)?);
        }

        if let Some(watcher) = &mut self.watcher {
            watcher.watch(path)?;
        }

        Ok(())
    }

    /// Stop watching a file or directory
    #[cfg(feature = "file-watcher")]
    pub fn unwatch(&mut self, path: &Path) -> Result<()> {
        if let Some(watcher) = &mut self.watcher {
            watcher.unwatch(path)?;
        }
        Ok(())
    }

    /// Poll for file change events (non-blocking)
    #[cfg(feature = "file-watcher")]
    pub fn poll_changes(&self) -> Option<WatchEvent> {
        self.watcher.as_ref().and_then(|w| w.try_recv())
    }

    /// Drain all pending file change events
    #[cfg(feature = "file-watcher")]
    pub fn drain_changes(&self) -> Vec<WatchEvent> {
        self.watcher
            .as_ref()
            .map(|w| w.drain_events())
            .unwrap_or_default()
    }

    /// Reload the current scene from its source file if it was modified
    ///
    /// Returns `Ok(true)` if the scene was reloaded, `Ok(false)` if no
    /// changes were detected or no source file is associated.
    #[cfg(feature = "file-watcher")]
    pub fn reload_if_changed(&mut self) -> Result<bool> {
        // Check for changes
        let events = self.drain_changes();
        if events.is_empty() {
            return Ok(false);
        }

        // Check if any event matches our source path
        let source_path = self
            .current_scene
            .as_ref()
            .and_then(|s| s.source_path.as_ref());

        let should_reload = source_path.is_some_and(|source| {
            events.iter().any(|event| match event {
                WatchEvent::Modified(p) | WatchEvent::Created(p) | WatchEvent::Deleted(p) => {
                    p == source
                }
                WatchEvent::Error(_) => false,
            })
        });

        if should_reload
            && let Some(path) = source_path.cloned()
        {
            self.load_script(&path)?;
            return Ok(true);
        }

        Ok(false)
    }

    // ========================================================================
    // Access to underlying components (for advanced use)
    // ========================================================================

    /// Get a reference to the underlying script engine
    pub fn scripting(&self) -> &ScriptEngine {
        &self.scripting
    }

    /// Get a mutable reference to the underlying script engine
    pub fn scripting_mut(&mut self) -> &mut ScriptEngine {
        &mut self.scripting
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_creation() {
        let engine = Engine::new();
        assert!(!engine.has_scene());
        assert!(engine.scene().is_none());
    }

    #[test]
    fn test_run_script() {
        let mut engine = Engine::new();
        let result = engine.run_script("sphere(0.5)");
        assert!(result.is_ok());
        assert!(engine.has_scene());
        assert!(engine.sdf().is_some());
    }

    #[test]
    fn test_compile() {
        let engine = Engine::new();
        assert!(engine.compile("sphere(0.5)").is_ok());
        assert!(engine.compile("sphere(").is_err());
    }

    #[test]
    fn test_clear_scene() {
        let mut engine = Engine::new();
        engine.run_script("sphere(0.5)").ok();
        assert!(engine.has_scene());
        engine.clear_scene();
        assert!(!engine.has_scene());
    }
}
