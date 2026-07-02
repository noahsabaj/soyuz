//! The SDF sampling interface consumed by the mesher
//!
//! SDFs represent shapes as mathematical functions that return the distance
//! from any point in space to the nearest surface. Negative values are inside,
//! positive values are outside, and zero is exactly on the surface.
//!
//! The live SDF representation is `soyuz_sdf::SdfOp`; this module only defines
//! the sampling trait ([`Sdf`]) and bounding box ([`Aabb`]) that marching cubes
//! needs. `soyuz-script`'s `CpuSdf` adapts an `SdfOp` tree to this trait.

// Aabb methods return modified copy, not Self builder pattern
#![allow(clippy::return_self_not_must_use)]

use glam::Vec3;

/// The core SDF trait - any type that can compute distance from a point
pub trait Sdf: Send + Sync {
    /// Calculate the signed distance from point `p` to the surface.
    ///
    /// - Returns negative values for points inside the shape
    /// - Returns positive values for points outside the shape
    /// - Returns zero for points exactly on the surface
    fn distance(&self, p: Vec3) -> f32;

    /// Get an approximate bounding box for this SDF.
    /// Used for mesh generation and ray marching optimization.
    fn bounds(&self) -> Aabb {
        // Default: large bounding box, can be overridden for better performance
        Aabb::new(Vec3::splat(-10.0), Vec3::splat(10.0))
    }
}

/// Axis-Aligned Bounding Box
#[derive(Debug, Clone, Copy)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    /// Create a cube centered at origin
    pub fn cube(half_size: f32) -> Self {
        Self::new(Vec3::splat(-half_size), Vec3::splat(half_size))
    }

    /// Create from center and half-extents
    pub fn from_center(center: Vec3, half_extents: Vec3) -> Self {
        Self::new(center - half_extents, center + half_extents)
    }

    /// Expand the bounding box by a margin
    pub fn expand(&self, margin: f32) -> Self {
        Self::new(
            self.min - Vec3::splat(margin),
            self.max + Vec3::splat(margin),
        )
    }

    /// Merge two bounding boxes
    pub fn union(&self, other: &Aabb) -> Self {
        Self::new(self.min.min(other.min), self.max.max(other.max))
    }

    /// Get the size of the bounding box
    pub fn size(&self) -> Vec3 {
        self.max - self.min
    }

    /// Get the center of the bounding box
    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }
}
