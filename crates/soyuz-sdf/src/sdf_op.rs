//! SDF operation types
//!
//! This module defines the SDF operation tree representation that can be
//! converted to WGSL shader code for GPU raymarching.

use std::sync::Arc;

/// Error returned when an SDF contains invalid float values
#[derive(Debug, Clone)]
pub struct InvalidFloatError {
    /// Description of where the invalid value was found
    pub context: String,
    /// The invalid value
    pub value: f32,
}

impl std::fmt::Display for InvalidFloatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kind = if self.value.is_nan() {
            "NaN"
        } else if self.value.is_infinite() {
            if self.value.is_sign_positive() {
                "+Infinity"
            } else {
                "-Infinity"
            }
        } else {
            "invalid"
        };
        write!(
            f,
            "Invalid float value {} ({}) in {}",
            kind, self.value, self.context
        )
    }
}

impl std::error::Error for InvalidFloatError {}

/// Check if a float value is valid (not NaN or Infinite)
fn check_float(value: f32, context: &str) -> Result<(), InvalidFloatError> {
    if value.is_nan() || value.is_infinite() {
        Err(InvalidFloatError {
            context: context.to_string(),
            value,
        })
    } else {
        Ok(())
    }
}

/// Check if an array of floats contains only valid values
fn check_floats(values: &[f32], context: &str) -> Result<(), InvalidFloatError> {
    for (i, &v) in values.iter().enumerate() {
        if v.is_nan() || v.is_infinite() {
            return Err(InvalidFloatError {
                context: format!("{}[{}]", context, i),
                value: v,
            });
        }
    }
    Ok(())
}

/// Profile shape for 2D-to-3D extrusion operations
#[derive(Debug, Clone)]
pub enum ExtrudeProfile {
    Circle {
        radius: f32,
    },
    Rectangle {
        width: f32,
        height: f32,
    },
    RoundedRectangle {
        width: f32,
        height: f32,
        radius: f32,
    },
}

/// Profile shape for 2D-to-3D revolution (lathe) operations
#[derive(Debug, Clone)]
pub enum RevolveProfile {
    Circle { radius: f32 },
    Rectangle { width: f32, height: f32 },
}

/// Represents an SDF operation in a format suitable for shader generation.
///
/// Uses `Arc` for child nodes to enable efficient cloning (O(1) reference count increment
/// instead of O(n) deep clone) and thread-safe sharing of SDF trees.
///
/// Deliberately NOT `#[non_exhaustive]`: every consumer (CPU evaluator, WGSL
/// generator, validation) must match exhaustively so that adding a variant is
/// a compile error everywhere at once, never a silent fallback.
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
    Pyramid {
        height: f32,
    },
    Link {
        length: f32,
        major_radius: f32,
        minor_radius: f32,
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
    Xor {
        a: Arc<SdfOp>,
        b: Arc<SdfOp>,
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
    Displacement {
        inner: Arc<SdfOp>,
        amount: f32,
        frequency: f32,
    },

    // 2D-to-3D Operations
    Extrude {
        profile: ExtrudeProfile,
        depth: f32,
    },
    Revolve {
        profile: RevolveProfile,
        offset: f32,
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

impl SdfOp {
    /// Iteratively bound the tree's depth and node count BEFORE any recursive
    /// pass (`validate`, CPU eval, WGSL codegen) walks it.
    ///
    /// A user script can build an arbitrarily deep tree at runtime (e.g.
    /// `while i < 200000 { s = s.union(sphere(0.5)) }`); the recursive passes
    /// would then overflow the stack (an uncatchable abort). This check uses an
    /// explicit stack so it cannot itself overflow, and returns an error first.
    ///
    /// # Errors
    /// Returns an error if the tree is deeper than `max_depth` or has more than
    /// `max_nodes` nodes.
    pub fn check_complexity(
        &self,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<(), InvalidFloatError> {
        let mut stack: Vec<(&SdfOp, usize)> = vec![(self, 1)];
        let mut nodes: usize = 0;
        while let Some((op, depth)) = stack.pop() {
            nodes += 1;
            if depth > max_depth || nodes > max_nodes {
                return Err(InvalidFloatError {
                    context: format!(
                        "SDF tree too complex (depth limit {max_depth}, node limit {max_nodes})"
                    ),
                    value: depth as f32,
                });
            }
            match op {
                SdfOp::Union { a, b }
                | SdfOp::Subtract { a, b }
                | SdfOp::Intersect { a, b }
                | SdfOp::Xor { a, b }
                | SdfOp::SmoothUnion { a, b, .. }
                | SdfOp::SmoothSubtract { a, b, .. }
                | SdfOp::SmoothIntersect { a, b, .. } => {
                    stack.push((a, depth + 1));
                    stack.push((b, depth + 1));
                }
                SdfOp::Shell { inner, .. }
                | SdfOp::Round { inner, .. }
                | SdfOp::Onion { inner, .. }
                | SdfOp::Elongate { inner, .. }
                | SdfOp::Translate { inner, .. }
                | SdfOp::RotateX { inner, .. }
                | SdfOp::RotateY { inner, .. }
                | SdfOp::RotateZ { inner, .. }
                | SdfOp::Scale { inner, .. }
                | SdfOp::Mirror { inner, .. }
                | SdfOp::SymmetryX { inner }
                | SdfOp::SymmetryY { inner }
                | SdfOp::SymmetryZ { inner }
                | SdfOp::Twist { inner, .. }
                | SdfOp::Bend { inner, .. }
                | SdfOp::Displacement { inner, .. }
                | SdfOp::RepeatInfinite { inner, .. }
                | SdfOp::RepeatLimited { inner, .. }
                | SdfOp::RepeatPolar { inner, .. } => {
                    stack.push((inner, depth + 1));
                }
                // Primitives and 2D-to-3D profile ops have no child SDFs.
                SdfOp::Sphere { .. }
                | SdfOp::Box { .. }
                | SdfOp::RoundedBox { .. }
                | SdfOp::Cylinder { .. }
                | SdfOp::Capsule { .. }
                | SdfOp::Torus { .. }
                | SdfOp::Cone { .. }
                | SdfOp::Plane { .. }
                | SdfOp::Ellipsoid { .. }
                | SdfOp::Octahedron { .. }
                | SdfOp::HexPrism { .. }
                | SdfOp::TriPrism { .. }
                | SdfOp::Pyramid { .. }
                | SdfOp::Link { .. }
                | SdfOp::Extrude { .. }
                | SdfOp::Revolve { .. } => {}
            }
        }
        Ok(())
    }

    /// Validate that all float values in the SDF tree are finite (not NaN or Infinite).
    ///
    /// This should be called before passing an SDF to the GPU to prevent crashes
    /// from invalid float values that can arise from expressions like `1.0/0.0`.
    ///
    /// # Errors
    /// Returns an error if any float value in the tree is NaN or Infinite.
    #[allow(clippy::too_many_lines)] // Exhaustive match over all SDF variants is inherently long
    pub fn validate(&self) -> Result<(), InvalidFloatError> {
        match self {
            // Primitives
            SdfOp::Sphere { radius } => check_float(*radius, "sphere radius"),
            SdfOp::Box { half_extents } => check_floats(half_extents, "box half_extents"),
            SdfOp::RoundedBox {
                half_extents,
                radius,
            } => {
                check_floats(half_extents, "rounded_box half_extents")?;
                check_float(*radius, "rounded_box radius")
            }
            SdfOp::Cylinder {
                radius,
                half_height,
            } => {
                check_float(*radius, "cylinder radius")?;
                check_float(*half_height, "cylinder half_height")
            }
            SdfOp::Capsule {
                radius,
                half_height,
            } => {
                check_float(*radius, "capsule radius")?;
                check_float(*half_height, "capsule half_height")
            }
            SdfOp::Torus {
                major_radius,
                minor_radius,
            } => {
                check_float(*major_radius, "torus major_radius")?;
                check_float(*minor_radius, "torus minor_radius")
            }
            SdfOp::Cone { radius, height } => {
                check_float(*radius, "cone radius")?;
                check_float(*height, "cone height")
            }
            SdfOp::Plane { normal, offset } => {
                check_floats(normal, "plane normal")?;
                check_float(*offset, "plane offset")
            }
            SdfOp::Ellipsoid { radii } => check_floats(radii, "ellipsoid radii"),
            SdfOp::Octahedron { size } => check_float(*size, "octahedron size"),
            SdfOp::HexPrism {
                half_height,
                radius,
            } => {
                check_float(*half_height, "hex_prism half_height")?;
                check_float(*radius, "hex_prism radius")
            }
            SdfOp::TriPrism { size } => check_floats(size, "tri_prism size"),
            SdfOp::Pyramid { height } => check_float(*height, "pyramid height"),
            SdfOp::Link {
                length,
                major_radius,
                minor_radius,
            } => {
                check_float(*length, "link length")?;
                check_float(*major_radius, "link major_radius")?;
                check_float(*minor_radius, "link minor_radius")
            }

            // Boolean operations
            SdfOp::Union { a, b }
            | SdfOp::Subtract { a, b }
            | SdfOp::Intersect { a, b }
            | SdfOp::Xor { a, b } => {
                a.validate()?;
                b.validate()
            }
            SdfOp::SmoothUnion { a, b, k }
            | SdfOp::SmoothSubtract { a, b, k }
            | SdfOp::SmoothIntersect { a, b, k } => {
                a.validate()?;
                b.validate()?;
                check_float(*k, "smooth operation k")?;
                if *k == 0.0 {
                    // Both the CPU evaluator and the generated WGSL divide by
                    // `k`, which goes NaN wherever the two surfaces are
                    // equidistant.
                    return Err(InvalidFloatError {
                        context: "smooth operation k must be non-zero".to_string(),
                        value: 0.0,
                    });
                }
                Ok(())
            }

            // Modifiers
            SdfOp::Shell { inner, thickness } => {
                inner.validate()?;
                check_float(*thickness, "shell thickness")
            }
            SdfOp::Round { inner, radius } => {
                inner.validate()?;
                check_float(*radius, "round radius")
            }
            SdfOp::Onion { inner, thickness } => {
                inner.validate()?;
                check_float(*thickness, "onion thickness")
            }
            SdfOp::Elongate { inner, h } => {
                inner.validate()?;
                check_floats(h, "elongate h")
            }

            // Transforms
            SdfOp::Translate { inner, offset } => {
                inner.validate()?;
                check_floats(offset, "translate offset")
            }
            SdfOp::RotateX { inner, angle }
            | SdfOp::RotateY { inner, angle }
            | SdfOp::RotateZ { inner, angle } => {
                inner.validate()?;
                check_float(*angle, "rotate angle")
            }
            SdfOp::Scale { inner, factor } => {
                inner.validate()?;
                check_float(*factor, "scale factor")?;
                if *factor == 0.0 {
                    return Err(InvalidFloatError {
                        context: "scale factor must be non-zero".to_string(),
                        value: 0.0,
                    });
                }
                Ok(())
            }
            SdfOp::Mirror { inner, axis } => {
                inner.validate()?;
                check_floats(axis, "mirror axis")
            }
            SdfOp::SymmetryX { inner }
            | SdfOp::SymmetryY { inner }
            | SdfOp::SymmetryZ { inner } => inner.validate(),
            SdfOp::RepeatPolar { inner, count } => {
                if *count == 0 {
                    return Err(InvalidFloatError {
                        context: "repeat_polar count must be >= 1".to_string(),
                        value: 0.0,
                    });
                }
                inner.validate()
            }

            // Deformations
            SdfOp::Twist { inner, amount } | SdfOp::Bend { inner, amount } => {
                inner.validate()?;
                check_float(*amount, "deformation amount")
            }
            SdfOp::Displacement {
                inner,
                amount,
                frequency,
            } => {
                inner.validate()?;
                check_float(*amount, "displacement amount")?;
                check_float(*frequency, "displacement frequency")
            }

            // 2D-to-3D Operations
            SdfOp::Extrude { profile, depth } => {
                check_float(*depth, "extrude depth")?;
                match profile {
                    ExtrudeProfile::Circle { radius } => {
                        check_float(*radius, "extrude circle radius")
                    }
                    ExtrudeProfile::Rectangle { width, height } => {
                        check_float(*width, "extrude rect width")?;
                        check_float(*height, "extrude rect height")
                    }
                    ExtrudeProfile::RoundedRectangle {
                        width,
                        height,
                        radius,
                    } => {
                        check_float(*width, "extrude rounded_rect width")?;
                        check_float(*height, "extrude rounded_rect height")?;
                        check_float(*radius, "extrude rounded_rect radius")
                    }
                }
            }
            SdfOp::Revolve { profile, offset } => {
                check_float(*offset, "revolve offset")?;
                match profile {
                    RevolveProfile::Circle { radius } => {
                        check_float(*radius, "revolve circle radius")
                    }
                    RevolveProfile::Rectangle { width, height } => {
                        check_float(*width, "revolve rect width")?;
                        check_float(*height, "revolve rect height")
                    }
                }
            }

            // Repetition
            SdfOp::RepeatInfinite { inner, spacing } => {
                inner.validate()?;
                check_floats(spacing, "repeat spacing")
            }
            SdfOp::RepeatLimited {
                inner,
                spacing,
                count,
            } => {
                inner.validate()?;
                check_floats(spacing, "repeat_limited spacing")?;
                check_floats(count, "repeat_limited count")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn smooth_union(k: f32) -> SdfOp {
        SdfOp::SmoothUnion {
            a: Arc::new(SdfOp::Sphere { radius: 0.5 }),
            b: Arc::new(SdfOp::Sphere { radius: 0.5 }),
            k,
        }
    }

    #[test]
    fn validate_rejects_zero_smooth_k() {
        assert!(smooth_union(0.0).validate().is_err());
        assert!(smooth_union(-0.0).validate().is_err());
    }

    #[test]
    fn validate_accepts_nonzero_smooth_k() {
        assert!(smooth_union(0.15).validate().is_ok());
    }
}
