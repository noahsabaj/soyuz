//! Rhai API for environment configuration
//!
//! This module provides functions to configure lighting, material, and background settings.

use rhai::Engine;
use soyuz_sdf::Environment;
use std::cell::RefCell;

// Thread-local environment that accumulates settings during script execution
thread_local! {
    static CURRENT_ENV: RefCell<Environment> = RefCell::new(Environment::default());
}

/// Reset the environment to defaults (called before each script evaluation)
pub fn reset_environment() {
    CURRENT_ENV.with(|env| {
        *env.borrow_mut() = Environment::default();
    });
}

/// Get the current environment (called after script evaluation)
pub fn get_current_environment() -> Environment {
    CURRENT_ENV.with(|env| env.borrow().clone())
}

// ============================================================================
// Sun/Light Settings
// ============================================================================

/// Set sun direction (will be normalized)
fn set_sun_direction(x: f64, y: f64, z: f64) {
    CURRENT_ENV.with(|env| {
        env.borrow_mut().sun_direction = [x as f32, y as f32, z as f32];
    });
}

/// Set sun color (RGB, 0-1)
fn set_sun_color(r: f64, g: f64, b: f64) {
    CURRENT_ENV.with(|env| {
        env.borrow_mut().sun_color = [r as f32, g as f32, b as f32];
    });
}

/// Set sun intensity
fn set_sun_intensity(intensity: f64) {
    CURRENT_ENV.with(|env| {
        env.borrow_mut().sun_intensity = intensity as f32;
    });
}

/// Set ambient light color (RGB, 0-1)
fn set_ambient_color(r: f64, g: f64, b: f64) {
    CURRENT_ENV.with(|env| {
        env.borrow_mut().ambient_color = [r as f32, g as f32, b as f32];
    });
}

/// Set ambient light intensity
fn set_ambient_intensity(intensity: f64) {
    CURRENT_ENV.with(|env| {
        env.borrow_mut().ambient_intensity = intensity as f32;
    });
}

// ============================================================================
// Material Settings
// ============================================================================

/// Set material color (RGB, 0-1)
fn set_material_color(r: f64, g: f64, b: f64) {
    CURRENT_ENV.with(|env| {
        env.borrow_mut().material_color = [r as f32, g as f32, b as f32];
    });
}

/// Set material color from hex string like "#ff5500" or "ff5500"
fn set_material_color_hex(hex: &str) {
    if let Some((r, g, b)) = parse_hex_color(hex) {
        CURRENT_ENV.with(|env| {
            env.borrow_mut().material_color = [r, g, b];
        });
    }
}

/// Set material shininess (specular exponent, higher = shinier)
fn set_material_shininess(shininess: f64) {
    CURRENT_ENV.with(|env| {
        env.borrow_mut().material_shininess = shininess as f32;
    });
}

/// Set specular intensity (0-1)
fn set_specular_intensity(intensity: f64) {
    CURRENT_ENV.with(|env| {
        env.borrow_mut().specular_intensity = intensity as f32;
    });
}

// ============================================================================
// Background/Sky Settings
// ============================================================================

/// Set sky horizon color (RGB, 0-1)
fn set_sky_horizon(r: f64, g: f64, b: f64) {
    CURRENT_ENV.with(|env| {
        env.borrow_mut().sky_horizon = [r as f32, g as f32, b as f32];
    });
}

/// Set sky zenith color (RGB, 0-1)
fn set_sky_zenith(r: f64, g: f64, b: f64) {
    CURRENT_ENV.with(|env| {
        env.borrow_mut().sky_zenith = [r as f32, g as f32, b as f32];
    });
}

/// Set fog color (RGB, 0-1)
fn set_fog_color(r: f64, g: f64, b: f64) {
    CURRENT_ENV.with(|env| {
        env.borrow_mut().fog_color = [r as f32, g as f32, b as f32];
    });
}

/// Set fog density (0 = no fog, higher = more fog)
fn set_fog_density(density: f64) {
    CURRENT_ENV.with(|env| {
        env.borrow_mut().fog_density = density as f32;
    });
}

// ============================================================================
// Effect Settings
// ============================================================================

/// Enable or disable ambient occlusion
fn set_ao_enabled(enabled: bool) {
    CURRENT_ENV.with(|env| {
        env.borrow_mut().ao_enabled = enabled;
    });
}

/// Set ambient occlusion intensity
fn set_ao_intensity(intensity: f64) {
    CURRENT_ENV.with(|env| {
        env.borrow_mut().ao_intensity = intensity as f32;
    });
}

/// Enable or disable soft shadows
fn set_shadows_enabled(enabled: bool) {
    CURRENT_ENV.with(|env| {
        env.borrow_mut().shadows_enabled = enabled;
    });
}

/// Set shadow softness (higher = softer shadows)
fn set_shadow_softness(softness: f64) {
    CURRENT_ENV.with(|env| {
        env.borrow_mut().shadow_softness = softness as f32;
    });
}

// ============================================================================
// Preset Environments
// ============================================================================

/// Apply a studio lighting preset (neutral, good for viewing models)
fn env_studio() {
    CURRENT_ENV.with(|env| {
        let mut e = env.borrow_mut();
        e.sun_direction = [1.0, 1.0, 0.5];
        e.sun_color = [1.0, 1.0, 1.0];
        e.sun_intensity = 0.8;
        e.ambient_color = [0.3, 0.3, 0.35];
        e.ambient_intensity = 1.0;
        e.sky_horizon = [0.9, 0.9, 0.95];
        e.sky_zenith = [0.8, 0.85, 0.95];
        e.fog_color = [0.85, 0.85, 0.9];
        e.fog_density = 0.005;
    });
}

/// Apply a sunset lighting preset
fn env_sunset() {
    CURRENT_ENV.with(|env| {
        let mut e = env.borrow_mut();
        e.sun_direction = [1.0, 0.2, 0.3];
        e.sun_color = [1.0, 0.6, 0.3];
        e.sun_intensity = 1.2;
        e.ambient_color = [0.3, 0.2, 0.3];
        e.ambient_intensity = 0.8;
        e.sky_horizon = [1.0, 0.7, 0.5];
        e.sky_zenith = [0.4, 0.3, 0.5];
        e.fog_color = [0.9, 0.6, 0.4];
        e.fog_density = 0.02;
    });
}

/// Apply a night lighting preset
fn env_night() {
    CURRENT_ENV.with(|env| {
        let mut e = env.borrow_mut();
        e.sun_direction = [0.5, 0.8, 0.2];
        e.sun_color = [0.7, 0.8, 1.0];
        e.sun_intensity = 0.3;
        e.ambient_color = [0.05, 0.07, 0.15];
        e.ambient_intensity = 1.0;
        e.sky_horizon = [0.1, 0.1, 0.2];
        e.sky_zenith = [0.02, 0.02, 0.08];
        e.fog_color = [0.05, 0.05, 0.1];
        e.fog_density = 0.03;
    });
}

/// Apply a bright daylight preset
fn env_daylight() {
    CURRENT_ENV.with(|env| {
        let mut e = env.borrow_mut();
        e.sun_direction = [0.5, 0.8, 0.3];
        e.sun_color = [1.0, 0.98, 0.95];
        e.sun_intensity = 1.0;
        e.ambient_color = [0.2, 0.25, 0.35];
        e.ambient_intensity = 1.0;
        e.sky_horizon = [0.7, 0.8, 0.9];
        e.sky_zenith = [0.3, 0.5, 0.8];
        e.fog_color = [0.6, 0.65, 0.7];
        e.fog_density = 0.01;
    });
}

/// Apply a clay render preset (no shadows, soft lighting)
fn env_clay() {
    CURRENT_ENV.with(|env| {
        let mut e = env.borrow_mut();
        e.sun_direction = [0.5, 1.0, 0.5];
        e.sun_color = [1.0, 1.0, 1.0];
        e.sun_intensity = 0.6;
        e.ambient_color = [0.5, 0.5, 0.5];
        e.ambient_intensity = 1.0;
        e.material_color = [0.85, 0.85, 0.85];
        e.material_shininess = 8.0;
        e.specular_intensity = 0.1;
        e.shadows_enabled = false;
        e.ao_enabled = true;
        e.ao_intensity = 2.0;
        e.sky_horizon = [0.95, 0.95, 0.95];
        e.sky_zenith = [0.9, 0.9, 0.95];
        e.fog_density = 0.0;
    });
}

// ============================================================================
// Color Helpers
// ============================================================================

/// Parse a hex color string like "#ff5500" or "ff5500"
fn parse_hex_color(hex: &str) -> Option<(f32, f32, f32)> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }

    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;

    Some((r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0))
}

/// Create RGB color from hex string (for use in scripts)
fn rgb_hex(hex: &str) -> rhai::Array {
    if let Some((r, g, b)) = parse_hex_color(hex) {
        vec![
            rhai::Dynamic::from(r as f64),
            rhai::Dynamic::from(g as f64),
            rhai::Dynamic::from(b as f64),
        ]
    } else {
        vec![
            rhai::Dynamic::from(1.0_f64),
            rhai::Dynamic::from(1.0_f64),
            rhai::Dynamic::from(1.0_f64),
        ]
    }
}

// ============================================================================
// Registration
// ============================================================================

/// Register all environment API functions with a Rhai engine
pub fn register_env_api(engine: &mut Engine) {
    // Sun/Light settings
    engine.register_fn("set_sun_direction", set_sun_direction);
    engine.register_fn("set_sun_color", set_sun_color);
    engine.register_fn("set_sun_intensity", set_sun_intensity);
    engine.register_fn("set_ambient_color", set_ambient_color);
    engine.register_fn("set_ambient_intensity", set_ambient_intensity);

    // Material settings
    engine.register_fn("set_material_color", set_material_color);
    engine.register_fn("set_material_color_hex", set_material_color_hex);
    engine.register_fn("set_material_shininess", set_material_shininess);
    engine.register_fn("set_specular_intensity", set_specular_intensity);

    // Background/Sky settings
    engine.register_fn("set_sky_horizon", set_sky_horizon);
    engine.register_fn("set_sky_zenith", set_sky_zenith);
    engine.register_fn("set_fog_color", set_fog_color);
    engine.register_fn("set_fog_density", set_fog_density);

    // Effect settings
    engine.register_fn("set_ao_enabled", set_ao_enabled);
    engine.register_fn("set_ao_intensity", set_ao_intensity);
    engine.register_fn("set_shadows_enabled", set_shadows_enabled);
    engine.register_fn("set_shadow_softness", set_shadow_softness);

    // Presets
    engine.register_fn("env_studio", env_studio);
    engine.register_fn("env_sunset", env_sunset);
    engine.register_fn("env_night", env_night);
    engine.register_fn("env_daylight", env_daylight);
    engine.register_fn("env_clay", env_clay);

    // Color helpers
    engine.register_fn("rgb_hex", rgb_hex);
}
