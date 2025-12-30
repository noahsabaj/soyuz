//! SDF operation types
//!
//! This module defines the SDF operation tree representation that can be
//! converted to WGSL shader code for GPU raymarching.

use std::sync::Arc;

/// Represents an SDF operation in a format suitable for shader generation.
///
/// Uses `Arc` for child nodes to enable efficient cloning (O(1) reference count increment
/// instead of O(n) deep clone) and thread-safe sharing of SDF trees.
///
/// Marked `#[non_exhaustive]` to allow adding new SDF primitives and operations
/// in future versions without breaking downstream code.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum SdfOp {
    // Primitives
    Sphere {
        radius: f32,
    },
    Box {
        half_extents: [f32; 3],
    },
    RoundedBox {
        half_extents: [f32; 3],
        radius: f32,
    },
    Cylinder {
        radius: f32,
        half_height: f32,
    },
    Capsule {
        radius: f32,
        half_height: f32,
    },
    Torus {
        major_radius: f32,
        minor_radius: f32,
    },
    Cone {
        radius: f32,
        height: f32,
    },
    Plane {
        normal: [f32; 3],
        offset: f32,
    },
    Ellipsoid {
        radii: [f32; 3],
    },
    Octahedron {
        size: f32,
    },
    HexPrism {
        half_height: f32,
        radius: f32,
    },
    TriPrism {
        size: [f32; 2],
    },

    // Boolean operations
    Union {
        a: Arc<SdfOp>,
        b: Arc<SdfOp>,
    },
    Subtract {
        a: Arc<SdfOp>,
        b: Arc<SdfOp>,
    },
    Intersect {
        a: Arc<SdfOp>,
        b: Arc<SdfOp>,
    },
    SmoothUnion {
        a: Arc<SdfOp>,
        b: Arc<SdfOp>,
        k: f32,
    },
    SmoothSubtract {
        a: Arc<SdfOp>,
        b: Arc<SdfOp>,
        k: f32,
    },
    SmoothIntersect {
        a: Arc<SdfOp>,
        b: Arc<SdfOp>,
        k: f32,
    },

    // Modifiers
    Shell {
        inner: Arc<SdfOp>,
        thickness: f32,
    },
    Round {
        inner: Arc<SdfOp>,
        radius: f32,
    },
    Onion {
        inner: Arc<SdfOp>,
        thickness: f32,
    },
    Elongate {
        inner: Arc<SdfOp>,
        h: [f32; 3],
    },

    // Transforms
    Translate {
        inner: Arc<SdfOp>,
        offset: [f32; 3],
    },
    RotateX {
        inner: Arc<SdfOp>,
        angle: f32,
    },
    RotateY {
        inner: Arc<SdfOp>,
        angle: f32,
    },
    RotateZ {
        inner: Arc<SdfOp>,
        angle: f32,
    },
    Scale {
        inner: Arc<SdfOp>,
        factor: f32,
    },
    Mirror {
        inner: Arc<SdfOp>,
        axis: [f32; 3],
    },
    SymmetryX {
        inner: Arc<SdfOp>,
    },
    SymmetryY {
        inner: Arc<SdfOp>,
    },
    SymmetryZ {
        inner: Arc<SdfOp>,
    },

    // Deformations
    Twist {
        inner: Arc<SdfOp>,
        amount: f32,
    },
    Bend {
        inner: Arc<SdfOp>,
        amount: f32,
    },

    // Repetition
    RepeatInfinite {
        inner: Arc<SdfOp>,
        spacing: [f32; 3],
    },
    RepeatLimited {
        inner: Arc<SdfOp>,
        spacing: [f32; 3],
        count: [f32; 3],
    },
    RepeatPolar {
        inner: Arc<SdfOp>,
        count: u32,
    },
}
