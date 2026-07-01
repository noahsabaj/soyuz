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
use soyuz_sdf::{ExtrudeProfile, RevolveProfile, SdfOp};
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
            // Distance to the central Y segment, minus radius. The previous code
            // subtracted a point that shared p's x/z, cancelling the radial term
            // and collapsing the pill into a Y slab. Matches GPU sd_capsule.
            let py = p.y.clamp(-*half_height, *half_height);
            (p - Vec3::new(0.0, py, 0.0)).length() - *radius
        }

        SdfOp::Torus {
            major_radius,
            minor_radius,
        } => {
            let q = Vec2::new(Vec2::new(p.x, p.z).length() - *major_radius, p.y);
            q.length() - *minor_radius
        }

        SdfOp::Cone { radius, height } => {
            // Exact solid cone: apex at origin, opening upward to base radius
            // `radius` at y = `height`. Identical to GPU sd_cone in raymarch.wgsl.
            let r = *radius;
            let h = *height;
            let q = Vec2::new(p.x, p.z).length();
            let paba = p.y / h;
            let cax = (q - if paba < 0.5 { 0.0 } else { r }).max(0.0);
            let cay = (paba - 0.5).abs() - 0.5;
            let k = r * r + h * h;
            let f = ((r * q + p.y * h) / k).clamp(0.0, 1.0);
            let cbx = q - f * r;
            let cby = paba - f;
            let s = if cbx < 0.0 && cay < 0.0 { -1.0 } else { 1.0 };
            let da = cax * cax + cay * cay * h * h;
            let db = cbx * cbx + cby * cby * h * h;
            s * da.min(db).sqrt()
        }

        SdfOp::Plane { normal, offset } => {
            // Use the stored normal as-is (GPU sd_plane does not normalize). The
            // script API normalizes at construction so the stored normal is unit.
            let n = Vec3::new(normal[0], normal[1], normal[2]);
            p.dot(n) + *offset
        }

        SdfOp::Ellipsoid { radii } => {
            let r = Vec3::new(radii[0], radii[1], radii[2]);
            let k0 = (p / r).length();
            let k1 = (p / (r * r)).length();
            // At the center k0,k1 ~ 0 (0/0 -> NaN). Return the inradius. Matches GPU.
            if k1 < 1e-6 {
                return -r.x.min(r.y).min(r.z);
            }
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
            // Fold: the second factor must be min(dot, 0.0), not min(K.x, dot).
            // The old form clamped to the constant -0.866 and broke the hexagon.
            let xy = xy - 2.0 * xy.dot(Vec2::new(K.x, K.y)).min(0.0) * Vec2::new(K.x, K.y);
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

        SdfOp::Pyramid { height } => sd_pyramid(p, *height),

        SdfOp::Link {
            length,
            major_radius,
            minor_radius,
        } => {
            let q = Vec3::new(p.x, (p.y.abs() - *length).max(0.0), p.z);
            Vec2::new(Vec2::new(q.x, q.y).length() - *major_radius, q.z).length() - *minor_radius
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

        SdfOp::Xor { a, b } => {
            let d1 = eval_distance(a, p);
            let d2 = eval_distance(b, p);
            // XOR = (A AND NOT B) OR (B AND NOT A)
            // In SDF terms: max(min(d1, d2), -max(d1, d2))
            d1.min(d2).max(-d1.max(d2))
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
            // Fold across the dominant axis plane, matching the GPU codegen which
            // maps Mirror to op_symmetry_{x,y,z}. The script only exposes the
            // axis-aligned mirror_x/y/z, so this is exact for every reachable case.
            let q = if axis[0].abs() > 0.5 {
                Vec3::new(p.x.abs(), p.y, p.z)
            } else if axis[1].abs() > 0.5 {
                Vec3::new(p.x, p.y.abs(), p.z)
            } else {
                Vec3::new(p.x, p.y, p.z.abs())
            };
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

        SdfOp::Displacement {
            inner,
            amount,
            frequency,
        } => {
            // Two-octave noise identical to GPU noise3d(p * freq) so the meshed
            // surface matches the preview.
            let d = eval_distance(inner, p);
            let pf = p * *frequency;
            let noise = pf.x.sin() * (pf.y * 1.1).sin() * (pf.z * 0.9).sin()
                + (pf.x * 2.3).sin() * (pf.y * 2.1).sin() * (pf.z * 2.5).sin() * 0.5;
            d + *amount * noise
        }

        // === 2D-to-3D Operations ===
        SdfOp::Extrude { profile, depth } => {
            let d2d = eval_profile_2d(profile, Vec2::new(p.x, p.y));
            op_extrude(d2d, p.z, *depth)
        }

        SdfOp::Revolve { profile, offset } => {
            let q = Vec2::new(Vec2::new(p.x, p.z).length() - *offset, p.y);
            eval_revolve_profile(profile, q)
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

        SdfOp::Pyramid { height } => {
            Aabb::new(Vec3::new(-0.5, 0.0, -0.5), Vec3::new(0.5, *height, 0.5))
        }

        SdfOp::Link {
            length,
            major_radius,
            minor_radius,
        } => {
            let r = *major_radius + *minor_radius;
            Aabb::new(
                Vec3::new(-r, -(*length + r), -*minor_radius),
                Vec3::new(r, *length + r, *minor_radius),
            )
        }

        // Boolean operations
        SdfOp::Union { a, b } => eval_bounds(a).union(&eval_bounds(b)),

        SdfOp::Subtract { a, .. } => eval_bounds(a),

        SdfOp::Intersect { a, .. } => eval_bounds(a),

        SdfOp::SmoothUnion { a, b, k } => eval_bounds(a).union(&eval_bounds(b)).expand(*k),

        SdfOp::SmoothSubtract { a, .. } => eval_bounds(a),

        SdfOp::SmoothIntersect { a, .. } => eval_bounds(a),

        SdfOp::Xor { a, b } => {
            // XOR result is bounded by the union of both shapes
            // since any point can only be in the non-overlapping regions
            eval_bounds(a).union(&eval_bounds(b))
        }

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
            // Twist/bend rotate space by an amount that varies across the shape;
            // bound by the inner shape's enclosing sphere, which scales with the
            // shape (a fixed 0.5 margin clipped large shapes).
            let bounds = eval_bounds(inner);
            let r = bounds.size().length() * 0.5;
            Aabb::from_center(bounds.center(), Vec3::splat(r))
        }

        SdfOp::Displacement { inner, amount, .. } => {
            // Two-octave noise has peak amplitude 1.5, so the surface can move out
            // by up to 1.5 * amount; expand bounds to avoid clipping the mesh.
            eval_bounds(inner).expand(amount.abs() * 1.5)
        }

        // 2D-to-3D operations
        SdfOp::Extrude { profile, depth } => {
            let (min, max) = profile_bounds_2d(profile);
            Aabb::new(
                Vec3::new(min.x, min.y, -*depth),
                Vec3::new(max.x, max.y, *depth),
            )
        }

        SdfOp::Revolve { profile, offset } => {
            let (min, max) = profile_bounds_2d_for_revolve(profile);
            let radius = (offset + max.x.abs()).abs().max((offset + min.x).abs());
            Aabb::new(
                Vec3::new(-radius, min.y, -radius),
                Vec3::new(radius, max.y, radius),
            )
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

fn sd_pyramid(p: Vec3, height: f32) -> f32 {
    let m2 = height * height + 0.25;
    let mut pxz = Vec2::new(p.x.abs(), p.z.abs());
    if pxz.y > pxz.x {
        pxz = Vec2::new(pxz.y, pxz.x);
    }
    pxz -= Vec2::splat(0.5);

    let q = Vec3::new(
        pxz.y,
        height * p.y - 0.5 * pxz.x,
        height * pxz.x + 0.5 * p.y,
    );
    let s = (-q.x).max(0.0);
    let t = ((q.y - 0.5 * pxz.y) / (m2 + 0.25)).clamp(0.0, 1.0);

    let a = m2 * (q.x + s) * (q.x + s) + q.y * q.y;
    let b = m2 * (q.x + 0.5 * t) * (q.x + 0.5 * t) + (q.y - m2 * t) * (q.y - m2 * t);
    let d2 = if q.y.min(-q.x * m2 - q.y * 0.5) > 0.0 {
        0.0
    } else {
        a.min(b)
    };

    ((d2 + q.z * q.z) / m2).sqrt() * q.z.max(-p.y).signum()
}

fn eval_profile_2d(profile: &ExtrudeProfile, p: Vec2) -> f32 {
    match profile {
        ExtrudeProfile::Circle { radius } => sd_circle_2d(p, *radius),
        ExtrudeProfile::Rectangle { width, height } => {
            sd_box_2d(p, Vec2::new(*width * 0.5, *height * 0.5))
        }
        ExtrudeProfile::RoundedRectangle {
            width,
            height,
            radius,
        } => sd_rounded_box_2d(p, Vec2::new(*width * 0.5, *height * 0.5), *radius),
    }
}

fn eval_revolve_profile(profile: &RevolveProfile, p: Vec2) -> f32 {
    match profile {
        RevolveProfile::Circle { radius } => sd_circle_2d(p, *radius),
        RevolveProfile::Rectangle { width, height } => {
            sd_box_2d(p, Vec2::new(*width * 0.5, *height * 0.5))
        }
    }
}

fn sd_circle_2d(p: Vec2, radius: f32) -> f32 {
    p.length() - radius
}

fn sd_box_2d(p: Vec2, half_extents: Vec2) -> f32 {
    let d = p.abs() - half_extents;
    d.max(Vec2::ZERO).length() + d.x.max(d.y).min(0.0)
}

fn sd_rounded_box_2d(p: Vec2, half_extents: Vec2, radius: f32) -> f32 {
    let q = p.abs() - half_extents + Vec2::splat(radius);
    q.max(Vec2::ZERO).length() + q.x.max(q.y).min(0.0) - radius
}

fn op_extrude(d2d: f32, z: f32, half_depth: f32) -> f32 {
    let w = Vec2::new(d2d, z.abs() - half_depth);
    w.x.max(w.y).min(0.0) + w.max(Vec2::ZERO).length()
}

fn profile_bounds_2d(profile: &ExtrudeProfile) -> (Vec2, Vec2) {
    match profile {
        ExtrudeProfile::Circle { radius } => (Vec2::splat(-*radius), Vec2::splat(*radius)),
        ExtrudeProfile::Rectangle { width, height }
        | ExtrudeProfile::RoundedRectangle { width, height, .. } => {
            let half = Vec2::new(*width * 0.5, *height * 0.5);
            (-half, half)
        }
    }
}

fn profile_bounds_2d_for_revolve(profile: &RevolveProfile) -> (Vec2, Vec2) {
    match profile {
        RevolveProfile::Circle { radius } => (Vec2::splat(-*radius), Vec2::splat(*radius)),
        RevolveProfile::Rectangle { width, height } => {
            let half = Vec2::new(*width * 0.5, *height * 0.5);
            (-half, half)
        }
    }
}

/// Regression tests for the CPU evaluator. These lock in the behaviours that
/// were fixed so the CPU mesh stays in agreement with the GPU preview (RC-A).
#[cfg(test)]
mod tests {
    use super::*;

    fn dist(op: &SdfOp, p: [f32; 3]) -> f32 {
        eval_distance(op, Vec3::new(p[0], p[1], p[2]))
    }

    #[test]
    fn capsule_has_radial_distance() {
        // r=0.3, half-height 0.5. A point 0.5 out radially at y=0 is 0.2 OUTSIDE.
        // The old code cancelled the radial term and returned -0.3 (inside).
        let op = SdfOp::Capsule {
            radius: 0.3,
            half_height: 0.5,
        };
        assert!((dist(&op, [0.5, 0.0, 0.0]) - 0.2).abs() < 1e-5);
    }

    #[test]
    fn cone_apex_at_origin_and_solid() {
        let op = SdfOp::Cone {
            radius: 1.0,
            height: 2.0,
        };
        assert!(
            dist(&op, [0.0, 1.0, 0.0]) < 0.0,
            "interior should be inside"
        );
        assert!(
            dist(&op, [0.0, -0.5, 0.0]) > 0.0,
            "below the apex should be outside"
        );
    }

    #[test]
    fn rotate_z_90_sends_plus_x_to_plus_y() {
        let feature = Arc::new(SdfOp::Translate {
            inner: Arc::new(SdfOp::Sphere { radius: 0.2 }),
            offset: [1.0, 0.0, 0.0],
        });
        let op = SdfOp::RotateZ {
            inner: feature,
            angle: std::f32::consts::FRAC_PI_2,
        };
        // +90deg about Z (CCW, right-hand rule) takes the +X feature to +Y.
        assert!(dist(&op, [0.0, 1.0, 0.0]) < 0.0, "feature should be at +Y");
        assert!(
            dist(&op, [0.0, -1.0, 0.0]) > 0.0,
            "feature should not be at -Y"
        );
    }

    #[test]
    fn repeat_tiles_negative_coordinates() {
        let op = SdfOp::RepeatInfinite {
            inner: Arc::new(SdfOp::Sphere { radius: 0.3 }),
            spacing: [2.0, 0.0, 0.0],
        };
        let at_neg = dist(&op, [-2.0, 0.0, 0.0]);
        let at_zero = dist(&op, [0.0, 0.0, 0.0]);
        assert!(
            (at_neg - at_zero).abs() < 1e-4,
            "negative-coord cell ({at_neg}) must equal the origin cell ({at_zero})"
        );
    }

    #[test]
    fn ellipsoid_center_is_finite_and_inside() {
        let op = SdfOp::Ellipsoid {
            radii: [1.0, 0.5, 0.8],
        };
        let v = dist(&op, [0.0, 0.0, 0.0]);
        assert!(
            v.is_finite() && v < 0.0,
            "center must be finite & inside, got {v}"
        );
    }

    /// Every primitive must evaluate to a finite, bounded value across a grid of
    /// points. Finiteness catches NaN/Inf regressions; the magnitude bound also
    /// catches a primitive falling through to the `_ => f32::MAX` arm (that
    /// sentinel is ~3.4e38). Each primitive must also report at least one inside
    /// point so it isn't silently empty.
    #[test]
    fn all_primitives_evaluate_finitely() {
        let prims = [
            SdfOp::Sphere { radius: 0.6 },
            SdfOp::Box {
                half_extents: [0.5, 0.4, 0.3],
            },
            SdfOp::RoundedBox {
                half_extents: [0.5, 0.4, 0.3],
                radius: 0.1,
            },
            SdfOp::Cylinder {
                radius: 0.5,
                half_height: 0.6,
            },
            SdfOp::Capsule {
                radius: 0.3,
                half_height: 0.5,
            },
            SdfOp::Torus {
                major_radius: 0.6,
                minor_radius: 0.2,
            },
            SdfOp::Cone {
                radius: 0.5,
                height: 1.0,
            },
            SdfOp::Plane {
                normal: [0.0, 1.0, 0.0],
                offset: 0.0,
            },
            SdfOp::Ellipsoid {
                radii: [0.6, 0.4, 0.5],
            },
            SdfOp::Octahedron { size: 0.6 },
            SdfOp::HexPrism {
                half_height: 0.4,
                radius: 0.5,
            },
            SdfOp::TriPrism { size: [0.6, 0.4] },
            SdfOp::Pyramid { height: 0.8 },
            SdfOp::Link {
                length: 0.3,
                major_radius: 0.4,
                minor_radius: 0.15,
            },
        ];
        // A grid dense enough that every shape above contains at least one point.
        let mut samples = Vec::new();
        let mut g = -0.9_f32;
        while g <= 0.9 {
            samples.push([g, 0.1, 0.0]);
            samples.push([0.0, g, 0.0]);
            samples.push([0.35, g, 0.0]);
            samples.push([g, 0.0, g]);
            g += 0.15;
        }
        for op in prims {
            let mut any_inside = false;
            for p in &samples {
                let v = dist(&op, *p);
                assert!(
                    v.is_finite() && v.abs() < 1e6,
                    "{op:?} at {p:?} produced {v} (NaN/Inf or f32::MAX fallthrough?)"
                );
                if v < 0.0 {
                    any_inside = true;
                }
            }
            assert!(any_inside, "{op:?} never reported an inside point");
        }
    }
}
