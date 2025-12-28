//! Soyuz Render - WGPU-based raymarching renderer
//!
//! This crate provides real-time preview rendering for SDFs
//! using raymarching on the GPU.
//!
//! ## Features
//!
//! - Real-time raymarching of SDFs
//! - Interactive camera controls (orbit, pan, zoom)
//! - Headless rendering to image files
//! - SDF-to-WGSL shader code generation
//! - Hot reload watch mode
//! - Embedded preview as X11 child window
//!
//! ## Example
//!
//! ```rust,ignore
//! use soyuz_render::{window, WindowConfig};
//!
//! // Run preview with default scene
//! window::run_preview(WindowConfig::default())?;
//! ```

pub mod camera;
pub mod embedded;
pub mod environment;
pub mod raymarcher;
pub mod shader_gen;
pub mod text_overlay;
pub mod watch_window;
pub mod window;

// Re-export wgpu for users who need texture formats, etc.
pub use wgpu;
pub use winit;

pub use camera::Camera;
pub use embedded::{EmbeddedConfig, embedded_controls_help, run_embedded_preview};
pub use environment::{Environment, EnvironmentUniforms};
pub use raymarcher::{Raymarcher, Uniforms, init_headless, init_with_surface};
pub use shader_gen::{SdfOp, WgslGenerator, build_shader, get_base_shader};
pub use text_overlay::{FpsCounter, FpsOverlay, TextOverlay};
pub use watch_window::{WatchWindowConfig, run_watch_window, watch_controls_help};
pub use window::{WindowConfig, controls_help, run_preview, run_preview_with_sdf};
