//! SDF Operations - Boolean, modifiers, repetition

use super::{Aabb, Sdf};
use glam::{UVec3, Vec3};

// ============================================================================
// Boolean Operations
// ============================================================================

/// Union of two SDFs (combine shapes)
pub struct Union<A: Sdf, B: Sdf> {
    pub a: A,
    pub b: B,
}

impl<A: Sdf, B: Sdf> Union<A, B> {
    pub fn new(a: A, b: B) -> Self {
        Self { a, b }
    }
}

impl<A: Sdf + Send + Sync, B: Sdf + Send + Sync> Sdf for Union<A, B> {
    fn distance(&self, p: Vec3) -> f32 {
        self.a.distance(p).min(self.b.distance(p))
    }

    fn bounds(&self) -> Aabb {
        self.a.bounds().union(&self.b.bounds())
    }
}

/// Subtraction of two SDFs (cut B from A)
pub struct Subtract<A: Sdf, B: Sdf> {
    pub a: A,
    pub b: B,
}

impl<A: Sdf, B: Sdf> Subtract<A, B> {
    pub fn new(a: A, b: B) -> Self {
        Self { a, b }
    }
}

impl<A: Sdf + Send + Sync, B: Sdf + Send + Sync> Sdf for Subtract<A, B> {
    fn distance(&self, p: Vec3) -> f32 {
        self.a.distance(p).max(-self.b.distance(p))
    }

    fn bounds(&self) -> Aabb {
        self.a.bounds() // Subtraction can only reduce, not expand
    }
}

/// Intersection of two SDFs (keep only overlap)
pub struct Intersect<A: Sdf, B: Sdf> {
    pub a: A,
    pub b: B,
}

impl<A: Sdf, B: Sdf> Intersect<A, B> {
    pub fn new(a: A, b: B) -> Self {
        Self { a, b }
    }
}

impl<A: Sdf + Send + Sync, B: Sdf + Send + Sync> Sdf for Intersect<A, B> {
    fn distance(&self, p: Vec3) -> f32 {
        self.a.distance(p).max(self.b.distance(p))
    }

    fn bounds(&self) -> Aabb {
        // Intersection is smaller than either input
        // This is a simplification - could be tighter
        self.a.bounds()
    }
}

// ============================================================================
// Smooth Boolean Operations
// ============================================================================

/// Smooth union with polynomial blending
pub struct SmoothUnion<A: Sdf, B: Sdf> {
    pub a: A,
    pub b: B,
    pub k: f32,
}

impl<A: Sdf, B: Sdf> SmoothUnion<A, B> {
    pub fn new(a: A, b: B, k: f32) -> Self {
        Self { a, b, k }
    }
}

impl<A: Sdf + Send + Sync, B: Sdf + Send + Sync> Sdf for SmoothUnion<A, B> {
    fn distance(&self, p: Vec3) -> f32 {
        let d1 = self.a.distance(p);
        let d2 = self.b.distance(p);
        let h = (0.5 + 0.5 * (d2 - d1) / self.k).clamp(0.0, 1.0);
        lerp(d2, d1, h) - self.k * h * (1.0 - h)
    }

    fn bounds(&self) -> Aabb {
        self.a.bounds().union(&self.b.bounds()).expand(self.k)
    }
}

/// Smooth subtraction
pub struct SmoothSubtract<A: Sdf, B: Sdf> {
    pub a: A,
    pub b: B,
    pub k: f32,
}

impl<A: Sdf, B: Sdf> SmoothSubtract<A, B> {
    pub fn new(a: A, b: B, k: f32) -> Self {
        Self { a, b, k }
    }
}

impl<A: Sdf + Send + Sync, B: Sdf + Send + Sync> Sdf for SmoothSubtract<A, B> {
    fn distance(&self, p: Vec3) -> f32 {
        let d1 = self.a.distance(p);
        let d2 = self.b.distance(p);
        let h = (0.5 - 0.5 * (d2 + d1) / self.k).clamp(0.0, 1.0);
        lerp(d1, -d2, h) + self.k * h * (1.0 - h)
    }

    fn bounds(&self) -> Aabb {
        self.a.bounds()
    }
}

/// Smooth intersection
pub struct SmoothIntersect<A: Sdf, B: Sdf> {
    pub a: A,
    pub b: B,
    pub k: f32,
}

impl<A: Sdf, B: Sdf> SmoothIntersect<A, B> {
    pub fn new(a: A, b: B, k: f32) -> Self {
        Self { a, b, k }
    }
}

impl<A: Sdf + Send + Sync, B: Sdf + Send + Sync> Sdf for SmoothIntersect<A, B> {
    fn distance(&self, p: Vec3) -> f32 {
        let d1 = self.a.distance(p);
        let d2 = self.b.distance(p);
        let h = (0.5 - 0.5 * (d2 - d1) / self.k).clamp(0.0, 1.0);
        lerp(d2, d1, h) + self.k * h * (1.0 - h)
    }

    fn bounds(&self) -> Aabb {
        self.a.bounds()
    }
}

// ============================================================================
// Modifier Operations
// ============================================================================

/// Shell (hollow) operation
pub struct Shell<S: Sdf> {
    pub inner: S,
    pub thickness: f32,
}

impl<S: Sdf> Shell<S> {
    pub fn new(inner: S, thickness: f32) -> Self {
        Self { inner, thickness }
    }
}

impl<S: Sdf + Send + Sync> Sdf for Shell<S> {
    fn distance(&self, p: Vec3) -> f32 {
        self.inner.distance(p).abs() - self.thickness
    }

    fn bounds(&self) -> Aabb {
        self.inner.bounds().expand(self.thickness)
    }
}

/// Round (offset) operation - rounds edges
pub struct Round<S: Sdf> {
    pub inner: S,
    pub radius: f32,
}

impl<S: Sdf> Round<S> {
    pub fn new(inner: S, radius: f32) -> Self {
        Self { inner, radius }
    }
}

impl<S: Sdf + Send + Sync> Sdf for Round<S> {
    fn distance(&self, p: Vec3) -> f32 {
        self.inner.distance(p) - self.radius
    }

    fn bounds(&self) -> Aabb {
        self.inner.bounds().expand(self.radius)
    }
}

/// Onion (multiple shells) operation
pub struct Onion<S: Sdf> {
    pub inner: S,
    pub thickness: f32,
}

impl<S: Sdf> Onion<S> {
    pub fn new(inner: S, thickness: f32) -> Self {
        Self { inner, thickness }
    }
}

impl<S: Sdf + Send + Sync> Sdf for Onion<S> {
    fn distance(&self, p: Vec3) -> f32 {
        // Creates concentric shells
        (self.inner.distance(p).abs() % (self.thickness * 2.0)) - self.thickness
    }

    fn bounds(&self) -> Aabb {
        self.inner.bounds().expand(self.thickness)
    }
}

/// Elongate operation - stretches the center of a shape
pub struct Elongate<S: Sdf> {
    pub inner: S,
    pub h: Vec3,
}

impl<S: Sdf> Elongate<S> {
    pub fn new(inner: S, h: Vec3) -> Self {
        Self { inner, h }
    }
}

impl<S: Sdf + Send + Sync> Sdf for Elongate<S> {
    fn distance(&self, p: Vec3) -> f32 {
        let q = p.abs() - self.h;
        self.inner.distance(q.max(Vec3::ZERO)) + q.x.max(q.y.max(q.z)).min(0.0)
    }

    fn bounds(&self) -> Aabb {
        let bounds = self.inner.bounds();
        Aabb::new(bounds.min - self.h, bounds.max + self.h)
    }
}

// ============================================================================
// Repetition Operations
// ============================================================================

/// Infinite repetition
pub struct RepeatInfinite<S: Sdf> {
    pub inner: S,
    pub spacing: Vec3,
}

impl<S: Sdf> RepeatInfinite<S> {
    pub fn new(inner: S, spacing: Vec3) -> Self {
        Self { inner, spacing }
    }
}

impl<S: Sdf + Send + Sync> Sdf for RepeatInfinite<S> {
    fn distance(&self, p: Vec3) -> f32 {
        let q = (p + self.spacing * 0.5).rem_euclid(self.spacing) - self.spacing * 0.5;
        self.inner.distance(q)
    }

    fn bounds(&self) -> Aabb {
        // Infinite repetition - return large bounds
        Aabb::cube(1000.0)
    }
}

/// Limited (finite) repetition
pub struct RepeatLimited<S: Sdf> {
    pub inner: S,
    pub spacing: Vec3,
    pub count: UVec3,
}

impl<S: Sdf> RepeatLimited<S> {
    pub fn new(inner: S, spacing: Vec3, count: UVec3) -> Self {
        Self {
            inner,
            spacing,
            count,
        }
    }
}

impl<S: Sdf + Send + Sync> Sdf for RepeatLimited<S> {
    fn distance(&self, p: Vec3) -> f32 {
        let count_f = Vec3::new(
            self.count.x as f32,
            self.count.y as f32,
            self.count.z as f32,
        );
        let id = (p / self.spacing + count_f * 0.5).floor();
        let o = (p - self.spacing * id).signum();
        let mut d = f32::MAX;

        for z in 0..=1 {
            for y in 0..=1 {
                for x in 0..=1 {
                    let rid = id + Vec3::new(x as f32, y as f32, z as f32) * o;
                    let rid = rid.clamp(Vec3::ZERO, count_f - Vec3::ONE);
                    let r = p - self.spacing * rid;
                    d = d.min(self.inner.distance(r));
                }
            }
        }

        d
    }

    fn bounds(&self) -> Aabb {
        let inner_bounds = self.inner.bounds();
        let count_f = Vec3::new(
            self.count.x as f32,
            self.count.y as f32,
            self.count.z as f32,
        );
        let total_size = self.spacing * (count_f - Vec3::ONE);
        Aabb::new(inner_bounds.min, inner_bounds.max + total_size)
    }
}

/// Polar (radial) repetition around Y axis
pub struct RepeatPolar<S: Sdf> {
    pub inner: S,
    pub count: u32,
}

impl<S: Sdf> RepeatPolar<S> {
    pub fn new(inner: S, count: u32) -> Self {
        Self { inner, count }
    }
}

impl<S: Sdf + Send + Sync> Sdf for RepeatPolar<S> {
    fn distance(&self, p: Vec3) -> f32 {
        // Use SSOT formula from soyuz-math (generated from formulas/repeat_polar.toml)
        let q = soyuz_math::repeat_polar(p, self.count as f32);
        self.inner.distance(q)
    }

    fn bounds(&self) -> Aabb {
        // Polar repetition expands to a cylinder encompassing all copies
        let inner_bounds = self.inner.bounds();
        let r = inner_bounds
            .max
            .x
            .abs()
            .max(inner_bounds.max.z.abs())
            .max(inner_bounds.min.x.abs())
            .max(inner_bounds.min.z.abs());
        Aabb::new(
            Vec3::new(-r, inner_bounds.min.y, -r),
            Vec3::new(r, inner_bounds.max.y, r),
        )
    }
}

// ============================================================================
// Helpers
// ============================================================================

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
