//! SDF Transforms - Translation, rotation, scale, deformations

use super::{Aabb, Sdf};
use glam::{Quat, Vec3};

// ============================================================================
// Basic Transforms
// ============================================================================

/// Translation transform
pub struct Translate<S: Sdf> {
    pub inner: S,
    pub offset: Vec3,
}

impl<S: Sdf> Translate<S> {
    pub fn new(inner: S, offset: Vec3) -> Self {
        Self { inner, offset }
    }
}

impl<S: Sdf + Send + Sync> Sdf for Translate<S> {
    fn distance(&self, p: Vec3) -> f32 {
        self.inner.distance(p - self.offset)
    }

    fn bounds(&self) -> Aabb {
        let b = self.inner.bounds();
        Aabb::new(b.min + self.offset, b.max + self.offset)
    }
}

/// Rotation transform
pub struct Rotate<S: Sdf> {
    pub inner: S,
    pub rotation: Quat,
    pub inverse: Quat,
}

impl<S: Sdf> Rotate<S> {
    pub fn new(inner: S, rotation: Quat) -> Self {
        Self {
            inner,
            rotation,
            inverse: rotation.inverse(),
        }
    }
}

impl<S: Sdf + Send + Sync> Sdf for Rotate<S> {
    fn distance(&self, p: Vec3) -> f32 {
        // Rotate point into local space
        self.inner.distance(self.inverse * p)
    }

    fn bounds(&self) -> Aabb {
        // For rotated bounds, we need to compute the AABB of rotated corners
        let b = self.inner.bounds();
        let corners = [
            Vec3::new(b.min.x, b.min.y, b.min.z),
            Vec3::new(b.max.x, b.min.y, b.min.z),
            Vec3::new(b.min.x, b.max.y, b.min.z),
            Vec3::new(b.max.x, b.max.y, b.min.z),
            Vec3::new(b.min.x, b.min.y, b.max.z),
            Vec3::new(b.max.x, b.min.y, b.max.z),
            Vec3::new(b.min.x, b.max.y, b.max.z),
            Vec3::new(b.max.x, b.max.y, b.max.z),
        ];

        let mut new_min = Vec3::splat(f32::MAX);
        let mut new_max = Vec3::splat(f32::MIN);

        for corner in corners {
            let rotated = self.rotation * corner;
            new_min = new_min.min(rotated);
            new_max = new_max.max(rotated);
        }

        Aabb::new(new_min, new_max)
    }
}

/// Uniform scale transform
pub struct Scale<S: Sdf> {
    pub inner: S,
    pub factor: f32,
}

impl<S: Sdf> Scale<S> {
    pub fn new(inner: S, factor: f32) -> Self {
        Self { inner, factor }
    }
}

impl<S: Sdf + Send + Sync> Sdf for Scale<S> {
    fn distance(&self, p: Vec3) -> f32 {
        self.inner.distance(p / self.factor) * self.factor
    }

    fn bounds(&self) -> Aabb {
        let b = self.inner.bounds();
        Aabb::new(b.min * self.factor, b.max * self.factor)
    }
}

/// Mirror across a plane through origin
pub struct Mirror<S: Sdf> {
    pub inner: S,
    pub axis: Vec3,
}

impl<S: Sdf> Mirror<S> {
    pub fn new(inner: S, axis: Vec3) -> Self {
        Self {
            inner,
            axis: axis.normalize(),
        }
    }
}

impl<S: Sdf + Send + Sync> Sdf for Mirror<S> {
    fn distance(&self, p: Vec3) -> f32 {
        // Reflect point if it's on the negative side of the plane
        let d = p.dot(self.axis);
        let p_mirrored = if d < 0.0 { p - 2.0 * d * self.axis } else { p };
        self.inner.distance(p_mirrored)
    }

    fn bounds(&self) -> Aabb {
        let b = self.inner.bounds();
        // Mirror expands bounds to include both sides
        let mirrored_min = reflect_point(b.min, self.axis);
        let mirrored_max = reflect_point(b.max, self.axis);
        Aabb::new(
            b.min.min(mirrored_min).min(b.max).min(mirrored_max),
            b.max.max(mirrored_max).max(b.min).max(mirrored_min),
        )
    }
}

fn reflect_point(p: Vec3, axis: Vec3) -> Vec3 {
    p - 2.0 * p.dot(axis) * axis
}

// ============================================================================
// Deformation Transforms
// ============================================================================

/// Twist deformation around Y axis
pub struct Twist<S: Sdf> {
    pub inner: S,
    pub amount: f32, // radians per unit along Y
}

impl<S: Sdf> Twist<S> {
    pub fn new(inner: S, amount: f32) -> Self {
        Self { inner, amount }
    }
}

impl<S: Sdf + Send + Sync> Sdf for Twist<S> {
    fn distance(&self, p: Vec3) -> f32 {
        let angle = self.amount * p.y;
        let c = angle.cos();
        let s = angle.sin();
        let q = Vec3::new(c * p.x - s * p.z, p.y, s * p.x + c * p.z);
        self.inner.distance(q)
    }

    fn bounds(&self) -> Aabb {
        // Twisted bounds are complex - use a conservative estimate
        let b = self.inner.bounds();
        let r = (b.max.x.abs().max(b.min.x.abs())).max(b.max.z.abs().max(b.min.z.abs()));
        Aabb::new(Vec3::new(-r, b.min.y, -r), Vec3::new(r, b.max.y, r))
    }
}

/// Bend deformation along Y axis
pub struct Bend<S: Sdf> {
    pub inner: S,
    pub amount: f32, // curvature
}

impl<S: Sdf> Bend<S> {
    pub fn new(inner: S, amount: f32) -> Self {
        Self { inner, amount }
    }
}

impl<S: Sdf + Send + Sync> Sdf for Bend<S> {
    fn distance(&self, p: Vec3) -> f32 {
        let k = self.amount;
        if k.abs() < 0.0001 {
            return self.inner.distance(p);
        }

        let c = k.recip() - p.y;
        let angle = p.x * k;
        let q = Vec3::new(c * angle.sin(), c * angle.cos() - k.recip(), p.z);
        self.inner.distance(q)
    }

    fn bounds(&self) -> Aabb {
        // Bent bounds are complex - use a conservative estimate
        let b = self.inner.bounds();
        let max_extent = (b.max - b.min).max_element();
        Aabb::cube(max_extent)
    }
}

/// Taper deformation (scale varies along Y)
pub struct Taper<S: Sdf> {
    pub inner: S,
    pub amount: f32, // scale change per unit Y
}

impl<S: Sdf> Taper<S> {
    pub fn new(inner: S, amount: f32) -> Self {
        Self { inner, amount }
    }
}

impl<S: Sdf + Send + Sync> Sdf for Taper<S> {
    fn distance(&self, p: Vec3) -> f32 {
        let scale = 1.0 + self.amount * p.y;
        if scale <= 0.0 {
            return f32::MAX;
        }
        let q = Vec3::new(p.x / scale, p.y, p.z / scale);
        self.inner.distance(q) * scale
    }

    fn bounds(&self) -> Aabb {
        let b = self.inner.bounds();
        // Estimate bounds based on taper
        let scale_at_min = 1.0 + self.amount * b.min.y;
        let scale_at_max = 1.0 + self.amount * b.max.y;
        let max_scale = scale_at_min.max(scale_at_max).max(1.0);
        Aabb::new(
            Vec3::new(b.min.x * max_scale, b.min.y, b.min.z * max_scale),
            Vec3::new(b.max.x * max_scale, b.max.y, b.max.z * max_scale),
        )
    }
}

/// Displacement by noise (requires a noise function)
pub struct Displace<S: Sdf, N: Fn(Vec3) -> f32 + Send + Sync> {
    pub inner: S,
    pub noise: N,
    pub amount: f32,
}

impl<S: Sdf, N: Fn(Vec3) -> f32 + Send + Sync> Displace<S, N> {
    pub fn new(inner: S, noise: N, amount: f32) -> Self {
        Self {
            inner,
            noise,
            amount,
        }
    }
}

impl<S: Sdf + Send + Sync, N: Fn(Vec3) -> f32 + Send + Sync> Sdf for Displace<S, N> {
    fn distance(&self, p: Vec3) -> f32 {
        self.inner.distance(p) + (self.noise)(p) * self.amount
    }

    fn bounds(&self) -> Aabb {
        self.inner.bounds().expand(self.amount.abs())
    }
}

// ============================================================================
// Symmetry Operations
// ============================================================================

/// Symmetry across multiple axes (combines mirrors)
pub struct Symmetry<S: Sdf> {
    pub inner: S,
    pub x: bool,
    pub y: bool,
    pub z: bool,
}

impl<S: Sdf> Symmetry<S> {
    pub fn new(inner: S, x: bool, y: bool, z: bool) -> Self {
        Self { inner, x, y, z }
    }
}

impl<S: Sdf + Send + Sync> Sdf for Symmetry<S> {
    fn distance(&self, p: Vec3) -> f32 {
        let p = Vec3::new(
            if self.x { p.x.abs() } else { p.x },
            if self.y { p.y.abs() } else { p.y },
            if self.z { p.z.abs() } else { p.z },
        );
        self.inner.distance(p)
    }

    fn bounds(&self) -> Aabb {
        let b = self.inner.bounds();
        Aabb::new(
            Vec3::new(
                if self.x {
                    -b.max.x.abs().max(b.min.x.abs())
                } else {
                    b.min.x
                },
                if self.y {
                    -b.max.y.abs().max(b.min.y.abs())
                } else {
                    b.min.y
                },
                if self.z {
                    -b.max.z.abs().max(b.min.z.abs())
                } else {
                    b.min.z
                },
            ),
            Vec3::new(
                if self.x {
                    b.max.x.abs().max(b.min.x.abs())
                } else {
                    b.max.x
                },
                if self.y {
                    b.max.y.abs().max(b.min.y.abs())
                } else {
                    b.max.y
                },
                if self.z {
                    b.max.z.abs().max(b.min.z.abs())
                } else {
                    b.max.z
                },
            ),
        )
    }
}
