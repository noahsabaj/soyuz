//! # Soyuz Core
//!
//! Mesh generation and export for Soyuz.
//!
//! This crate turns signed distance fields into triangle meshes (marching
//! cubes, plus optimization and LOD) and exports them to GLB/GLTF/OBJ/STL.
//! The SDF representation itself lives in `soyuz-sdf`; scripts produce it via
//! `soyuz-script`, whose `CpuSdf` implements this crate's [`sdf::Sdf`]
//! sampling trait.
//!
//! ## Units and Conventions
//!
//! - **Distances**: Arbitrary units (typically interpreted as meters). `1.0` = 1 meter.
//! - **Angles**: All rotation functions use **radians**
//! - **Precision**: All SDF operations use `f32` for GPU compatibility
//! - **Coordinate system**: Right-handed, Y-up

pub mod export;
pub mod mesh;
pub mod sdf;

mod error;

pub use error::{Error, Result};

/// Prelude module for convenient imports
pub mod prelude {
    // SDF sampling interface
    pub use crate::sdf::{Aabb, Sdf};

    // Mesh generation
    pub use crate::mesh::{
        LodConfig, LodMesh, Mesh, MeshConfig, OptimizeConfig, UvProjection, Vertex,
    };

    // Export
    pub use crate::export::{ExportFormat, MeshExport};

    // Math (re-export glam)
    pub use glam::{Mat3, Mat4, Quat, Vec2, Vec3, Vec4};

    // Error handling
    pub use crate::{Error, Result};
}
