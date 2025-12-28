//! Environment settings for the preview renderer
//!
//! This module provides configurable lighting, material, and background settings
//! that can be controlled from Rhai scripts.

use bytemuck::{Pod, Zeroable};

/// Environment settings that control the appearance of the scene
#[derive(Debug, Clone)]
pub struct Environment {
    // Lighting
    /// Sun light direction (will be normalized)
    pub sun_direction: [f32; 3],
    /// Sun light color (RGB, 0-1)
    pub sun_color: [f32; 3],
    /// Sun intensity multiplier
    pub sun_intensity: f32,
    /// Ambient light color (RGB, 0-1)
    pub ambient_color: [f32; 3],
    /// Ambient intensity multiplier
    pub ambient_intensity: f32,

    // Material
    /// Base material color (RGB, 0-1)
    pub material_color: [f32; 3],
    /// Material shininess (specular exponent)
    pub material_shininess: f32,
    /// Specular intensity (0-1)
    pub specular_intensity: f32,

    // Background
    /// Sky color at horizon (RGB, 0-1)
    pub sky_horizon: [f32; 3],
    /// Sky color at zenith (RGB, 0-1)
    pub sky_zenith: [f32; 3],
    /// Ground fog color (RGB, 0-1)
    pub fog_color: [f32; 3],
    /// Fog density (0 = no fog, higher = more fog)
    pub fog_density: f32,

    // Effects
    /// Enable ambient occlusion
    pub ao_enabled: bool,
    /// Ambient occlusion intensity
    pub ao_intensity: f32,
    /// Enable soft shadows
    pub shadows_enabled: bool,
    /// Shadow softness
    pub shadow_softness: f32,
}

impl Default for Environment {
    fn default() -> Self {
        Self {
            // Warm sun from upper right
            sun_direction: [0.8, 0.4, 0.6],
            sun_color: [1.0, 0.95, 0.85],
            sun_intensity: 1.0,

            // Cool ambient
            ambient_color: [0.15, 0.17, 0.2],
            ambient_intensity: 1.0,

            // Light gray material
            material_color: [0.75, 0.75, 0.75],
            material_shininess: 32.0,
            specular_intensity: 0.5,

            // Blue sky gradient
            sky_horizon: [0.7, 0.8, 0.9],
            sky_zenith: [0.3, 0.5, 0.8],
            fog_color: [0.6, 0.65, 0.7],
            fog_density: 0.01,

            // Effects enabled
            ao_enabled: true,
            ao_intensity: 3.0,
            shadows_enabled: true,
            shadow_softness: 8.0,
        }
    }
}

/// GPU-ready environment uniforms
/// This struct must match the WGSL struct layout exactly
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct EnvironmentUniforms {
    // Lighting (vec4 aligned)
    pub sun_direction: [f32; 3],
    pub sun_intensity: f32,
    pub sun_color: [f32; 3],
    pub ambient_intensity: f32,
    pub ambient_color: [f32; 3],
    pub material_shininess: f32,

    // Material
    pub material_color: [f32; 3],
    pub specular_intensity: f32,

    // Background
    pub sky_horizon: [f32; 3],
    pub fog_density: f32,
    pub sky_zenith: [f32; 3],
    pub ao_intensity: f32,
    pub fog_color: [f32; 3],
    pub shadow_softness: f32,

    // Flags (packed as floats for GPU compatibility)
    pub ao_enabled: f32,
    pub shadows_enabled: f32,
    pub _pad1: f32,
    pub _pad2: f32,
}

impl From<&Environment> for EnvironmentUniforms {
    fn from(env: &Environment) -> Self {
        // Normalize sun direction
        let sun_dir = env.sun_direction;
        let len =
            (sun_dir[0] * sun_dir[0] + sun_dir[1] * sun_dir[1] + sun_dir[2] * sun_dir[2]).sqrt();
        let sun_direction = if len > 0.0 {
            [sun_dir[0] / len, sun_dir[1] / len, sun_dir[2] / len]
        } else {
            [0.0, 1.0, 0.0] // Default to up if zero
        };

        Self {
            sun_direction,
            sun_intensity: env.sun_intensity,
            sun_color: env.sun_color,
            ambient_intensity: env.ambient_intensity,
            ambient_color: env.ambient_color,
            material_shininess: env.material_shininess,
            material_color: env.material_color,
            specular_intensity: env.specular_intensity,
            sky_horizon: env.sky_horizon,
            fog_density: env.fog_density,
            sky_zenith: env.sky_zenith,
            ao_intensity: env.ao_intensity,
            fog_color: env.fog_color,
            shadow_softness: env.shadow_softness,
            ao_enabled: if env.ao_enabled { 1.0 } else { 0.0 },
            shadows_enabled: if env.shadows_enabled { 1.0 } else { 0.0 },
            _pad1: 0.0,
            _pad2: 0.0,
        }
    }
}

impl Default for EnvironmentUniforms {
    fn default() -> Self {
        EnvironmentUniforms::from(&Environment::default())
    }
}
