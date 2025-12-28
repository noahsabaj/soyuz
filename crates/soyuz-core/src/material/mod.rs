//! Material definitions for procedural assets

use crate::texture::Texture;
use image::{ImageBuffer, Rgba, RgbaImage};
use std::sync::Arc;

/// Helper function to rasterize a texture trait object
fn rasterize_texture(tex: &dyn Texture, size: u32) -> RgbaImage {
    let mut img = ImageBuffer::new(size, size);
    for y in 0..size {
        for x in 0..size {
            let uv = glam::Vec2::new(x as f32 / size as f32, y as f32 / size as f32);
            let color = tex.sample_color(uv);
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

/// A PBR material
#[derive(Clone)]
pub struct Material {
    pub albedo: MaterialChannel,
    pub roughness: MaterialChannel,
    pub metallic: MaterialChannel,
    pub normal: Option<Arc<dyn Texture>>,
    pub ao: MaterialChannel,
    pub emissive: MaterialChannel,
    pub emissive_strength: f32,
}

impl Default for Material {
    fn default() -> Self {
        Self {
            albedo: MaterialChannel::Color([0.8, 0.8, 0.8, 1.0]),
            roughness: MaterialChannel::Value(0.5),
            metallic: MaterialChannel::Value(0.0),
            normal: None,
            ao: MaterialChannel::Value(1.0),
            emissive: MaterialChannel::Color([0.0, 0.0, 0.0, 1.0]),
            emissive_strength: 0.0,
        }
    }
}

impl Material {
    /// Create a new PBR material with defaults
    pub fn pbr() -> Self {
        Self::default()
    }

    /// Set albedo color
    pub fn albedo_color(mut self, r: f32, g: f32, b: f32) -> Self {
        self.albedo = MaterialChannel::Color([r, g, b, 1.0]);
        self
    }

    /// Set albedo from texture
    pub fn albedo_texture<T: Texture + 'static>(mut self, tex: T) -> Self {
        self.albedo = MaterialChannel::Texture(Arc::new(tex));
        self
    }

    /// Set roughness value
    pub fn roughness(mut self, value: f32) -> Self {
        self.roughness = MaterialChannel::Value(value);
        self
    }

    /// Set roughness from texture
    pub fn roughness_texture<T: Texture + 'static>(mut self, tex: T) -> Self {
        self.roughness = MaterialChannel::Texture(Arc::new(tex));
        self
    }

    /// Set metallic value
    pub fn metallic(mut self, value: f32) -> Self {
        self.metallic = MaterialChannel::Value(value);
        self
    }

    /// Set metallic from texture
    pub fn metallic_texture<T: Texture + 'static>(mut self, tex: T) -> Self {
        self.metallic = MaterialChannel::Texture(Arc::new(tex));
        self
    }

    /// Set normal map
    pub fn normal<T: Texture + 'static>(mut self, tex: T) -> Self {
        self.normal = Some(Arc::new(tex));
        self
    }

    /// Set ambient occlusion value
    pub fn ao(mut self, value: f32) -> Self {
        self.ao = MaterialChannel::Value(value);
        self
    }

    /// Set AO from texture
    pub fn ao_texture<T: Texture + 'static>(mut self, tex: T) -> Self {
        self.ao = MaterialChannel::Texture(Arc::new(tex));
        self
    }

    /// Set emissive color
    pub fn emissive(mut self, r: f32, g: f32, b: f32, strength: f32) -> Self {
        self.emissive = MaterialChannel::Color([r, g, b, 1.0]);
        self.emissive_strength = strength;
        self
    }

    /// Rasterize the material to a set of images
    pub fn rasterize(&self, size: u32) -> RasterizedMaterial {
        RasterizedMaterial {
            albedo: self.albedo.rasterize(size),
            metallic_roughness: self.rasterize_metallic_roughness(size),
            normal: self.normal.as_ref().map(|n| {
                let mut img = ImageBuffer::new(size, size);
                for y in 0..size {
                    for x in 0..size {
                        let uv = glam::Vec2::new(x as f32 / size as f32, y as f32 / size as f32);
                        let color = n.sample_color(uv);
                        // Normal maps are stored as RGB where R=X, G=Y, B=Z
                        // Values are remapped from [-1,1] to [0,1]
                        img.put_pixel(
                            x,
                            y,
                            Rgba([
                                ((color[0] * 0.5 + 0.5).clamp(0.0, 1.0) * 255.0) as u8,
                                ((color[1] * 0.5 + 0.5).clamp(0.0, 1.0) * 255.0) as u8,
                                ((color[2] * 0.5 + 0.5).clamp(0.0, 1.0) * 255.0) as u8,
                                255,
                            ]),
                        );
                    }
                }
                img
            }),
            ao: match &self.ao {
                MaterialChannel::Value(v) => {
                    let byte = (v.clamp(0.0, 1.0) * 255.0) as u8;
                    let mut img = ImageBuffer::new(size, size);
                    for pixel in img.pixels_mut() {
                        *pixel = Rgba([byte, byte, byte, 255]);
                    }
                    Some(img)
                }
                MaterialChannel::Texture(t) => Some(rasterize_texture(t.as_ref(), size)),
                _ => None,
            },
            emissive: if self.emissive_strength > 0.0 {
                Some(self.emissive.rasterize(size))
            } else {
                None
            },
            emissive_strength: self.emissive_strength,
        }
    }

    /// Rasterize metallic-roughness into GLTF format (R=unused, G=roughness, B=metallic)
    fn rasterize_metallic_roughness(&self, size: u32) -> RgbaImage {
        let mut img = ImageBuffer::new(size, size);

        for y in 0..size {
            for x in 0..size {
                let uv = glam::Vec2::new(x as f32 / size as f32, y as f32 / size as f32);

                let roughness = self.roughness.sample(uv)[0];
                let metallic = self.metallic.sample(uv)[0];

                // GLTF metallic-roughness format:
                // R = unused (or AO), G = roughness, B = metallic, A = unused
                img.put_pixel(
                    x,
                    y,
                    Rgba([
                        255, // Unused
                        (roughness.clamp(0.0, 1.0) * 255.0) as u8,
                        (metallic.clamp(0.0, 1.0) * 255.0) as u8,
                        255,
                    ]),
                );
            }
        }

        img
    }

    /// Check if this material has any textures
    pub fn has_textures(&self) -> bool {
        matches!(self.albedo, MaterialChannel::Texture(_))
            || matches!(self.roughness, MaterialChannel::Texture(_))
            || matches!(self.metallic, MaterialChannel::Texture(_))
            || self.normal.is_some()
            || matches!(self.ao, MaterialChannel::Texture(_))
            || matches!(self.emissive, MaterialChannel::Texture(_))
    }

    /// Get base color factor (for materials without texture)
    pub fn base_color_factor(&self) -> [f32; 4] {
        match &self.albedo {
            MaterialChannel::Color(c) => *c,
            MaterialChannel::Value(v) => [*v, *v, *v, 1.0],
            MaterialChannel::Texture(_) => [1.0, 1.0, 1.0, 1.0],
        }
    }

    /// Get metallic factor
    pub fn metallic_factor(&self) -> f32 {
        match &self.metallic {
            MaterialChannel::Value(v) => *v,
            _ => 1.0,
        }
    }

    /// Get roughness factor
    pub fn roughness_factor(&self) -> f32 {
        match &self.roughness {
            MaterialChannel::Value(v) => *v,
            _ => 1.0,
        }
    }
}

/// A material channel can be a constant value, color, or texture
#[derive(Clone)]
pub enum MaterialChannel {
    Value(f32),
    Color([f32; 4]),
    Texture(Arc<dyn Texture>),
}

impl MaterialChannel {
    /// Sample this channel at given UV
    pub fn sample(&self, uv: glam::Vec2) -> [f32; 4] {
        match self {
            MaterialChannel::Value(v) => [*v, *v, *v, 1.0],
            MaterialChannel::Color(c) => *c,
            MaterialChannel::Texture(t) => t.sample_color(uv),
        }
    }

    /// Rasterize this channel to an image
    pub fn rasterize(&self, size: u32) -> RgbaImage {
        let mut img = ImageBuffer::new(size, size);

        match self {
            MaterialChannel::Value(v) => {
                let byte = (v.clamp(0.0, 1.0) * 255.0) as u8;
                for pixel in img.pixels_mut() {
                    *pixel = Rgba([byte, byte, byte, 255]);
                }
            }
            MaterialChannel::Color(c) => {
                let rgba = Rgba([
                    (c[0].clamp(0.0, 1.0) * 255.0) as u8,
                    (c[1].clamp(0.0, 1.0) * 255.0) as u8,
                    (c[2].clamp(0.0, 1.0) * 255.0) as u8,
                    (c[3].clamp(0.0, 1.0) * 255.0) as u8,
                ]);
                for pixel in img.pixels_mut() {
                    *pixel = rgba;
                }
            }
            MaterialChannel::Texture(t) => {
                for y in 0..size {
                    for x in 0..size {
                        let uv = glam::Vec2::new(x as f32 / size as f32, y as f32 / size as f32);
                        let color = t.sample_color(uv);
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
            }
        }

        img
    }
}

/// Shorthand for creating a PBR material
pub fn pbr() -> Material {
    Material::pbr()
}

/// Alias for Material
pub type PbrMaterial = Material;

/// Rasterized PBR material textures ready for export
pub struct RasterizedMaterial {
    /// Albedo/base color texture
    pub albedo: RgbaImage,
    /// Combined metallic-roughness texture (G=roughness, B=metallic)
    pub metallic_roughness: RgbaImage,
    /// Normal map texture
    pub normal: Option<RgbaImage>,
    /// Ambient occlusion texture
    pub ao: Option<RgbaImage>,
    /// Emissive texture
    pub emissive: Option<RgbaImage>,
    /// Emissive strength multiplier
    pub emissive_strength: f32,
}

impl RasterizedMaterial {
    /// Encode an image as PNG bytes
    pub fn encode_png(img: &RgbaImage) -> Vec<u8> {
        use image::ImageEncoder;
        use image::codecs::png::PngEncoder;
        use std::io::Cursor;

        let mut bytes = Vec::new();
        let cursor = Cursor::new(&mut bytes);
        let encoder = PngEncoder::new(cursor);
        encoder
            .write_image(
                img.as_raw(),
                img.width(),
                img.height(),
                image::ExtendedColorType::Rgba8,
            )
            .expect("Failed to encode PNG");
        bytes
    }

    /// Get all textures as PNG-encoded bytes for embedding
    pub fn as_png_bytes(&self) -> MaterialTextureBytes {
        MaterialTextureBytes {
            albedo: Self::encode_png(&self.albedo),
            metallic_roughness: Self::encode_png(&self.metallic_roughness),
            normal: self.normal.as_ref().map(Self::encode_png),
            ao: self.ao.as_ref().map(Self::encode_png),
            emissive: self.emissive.as_ref().map(Self::encode_png),
        }
    }
}

/// PNG-encoded material textures
pub struct MaterialTextureBytes {
    pub albedo: Vec<u8>,
    pub metallic_roughness: Vec<u8>,
    pub normal: Option<Vec<u8>>,
    pub ao: Option<Vec<u8>>,
    pub emissive: Option<Vec<u8>>,
}

/// A mesh combined with a material
#[derive(Clone)]
pub struct MeshWithMaterial {
    pub mesh: crate::mesh::Mesh,
    pub material: Material,
}

impl MeshWithMaterial {
    pub fn new(mesh: crate::mesh::Mesh, material: Material) -> Self {
        Self { mesh, material }
    }
}
