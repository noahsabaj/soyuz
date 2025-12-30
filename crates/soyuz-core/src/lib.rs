//! # Soyuz Core
//!
//! Procedural asset generation through code.
//!
//! Soyuz provides a code-first approach to creating 2D and 3D game assets
//! using Signed Distance Functions (SDFs) and procedural textures.
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use soyuz_core::prelude::*;
//!
//! // Create a simple barrel
//! let barrel = cylinder(0.5, 1.2)
//!     .hollow(0.05)
//!     .smooth_union(torus(0.5, 0.08).translate_y(0.5), 0.05);
//!
//! // Export to mesh
//! let mesh = barrel.to_mesh(MeshConfig::default())?;
//! mesh.export_obj("barrel.obj")?;
//! ```
//!
//! ## Units and Conventions
//!
//! - **Distances**: Arbitrary units (typically interpreted as meters). `1.0` = 1 meter.
//! - **Angles**: All rotation functions use **radians**
//! - **Precision**: All SDF operations use `f32` for GPU compatibility
//! - **Coordinate system**: Right-handed, Y-up

pub mod export;
pub mod material;
pub mod mesh;
pub mod sdf;
pub mod texture;

mod error;

pub use error::{Error, Result};

/// Prelude module for convenient imports
pub mod prelude {
    // SDF primitives
    pub use crate::sdf::{Sdf, SdfExt, SdfNode, primitives::*};

    // Texture generation
    pub use crate::texture::{Texture, TextureExt, noise::*, pattern::*};

    // Mesh generation
    pub use crate::mesh::{LodConfig, LodMesh, Mesh, MeshConfig, OptimizeConfig, Vertex};

    // Materials
    pub use crate::material::{Material, MeshWithMaterial, PbrMaterial, RasterizedMaterial};

    // Export
    pub use crate::export::{ExportFormat, ExportOptions, MeshExport};

    // Math (re-export glam)
    pub use glam::{Mat3, Mat4, Quat, Vec2, Vec3, Vec4};

    // Error handling
    pub use crate::{Error, Result};
}
