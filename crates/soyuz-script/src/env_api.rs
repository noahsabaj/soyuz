//! Rhai API for environment configuration
//!
//! This module provides functions to configure lighting, material, and background settings.

// Mutex lock only panics if poisoned (another thread panicked), which is unrecoverable
#![allow(clippy::unwrap_used)]
#![allow(clippy::missing_panics_doc)]

use rhai::Engine;
use soyuz_sdf::Environment;
use std::sync::{Arc, Mutex};

// ============================================================================
// Color Helpers
// ============================================================================

/// Parse a hex color string like "#ff5500" or "ff5500"
pub(crate) fn parse_hex_color(hex: &str) -> Option<(f32, f32, f32)> {
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
pub(crate) fn rgb_hex(hex: &str) -> rhai::Array {
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
pub fn register_env_api(engine: &mut Engine, env: &Arc<Mutex<Environment>>) {
    crate::api_generated::register_env_api_generated(engine, env);
}
