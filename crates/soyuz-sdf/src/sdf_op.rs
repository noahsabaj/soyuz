//! SDF operation types
//!
//! This module defines the SDF operation tree representation that can be
//! converted to WGSL shader code for GPU raymarching.

/// Represents an SDF operation in a format suitable for shader generation
#[derive(Debug, Clone)]
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
        a: Box<SdfOp>,
        b: Box<SdfOp>,
    },
    Subtract {
        a: Box<SdfOp>,
        b: Box<SdfOp>,
    },
    Intersect {
        a: Box<SdfOp>,
        b: Box<SdfOp>,
    },
    SmoothUnion {
        a: Box<SdfOp>,
        b: Box<SdfOp>,
        k: f32,
    },
    SmoothSubtract {
        a: Box<SdfOp>,
        b: Box<SdfOp>,
        k: f32,
    },
    SmoothIntersect {
        a: Box<SdfOp>,
        b: Box<SdfOp>,
        k: f32,
    },

    // Modifiers
    Shell {
        inner: Box<SdfOp>,
        thickness: f32,
    },
    Round {
        inner: Box<SdfOp>,
        radius: f32,
    },
    Onion {
        inner: Box<SdfOp>,
        thickness: f32,
    },
    Elongate {
        inner: Box<SdfOp>,
        h: [f32; 3],
    },

    // Transforms
    Translate {
        inner: Box<SdfOp>,
        offset: [f32; 3],
    },
    RotateX {
        inner: Box<SdfOp>,
        angle: f32,
    },
    RotateY {
        inner: Box<SdfOp>,
        angle: f32,
    },
    RotateZ {
        inner: Box<SdfOp>,
        angle: f32,
    },
    Scale {
        inner: Box<SdfOp>,
        factor: f32,
    },
    Mirror {
        inner: Box<SdfOp>,
        axis: [f32; 3],
    },
    SymmetryX {
        inner: Box<SdfOp>,
    },
    SymmetryY {
        inner: Box<SdfOp>,
    },
    SymmetryZ {
        inner: Box<SdfOp>,
    },

    // Deformations
    Twist {
        inner: Box<SdfOp>,
        amount: f32,
    },
    Bend {
        inner: Box<SdfOp>,
        amount: f32,
    },

    // Repetition
    RepeatInfinite {
        inner: Box<SdfOp>,
        spacing: [f32; 3],
    },
    RepeatLimited {
        inner: Box<SdfOp>,
        spacing: [f32; 3],
        count: [f32; 3],
    },
    RepeatPolar {
        inner: Box<SdfOp>,
        count: u32,
    },
}
