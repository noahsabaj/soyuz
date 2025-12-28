//! Signed Distance Functions (SDF) for 3D shape definition
//!
//! SDFs represent shapes as mathematical functions that return the distance
//! from any point in space to the nearest surface. Negative values are inside,
//! positive values are outside, and zero is exactly on the surface.
//!
//! ## Example
//!
//! ```rust,ignore
//! use soyuz_core::prelude::*;
//!
//! // Create a sphere with radius 1 meter
//! let ball = sphere(1.0);
//!
//! // Combine shapes
//! let snowman = sphere(1.0)
//!     .union(sphere(0.7).translate_y(1.5))
//!     .union(sphere(0.5).translate_y(2.5));
//! ```

pub mod operations;
pub mod primitives;
pub mod transforms;

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

/// A boxed SDF for dynamic dispatch and composition
pub type BoxedSdf = Box<dyn Sdf>;

/// An SDF node that can be composed and transformed
#[derive(Clone)]
pub struct SdfNode {
    inner: std::sync::Arc<dyn Sdf>,
}

impl SdfNode {
    /// Create a new SDF node from any type implementing Sdf
    pub fn new<S: Sdf + 'static>(sdf: S) -> Self {
        Self {
            inner: std::sync::Arc::new(sdf),
        }
    }
}

impl Sdf for SdfNode {
    fn distance(&self, p: Vec3) -> f32 {
        self.inner.distance(p)
    }

    fn bounds(&self) -> Aabb {
        self.inner.bounds()
    }
}

/// Extension trait providing chainable operations on SDFs
pub trait SdfExt: Sdf + Sized + 'static {
    // === Boolean Operations ===

    /// Union: combine two shapes (OR)
    fn union<S: Sdf + 'static>(self, other: S) -> SdfNode {
        SdfNode::new(operations::Union::new(self, other))
    }

    /// Subtraction: cut shape `other` from `self`
    fn subtract<S: Sdf + 'static>(self, other: S) -> SdfNode {
        SdfNode::new(operations::Subtract::new(self, other))
    }

    /// Intersection: keep only where both shapes overlap (AND)
    fn intersect<S: Sdf + 'static>(self, other: S) -> SdfNode {
        SdfNode::new(operations::Intersect::new(self, other))
    }

    // === Smooth Boolean Operations ===

    /// Smooth union with blend radius `k`
    fn smooth_union<S: Sdf + 'static>(self, other: S, k: f32) -> SdfNode {
        SdfNode::new(operations::SmoothUnion::new(self, other, k))
    }

    /// Smooth subtraction with blend radius `k`
    fn smooth_subtract<S: Sdf + 'static>(self, other: S, k: f32) -> SdfNode {
        SdfNode::new(operations::SmoothSubtract::new(self, other, k))
    }

    /// Smooth intersection with blend radius `k`
    fn smooth_intersect<S: Sdf + 'static>(self, other: S, k: f32) -> SdfNode {
        SdfNode::new(operations::SmoothIntersect::new(self, other, k))
    }

    // === Transforms ===

    /// Translate (move) the shape
    fn translate(self, x: f32, y: f32, z: f32) -> SdfNode {
        SdfNode::new(transforms::Translate::new(self, Vec3::new(x, y, z)))
    }

    /// Translate along X axis
    fn translate_x(self, x: f32) -> SdfNode {
        self.translate(x, 0.0, 0.0)
    }

    /// Translate along Y axis
    fn translate_y(self, y: f32) -> SdfNode {
        self.translate(0.0, y, 0.0)
    }

    /// Translate along Z axis
    fn translate_z(self, z: f32) -> SdfNode {
        self.translate(0.0, 0.0, z)
    }

    /// Rotate around X axis (angle in radians)
    fn rotate_x(self, angle: f32) -> SdfNode {
        SdfNode::new(transforms::Rotate::new(
            self,
            glam::Quat::from_rotation_x(angle),
        ))
    }

    /// Rotate around Y axis (angle in radians)
    fn rotate_y(self, angle: f32) -> SdfNode {
        SdfNode::new(transforms::Rotate::new(
            self,
            glam::Quat::from_rotation_y(angle),
        ))
    }

    /// Rotate around Z axis (angle in radians)
    fn rotate_z(self, angle: f32) -> SdfNode {
        SdfNode::new(transforms::Rotate::new(
            self,
            glam::Quat::from_rotation_z(angle),
        ))
    }

    /// Rotate around arbitrary axis (angle in radians)
    fn rotate(self, axis: Vec3, angle: f32) -> SdfNode {
        SdfNode::new(transforms::Rotate::new(
            self,
            glam::Quat::from_axis_angle(axis.normalize(), angle),
        ))
    }

    /// Uniform scale
    fn scale(self, factor: f32) -> SdfNode {
        SdfNode::new(transforms::Scale::new(self, factor))
    }

    /// Mirror across a plane defined by its normal
    fn mirror(self, axis: Vec3) -> SdfNode {
        SdfNode::new(transforms::Mirror::new(self, axis.normalize()))
    }

    /// Mirror across X axis (YZ plane)
    fn mirror_x(self) -> SdfNode {
        self.mirror(Vec3::X)
    }

    /// Mirror across Y axis (XZ plane)
    fn mirror_y(self) -> SdfNode {
        self.mirror(Vec3::Y)
    }

    /// Mirror across Z axis (XY plane)
    fn mirror_z(self) -> SdfNode {
        self.mirror(Vec3::Z)
    }

    // === Modifiers ===

    /// Create a hollow shell with given wall thickness
    fn shell(self, thickness: f32) -> SdfNode {
        SdfNode::new(operations::Shell::new(self, thickness))
    }

    /// Shorthand for shell - hollow out the shape
    fn hollow(self, thickness: f32) -> SdfNode {
        self.shell(thickness)
    }

    /// Round all edges with given radius
    fn round(self, radius: f32) -> SdfNode {
        SdfNode::new(operations::Round::new(self, radius))
    }

    /// Create onion layers (multiple shells)
    fn onion(self, thickness: f32) -> SdfNode {
        SdfNode::new(operations::Onion::new(self, thickness))
    }

    /// Elongate the shape along given half-extents
    fn elongate(self, h: Vec3) -> SdfNode {
        SdfNode::new(operations::Elongate::new(self, h))
    }

    // === Repetition ===

    /// Infinite repetition with given spacing
    fn repeat(self, spacing: Vec3) -> SdfNode {
        SdfNode::new(operations::RepeatInfinite::new(self, spacing))
    }

    /// Limited repetition with given spacing and count
    fn repeat_limited(self, spacing: Vec3, count: glam::UVec3) -> SdfNode {
        SdfNode::new(operations::RepeatLimited::new(self, spacing, count))
    }

    /// Polar (radial) repetition around Y axis
    fn repeat_polar(self, count: u32) -> SdfNode {
        SdfNode::new(operations::RepeatPolar::new(self, count))
    }

    // === Deformations ===

    /// Twist around Y axis
    fn twist(self, amount: f32) -> SdfNode {
        SdfNode::new(transforms::Twist::new(self, amount))
    }

    /// Bend along Y axis
    fn bend(self, amount: f32) -> SdfNode {
        SdfNode::new(transforms::Bend::new(self, amount))
    }
}

// Implement SdfExt for all types that implement Sdf
impl<T: Sdf + 'static> SdfExt for T {}

// Re-exports
pub use operations::*;
pub use primitives::*;
pub use transforms::*;
