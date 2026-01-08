//! MCP tool implementations for Soyuz
//!
//! This module contains all the MCP tools exposed by the Soyuz server:
//! - Script execution (run_script, compile_script)
//! - Rendering (render_preview)
//! - Export (export_mesh, get_wgsl)
//! - Discovery (list_primitives, list_operations, list_transforms, list_modifiers)

pub mod discovery;
pub mod export;
pub mod render;
pub mod script;
