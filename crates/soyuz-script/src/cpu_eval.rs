//! CPU-side SDF evaluation for mesh generation
//!
//! This module implements the `Sdf` trait from soyuz-core for `SdfOperation`,
//! enabling CPU evaluation of script-generated SDFs for mesh export via marching cubes.

use crate::sdf_api::SdfOperation;
use soyuz_core::sdf::{Aabb, Sdf};

// Re-export from soyuz-core prelude
use soyuz_core::prelude::{Vec2, Vec3};

/// Wrapper around SdfOperation that implements the Sdf trait
pub struct CpuSdf {
    pub op: SdfOperation,
}

impl CpuSdf {
    pub fn new(op: SdfOperation) -> Self {
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

// Also implement for SdfOperation directly for convenience
impl Sdf for SdfOperation {
    fn distance(&self, p: Vec3) -> f32 {
        eval_distance(self, p)
    }

    fn bounds(&self) -> Aabb {
        eval_bounds(self)
    }
}

/// Evaluate SDF distance at point p
fn eval_distance(op: &SdfOperation, p: Vec3) -> f32 {
    match op {
        // === Primitives ===
        SdfOperation::Sphere { radius } => p.length() - *radius as f32,

        SdfOperation::Box3 { half_extents } => {
            let h = Vec3::new(
                half_extents[0] as f32,
                half_extents[1] as f32,
                half_extents[2] as f32,
            );
            let q = p.abs() - h;
            q.max(Vec3::ZERO).length() + q.x.max(q.y.max(q.z)).min(0.0)
        }

        SdfOperation::RoundedBox {
            half_extents,
            radius,
        } => {
            let h = Vec3::new(
                half_extents[0] as f32,
                half_extents[1] as f32,
                half_extents[2] as f32,
            );
            let r = *radius as f32;
            let q = p.abs() - h + Vec3::splat(r);
            q.max(Vec3::ZERO).length() + q.x.max(q.y.max(q.z)).min(0.0) - r
        }

        SdfOperation::Cylinder {
            radius,
            half_height,
        } => {
            let r = *radius as f32;
            let h = *half_height as f32;
            let d = Vec2::new(Vec2::new(p.x, p.z).length(), p.y).abs() - Vec2::new(r, h);
            d.x.max(d.y).min(0.0) + d.max(Vec2::ZERO).length()
        }

        SdfOperation::Capsule {
            radius,
            half_height,
        } => {
            let r = *radius as f32;
            let h = *half_height as f32;
            let p_clamped = Vec3::new(p.x, p.y.clamp(-h, h), p.z);
            (p - p_clamped).length() - r
        }

        SdfOperation::Torus {
            major_radius,
            minor_radius,
        } => {
            let r1 = *major_radius as f32;
            let r2 = *minor_radius as f32;
            let q = Vec2::new(Vec2::new(p.x, p.z).length() - r1, p.y);
            q.length() - r2
        }

        SdfOperation::Cone { radius, height } => {
            let r = *radius as f32;
            let h = *height as f32;
            let q = Vec2::new(h, -r).normalize();
            let w = Vec2::new(Vec2::new(p.x, p.z).length(), p.y);
            let a = w - q * w.dot(q).clamp(0.0, h / q.x);
            let b = w - q * Vec2::new(h / q.x, 0.0).min(w);
            let k = q.y.signum();
            let d = a.length_squared().min(b.length_squared());
            let s = (k * (w.x * q.y - w.y * q.x)).max(k * (w.y - h));
            d.sqrt() * s.signum()
        }

        SdfOperation::Plane { normal, offset } => {
            let n = Vec3::new(normal[0] as f32, normal[1] as f32, normal[2] as f32).normalize();
            p.dot(n) + *offset as f32
        }

        SdfOperation::Ellipsoid { radii } => {
            let r = Vec3::new(radii[0] as f32, radii[1] as f32, radii[2] as f32);
            let k0 = (p / r).length();
            let k1 = (p / (r * r)).length();
            k0 * (k0 - 1.0) / k1
        }

        SdfOperation::Octahedron { size } => {
            let s = *size as f32;
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

        SdfOperation::HexPrism {
            half_height,
            radius,
        } => {
            let h = *half_height as f32;
            let r = *radius as f32;
            const K: Vec3 = Vec3::new(-0.866025404, 0.5, 0.577350269);
            let p_abs = p.abs();
            let xy = Vec2::new(p_abs.x, p_abs.z);
            let xy = xy - 2.0 * K.x.min(xy.dot(Vec2::new(K.x, K.y))) * Vec2::new(K.x, K.y);
            let d = Vec2::new(
                (xy - Vec2::new(xy.x.clamp(-K.z * r, K.z * r), r)).length() * (xy.y - r).signum(),
                p_abs.y - h,
            );
            d.x.max(d.y).min(0.0) + d.max(Vec2::ZERO).length()
        }

        SdfOperation::TriPrism { size } => {
            let s = Vec2::new(size[0] as f32, size[1] as f32);
            let q = p.abs();
            (q.z - s.y).max((q.x * 0.866025 + p.y * 0.5).max(-p.y) - s.x * 0.5)
        }

        // === Boolean Operations ===
        SdfOperation::Union { a, b } => eval_distance(a, p).min(eval_distance(b, p)),

        SdfOperation::Subtract { a, b } => eval_distance(a, p).max(-eval_distance(b, p)),

        SdfOperation::Intersect { a, b } => eval_distance(a, p).max(eval_distance(b, p)),

        SdfOperation::SmoothUnion { a, b, k } => {
            let k = *k as f32;
            let d1 = eval_distance(a, p);
            let d2 = eval_distance(b, p);
            let h = (0.5 + 0.5 * (d2 - d1) / k).clamp(0.0, 1.0);
            lerp(d2, d1, h) - k * h * (1.0 - h)
        }

        SdfOperation::SmoothSubtract { a, b, k } => {
            let k = *k as f32;
            let d1 = eval_distance(a, p);
            let d2 = eval_distance(b, p);
            let h = (0.5 - 0.5 * (d2 + d1) / k).clamp(0.0, 1.0);
            lerp(d1, -d2, h) + k * h * (1.0 - h)
        }

        SdfOperation::SmoothIntersect { a, b, k } => {
            let k = *k as f32;
            let d1 = eval_distance(a, p);
            let d2 = eval_distance(b, p);
            let h = (0.5 - 0.5 * (d2 - d1) / k).clamp(0.0, 1.0);
            lerp(d2, d1, h) + k * h * (1.0 - h)
        }

        // === Modifiers ===
        SdfOperation::Shell { inner, thickness } => {
            eval_distance(inner, p).abs() - *thickness as f32
        }

        SdfOperation::Round { inner, radius } => eval_distance(inner, p) - *radius as f32,

        SdfOperation::Onion { inner, thickness } => {
            let t = *thickness as f32;
            (eval_distance(inner, p).abs() % (t * 2.0)) - t
        }

        SdfOperation::Elongate { inner, h } => {
            let h = Vec3::new(h[0] as f32, h[1] as f32, h[2] as f32);
            let q = p.abs() - h;
            eval_distance(inner, q.max(Vec3::ZERO)) + q.x.max(q.y.max(q.z)).min(0.0)
        }

        // === Transforms ===
        SdfOperation::Translate { inner, offset } => {
            let o = Vec3::new(offset[0] as f32, offset[1] as f32, offset[2] as f32);
            eval_distance(inner, p - o)
        }

        SdfOperation::RotateX { inner, angle } => {
            let a = *angle as f32;
            let c = a.cos();
            let s = a.sin();
            let q = Vec3::new(p.x, c * p.y + s * p.z, -s * p.y + c * p.z);
            eval_distance(inner, q)
        }

        SdfOperation::RotateY { inner, angle } => {
            let a = *angle as f32;
            let c = a.cos();
            let s = a.sin();
            let q = Vec3::new(c * p.x - s * p.z, p.y, s * p.x + c * p.z);
            eval_distance(inner, q)
        }

        SdfOperation::RotateZ { inner, angle } => {
            let a = *angle as f32;
            let c = a.cos();
            let s = a.sin();
            let q = Vec3::new(c * p.x + s * p.y, -s * p.x + c * p.y, p.z);
            eval_distance(inner, q)
        }

        SdfOperation::Scale { inner, factor } => {
            let f = *factor as f32;
            eval_distance(inner, p / f) * f
        }

        SdfOperation::MirrorX { inner } => {
            let q = Vec3::new(p.x.abs(), p.y, p.z);
            eval_distance(inner, q)
        }

        SdfOperation::MirrorY { inner } => {
            let q = Vec3::new(p.x, p.y.abs(), p.z);
            eval_distance(inner, q)
        }

        SdfOperation::MirrorZ { inner } => {
            let q = Vec3::new(p.x, p.y, p.z.abs());
            eval_distance(inner, q)
        }

        SdfOperation::SymmetryX { inner } => {
            let q = Vec3::new(p.x.abs(), p.y, p.z);
            eval_distance(inner, q)
        }

        SdfOperation::SymmetryY { inner } => {
            let q = Vec3::new(p.x, p.y.abs(), p.z);
            eval_distance(inner, q)
        }

        SdfOperation::SymmetryZ { inner } => {
            let q = Vec3::new(p.x, p.y, p.z.abs());
            eval_distance(inner, q)
        }

        // === Deformations ===
        SdfOperation::Twist { inner, amount } => {
            let k = *amount as f32;
            let c = (k * p.y).cos();
            let s = (k * p.y).sin();
            let q = Vec3::new(c * p.x - s * p.z, p.y, s * p.x + c * p.z);
            eval_distance(inner, q)
        }

        SdfOperation::Bend { inner, amount } => {
            let k = *amount as f32;
            let c = (k * p.x).cos();
            let s = (k * p.x).sin();
            let q = Vec3::new(c * p.x - s * p.y, s * p.x + c * p.y, p.z);
            eval_distance(inner, q)
        }

        // === Repetition ===
        SdfOperation::RepeatInfinite { inner, spacing } => {
            let s = Vec3::new(spacing[0] as f32, spacing[1] as f32, spacing[2] as f32);
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

        SdfOperation::RepeatLimited {
            inner,
            spacing,
            count,
        } => {
            let s = Vec3::new(spacing[0] as f32, spacing[1] as f32, spacing[2] as f32);
            let c = Vec3::new(count[0] as f32, count[1] as f32, count[2] as f32);
            let q = p - s * (p / s).round().clamp(-c, c);
            eval_distance(inner, q)
        }

        SdfOperation::RepeatPolar { inner, count } => {
            // Use SSOT formula from soyuz-math (generated from formulas/repeat_polar.toml)
            let q = soyuz_math::repeat_polar(p, *count as f32);
            eval_distance(inner, q)
        }
    }
}

/// Evaluate bounding box for an SDF operation
fn eval_bounds(op: &SdfOperation) -> Aabb {
    match op {
        SdfOperation::Sphere { radius } => {
            let r = *radius as f32;
            Aabb::cube(r)
        }

        SdfOperation::Box3 { half_extents } => {
            let h = Vec3::new(
                half_extents[0] as f32,
                half_extents[1] as f32,
                half_extents[2] as f32,
            );
            Aabb::new(-h, h)
        }

        SdfOperation::RoundedBox { half_extents, .. } => {
            let h = Vec3::new(
                half_extents[0] as f32,
                half_extents[1] as f32,
                half_extents[2] as f32,
            );
            Aabb::new(-h, h)
        }

        SdfOperation::Cylinder {
            radius,
            half_height,
        } => {
            let r = *radius as f32;
            let h = *half_height as f32;
            Aabb::new(Vec3::new(-r, -h, -r), Vec3::new(r, h, r))
        }

        SdfOperation::Capsule {
            radius,
            half_height,
        } => {
            let r = *radius as f32;
            let h = *half_height as f32 + r;
            Aabb::new(Vec3::new(-r, -h, -r), Vec3::new(r, h, r))
        }

        SdfOperation::Torus {
            major_radius,
            minor_radius,
        } => {
            let r1 = *major_radius as f32;
            let r2 = *minor_radius as f32;
            let r = r1 + r2;
            Aabb::new(Vec3::new(-r, -r2, -r), Vec3::new(r, r2, r))
        }

        SdfOperation::Cone { radius, height } => {
            let r = *radius as f32;
            let h = *height as f32;
            Aabb::new(Vec3::new(-r, 0.0, -r), Vec3::new(r, h, r))
        }

        SdfOperation::Plane { .. } => Aabb::cube(100.0),

        SdfOperation::Ellipsoid { radii } => {
            let r = Vec3::new(radii[0] as f32, radii[1] as f32, radii[2] as f32);
            Aabb::new(-r, r)
        }

        SdfOperation::Octahedron { size } => {
            let s = *size as f32;
            Aabb::cube(s)
        }

        SdfOperation::HexPrism {
            half_height,
            radius,
        } => {
            let h = *half_height as f32;
            let r = *radius as f32;
            Aabb::new(Vec3::new(-r, -h, -r), Vec3::new(r, h, r))
        }

        SdfOperation::TriPrism { size } => {
            let s = size[0].max(size[1]) as f32;
            Aabb::cube(s)
        }

        // Boolean operations
        SdfOperation::Union { a, b } => eval_bounds(a).union(&eval_bounds(b)),

        SdfOperation::Subtract { a, .. } => eval_bounds(a),

        SdfOperation::Intersect { a, .. } => eval_bounds(a),

        SdfOperation::SmoothUnion { a, b, k } => {
            eval_bounds(a).union(&eval_bounds(b)).expand(*k as f32)
        }

        SdfOperation::SmoothSubtract { a, .. } => eval_bounds(a),

        SdfOperation::SmoothIntersect { a, .. } => eval_bounds(a),

        // Modifiers
        SdfOperation::Shell { inner, thickness } => eval_bounds(inner).expand(*thickness as f32),

        SdfOperation::Round { inner, radius } => eval_bounds(inner).expand(*radius as f32),

        SdfOperation::Onion { inner, thickness } => eval_bounds(inner).expand(*thickness as f32),

        SdfOperation::Elongate { inner, h } => {
            let bounds = eval_bounds(inner);
            let h = Vec3::new(h[0] as f32, h[1] as f32, h[2] as f32);
            Aabb::new(bounds.min - h, bounds.max + h)
        }

        // Transforms
        SdfOperation::Translate { inner, offset } => {
            let bounds = eval_bounds(inner);
            let o = Vec3::new(offset[0] as f32, offset[1] as f32, offset[2] as f32);
            Aabb::new(bounds.min + o, bounds.max + o)
        }

        SdfOperation::RotateX { inner, .. }
        | SdfOperation::RotateY { inner, .. }
        | SdfOperation::RotateZ { inner, .. } => {
            // Conservative: expand to enclosing sphere
            let bounds = eval_bounds(inner);
            let r = bounds.size().length() * 0.5;
            let center = bounds.center();
            Aabb::from_center(center, Vec3::splat(r))
        }

        SdfOperation::Scale { inner, factor } => {
            let bounds = eval_bounds(inner);
            let f = *factor as f32;
            Aabb::new(bounds.min * f, bounds.max * f)
        }

        SdfOperation::MirrorX { inner }
        | SdfOperation::MirrorY { inner }
        | SdfOperation::MirrorZ { inner }
        | SdfOperation::SymmetryX { inner }
        | SdfOperation::SymmetryY { inner }
        | SdfOperation::SymmetryZ { inner } => {
            let bounds = eval_bounds(inner);
            // Mirror expands to cover both sides
            let max_extent = bounds.min.abs().max(bounds.max.abs());
            Aabb::new(-max_extent, max_extent)
        }

        // Deformations
        SdfOperation::Twist { inner, .. } | SdfOperation::Bend { inner, .. } => {
            // Conservative bounds for deformations
            eval_bounds(inner).expand(0.5)
        }

        // Repetition
        SdfOperation::RepeatInfinite { .. } => {
            // Can't have finite bounds for infinite repetition
            // Return large bounds, mesh generation will need to limit sampling area
            Aabb::cube(10.0)
        }

        SdfOperation::RepeatLimited {
            inner,
            spacing,
            count,
        } => {
            let bounds = eval_bounds(inner);
            let s = Vec3::new(spacing[0] as f32, spacing[1] as f32, spacing[2] as f32);
            let c = Vec3::new(count[0] as f32, count[1] as f32, count[2] as f32);
            let total = s * c * 2.0;
            Aabb::new(bounds.min - total * 0.5, bounds.max + total * 0.5)
        }

        SdfOperation::RepeatPolar { inner, .. } => {
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
    }
}

#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
