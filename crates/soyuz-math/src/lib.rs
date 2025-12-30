//! Soyuz Math - Single Source of Truth for SDF Mathematical Formulas
//!
//! This crate provides verified mathematical implementations for SDF operations.
//! All formulas are defined in TOML specification files and code is auto-generated
//! to ensure Rust and WGSL implementations stay in sync.
//!
//! # Architecture
//!
//! ```text
//! formulas/*.toml  →  [build.rs]  →  Rust code (this crate)
//!                                 →  WGSL code (for shaders)
//!                                 →  Test vectors
//!                                 →  Documentation
//! ```
//!
//! # Adding New Formulas
//!
//! 1. Create a new `.toml` file in `formulas/`
//! 2. Define the formula spec (see `repeat_polar.toml` as example)
//! 3. Run `cargo build` to generate code
//! 4. Use the generated functions in your code
//!
//! # Example
//!
//! ```rust
//! use soyuz_math::repeat_polar;
//! use glam::Vec3;
//!
//! let p = Vec3::new(0.0, 0.5, 0.6);
//! let folded = repeat_polar(p, 8.0);
//! // folded is now in the primary sector
//! ```

// Include the auto-generated Rust implementations
// Allow doc_markdown because generated docs contain function names like cos(), sin(), etc.
#[allow(clippy::doc_markdown)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/formulas.rs"));
}
pub use generated::*;

/// Get the WGSL code for all formulas
///
/// This returns the auto-generated WGSL code that should be injected
/// into shaders. The code is generated from the same TOML specs as
/// the Rust implementations, ensuring they stay in sync.
pub fn get_wgsl_code() -> &'static str {
    include_str!(concat!(env!("OUT_DIR"), "/formulas.wgsl"))
}

/// Get the markdown documentation for all formulas
pub fn get_docs() -> &'static str {
    include_str!(concat!(env!("OUT_DIR"), "/FORMULAS.md"))
}

#[cfg(test)]
#[allow(clippy::unreadable_literal)]
mod tests {
    use super::*;

    // Include auto-generated tests
    include!(concat!(env!("OUT_DIR"), "/tests.rs"));

    #[test]
    fn test_wgsl_code_generated() {
        let wgsl = get_wgsl_code();
        assert!(
            wgsl.contains("op_repeat_polar"),
            "WGSL should contain repeat_polar"
        );
    }

    #[test]
    fn test_docs_generated() {
        let docs = get_docs();
        assert!(
            docs.contains("repeat_polar"),
            "Docs should contain repeat_polar"
        );
    }
}
