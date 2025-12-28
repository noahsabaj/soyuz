//! Procedural texture generation
//!
//! Generate 2D textures using noise functions, patterns, and compositing.

pub mod noise;
pub mod ops;
pub mod pattern;

use glam::Vec2;
use image::{ImageBuffer, Rgba};

/// Trait for texture generators
pub trait Texture: Send + Sync {
    /// Sample the texture at UV coordinates (0..1)
    fn sample(&self, uv: Vec2) -> f32;

    /// Sample with color output
    fn sample_color(&self, uv: Vec2) -> [f32; 4] {
        let v = self.sample(uv);
        [v, v, v, 1.0]
    }
}

/// Extension trait for texture operations
pub trait TextureExt: Texture + Sized + 'static {
    /// Scale the texture coordinates
    fn scale(self, factor: f32) -> ops::Scale<Self> {
        ops::Scale::new(self, factor)
    }

    /// Invert the texture values
    fn invert(self) -> ops::Invert<Self> {
        ops::Invert::new(self)
    }

    /// Remap values from one range to another
    fn remap(self, in_min: f32, in_max: f32, out_min: f32, out_max: f32) -> ops::Remap<Self> {
        ops::Remap::new(self, in_min, in_max, out_min, out_max)
    }

    /// Clamp values to a range
    fn clamp(self, min: f32, max: f32) -> ops::Clamp<Self> {
        ops::Clamp::new(self, min, max)
    }

    /// Raise to a power
    fn pow(self, exp: f32) -> ops::Pow<Self> {
        ops::Pow::new(self, exp)
    }

    /// Add another texture
    fn add<T: Texture + 'static>(self, other: T) -> ops::Add<Self, T> {
        ops::Add::new(self, other)
    }

    /// Multiply by another texture
    fn multiply<T: Texture + 'static>(self, other: T) -> ops::Multiply<Self, T> {
        ops::Multiply::new(self, other)
    }

    /// Mix with another texture
    fn mix<T: Texture + 'static>(self, other: T, factor: f32) -> ops::Mix<Self, T> {
        ops::Mix::new(self, other, factor)
    }

    /// Apply threshold
    fn threshold(self, value: f32) -> ops::Threshold<Self> {
        ops::Threshold::new(self, value)
    }

    /// Convert to normal map
    fn as_normal(self, strength: f32) -> ops::ToNormal<Self> {
        ops::ToNormal::new(self, strength)
    }

    /// Warp by another texture
    fn warp<T: Texture + 'static>(self, warper: T, amount: f32) -> ops::Warp<Self, T> {
        ops::Warp::new(self, warper, amount)
    }

    /// Generate an image
    fn to_image(&self, size: u32) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
        let mut img = ImageBuffer::new(size, size);
        for y in 0..size {
            for x in 0..size {
                let uv = Vec2::new(x as f32 / size as f32, y as f32 / size as f32);
                let color = self.sample_color(uv);
                img.put_pixel(
                    x,
                    y,
                    Rgba([
                        (color[0].clamp(0.0, 1.0) * 255.0) as u8,
                        (color[1].clamp(0.0, 1.0) * 255.0) as u8,
                        (color[2].clamp(0.0, 1.0) * 255.0) as u8,
                        (color[3].clamp(0.0, 1.0) * 255.0) as u8,
                    ]),
                );
            }
        }
        img
    }

    /// Save to PNG file
    fn save_png(&self, path: &str, size: u32) -> crate::Result<()> {
        let img = self.to_image(size);
        img.save(path)?;
        Ok(())
    }
}

impl<T: Texture + 'static> TextureExt for T {}

/// Constant value texture
pub struct Constant {
    pub value: f32,
}

impl Constant {
    pub fn new(value: f32) -> Self {
        Self { value }
    }
}

impl Texture for Constant {
    fn sample(&self, _uv: Vec2) -> f32 {
        self.value
    }
}

/// Create a constant texture
pub fn constant(value: f32) -> Constant {
    Constant::new(value)
}
