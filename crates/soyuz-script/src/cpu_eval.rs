//! CPU-side SDF evaluation for mesh generation
//!
//! This module implements the `Sdf` trait from soyuz-core for `SdfOp`,
//! enabling CPU evaluation of script-generated SDFs for mesh export via marching cubes.

// Mathematical formulas use standard notation with single-char variable names
// and mathematical constants without separators (excess precision truncated)
// Large eval_distance function handles many SDF variants
// Explicit match arms for each SDF type improve readability even if bodies are similar
#![allow(clippy::many_single_char_names)]
#![allow(clippy::unreadable_literal)]
#![allow(clippy::excessive_precision)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::match_same_arms)]

use soyuz_core::sdf::{Aabb, Sdf};
use soyuz_sdf::SdfOp;
use std::sync::Arc;

// Re-export from soyuz-core prelude
use soyuz_core::prelude::{Vec2, Vec3};

/// Wrapper around [`SdfOp`] that implements the [`Sdf`] trait.
///
/// This wrapper is necessary due to Rust's orphan rules - we cannot implement
/// a foreign trait ([`Sdf`] from soyuz-core) for a foreign type ([`SdfOp`] from soyuz-sdf)
/// in this crate.
#[derive(Debug, Clone)]
pub struct CpuSdf {
    /// The underlying SDF operation tree
    pub op: Arc<SdfOp>,
}

impl CpuSdf {
    /// Create a new [`CpuSdf`] from an [`SdfOp`]
    pub fn new(op: SdfOp) -> Self {
        Self { op: Arc::new(op) }
    }

    /// Create a new [`CpuSdf`] from an `Arc<SdfOp>`
    pub fn from_arc(op: Arc<SdfOp>) -> Self {
        Self { op }
    }
}

impl Sdf for CpuSdf {
    fn distance(&self, p: Vec3) -> f32 {
        eval_distance(&self.op, p)
    }

    fn bounds(&self) -> Aabb {
        eval_bounds(&self.op)
    }
}

/// Evaluate SDF distance at point p
fn eval_distance(op: &SdfOp, p: Vec3) -> f32 {
    match op {
        // === Primitives ===
        SdfOp::Sphere { radius } => p.length() - *radius,

        SdfOp::Box { half_extents } => {
            let h = Vec3::new(half_extents[0], half_extents[1], half_extents[2]);
            let q = p.abs() - h;
            q.max(Vec3::ZERO).length() + q.x.max(q.y.max(q.z)).min(0.0)
        }

        SdfOp::RoundedBox {
            half_extents,
            radius,
        } => {
            let h = Vec3::new(half_extents[0], half_extents[1], half_extents[2]);
            let q = p.abs() - h + Vec3::splat(*radius);
            q.max(Vec3::ZERO).length() + q.x.max(q.y.max(q.z)).min(0.0) - *radius
        }

        SdfOp::Cylinder {
            radius,
            half_height,
        } => {
            let d = Vec2::new(Vec2::new(p.x, p.z).length(), p.y).abs()
                - Vec2::new(*radius, *half_height);
            d.x.max(d.y).min(0.0) + d.max(Vec2::ZERO).length()
        }

        SdfOp::Capsule {
            radius,
            half_height,
        } => {
            let p_clamped = Vec3::new(p.x, p.y.clamp(-*half_height, *half_height), p.z);
            (p - p_clamped).length() - *radius
        }

        SdfOp::Torus {
            major_radius,
            minor_radius,
        } => {
            let q = Vec2::new(Vec2::new(p.x, p.z).length() - *major_radius, p.y);
            q.length() - *minor_radius
        }

        SdfOp::Cone { radius, height } => {
            let q = Vec2::new(*height, -*radius).normalize();
            let w = Vec2::new(Vec2::new(p.x, p.z).length(), p.y);
            let a = w - q * w.dot(q).clamp(0.0, *height / q.x);
            let b = w - q * Vec2::new(*height / q.x, 0.0).min(w);
            let k = q.y.signum();
            let d = a.length_squared().min(b.length_squared());
            let s = (k * (w.x * q.y - w.y * q.x)).max(k * (w.y - *height));
            d.sqrt() * s.signum()
        }

        SdfOp::Plane { normal, offset } => {
            let n = Vec3::new(normal[0], normal[1], normal[2]).normalize();
            p.dot(n) + *offset
        }

        SdfOp::Ellipsoid { radii } => {
            let r = Vec3::new(radii[0], radii[1], radii[2]);
            let k0 = (p / r).length();
            let k1 = (p / (r * r)).length();
            k0 * (k0 - 1.0) / k1
        }

        SdfOp::Octahedron { size } => {
            let s = *size;
            let p = p.abs();
            let m = p.x + p.y + p.z - s;

            let q = if 3.0 * p.x < m {
                p
            } else if 3.0 * p.y < m {
                Vec3::new(p.y, p.z, p.x)
            } else if 3.0 * p.z < m {
                Vec3::new(p.z, p.x, p.y)
            } else {
                return m * 0.57735027; // 1/sqrt(3)
            };

            let k = (0.5_f32 * (q.z - q.y + s)).clamp(0.0, s);
            Vec3::new(q.x, q.y - s + k, q.z - k).length()
        }

        SdfOp::HexPrism {
            half_height,
            radius,
        } => {
            const K: Vec3 = Vec3::new(-0.866025404, 0.5, 0.577350269);
            let p_abs = p.abs();
            let xy = Vec2::new(p_abs.x, p_abs.z);
            let xy = xy - 2.0 * K.x.min(xy.dot(Vec2::new(K.x, K.y))) * Vec2::new(K.x, K.y);
            let d = Vec2::new(
                (xy - Vec2::new(xy.x.clamp(-K.z * *radius, K.z * *radius), *radius)).length()
                    * (xy.y - *radius).signum(),
                p_abs.y - *half_height,
            );
            d.x.max(d.y).min(0.0) + d.max(Vec2::ZERO).length()
        }

        SdfOp::TriPrism { size } => {
            let s = Vec2::new(size[0], size[1]);
            let q = p.abs();
            (q.z - s.y).max((q.x * 0.866025 + p.y * 0.5).max(-p.y) - s.x * 0.5)
        }

        // === Boolean Operations ===
        SdfOp::Union { a, b } => eval_distance(a, p).min(eval_distance(b, p)),

        SdfOp::Subtract { a, b } => eval_distance(a, p).max(-eval_distance(b, p)),

        SdfOp::Intersect { a, b } => eval_distance(a, p).max(eval_distance(b, p)),

        SdfOp::SmoothUnion { a, b, k } => {
            let d1 = eval_distance(a, p);
            let d2 = eval_distance(b, p);
            let h = (0.5 + 0.5 * (d2 - d1) / *k).clamp(0.0, 1.0);
            lerp(d2, d1, h) - *k * h * (1.0 - h)
        }

        SdfOp::SmoothSubtract { a, b, k } => {
            let d1 = eval_distance(a, p);
            let d2 = eval_distance(b, p);
            let h = (0.5 - 0.5 * (d2 + d1) / *k).clamp(0.0, 1.0);
            lerp(d1, -d2, h) + *k * h * (1.0 - h)
        }

        SdfOp::SmoothIntersect { a, b, k } => {
            let d1 = eval_distance(a, p);
            let d2 = eval_distance(b, p);
            let h = (0.5 - 0.5 * (d2 - d1) / *k).clamp(0.0, 1.0);
            lerp(d2, d1, h) + *k * h * (1.0 - h)
        }

        // === Modifiers ===
        SdfOp::Shell { inner, thickness } => eval_distance(inner, p).abs() - *thickness,

        SdfOp::Round { inner, radius } => eval_distance(inner, p) - *radius,

        SdfOp::Onion { inner, thickness } => {
            (eval_distance(inner, p).abs() % (*thickness * 2.0)) - *thickness
        }

        SdfOp::Elongate { inner, h } => {
            let h = Vec3::new(h[0], h[1], h[2]);
            let q = p.abs() - h;
            eval_distance(inner, q.max(Vec3::ZERO)) + q.x.max(q.y.max(q.z)).min(0.0)
        }

        // === Transforms ===
        SdfOp::Translate { inner, offset } => {
            let o = Vec3::new(offset[0], offset[1], offset[2]);
            eval_distance(inner, p - o)
        }

        SdfOp::RotateX { inner, angle } => {
            let c = angle.cos();
            let s = angle.sin();
            let q = Vec3::new(p.x, c * p.y + s * p.z, -s * p.y + c * p.z);
            eval_distance(inner, q)
        }

        SdfOp::RotateY { inner, angle } => {
            let c = angle.cos();
            let s = angle.sin();
            let q = Vec3::new(c * p.x - s * p.z, p.y, s * p.x + c * p.z);
            eval_distance(inner, q)
        }

        SdfOp::RotateZ { inner, angle } => {
            let c = angle.cos();
            let s = angle.sin();
            let q = Vec3::new(c * p.x + s * p.y, -s * p.x + c * p.y, p.z);
            eval_distance(inner, q)
        }

        SdfOp::Scale { inner, factor } => eval_distance(inner, p / *factor) * *factor,

        SdfOp::Mirror { inner, axis } => {
            // Mirror along arbitrary axis by reflecting the point
            let axis = Vec3::new(axis[0], axis[1], axis[2]).normalize();
            let d = p.dot(axis);
            let q = if d < 0.0 { p - 2.0 * d * axis } else { p };
            eval_distance(inner, q)
        }

        SdfOp::SymmetryX { inner } => {
            let q = Vec3::new(p.x.abs(), p.y, p.z);
            eval_distance(inner, q)
        }

        SdfOp::SymmetryY { inner } => {
            let q = Vec3::new(p.x, p.y.abs(), p.z);
            eval_distance(inner, q)
        }

        SdfOp::SymmetryZ { inner } => {
            let q = Vec3::new(p.x, p.y, p.z.abs());
            eval_distance(inner, q)
        }

        // === Deformations ===
        SdfOp::Twist { inner, amount } => {
            let c = (*amount * p.y).cos();
            let s = (*amount * p.y).sin();
            let q = Vec3::new(c * p.x - s * p.z, p.y, s * p.x + c * p.z);
            eval_distance(inner, q)
        }

        SdfOp::Bend { inner, amount } => {
            let c = (*amount * p.x).cos();
            let s = (*amount * p.x).sin();
            let q = Vec3::new(c * p.x - s * p.y, s * p.x + c * p.y, p.z);
            eval_distance(inner, q)
        }

        // === Repetition ===
        SdfOp::RepeatInfinite { inner, spacing } => {
            let s = Vec3::new(spacing[0], spacing[1], spacing[2]);
            // Only repeat along axes with non-zero spacing
            let q = Vec3::new(
                if s.x > 0.0 {
                    (p.x + s.x * 0.5).rem_euclid(s.x) - s.x * 0.5
                } else {
                    p.x
                },
                if s.y > 0.0 {
                    (p.y + s.y * 0.5).rem_euclid(s.y) - s.y * 0.5
                } else {
                    p.y
                },
                if s.z > 0.0 {
                    (p.z + s.z * 0.5).rem_euclid(s.z) - s.z * 0.5
                } else {
                    p.z
                },
            );
            eval_distance(inner, q)
        }

        SdfOp::RepeatLimited {
            inner,
            spacing,
            count,
        } => {
            let s = Vec3::new(spacing[0], spacing[1], spacing[2]);
            let c = Vec3::new(count[0], count[1], count[2]);
            let q = p - s * (p / s).round().clamp(-c, c);
            eval_distance(inner, q)
        }

        SdfOp::RepeatPolar { inner, count } => {
            // Use SSOT formula from soyuz-math (generated from formulas/repeat_polar.toml)
            let q = soyuz_math::repeat_polar(p, *count as f32);
            eval_distance(inner, q)
        }

        // Handle non-exhaustive enum
        _ => {
            // Unknown variant - return a large distance
            f32::MAX
        }
    }
}

/// Evaluate bounding box for an SDF operation
fn eval_bounds(op: &SdfOp) -> Aabb {
    match op {
        SdfOp::Sphere { radius } => Aabb::cube(*radius),

        SdfOp::Box { half_extents } => {
            let h = Vec3::new(half_extents[0], half_extents[1], half_extents[2]);
            Aabb::new(-h, h)
        }

        SdfOp::RoundedBox { half_extents, .. } => {
            let h = Vec3::new(half_extents[0], half_extents[1], half_extents[2]);
            Aabb::new(-h, h)
        }

        SdfOp::Cylinder {
            radius,
            half_height,
        } => Aabb::new(
            Vec3::new(-*radius, -*half_height, -*radius),
            Vec3::new(*radius, *half_height, *radius),
        ),

        SdfOp::Capsule {
            radius,
            half_height,
        } => {
            let h = *half_height + *radius;
            Aabb::new(
                Vec3::new(-*radius, -h, -*radius),
                Vec3::new(*radius, h, *radius),
            )
        }

        SdfOp::Torus {
            major_radius,
            minor_radius,
        } => {
            let r = *major_radius + *minor_radius;
            Aabb::new(
                Vec3::new(-r, -*minor_radius, -r),
                Vec3::new(r, *minor_radius, r),
            )
        }

        SdfOp::Cone { radius, height } => Aabb::new(
            Vec3::new(-*radius, 0.0, -*radius),
            Vec3::new(*radius, *height, *radius),
        ),

        SdfOp::Plane { .. } => Aabb::cube(100.0),

        SdfOp::Ellipsoid { radii } => {
            let r = Vec3::new(radii[0], radii[1], radii[2]);
            Aabb::new(-r, r)
        }

        SdfOp::Octahedron { size } => Aabb::cube(*size),

        SdfOp::HexPrism {
            half_height,
            radius,
        } => Aabb::new(
            Vec3::new(-*radius, -*half_height, -*radius),
            Vec3::new(*radius, *half_height, *radius),
        ),

        SdfOp::TriPrism { size } => {
            let s = size[0].max(size[1]);
            Aabb::cube(s)
        }

        // Boolean operations
        SdfOp::Union { a, b } => eval_bounds(a).union(&eval_bounds(b)),

        SdfOp::Subtract { a, .. } => eval_bounds(a),

        SdfOp::Intersect { a, .. } => eval_bounds(a),

        SdfOp::SmoothUnion { a, b, k } => eval_bounds(a).union(&eval_bounds(b)).expand(*k),

        SdfOp::SmoothSubtract { a, .. } => eval_bounds(a),

        SdfOp::SmoothIntersect { a, .. } => eval_bounds(a),

        // Modifiers
        SdfOp::Shell { inner, thickness } => eval_bounds(inner).expand(*thickness),

        SdfOp::Round { inner, radius } => eval_bounds(inner).expand(*radius),

        SdfOp::Onion { inner, thickness } => eval_bounds(inner).expand(*thickness),

        SdfOp::Elongate { inner, h } => {
            let bounds = eval_bounds(inner);
            let h = Vec3::new(h[0], h[1], h[2]);
            Aabb::new(bounds.min - h, bounds.max + h)
        }

        // Transforms
        SdfOp::Translate { inner, offset } => {
            let bounds = eval_bounds(inner);
            let o = Vec3::new(offset[0], offset[1], offset[2]);
            Aabb::new(bounds.min + o, bounds.max + o)
        }

        SdfOp::RotateX { inner, .. }
        | SdfOp::RotateY { inner, .. }
        | SdfOp::RotateZ { inner, .. } => {
            // Conservative: expand to enclosing sphere
            let bounds = eval_bounds(inner);
            let r = bounds.size().length() * 0.5;
            let center = bounds.center();
            Aabb::from_center(center, Vec3::splat(r))
        }

        SdfOp::Scale { inner, factor } => {
            let bounds = eval_bounds(inner);
            Aabb::new(bounds.min * *factor, bounds.max * *factor)
        }

        SdfOp::Mirror { inner, .. }
        | SdfOp::SymmetryX { inner }
        | SdfOp::SymmetryY { inner }
        | SdfOp::SymmetryZ { inner } => {
            let bounds = eval_bounds(inner);
            // Mirror expands to cover both sides
            let max_extent = bounds.min.abs().max(bounds.max.abs());
            Aabb::new(-max_extent, max_extent)
        }

        // Deformations
        SdfOp::Twist { inner, .. } | SdfOp::Bend { inner, .. } => {
            // Conservative bounds for deformations
            eval_bounds(inner).expand(0.5)
        }

        // Repetition
        SdfOp::RepeatInfinite { .. } => {
            // Can't have finite bounds for infinite repetition
            // Return large bounds, mesh generation will need to limit sampling area
            Aabb::cube(10.0)
        }

        SdfOp::RepeatLimited {
            inner,
            spacing,
            count,
        } => {
            let bounds = eval_bounds(inner);
            let s = Vec3::new(spacing[0], spacing[1], spacing[2]);
            let c = Vec3::new(count[0], count[1], count[2]);
            let total = s * c * 2.0;
            Aabb::new(bounds.min - total * 0.5, bounds.max + total * 0.5)
        }

        SdfOp::RepeatPolar { inner, .. } => {
            let bounds = eval_bounds(inner);
            let r = bounds
                .max
                .x
                .abs()
                .max(bounds.max.z.abs())
                .max(bounds.min.x.abs())
                .max(bounds.min.z.abs());
            Aabb::new(
                Vec3::new(-r, bounds.min.y, -r),
                Vec3::new(r, bounds.max.y, r),
            )
        }

        // Handle non-exhaustive enum
        _ => Aabb::cube(10.0),
    }
}

#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
