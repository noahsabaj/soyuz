//! Texture operations - math, filters, combinators

use super::Texture;
use glam::Vec2;

/// Scale texture coordinates
pub struct Scale<T: Texture> {
    inner: T,
    factor: f32,
}

impl<T: Texture> Scale<T> {
    pub fn new(inner: T, factor: f32) -> Self {
        Self { inner, factor }
    }
}

impl<T: Texture> Texture for Scale<T> {
    fn sample(&self, uv: Vec2) -> f32 {
        self.inner.sample(uv * self.factor)
    }
}

/// Invert texture values
pub struct Invert<T: Texture> {
    inner: T,
}

impl<T: Texture> Invert<T> {
    pub fn new(inner: T) -> Self {
        Self { inner }
    }
}

impl<T: Texture> Texture for Invert<T> {
    fn sample(&self, uv: Vec2) -> f32 {
        1.0 - self.inner.sample(uv)
    }
}

/// Remap values from one range to another
pub struct Remap<T: Texture> {
    inner: T,
    in_min: f32,
    in_max: f32,
    out_min: f32,
    out_max: f32,
}

impl<T: Texture> Remap<T> {
    pub fn new(inner: T, in_min: f32, in_max: f32, out_min: f32, out_max: f32) -> Self {
        Self {
            inner,
            in_min,
            in_max,
            out_min,
            out_max,
        }
    }
}

impl<T: Texture> Texture for Remap<T> {
    fn sample(&self, uv: Vec2) -> f32 {
        let v = self.inner.sample(uv);
        let t = (v - self.in_min) / (self.in_max - self.in_min);
        self.out_min + t * (self.out_max - self.out_min)
    }
}

/// Clamp values
pub struct Clamp<T: Texture> {
    inner: T,
    min: f32,
    max: f32,
}

impl<T: Texture> Clamp<T> {
    pub fn new(inner: T, min: f32, max: f32) -> Self {
        Self { inner, min, max }
    }
}

impl<T: Texture> Texture for Clamp<T> {
    fn sample(&self, uv: Vec2) -> f32 {
        self.inner.sample(uv).clamp(self.min, self.max)
    }
}

/// Power operation
pub struct Pow<T: Texture> {
    inner: T,
    exp: f32,
}

impl<T: Texture> Pow<T> {
    pub fn new(inner: T, exp: f32) -> Self {
        Self { inner, exp }
    }
}

impl<T: Texture> Texture for Pow<T> {
    fn sample(&self, uv: Vec2) -> f32 {
        self.inner.sample(uv).powf(self.exp)
    }
}

/// Add two textures
pub struct Add<A: Texture, B: Texture> {
    a: A,
    b: B,
}

impl<A: Texture, B: Texture> Add<A, B> {
    pub fn new(a: A, b: B) -> Self {
        Self { a, b }
    }
}

impl<A: Texture, B: Texture> Texture for Add<A, B> {
    fn sample(&self, uv: Vec2) -> f32 {
        self.a.sample(uv) + self.b.sample(uv)
    }
}

/// Multiply two textures
pub struct Multiply<A: Texture, B: Texture> {
    a: A,
    b: B,
}

impl<A: Texture, B: Texture> Multiply<A, B> {
    pub fn new(a: A, b: B) -> Self {
        Self { a, b }
    }
}

impl<A: Texture, B: Texture> Texture for Multiply<A, B> {
    fn sample(&self, uv: Vec2) -> f32 {
        self.a.sample(uv) * self.b.sample(uv)
    }
}

/// Mix two textures
pub struct Mix<A: Texture, B: Texture> {
    a: A,
    b: B,
    factor: f32,
}

impl<A: Texture, B: Texture> Mix<A, B> {
    pub fn new(a: A, b: B, factor: f32) -> Self {
        Self { a, b, factor }
    }
}

impl<A: Texture, B: Texture> Texture for Mix<A, B> {
    fn sample(&self, uv: Vec2) -> f32 {
        let va = self.a.sample(uv);
        let vb = self.b.sample(uv);
        va * (1.0 - self.factor) + vb * self.factor
    }
}

/// Threshold operation
pub struct Threshold<T: Texture> {
    inner: T,
    value: f32,
}

impl<T: Texture> Threshold<T> {
    pub fn new(inner: T, value: f32) -> Self {
        Self { inner, value }
    }
}

impl<T: Texture> Texture for Threshold<T> {
    fn sample(&self, uv: Vec2) -> f32 {
        if self.inner.sample(uv) > self.value {
            1.0
        } else {
            0.0
        }
    }
}

/// Convert heightmap to normal map
pub struct ToNormal<T: Texture> {
    inner: T,
    strength: f32,
}

impl<T: Texture> ToNormal<T> {
    pub fn new(inner: T, strength: f32) -> Self {
        Self { inner, strength }
    }
}

impl<T: Texture> Texture for ToNormal<T> {
    fn sample(&self, _uv: Vec2) -> f32 {
        // Normal maps need RGB output, this returns grayscale
        // Real implementation would use sample_color
        0.5
    }

    fn sample_color(&self, uv: Vec2) -> [f32; 4] {
        let eps = 0.001;

        // Sample neighbors
        let h_l = self.inner.sample(uv - Vec2::new(eps, 0.0));
        let h_r = self.inner.sample(uv + Vec2::new(eps, 0.0));
        let h_d = self.inner.sample(uv - Vec2::new(0.0, eps));
        let h_u = self.inner.sample(uv + Vec2::new(0.0, eps));

        // Calculate normal
        let dx = (h_r - h_l) * self.strength;
        let dy = (h_u - h_d) * self.strength;

        // Normal in tangent space
        let normal = glam::Vec3::new(-dx, -dy, 1.0).normalize();

        // Convert to 0..1 range for storage
        [
            normal.x * 0.5 + 0.5,
            normal.y * 0.5 + 0.5,
            normal.z * 0.5 + 0.5,
            1.0,
        ]
    }
}

/// Warp/distort texture by another texture
pub struct Warp<T: Texture, W: Texture> {
    inner: T,
    warper: W,
    amount: f32,
}

impl<T: Texture, W: Texture> Warp<T, W> {
    pub fn new(inner: T, warper: W, amount: f32) -> Self {
        Self {
            inner,
            warper,
            amount,
        }
    }
}

impl<T: Texture, W: Texture> Texture for Warp<T, W> {
    fn sample(&self, uv: Vec2) -> f32 {
        let warp_x = self.warper.sample(uv);
        let warp_y = self.warper.sample(uv + Vec2::new(0.5, 0.5));
        let offset = Vec2::new(warp_x - 0.5, warp_y - 0.5) * self.amount;
        self.inner.sample(uv + offset)
    }
}

/// Rotate texture
pub struct Rotate<T: Texture> {
    inner: T,
    angle: f32,
}

impl<T: Texture> Rotate<T> {
    pub fn new(inner: T, angle: f32) -> Self {
        Self { inner, angle }
    }
}

impl<T: Texture> Texture for Rotate<T> {
    fn sample(&self, uv: Vec2) -> f32 {
        let center = Vec2::splat(0.5);
        let uv = uv - center;
        let c = self.angle.cos();
        let s = self.angle.sin();
        let rotated = Vec2::new(uv.x * c - uv.y * s, uv.x * s + uv.y * c);
        self.inner.sample(rotated + center)
    }
}

/// Translate texture
pub struct Translate<T: Texture> {
    inner: T,
    offset: Vec2,
}

impl<T: Texture> Translate<T> {
    pub fn new(inner: T, offset: Vec2) -> Self {
        Self { inner, offset }
    }
}

impl<T: Texture> Texture for Translate<T> {
    fn sample(&self, uv: Vec2) -> f32 {
        self.inner.sample(uv - self.offset)
    }
}

/// Tile (repeat) texture
pub struct Tile<T: Texture> {
    inner: T,
    count: Vec2,
}

impl<T: Texture> Tile<T> {
    pub fn new(inner: T, count: Vec2) -> Self {
        Self { inner, count }
    }
}

impl<T: Texture> Texture for Tile<T> {
    fn sample(&self, uv: Vec2) -> f32 {
        let tiled = (uv * self.count).fract();
        self.inner.sample(tiled)
    }
}

/// Smoothstep operation
pub struct Smoothstep<T: Texture> {
    inner: T,
    edge0: f32,
    edge1: f32,
}

impl<T: Texture> Smoothstep<T> {
    pub fn new(inner: T, edge0: f32, edge1: f32) -> Self {
        Self {
            inner,
            edge0,
            edge1,
        }
    }
}

impl<T: Texture> Texture for Smoothstep<T> {
    fn sample(&self, uv: Vec2) -> f32 {
        let v = self.inner.sample(uv);
        let t = ((v - self.edge0) / (self.edge1 - self.edge0)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    }
}

/// Absolute value
pub struct Abs<T: Texture> {
    inner: T,
}

impl<T: Texture> Abs<T> {
    pub fn new(inner: T) -> Self {
        Self { inner }
    }
}

impl<T: Texture> Texture for Abs<T> {
    fn sample(&self, uv: Vec2) -> f32 {
        self.inner.sample(uv).abs()
    }
}
