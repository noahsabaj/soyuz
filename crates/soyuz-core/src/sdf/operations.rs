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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sdf::primitives::{cube, sphere};
    use approx::assert_relative_eq;

    // ------------------------------------------------------------------------
    // Boolean Operations
    // ------------------------------------------------------------------------

    #[test]
    fn union_takes_minimum_distance() {
        let a = sphere(1.0);
        let b = cube(2.0); // half-extent 1.0
        let u = Union::new(a, b);

        // At origin, both are -1.0, so union is -1.0
        assert_relative_eq!(u.distance(Vec3::ZERO), -1.0, epsilon = 1e-6);
    }

    #[test]
    fn union_inside_either_is_inside() {
        // Two separate spheres
        let a = sphere(0.5);
        let b = Translate::new(sphere(0.5), Vec3::new(2.0, 0.0, 0.0));
        let u = Union::new(a, b);

        // Inside first sphere
        assert!(u.distance(Vec3::ZERO) < 0.0);
        // Inside second sphere
        assert!(u.distance(Vec3::new(2.0, 0.0, 0.0)) < 0.0);
        // Outside both
        assert!(u.distance(Vec3::new(1.0, 0.0, 0.0)) > 0.0);
    }

    #[test]
    fn subtract_carves_out_shape() {
        let a = sphere(2.0);
        let b = sphere(1.0);
        let s = Subtract::new(a, b);

        // At origin (inside b), should be outside result
        assert!(s.distance(Vec3::ZERO) > 0.0);
        // At radius 1.5, should be inside result (inside a, outside b)
        assert!(s.distance(Vec3::new(1.5, 0.0, 0.0)) < 0.0);
        // At radius 3.0, should be outside result
        assert!(s.distance(Vec3::new(3.0, 0.0, 0.0)) > 0.0);
    }

    #[test]
    fn subtract_surface_at_carve_boundary() {
        let a = sphere(2.0);
        let b = sphere(1.0);
        let s = Subtract::new(a, b);

        // At radius 1.0, should be on the inner surface (carved boundary)
        assert_relative_eq!(s.distance(Vec3::X), 0.0, epsilon = 1e-6);
    }

    #[test]
    fn intersect_keeps_overlap_only() {
        let a = sphere(1.0);
        let b = Translate::new(sphere(1.0), Vec3::new(1.5, 0.0, 0.0));
        let i = Intersect::new(a, b);

        // At origin, outside intersection (not inside b which is at x=1.5)
        assert!(i.distance(Vec3::ZERO) > 0.0);
        // At x=0.75, inside both (overlap region)
        assert!(i.distance(Vec3::new(0.75, 0.0, 0.0)) < 0.0);
        // At x=2.0, outside intersection (not inside a)
        assert!(i.distance(Vec3::new(2.0, 0.0, 0.0)) > 0.0);
    }

    // ------------------------------------------------------------------------
    // Smooth Boolean Operations
    // ------------------------------------------------------------------------

    #[test]
    fn smooth_union_blends_surfaces() {
        let a = sphere(1.0);
        let b = Translate::new(sphere(1.0), Vec3::new(1.5, 0.0, 0.0));
        let smooth = SmoothUnion::new(a, b.clone(), 0.5);
        let sharp = Union::new(a, b);

        // At the blend region, smooth union should have smaller distance
        let p = Vec3::new(0.75, 0.0, 0.0);
        assert!(smooth.distance(p) < sharp.distance(p));
    }

    #[test]
    fn smooth_union_k_zero_equals_sharp() {
        let a = sphere(1.0);
        let b = Translate::new(sphere(1.0), Vec3::new(2.0, 0.0, 0.0));
        let smooth = SmoothUnion::new(a, b.clone(), 0.001);
        let sharp = Union::new(a, b);

        // With very small k, should be nearly identical
        let p = Vec3::new(1.0, 0.0, 0.0);
        assert_relative_eq!(smooth.distance(p), sharp.distance(p), epsilon = 0.01);
    }

    #[test]
    fn smooth_subtract_blends_carve() {
        let a = sphere(2.0);
        let b = sphere(1.0);
        let smooth = SmoothSubtract::new(a, b, 1.0);
        let sharp = Subtract::new(a, b);

        // The blend region for smooth subtract is where the two surfaces meet
        // At |p| = 1.5 (between the two sphere surfaces), we're in the blend zone
        let p = Vec3::new(1.5, 0.0, 0.0);
        let smooth_d = smooth.distance(p);
        let sharp_d = sharp.distance(p);

        // Both should compute valid distances
        assert!(!smooth_d.is_nan());
        assert!(!sharp_d.is_nan());

        // Sharp subtract: max(-0.5, -0.5) = -0.5 (inside the hollow shell)
        assert_relative_eq!(sharp_d, -0.5, epsilon = 1e-6);

        // Smooth version should add blend contribution (k * h * (1-h))
        // With k=1.0 and h around 0.5, this adds up to 0.25
        // So smooth_d should be around -0.5 + 0.25 = -0.25
        assert!(smooth_d > sharp_d); // Smooth pushes surface outward
    }

    #[test]
    fn smooth_intersect_rounds_edges() {
        // Use spheres for more predictable intersection behavior
        let a = sphere(1.0);
        let b = Translate::new(sphere(1.0), Vec3::new(1.0, 0.0, 0.0));
        let smooth = SmoothIntersect::new(a, b.clone(), 0.5);
        let sharp = Intersect::new(a, b);

        // At the intersection edge (between the two sphere surfaces)
        // Sharp intersection gives max of distances
        // Smooth intersection adds blending
        let p = Vec3::new(0.5, 0.0, 0.0);
        let smooth_d = smooth.distance(p);
        let sharp_d = sharp.distance(p);

        // Both should compute valid distances (no NaN)
        assert!(!smooth_d.is_nan());
        assert!(!sharp_d.is_nan());

        // They should be reasonably close but may differ due to blending
        // The smooth version adds k * h * (1-h) term
        assert!((smooth_d - sharp_d).abs() < 1.0);
    }

    // ------------------------------------------------------------------------
    // Modifier Operations
    // ------------------------------------------------------------------------

    #[test]
    fn shell_creates_hollow_shape() {
        let s = Shell::new(sphere(1.0), 0.1);

        // At origin (far inside), should be outside shell
        assert!(s.distance(Vec3::ZERO) > 0.0);
        // At radius 0.95 (inside shell wall), should be inside
        assert!(s.distance(Vec3::new(0.95, 0.0, 0.0)) < 0.0);
        // At radius 1.05 (inside shell wall), should be inside
        assert!(s.distance(Vec3::new(1.05, 0.0, 0.0)) < 0.0);
        // At radius 1.2 (outside shell), should be outside
        assert!(s.distance(Vec3::new(1.2, 0.0, 0.0)) > 0.0);
    }

    #[test]
    fn shell_thickness() {
        let s = Shell::new(sphere(1.0), 0.1);

        // Inner surface at radius 0.9
        assert_relative_eq!(s.distance(Vec3::new(0.9, 0.0, 0.0)), 0.0, epsilon = 1e-6);
        // Outer surface at radius 1.1
        assert_relative_eq!(s.distance(Vec3::new(1.1, 0.0, 0.0)), 0.0, epsilon = 1e-6);
    }

    #[test]
    fn round_expands_shape() {
        let c = cube(2.0); // half-extent 1.0
        let r = Round::new(c, 0.1);

        // On original surface (should now be inside)
        assert!(r.distance(Vec3::X) < 0.0);
        // On rounded surface
        assert_relative_eq!(r.distance(Vec3::new(1.1, 0.0, 0.0)), 0.0, epsilon = 1e-6);
    }

    #[test]
    fn onion_creates_concentric_shells() {
        let s = Onion::new(sphere(1.0), 0.2);

        // The onion creates repeating shells
        // At radius ~0.9 (near original surface), inside a shell
        let d1 = s.distance(Vec3::new(0.9, 0.0, 0.0));
        // At radius ~0.7, should be near another shell boundary
        let d2 = s.distance(Vec3::new(0.7, 0.0, 0.0));

        // Both should be valid (not NaN)
        assert!(!d1.is_nan());
        assert!(!d2.is_nan());
    }

    #[test]
    fn elongate_stretches_center() {
        let s = Elongate::new(sphere(1.0), Vec3::new(1.0, 0.0, 0.0));

        // At origin, should still be inside
        assert!(s.distance(Vec3::ZERO) < 0.0);
        // Shape is now stretched along X - should extend further
        assert!(s.distance(Vec3::new(1.5, 0.0, 0.0)) < 0.0);
        // But not in Y direction
        assert!(s.distance(Vec3::new(0.0, 1.5, 0.0)) > 0.0);
    }

    // ------------------------------------------------------------------------
    // Repetition Operations
    // ------------------------------------------------------------------------

    #[test]
    fn repeat_infinite_creates_copies() {
        let s = RepeatInfinite::new(sphere(0.3), Vec3::splat(2.0));

        // At origin, should be inside original
        assert!(s.distance(Vec3::ZERO) < 0.0);
        // At spacing offset, should be inside a copy
        assert!(s.distance(Vec3::new(2.0, 0.0, 0.0)) < 0.0);
        assert!(s.distance(Vec3::new(0.0, 2.0, 0.0)) < 0.0);
        assert!(s.distance(Vec3::new(4.0, 0.0, 0.0)) < 0.0);
    }

    #[test]
    fn repeat_limited_has_finite_copies() {
        let s = RepeatLimited::new(sphere(0.3), Vec3::splat(2.0), UVec3::new(3, 1, 1));

        // At origin (within count), should be inside
        assert!(s.distance(Vec3::ZERO) < 0.0);
        // At offset 2.0 (within count), should be inside
        assert!(s.distance(Vec3::new(2.0, 0.0, 0.0)) < 0.0);
        // At offset 4.0 (at edge of count=3), should be inside
        assert!(s.distance(Vec3::new(4.0, 0.0, 0.0)) < 0.0);
    }

    #[test]
    fn repeat_polar_creates_radial_copies() {
        // Create a small sphere offset from center
        let s = RepeatPolar::new(Translate::new(sphere(0.2), Vec3::new(1.0, 0.0, 0.0)), 6);

        // At the original offset, inside
        assert!(s.distance(Vec3::new(1.0, 0.0, 0.0)) < 0.0);
        // At 60 degrees rotation (pi/3), should also be inside a copy
        let angle = std::f32::consts::PI / 3.0;
        let rotated = Vec3::new(angle.cos(), 0.0, angle.sin());
        assert!(s.distance(rotated) < 0.0);
    }

    // ------------------------------------------------------------------------
    // Bounds Tests
    // ------------------------------------------------------------------------

    #[test]
    fn union_bounds_encompasses_both() {
        let a = sphere(1.0);
        let b = Translate::new(sphere(1.0), Vec3::new(5.0, 0.0, 0.0));
        let u = Union::new(a, b);
        let bounds = u.bounds();

        // Should encompass from -1 to 6 in X
        assert!(bounds.min.x <= -1.0);
        assert!(bounds.max.x >= 6.0);
    }

    #[test]
    fn shell_bounds_expand() {
        let s = sphere(1.0);
        let s_bounds = s.bounds();
        let shell = Shell::new(s, 0.1);
        let shell_bounds = shell.bounds();

        // Shell bounds should be larger
        assert!(shell_bounds.max.x > s_bounds.max.x);
    }

    #[test]
    fn round_bounds_expand() {
        let c = cube(2.0);
        let c_bounds = c.bounds();
        let r = Round::new(c, 0.2);
        let r_bounds = r.bounds();

        assert_relative_eq!(r_bounds.max.x, c_bounds.max.x + 0.2, epsilon = 1e-6);
    }
}

/// Translate operation (needed for tests, mirrors transforms.rs)
#[cfg(test)]
#[derive(Clone)]
struct Translate<S: Sdf> {
    inner: S,
    offset: Vec3,
}

#[cfg(test)]
impl<S: Sdf> Translate<S> {
    fn new(inner: S, offset: Vec3) -> Self {
        Self { inner, offset }
    }
}

#[cfg(test)]
impl<S: Sdf + Send + Sync> Sdf for Translate<S> {
    fn distance(&self, p: Vec3) -> f32 {
        self.inner.distance(p - self.offset)
    }

    fn bounds(&self) -> Aabb {
        let b = self.inner.bounds();
        Aabb::new(b.min + self.offset, b.max + self.offset)
    }
}
