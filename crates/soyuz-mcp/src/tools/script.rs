//! Script execution tools for the MCP server
//!
//! Provides tools for executing and validating Rhai scripts.

use schemars::JsonSchema;
use serde::Deserialize;

/// Request for running a script
#[derive(Debug, Deserialize, JsonSchema)]
pub struct RunScriptRequest {
    /// Rhai script code to execute.
    /// The script must return an SDF expression as its final value
    /// (no trailing semicolon on the final line).
    ///
    /// Example:
    /// ```rhai
    /// let body = sphere(0.5);
    /// let hole = cube(0.3);
    /// body.subtract(hole)
    /// ```
    pub code: String,
}

/// Request for compiling (validating) a script
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CompileScriptRequest {
    /// Rhai script code to validate.
    /// The script is checked for syntax errors without executing it.
    pub code: String,
}
