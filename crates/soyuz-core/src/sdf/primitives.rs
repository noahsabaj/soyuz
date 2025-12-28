//! SDF Primitive shapes
//!
//! All primitives are centered at the origin. Use transforms to position them.

use super::{Aabb, Sdf};
use glam::Vec3;

// ============================================================================
// Constructor functions (ergonomic API)
// ============================================================================

/// Create a sphere with given radius
pub fn sphere(radius: f32) -> Sphere {
    Sphere::new(radius)
}

/// Create a box with given half-extents (size/2 in each direction)
pub fn box3(half_extents: Vec3) -> Box3 {
    Box3::new(half_extents)
}

/// Create a cube with given size
pub fn cube(size: f32) -> Box3 {
    Box3::new(Vec3::splat(size * 0.5))
}

/// Create a rounded box
pub fn rounded_box(half_extents: Vec3, radius: f32) -> RoundedBox {
    RoundedBox::new(half_extents, radius)
}

/// Create a cylinder (Y-axis aligned) with given radius and height
pub fn cylinder(radius: f32, height: f32) -> Cylinder {
    Cylinder::new(radius, height)
}

/// Create a capsule (Y-axis aligned) with given radius and height
pub fn capsule(radius: f32, height: f32) -> Capsule {
    Capsule::new(radius, height)
}

/// Create a torus (donut) lying in the XZ plane
pub fn torus(major_radius: f32, minor_radius: f32) -> Torus {
    Torus::new(major_radius, minor_radius)
}

/// Create a cone (tip at origin, opens upward)
pub fn cone(radius: f32, height: f32) -> Cone {
    Cone::new(radius, height)
}

/// Create a plane with given normal and offset from origin
pub fn plane(normal: Vec3, offset: f32) -> Plane {
    Plane::new(normal, offset)
}

/// Create an infinite ground plane (Y = 0)
pub fn ground_plane() -> Plane {
    Plane::new(Vec3::Y, 0.0)
}

/// Create an ellipsoid with given radii
pub fn ellipsoid(radii: Vec3) -> Ellipsoid {
    Ellipsoid::new(radii)
}

/// Create an octahedron with given size
pub fn octahedron(size: f32) -> Octahedron {
    Octahedron::new(size)
}

/// Create a triangular prism (Y-axis aligned)
pub fn tri_prism(size: Vec2) -> TriPrism {
    TriPrism::new(size)
}

/// Create a hexagonal prism (Y-axis aligned)
pub fn hex_prism(height: f32, radius: f32) -> HexPrism {
    HexPrism::new(height, radius)
}

/// Create a pyramid (square base, tip pointing up)
pub fn pyramid(base: f32, height: f32) -> Pyramid {
    Pyramid::new(base, height)
}

// ============================================================================
// Primitive Structs
// ============================================================================

use glam::Vec2;

/// Sphere centered at origin
#[derive(Debug, Clone, Copy)]
pub struct Sphere {
    pub radius: f32,
}

impl Sphere {
    pub fn new(radius: f32) -> Self {
        Self { radius }
    }
}

impl Sdf for Sphere {
    fn distance(&self, p: Vec3) -> f32 {
        p.length() - self.radius
    }

    fn bounds(&self) -> Aabb {
        Aabb::cube(self.radius)
    }
}

/// Axis-aligned box (rectangular prism)
#[derive(Debug, Clone, Copy)]
pub struct Box3 {
    pub half_extents: Vec3,
}

impl Box3 {
    pub fn new(half_extents: Vec3) -> Self {
        Self { half_extents }
    }
}

impl Sdf for Box3 {
    fn distance(&self, p: Vec3) -> f32 {
        let q = p.abs() - self.half_extents;
        q.max(Vec3::ZERO).length() + q.x.max(q.y.max(q.z)).min(0.0)
    }

    fn bounds(&self) -> Aabb {
        Aabb::new(-self.half_extents, self.half_extents)
    }
}

/// Box with rounded edges
#[derive(Debug, Clone, Copy)]
pub struct RoundedBox {
    pub half_extents: Vec3,
    pub radius: f32,
}

impl RoundedBox {
    pub fn new(half_extents: Vec3, radius: f32) -> Self {
        Self {
            half_extents,
            radius,
        }
    }
}

impl Sdf for RoundedBox {
    fn distance(&self, p: Vec3) -> f32 {
        let q = p.abs() - self.half_extents + Vec3::splat(self.radius);
        q.max(Vec3::ZERO).length() + q.x.max(q.y.max(q.z)).min(0.0) - self.radius
    }

    fn bounds(&self) -> Aabb {
        Aabb::new(-self.half_extents, self.half_extents)
    }
}

/// Cylinder aligned with Y axis
#[derive(Debug, Clone, Copy)]
pub struct Cylinder {
    pub radius: f32,
    pub half_height: f32,
}

impl Cylinder {
    pub fn new(radius: f32, height: f32) -> Self {
        Self {
            radius,
            half_height: height * 0.5,
        }
    }
}

impl Sdf for Cylinder {
    fn distance(&self, p: Vec3) -> f32 {
        let d = Vec2::new(Vec2::new(p.x, p.z).length(), p.y).abs()
            - Vec2::new(self.radius, self.half_height);
        d.x.max(d.y).min(0.0) + d.max(Vec2::ZERO).length()
    }

    fn bounds(&self) -> Aabb {
        Aabb::new(
            Vec3::new(-self.radius, -self.half_height, -self.radius),
            Vec3::new(self.radius, self.half_height, self.radius),
        )
    }
}

/// Capsule (cylinder with hemispherical caps) aligned with Y axis
#[derive(Debug, Clone, Copy)]
pub struct Capsule {
    pub radius: f32,
    pub half_height: f32,
}

impl Capsule {
    pub fn new(radius: f32, height: f32) -> Self {
        Self {
            radius,
            half_height: height * 0.5,
        }
    }
}

impl Sdf for Capsule {
    fn distance(&self, p: Vec3) -> f32 {
        let p_clamped = Vec3::new(p.x, p.y.clamp(-self.half_height, self.half_height), p.z);
        (p - p_clamped).length() - self.radius
    }

    fn bounds(&self) -> Aabb {
        let h = self.half_height + self.radius;
        Aabb::new(
            Vec3::new(-self.radius, -h, -self.radius),
            Vec3::new(self.radius, h, self.radius),
        )
    }
}

/// Torus (donut) lying in the XZ plane
#[derive(Debug, Clone, Copy)]
pub struct Torus {
    pub major_radius: f32,
    pub minor_radius: f32,
}

impl Torus {
    pub fn new(major_radius: f32, minor_radius: f32) -> Self {
        Self {
            major_radius,
            minor_radius,
        }
    }
}

impl Sdf for Torus {
    fn distance(&self, p: Vec3) -> f32 {
        let q = Vec2::new(Vec2::new(p.x, p.z).length() - self.major_radius, p.y);
        q.length() - self.minor_radius
    }

    fn bounds(&self) -> Aabb {
        let r = self.major_radius + self.minor_radius;
        Aabb::new(
            Vec3::new(-r, -self.minor_radius, -r),
            Vec3::new(r, self.minor_radius, r),
        )
    }
}

/// Cone with tip at origin, opening upward along Y axis
#[derive(Debug, Clone, Copy)]
pub struct Cone {
    pub radius: f32,
    pub height: f32,
}

impl Cone {
    pub fn new(radius: f32, height: f32) -> Self {
        Self { radius, height }
    }
}

impl Sdf for Cone {
    fn distance(&self, p: Vec3) -> f32 {
        // Cone formula using 2D projection
        let q = Vec2::new(self.height, -self.radius).normalize();
        let w = Vec2::new(Vec2::new(p.x, p.z).length(), p.y);
        let a = w - q * w.dot(q).clamp(0.0, self.height / q.x);
        let b = w - q * Vec2::new(self.height / q.x, 0.0).min(w);
        let k = q.y.signum();
        let d = a.length_squared().min(b.length_squared());
        let s = (k * (w.x * q.y - w.y * q.x)).max(k * (w.y - self.height));
        d.sqrt() * s.signum()
    }

    fn bounds(&self) -> Aabb {
        Aabb::new(
            Vec3::new(-self.radius, 0.0, -self.radius),
            Vec3::new(self.radius, self.height, self.radius),
        )
    }
}

/// Infinite plane
#[derive(Debug, Clone, Copy)]
pub struct Plane {
    pub normal: Vec3,
    pub offset: f32,
}

impl Plane {
    pub fn new(normal: Vec3, offset: f32) -> Self {
        Self {
            normal: normal.normalize(),
            offset,
        }
    }
}

impl Sdf for Plane {
    fn distance(&self, p: Vec3) -> f32 {
        p.dot(self.normal) + self.offset
    }

    fn bounds(&self) -> Aabb {
        // Infinite plane - return large bounds
        Aabb::cube(1000.0)
    }
}

/// Ellipsoid (stretched sphere)
#[derive(Debug, Clone, Copy)]
pub struct Ellipsoid {
    pub radii: Vec3,
}

impl Ellipsoid {
    pub fn new(radii: Vec3) -> Self {
        Self { radii }
    }
}

impl Sdf for Ellipsoid {
    fn distance(&self, p: Vec3) -> f32 {
        // Approximate SDF for ellipsoid
        let k0 = (p / self.radii).length();
        let k1 = (p / (self.radii * self.radii)).length();
        k0 * (k0 - 1.0) / k1
    }

    fn bounds(&self) -> Aabb {
        Aabb::new(-self.radii, self.radii)
    }
}

/// Octahedron (8-faced polyhedron)
#[derive(Debug, Clone, Copy)]
pub struct Octahedron {
    pub size: f32,
}

impl Octahedron {
    pub fn new(size: f32) -> Self {
        Self { size }
    }
}

impl Sdf for Octahedron {
    fn distance(&self, p: Vec3) -> f32 {
        let p = p.abs();
        let m = p.x + p.y + p.z - self.size;

        let q = if 3.0 * p.x < m {
            p
        } else if 3.0 * p.y < m {
            Vec3::new(p.y, p.z, p.x)
        } else if 3.0 * p.z < m {
            Vec3::new(p.z, p.x, p.y)
        } else {
            return m * 0.57735027; // 1/sqrt(3)
        };

        let k = (0.5 * (q.z - q.y + self.size)).clamp(0.0, self.size);
        Vec3::new(q.x, q.y - self.size + k, q.z - k).length()
    }

    fn bounds(&self) -> Aabb {
        Aabb::cube(self.size)
    }
}

/// Triangular prism (Y-axis aligned)
#[derive(Debug, Clone, Copy)]
pub struct TriPrism {
    pub size: Vec2, // x = base, y = height (along Y axis)
}

impl TriPrism {
    pub fn new(size: Vec2) -> Self {
        Self { size }
    }
}

impl Sdf for TriPrism {
    fn distance(&self, p: Vec3) -> f32 {
        let q = p.abs();
        (q.z - self.size.y).max((q.x * 0.866025 + p.y * 0.5).max(-p.y) - self.size.x * 0.5)
    }

    fn bounds(&self) -> Aabb {
        Aabb::new(
            Vec3::new(-self.size.x, -self.size.x, -self.size.y),
            Vec3::new(self.size.x, self.size.x, self.size.y),
        )
    }
}

/// Hexagonal prism (Y-axis aligned)
#[derive(Debug, Clone, Copy)]
pub struct HexPrism {
    pub half_height: f32,
    pub radius: f32,
}

impl HexPrism {
    pub fn new(height: f32, radius: f32) -> Self {
        Self {
            half_height: height * 0.5,
            radius,
        }
    }
}

impl Sdf for HexPrism {
    fn distance(&self, p: Vec3) -> f32 {
        const K: Vec3 = Vec3::new(-0.866025404, 0.5, 0.577350269);
        let p_abs = p.abs();
        let xy = Vec2::new(p_abs.x, p_abs.z);
        let xy = xy - 2.0 * K.x.min(xy.dot(Vec2::new(K.x, K.y))) * Vec2::new(K.x, K.y);
        let d = Vec2::new(
            (xy - Vec2::new(
                xy.x.clamp(-K.z * self.radius, K.z * self.radius),
                self.radius,
            ))
            .length()
                * (xy.y - self.radius).signum(),
            p_abs.y - self.half_height,
        );
        d.x.max(d.y).min(0.0) + d.max(Vec2::ZERO).length()
    }

    fn bounds(&self) -> Aabb {
        Aabb::new(
            Vec3::new(-self.radius, -self.half_height, -self.radius),
            Vec3::new(self.radius, self.half_height, self.radius),
        )
    }
}

/// Pyramid with square base
#[derive(Debug, Clone, Copy)]
pub struct Pyramid {
    pub half_base: f32,
    pub height: f32,
}

impl Pyramid {
    pub fn new(base: f32, height: f32) -> Self {
        Self {
            half_base: base * 0.5,
            height,
        }
    }
}

impl Sdf for Pyramid {
    fn distance(&self, p: Vec3) -> f32 {
        let h = self.height;
        let b = self.half_base;

        // Symmetry
        let p = Vec3::new(p.x.abs(), p.y, p.z.abs());

        // Calculate distance
        let m2 = h * h + b * b;
        let qx = p.z.min(p.x) - b;
        let qy = p.y;
        let qz = p.z.max(p.x) - b;

        let s = (qx + qy * h / b).max(0.0);
        let t = (qy - qx * b / h).clamp(0.0, h);

        if s < 0.001 {
            let d1 = qy.max(0.0);
            let d2 = Vec2::new(qz, qy).max(Vec2::ZERO).length();
            return d1.min(d2);
        }

        let d = Vec2::new(qz + s * b / h - b, qy - t);
        d.length() * (d.x.max(d.y)).signum() / m2.sqrt()
    }

    fn bounds(&self) -> Aabb {
        Aabb::new(
            Vec3::new(-self.half_base, 0.0, -self.half_base),
            Vec3::new(self.half_base, self.height, self.half_base),
        )
    }
}

// ============================================================================
// Fractal SDFs
// ============================================================================

/// Mandelbulb fractal
#[derive(Debug, Clone, Copy)]
pub struct Mandelbulb {
    pub power: f32,
    pub iterations: u32,
}

impl Mandelbulb {
    pub fn new(power: f32, iterations: u32) -> Self {
        Self { power, iterations }
    }
}

/// Create a Mandelbulb fractal
pub fn mandelbulb(power: f32, iterations: u32) -> Mandelbulb {
    Mandelbulb::new(power, iterations)
}

impl Sdf for Mandelbulb {
    fn distance(&self, p: Vec3) -> f32 {
        let mut z = p;
        let mut dr = 1.0f32;
        let mut r = 0.0f32;

        for _ in 0..self.iterations {
            r = z.length();
            if r > 2.0 {
                break;
            }

            // Convert to spherical coordinates
            let theta = (z.z / r).acos();
            let phi = z.y.atan2(z.x);
            dr = r.powf(self.power - 1.0) * self.power * dr + 1.0;

            // Scale and rotate
            let zr = r.powf(self.power);
            let theta = theta * self.power;
            let phi = phi * self.power;

            // Convert back to cartesian
            z = zr
                * Vec3::new(
                    theta.sin() * phi.cos(),
                    theta.sin() * phi.sin(),
                    theta.cos(),
                );
            z += p;
        }

        0.5 * r.ln() * r / dr
    }

    fn bounds(&self) -> Aabb {
        Aabb::cube(2.0)
    }
}

/// Menger sponge fractal
#[derive(Debug, Clone, Copy)]
pub struct MengerSponge {
    pub iterations: u32,
    pub size: f32,
}

impl MengerSponge {
    pub fn new(iterations: u32, size: f32) -> Self {
        Self { iterations, size }
    }
}

/// Create a Menger sponge fractal
pub fn menger(iterations: u32) -> MengerSponge {
    MengerSponge::new(iterations, 1.0)
}

impl Sdf for MengerSponge {
    fn distance(&self, p: Vec3) -> f32 {
        let mut d = Box3::new(Vec3::splat(self.size)).distance(p);
        let mut s = 1.0f32;

        for _ in 0..self.iterations {
            let a = (p * s).rem_euclid(Vec3::splat(2.0)) - Vec3::ONE;
            s *= 3.0;
            let r = Vec3::ONE - a.abs() * 3.0;

            let c = cross_sdf(r) / s;
            d = d.max(c);
        }

        d
    }

    fn bounds(&self) -> Aabb {
        Aabb::cube(self.size)
    }
}

// Helper for Menger sponge
fn cross_sdf(p: Vec3) -> f32 {
    let inf_cross_x = p.y.abs().max(p.z.abs());
    let inf_cross_y = p.x.abs().max(p.z.abs());
    let inf_cross_z = p.x.abs().max(p.y.abs());
    inf_cross_x.min(inf_cross_y).min(inf_cross_z)
}
