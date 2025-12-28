//! Script engine for evaluating Rhai scripts that produce SDFs

use crate::env_api::{get_current_environment, register_env_api, reset_environment};
use crate::sdf_api::{RhaiSdf, register_sdf_api};
use anyhow::{Result, anyhow};
use rhai::{Dynamic, Engine, Scope};
use soyuz_render::{Environment, SdfOp};
use std::path::Path;

/// Result of evaluating a script - contains both the SDF and environment settings
#[derive(Debug, Clone)]
pub struct SceneResult {
    /// The SDF geometry
    pub sdf: SdfOp,
    /// Environment settings (lighting, material, background)
    pub environment: Environment,
}

/// Soyuz script engine for evaluating SDF scripts
pub struct ScriptEngine {
    engine: Engine,
}

impl ScriptEngine {
    /// Create a new script engine with all SDF and environment functions registered
    pub fn new() -> Self {
        let mut engine = Engine::new();

        // Register all SDF primitives and operations
        register_sdf_api(&mut engine);

        // Register environment configuration API
        register_env_api(&mut engine);

        // Configure engine for better errors
        engine.set_max_expr_depths(64, 64);

        Self { engine }
    }

    /// Evaluate a script and return the resulting SDF
    ///
    /// The script should evaluate to an SDF value as its final expression.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let engine = ScriptEngine::new();
    /// let sdf = engine.eval_sdf("sphere(1.0)")?;
    /// ```
    pub fn eval_sdf(&self, script: &str) -> Result<RhaiSdf> {
        // Reset environment before evaluation
        reset_environment();

        let result: Dynamic = self
            .engine
            .eval(script)
            .map_err(|e| anyhow!("Failed to evaluate script: {}", e))?;

        // Try to extract RhaiSdf from the result
        result.try_cast::<RhaiSdf>().ok_or_else(|| {
            let trimmed = script.trim();
            if trimmed.ends_with(';') {
                anyhow!(
                    "Script did not return an SDF.\n\n\
                    HINT: Your script ends with ';' which returns nothing.\n\
                    Add the variable name at the end:\n\n\
                      let shape = sphere(0.5);\n\
                      shape  // <- return it!"
                )
            } else {
                anyhow!("Script did not return an SDF. The last expression must be a shape.")
            }
        })
    }

    /// Evaluate a script file and return the resulting SDF
    pub fn eval_sdf_file(&self, path: &Path) -> Result<RhaiSdf> {
        let script = std::fs::read_to_string(path)
            .map_err(|e| anyhow!("Failed to read script file {}: {}", path.display(), e))?;

        self.eval_sdf(&script)
            .map_err(|e| anyhow!("Error in script {}: {}", path.display(), e))
    }

    /// Evaluate a script and return the SdfOp (for the renderer)
    pub fn eval_to_sdf_op(&self, script: &str) -> Result<SdfOp> {
        let rhai_sdf = self.eval_sdf(script)?;
        Ok(rhai_sdf.to_sdf_op())
    }

    /// Evaluate a script file and return the SdfOp (for the renderer)
    pub fn eval_file_to_sdf_op(&self, path: &Path) -> Result<SdfOp> {
        let rhai_sdf = self.eval_sdf_file(path)?;
        Ok(rhai_sdf.to_sdf_op())
    }

    /// Evaluate a script and return both SDF and environment settings
    ///
    /// This is the recommended method for preview rendering as it captures
    /// any environment configuration done in the script.
    pub fn eval_scene(&self, script: &str) -> Result<SceneResult> {
        // Reset environment before evaluation
        reset_environment();

        let result: Dynamic = self
            .engine
            .eval(script)
            .map_err(|e| anyhow!("Failed to evaluate script: {}", e))?;

        // Extract SDF
        let rhai_sdf = result.try_cast::<RhaiSdf>().ok_or_else(|| {
            let trimmed = script.trim();
            if trimmed.ends_with(';') {
                anyhow!(
                    "Script did not return an SDF.\n\n\
                    HINT: Your script ends with ';' which returns nothing.\n\
                    Add the variable name at the end:\n\n\
                      let shape = sphere(0.5);\n\
                      shape  // <- return it!"
                )
            } else {
                anyhow!("Script did not return an SDF. The last expression must be a shape.")
            }
        })?;

        // Get the environment that was configured during script execution
        let environment = get_current_environment();

        Ok(SceneResult {
            sdf: rhai_sdf.to_sdf_op(),
            environment,
        })
    }

    /// Evaluate a script file and return both SDF and environment settings
    pub fn eval_scene_file(&self, path: &Path) -> Result<SceneResult> {
        let script = std::fs::read_to_string(path)
            .map_err(|e| anyhow!("Failed to read script file {}: {}", path.display(), e))?;

        self.eval_scene(&script)
            .map_err(|e| anyhow!("Error in script {}: {}", path.display(), e))
    }

    /// Evaluate a script without expecting a return value
    pub fn run(&self, script: &str) -> Result<()> {
        self.engine
            .run(script)
            .map_err(|e| anyhow!("Failed to run script: {}", e))?;
        Ok(())
    }

    /// Evaluate a script with a pre-populated scope
    pub fn eval_sdf_with_scope(&self, script: &str, scope: &mut Scope) -> Result<RhaiSdf> {
        let result: Dynamic = self
            .engine
            .eval_with_scope(scope, script)
            .map_err(|e| anyhow!("Failed to evaluate script: {}", e))?;

        result
            .try_cast::<RhaiSdf>()
            .ok_or_else(|| anyhow!("Script did not return an SDF"))
    }

    /// Create a new scope for REPL-style evaluation
    pub fn new_scope(&self) -> Scope<'static> {
        Scope::new()
    }

    /// Compile a script to check for syntax errors without running it
    pub fn compile(&self, script: &str) -> Result<()> {
        self.engine
            .compile(script)
            .map_err(|e| anyhow!("Script compilation failed: {}", e))?;
        Ok(())
    }

    /// Get a reference to the underlying Rhai engine
    pub fn inner(&self) -> &Engine {
        &self.engine
    }

    /// Get a mutable reference to the underlying Rhai engine
    pub fn inner_mut(&mut self) -> &mut Engine {
        &mut self.engine
    }
}

impl Default for ScriptEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of evaluating a script - either an SDF or an error
#[derive(Debug)]
pub enum ScriptResult {
    /// Successfully produced an SDF
    Sdf(SdfOp),
    /// Script executed but didn't return an SDF
    NoReturn,
    /// Script error
    Error(String),
}

impl ScriptResult {
    /// Check if the result is an SDF
    pub fn is_sdf(&self) -> bool {
        matches!(self, ScriptResult::Sdf(_))
    }

    /// Check if the result is an error
    pub fn is_error(&self) -> bool {
        matches!(self, ScriptResult::Error(_))
    }

    /// Get the SDF if available
    pub fn sdf(&self) -> Option<&SdfOp> {
        match self {
            ScriptResult::Sdf(sdf) => Some(sdf),
            _ => None,
        }
    }

    /// Get the error message if available
    pub fn error(&self) -> Option<&str> {
        match self {
            ScriptResult::Error(e) => Some(e),
            _ => None,
        }
    }
}

/// Evaluate a script and return a ScriptResult
///
/// This is a convenience function for cases where you want to handle
/// errors gracefully without using Result.
pub fn try_eval_script(engine: &ScriptEngine, script: &str) -> ScriptResult {
    match engine.eval_to_sdf_op(script) {
        Ok(sdf) => ScriptResult::Sdf(sdf),
        Err(e) => {
            // Check if it's a "no return" type error
            let error_str = e.to_string();
            if error_str.contains("did not return an SDF") {
                ScriptResult::NoReturn
            } else {
                ScriptResult::Error(error_str)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eval_sphere() {
        let engine = ScriptEngine::new();
        let result = engine.eval_sdf("sphere(1.0)");
        assert!(result.is_ok());
    }

    #[test]
    fn test_eval_complex() {
        let engine = ScriptEngine::new();
        let script = r#"
            let body = cylinder(0.5, 1.0);
            let ring = torus(0.5, 0.1).translate_y(0.3);
            body.smooth_union(ring, 0.1)
        "#;
        let result = engine.eval_sdf(script);
        assert!(result.is_ok());
    }

    #[test]
    fn test_syntax_error() {
        let engine = ScriptEngine::new();
        let result = engine.eval_sdf("sphere(");
        assert!(result.is_err());
    }
}
