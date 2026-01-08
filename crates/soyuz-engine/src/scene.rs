//! Scene representation for the Soyuz engine
//!
//! A Scene contains an SDF geometry and its associated environment settings
//! (lighting, materials, background). It represents the complete renderable
//! state produced by evaluating a Rhai script.

use soyuz_sdf::{Environment, SdfOp};
use std::path::PathBuf;
use thiserror::Error;

/// Errors that can occur when working with scenes
#[derive(Error, Debug)]
pub enum SceneError {
    /// No scene is currently loaded
    #[error("No scene loaded")]
    NoScene,

    /// Scene has no associated source file
    #[error("Scene has no source file")]
    NoSourceFile,

    /// Failed to reload scene
    #[error("Failed to reload scene: {0}")]
    ReloadFailed(String),
}

/// A complete scene with geometry and environment
///
/// The Scene struct holds all the information needed to render or export
/// a procedural asset:
/// - The SDF geometry tree
/// - Environment settings (lighting, materials, background)
/// - Optionally, the source file path for reload/watch functionality
#[derive(Debug, Clone)]
pub struct Scene {
    /// The SDF geometry
    pub sdf: SdfOp,

    /// Environment configuration (lighting, materials, background)
    pub environment: Environment,

    /// Source file path (if loaded from file)
    pub source_path: Option<PathBuf>,
}

impl Scene {
    /// Create a new scene with geometry and environment
    pub fn new(sdf: SdfOp, environment: Environment) -> Self {
        Self {
            sdf,
            environment,
            source_path: None,
        }
    }

    /// Create a scene with a source file path
    pub fn with_source(sdf: SdfOp, environment: Environment, path: PathBuf) -> Self {
        Self {
            sdf,
            environment,
            source_path: Some(path),
        }
    }

    /// Create a scene from a SceneResult
    pub fn from_scene_result(result: soyuz_script::SceneResult) -> Self {
        Self {
            sdf: result.sdf,
            environment: result.environment,
            source_path: None,
        }
    }

    /// Check if this scene was loaded from a file
    pub fn has_source(&self) -> bool {
        self.source_path.is_some()
    }

    /// Get the source file name (without path)
    pub fn source_name(&self) -> Option<String> {
        self.source_path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
    }
}

impl Default for Scene {
    fn default() -> Self {
        Self {
            sdf: SdfOp::Sphere { radius: 0.5 },
            environment: Environment::default(),
            source_path: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scene_creation() {
        let scene = Scene::new(SdfOp::Sphere { radius: 1.0 }, Environment::default());

        assert!(!scene.has_source());
        assert!(scene.source_name().is_none());
    }

    #[test]
    fn test_scene_with_source() {
        let scene = Scene::with_source(
            SdfOp::Sphere { radius: 1.0 },
            Environment::default(),
            PathBuf::from("/path/to/model.rhai"),
        );

        assert!(scene.has_source());
        assert_eq!(scene.source_name(), Some("model.rhai".to_string()));
    }

    #[test]
    fn test_default_scene() {
        let scene = Scene::default();
        assert!(matches!(scene.sdf, SdfOp::Sphere { .. }));
    }
}
