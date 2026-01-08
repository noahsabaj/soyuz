//! Soyuz SDF - Platform-agnostic SDF types and shader generation
//!
//! This crate provides the core SDF representation and WGSL shader generation
//! that can be used both on desktop (native) and in the browser (WebAssembly).
//!
//! ## Key Types
//!
//! - [`SdfOp`] - The SDF operation tree representation
//! - [`WgslGenerator`] - Converts [`SdfOp`] trees to WGSL shader code
//! - [`Environment`] - Lighting, material, and background settings
//!
//! ## Example
//!
//! ```rust
//! use soyuz_sdf::{SdfOp, WgslGenerator, build_shader};
//!
//! // Create a simple sphere
//! let sdf = SdfOp::Sphere { radius: 1.0 };
//!
//! // Generate the complete shader
//! let shader = build_shader(&sdf);
//! ```

mod environment;
mod sdf_op;
mod wgsl_gen;

pub use environment::{Environment, EnvironmentUniforms};
pub use sdf_op::{ExtrudeProfile, RevolveProfile, SdfOp};
pub use wgsl_gen::{WgslGenerator, build_shader, get_base_shader, inject_scene_sdf};
