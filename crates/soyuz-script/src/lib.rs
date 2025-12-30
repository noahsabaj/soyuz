//! Soyuz Script - Rhai scripting integration
//!
//! Allows defining procedural assets using Rhai scripts with hot reloading.
//!
//! ## Example Script
//!
//! ```rhai
//! // Create a barrel shape
//! let body = cylinder(0.5, 1.2);
//! let band_top = torus(0.5, 0.08).translate_y(0.5);
//! let band_bottom = torus(0.5, 0.08).translate_y(-0.5);
//!
//! // Combine with smooth union
//! let barrel = body
//!     .smooth_union(band_top, 0.05)
//!     .smooth_union(band_bottom, 0.05)
//!     .hollow(0.05);
//!
//! // Return the final SDF
//! barrel
//! ```
//!
//! ## Environment Configuration
//!
//! You can configure lighting, materials, and background:
//!
//! ```rhai
//! // Use a preset
//! env_sunset();
//!
//! // Or configure individually
//! set_material_color(0.8, 0.2, 0.1);  // Red material
//! set_sun_direction(1.0, 0.5, 0.3);    // Sun position
//! set_fog_density(0.02);               // Add some fog
//!
//! // Your shape
//! sphere(0.5)
//! ```
//!
//! ## Precision Notes
//!
//! Rhai scripts use `f64` for numeric literals, but all values are
//! converted to `f32` when constructing SDF operations. This is
//! required for GPU shader compatibility. For most use cases,
//! the precision loss is negligible.

pub mod cpu_eval;
pub mod engine;
pub mod env_api;
pub mod sdf_api;

#[cfg(feature = "file-watcher")]
pub mod watcher;

pub use cpu_eval::CpuSdf;
pub use engine::{SceneResult, ScriptEngine};
pub use env_api::{get_current_environment, register_env_api, reset_environment};
pub use sdf_api::{RhaiSdf, register_sdf_api};

#[cfg(feature = "file-watcher")]
pub use watcher::{ScriptWatcher, WatchEvent};

// Re-export for convenience
pub use soyuz_sdf::{Environment, SdfOp};

// Re-export soyuz_core Sdf trait for users who need CPU evaluation
pub use soyuz_core::sdf::Sdf;
