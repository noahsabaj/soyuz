//! Pattern generators for procedural textures

use super::Texture;
use glam::{Vec2, Vec2Swizzles};
use std::f32::consts::PI;

/// Linear gradient
pub struct Gradient {
    pub direction: Vec2,
}

impl Gradient {
    pub fn new(direction: Vec2) -> Self {
        Self {
            direction: direction.normalize(),
        }
    }

    pub fn horizontal() -> Self {
        Self::new(Vec2::X)
    }

    pub fn vertical() -> Self {
        Self::new(Vec2::Y)
    }

    pub fn diagonal() -> Self {
        Self::new(Vec2::new(1.0, 1.0))
    }
}

impl Texture for Gradient {
    fn sample(&self, uv: Vec2) -> f32 {
        uv.dot(self.direction).clamp(0.0, 1.0)
    }
}

/// Create a linear gradient
pub fn gradient(direction: Vec2) -> Gradient {
    Gradient::new(direction)
}

/// Radial gradient from center
pub struct Radial {
    pub center: Vec2,
    pub radius: f32,
}

impl Radial {
    pub fn new(center: Vec2, radius: f32) -> Self {
        Self { center, radius }
    }

    pub fn centered() -> Self {
        Self::new(Vec2::splat(0.5), 0.5)
    }
}

impl Texture for Radial {
    fn sample(&self, uv: Vec2) -> f32 {
        let d = (uv - self.center).length();
        (1.0 - d / self.radius).clamp(0.0, 1.0)
    }
}

/// Create a radial gradient
pub fn radial(center: Vec2, radius: f32) -> Radial {
    Radial::new(center, radius)
}

/// Checkerboard pattern
pub struct Checker {
    pub scale: f32,
}

impl Checker {
    pub fn new(scale: f32) -> Self {
        Self { scale }
    }
}

impl Texture for Checker {
    fn sample(&self, uv: Vec2) -> f32 {
        let x = (uv.x * self.scale).floor() as i32;
        let y = (uv.y * self.scale).floor() as i32;
        if (x + y) % 2 == 0 { 1.0 } else { 0.0 }
    }
}

/// Create a checkerboard pattern
pub fn checker(scale: f32) -> Checker {
    Checker::new(scale)
}

/// Brick pattern
pub struct Bricks {
    pub size: Vec2,
    pub mortar: f32,
    pub offset: f32,
}

impl Bricks {
    pub fn new(size: Vec2, mortar: f32) -> Self {
        Self {
            size,
            mortar,
            offset: 0.5,
        }
    }
}

impl Texture for Bricks {
    fn sample(&self, uv: Vec2) -> f32 {
        let scaled = uv / self.size;
        let row = scaled.y.floor() as i32;
        let mut x = scaled.x;

        // Offset every other row
        if row % 2 != 0 {
            x += self.offset;
        }

        let brick_x = x.fract();
        let brick_y = scaled.y.fract();

        // Check if in mortar
        let mortar_x = self.mortar / self.size.x;
        let mortar_y = self.mortar / self.size.y;

        if brick_x < mortar_x || brick_y < mortar_y {
            0.0 // Mortar
        } else {
            1.0 // Brick
        }
    }
}

/// Create a brick pattern
pub fn bricks(size: Vec2, mortar: f32) -> Bricks {
    Bricks::new(size, mortar)
}

/// Hexagonal pattern
pub struct Hexagons {
    pub scale: f32,
}

impl Hexagons {
    pub fn new(scale: f32) -> Self {
        Self { scale }
    }
}

impl Texture for Hexagons {
    fn sample(&self, uv: Vec2) -> f32 {
        let uv = uv * self.scale;

        // Hexagonal coordinates
        let s = Vec2::new(1.0, 1.732051);
        let p = uv.abs();
        let p = p - s * (p / s).floor();

        // Distance to edge
        let d = (p - Vec2::new(0.5, 0.5)).length();
        d.clamp(0.0, 1.0)
    }
}

/// Create a hexagonal pattern
pub fn hexagons(scale: f32) -> Hexagons {
    Hexagons::new(scale)
}

/// Voronoi pattern (cell-based)
pub struct Voronoi {
    pub scale: f32,
    pub seed: u32,
}

impl Voronoi {
    pub fn new(scale: f32) -> Self {
        Self { scale, seed: 0 }
    }

    pub fn with_seed(scale: f32, seed: u32) -> Self {
        Self { scale, seed }
    }
}

impl Texture for Voronoi {
    fn sample(&self, uv: Vec2) -> f32 {
        let uv = uv * self.scale;
        let cell = uv.floor();
        let fract = uv.fract();

        let mut min_dist = f32::MAX;

        for y in -1..=1 {
            for x in -1..=1 {
                let neighbor = Vec2::new(x as f32, y as f32);
                let point = hash2(cell + neighbor, self.seed);
                let diff = neighbor + point - fract;
                let dist = diff.length();
                min_dist = min_dist.min(dist);
            }
        }

        min_dist.clamp(0.0, 1.0)
    }
}

// Simple hash function for Voronoi
// Note: These magic constants are arbitrary hash coefficients, not math constants
#[allow(clippy::approx_constant)]
fn hash2(p: Vec2, seed: u32) -> Vec2 {
    let k = Vec2::new(0.3183099, 0.3678794);
    let p = p + Vec2::splat(seed as f32 * 0.1);
    let p = p * k + k.yx();
    Vec2::new(
        (16.0 * (p.x * p.y * (p.x + p.y)).fract()).fract(),
        (16.0 * (p.y * p.x * (p.y - p.x + 1.0)).fract()).fract(),
    )
}

/// Create a Voronoi pattern
pub fn voronoi(scale: f32) -> Voronoi {
    Voronoi::new(scale)
}

/// Dot pattern
pub struct Dots {
    pub scale: f32,
    pub size: f32,
}

impl Dots {
    pub fn new(scale: f32, size: f32) -> Self {
        Self { scale, size }
    }
}

impl Texture for Dots {
    fn sample(&self, uv: Vec2) -> f32 {
        let uv = uv * self.scale;
        let cell = uv.fract() - Vec2::splat(0.5);
        let dist = cell.length();
        if dist < self.size { 1.0 } else { 0.0 }
    }
}

/// Create a dot pattern
pub fn dots(scale: f32, size: f32) -> Dots {
    Dots::new(scale, size)
}

/// Stripe pattern
pub struct Stripes {
    pub scale: f32,
    pub angle: f32,
}

impl Stripes {
    pub fn new(scale: f32, angle: f32) -> Self {
        Self { scale, angle }
    }
}

impl Texture for Stripes {
    fn sample(&self, uv: Vec2) -> f32 {
        let c = self.angle.cos();
        let s = self.angle.sin();
        let rotated = uv.x * c - uv.y * s;
        let t = (rotated * self.scale * 2.0 * PI).sin() * 0.5 + 0.5;
        t
    }
}

/// Create a stripe pattern
pub fn stripes(scale: f32, angle: f32) -> Stripes {
    Stripes::new(scale, angle)
}

/// Wave pattern
pub struct Waves {
    pub scale: f32,
    pub amplitude: f32,
}

impl Waves {
    pub fn new(scale: f32, amplitude: f32) -> Self {
        Self { scale, amplitude }
    }
}

impl Texture for Waves {
    fn sample(&self, uv: Vec2) -> f32 {
        let wave = (uv.x * self.scale * 2.0 * PI).sin() * self.amplitude;
        let y = uv.y + wave;
        (y * self.scale).fract()
    }
}

/// Create a wave pattern
pub fn waves(scale: f32, amplitude: f32) -> Waves {
    Waves::new(scale, amplitude)
}
