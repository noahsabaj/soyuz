//! Noise functions for procedural textures

use super::Texture;
use glam::Vec2;
use noise::{NoiseFn, Perlin, Simplex};

/// Perlin noise texture
pub struct PerlinNoise {
    noise: Perlin,
    scale: f32,
}

impl PerlinNoise {
    pub fn new(scale: f32) -> Self {
        Self {
            noise: Perlin::new(0),
            scale,
        }
    }

    pub fn with_seed(seed: u32, scale: f32) -> Self {
        Self {
            noise: Perlin::new(seed),
            scale,
        }
    }
}

impl Texture for PerlinNoise {
    fn sample(&self, uv: Vec2) -> f32 {
        let p = [
            uv.x as f64 * self.scale as f64,
            uv.y as f64 * self.scale as f64,
        ];
        (self.noise.get(p) as f32 + 1.0) * 0.5 // Normalize to 0..1
    }
}

/// Create Perlin noise
pub fn perlin(scale: f32) -> PerlinNoise {
    PerlinNoise::new(scale)
}

/// Simplex noise texture
pub struct SimplexNoise {
    noise: Simplex,
    scale: f32,
}

impl SimplexNoise {
    pub fn new(scale: f32) -> Self {
        Self {
            noise: Simplex::new(0),
            scale,
        }
    }

    pub fn with_seed(seed: u32, scale: f32) -> Self {
        Self {
            noise: Simplex::new(seed),
            scale,
        }
    }
}

impl Texture for SimplexNoise {
    fn sample(&self, uv: Vec2) -> f32 {
        let p = [
            uv.x as f64 * self.scale as f64,
            uv.y as f64 * self.scale as f64,
        ];
        (self.noise.get(p) as f32 + 1.0) * 0.5
    }
}

/// Create Simplex noise
pub fn simplex(scale: f32) -> SimplexNoise {
    SimplexNoise::new(scale)
}

/// Worley (cellular/Voronoi) noise texture
/// Custom implementation that is thread-safe
pub struct WorleyNoise {
    scale: f32,
    seed: u32,
}

impl WorleyNoise {
    pub fn new(scale: f32) -> Self {
        Self { scale, seed: 0 }
    }

    pub fn with_seed(seed: u32, scale: f32) -> Self {
        Self { scale, seed }
    }
}

impl Texture for WorleyNoise {
    fn sample(&self, uv: Vec2) -> f32 {
        let p = uv * self.scale;
        let cell = p.floor();
        let fract = p.fract();

        let mut min_dist = f32::MAX;

        // Check neighboring cells
        for dy in -1..=1 {
            for dx in -1..=1 {
                let neighbor = Vec2::new(dx as f32, dy as f32);
                let point = hash_vec2(cell + neighbor, self.seed);
                let diff = neighbor + point - fract;
                let dist = diff.length();
                min_dist = min_dist.min(dist);
            }
        }

        min_dist.clamp(0.0, 1.0)
    }
}

// Simple hash function for Worley noise
// Note: These magic constants are arbitrary hash coefficients, not math constants
#[allow(clippy::approx_constant)]
fn hash_vec2(p: Vec2, seed: u32) -> Vec2 {
    let k = Vec2::new(0.3183099, 0.3678794);
    let p = p + Vec2::splat(seed as f32 * 0.1);
    let p = p * k + Vec2::new(k.y, k.x);
    Vec2::new(
        (16.0 * (p.x * p.y * (p.x + p.y)).fract()).fract(),
        (16.0 * (p.y * p.x * (p.y - p.x + 1.0)).fract()).fract(),
    )
}

/// Create Worley (cellular) noise
pub fn worley(scale: f32) -> WorleyNoise {
    WorleyNoise::new(scale)
}

/// Fractal Brownian Motion (fBm) - layered noise
pub struct Fbm {
    noise: Perlin,
    octaves: u32,
    lacunarity: f32,
    persistence: f32,
    scale: f32,
}

impl Fbm {
    pub fn new(octaves: u32) -> Self {
        Self {
            noise: Perlin::new(0),
            octaves,
            lacunarity: 2.0,
            persistence: 0.5,
            scale: 1.0,
        }
    }

    pub fn with_seed(seed: u32, octaves: u32) -> Self {
        Self {
            noise: Perlin::new(seed),
            octaves,
            lacunarity: 2.0,
            persistence: 0.5,
            scale: 1.0,
        }
    }

    pub fn lacunarity(mut self, value: f32) -> Self {
        self.lacunarity = value;
        self
    }

    pub fn persistence(mut self, value: f32) -> Self {
        self.persistence = value;
        self
    }

    pub fn scale(mut self, value: f32) -> Self {
        self.scale = value;
        self
    }
}

impl Texture for Fbm {
    fn sample(&self, uv: Vec2) -> f32 {
        let mut value = 0.0f32;
        let mut amplitude = 1.0f32;
        let mut frequency = self.scale;
        let mut max_value = 0.0f32;

        for _ in 0..self.octaves {
            let p = [
                uv.x as f64 * frequency as f64,
                uv.y as f64 * frequency as f64,
            ];
            value += self.noise.get(p) as f32 * amplitude;
            max_value += amplitude;
            amplitude *= self.persistence;
            frequency *= self.lacunarity;
        }

        (value / max_value + 1.0) * 0.5 // Normalize to 0..1
    }
}

/// Create fBm (Fractal Brownian Motion) noise
pub fn fbm(octaves: u32) -> Fbm {
    Fbm::new(octaves)
}

/// Ridged multifractal noise
pub struct Ridged {
    noise: Perlin,
    octaves: u32,
    lacunarity: f32,
    persistence: f32,
    scale: f32,
}

impl Ridged {
    pub fn new(octaves: u32) -> Self {
        Self {
            noise: Perlin::new(0),
            octaves,
            lacunarity: 2.0,
            persistence: 0.5,
            scale: 1.0,
        }
    }

    pub fn with_seed(seed: u32, octaves: u32) -> Self {
        Self {
            noise: Perlin::new(seed),
            octaves,
            lacunarity: 2.0,
            persistence: 0.5,
            scale: 1.0,
        }
    }
}

impl Texture for Ridged {
    fn sample(&self, uv: Vec2) -> f32 {
        let mut value = 0.0f32;
        let mut amplitude = 1.0f32;
        let mut frequency = self.scale;
        let mut weight = 1.0f32;

        for _ in 0..self.octaves {
            let p = [
                uv.x as f64 * frequency as f64,
                uv.y as f64 * frequency as f64,
            ];
            let mut signal = self.noise.get(p) as f32;
            signal = 1.0 - signal.abs();
            signal *= signal * weight;
            weight = signal.clamp(0.0, 1.0);
            value += signal * amplitude;
            amplitude *= self.persistence;
            frequency *= self.lacunarity;
        }

        value * 0.5 // Adjust range
    }
}

/// Create ridged multifractal noise
pub fn ridged(octaves: u32) -> Ridged {
    Ridged::new(octaves)
}

/// Turbulence noise (absolute value fBm)
pub struct Turbulence {
    noise: Perlin,
    octaves: u32,
    scale: f32,
}

impl Turbulence {
    pub fn new(octaves: u32) -> Self {
        Self {
            noise: Perlin::new(0),
            octaves,
            scale: 1.0,
        }
    }
}

impl Texture for Turbulence {
    fn sample(&self, uv: Vec2) -> f32 {
        let mut value = 0.0f32;
        let mut amplitude = 1.0f32;
        let mut frequency = self.scale;
        let mut max_value = 0.0f32;

        for _ in 0..self.octaves {
            let p = [
                uv.x as f64 * frequency as f64,
                uv.y as f64 * frequency as f64,
            ];
            value += (self.noise.get(p) as f32).abs() * amplitude;
            max_value += amplitude;
            amplitude *= 0.5;
            frequency *= 2.0;
        }

        value / max_value
    }
}

/// Create turbulence noise
pub fn turbulence(octaves: u32) -> Turbulence {
    Turbulence::new(octaves)
}
