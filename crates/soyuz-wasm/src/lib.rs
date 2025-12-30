//! Soyuz WASM - WebAssembly bindings for browser-based SDF scripting
//!
//! This crate provides WASM bindings that allow Rhai SDF scripts to be
//! validated and compiled to WGSL shader code in the browser.

use wasm_bindgen::prelude::*;

/// Initialize panic hook for better error messages in browser console
#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
}

/// Compile a Rhai script and return any errors
#[wasm_bindgen]
pub fn validate_script(code: &str) -> Result<JsValue, JsValue> {
    use soyuz_script::ScriptEngine;

    let engine = ScriptEngine::new();
    match engine.compile(code) {
        Ok(()) => Ok(JsValue::NULL),
        Err(e) => Err(JsValue::from_str(&e.to_string())),
    }
}

/// Compile a Rhai script and return the generated WGSL shader code
#[wasm_bindgen]
pub fn compile_to_wgsl(code: &str) -> Result<String, JsValue> {
    use soyuz_script::ScriptEngine;
    use soyuz_sdf::build_shader;

    let engine = ScriptEngine::new();
    let scene_result = engine
        .eval_scene(code)
        .map_err(|e| JsValue::from_str(&format!("Script error: {}", e)))?;

    let wgsl = build_shader(&scene_result.sdf);
    Ok(wgsl)
}

#[wasm_bindgen]
pub struct ScriptResult {
    success: bool,
    error_message: Option<String>,
    error_line: Option<u32>,
}

#[wasm_bindgen]
impl ScriptResult {
    #[wasm_bindgen(getter)]
    pub fn success(&self) -> bool {
        self.success
    }

    #[wasm_bindgen(getter)]
    pub fn error_message(&self) -> Option<String> {
        self.error_message.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn error_line(&self) -> Option<u32> {
        self.error_line
    }
}

/// Parse and validate a script, returning detailed error information
#[wasm_bindgen]
pub fn parse_script(code: &str) -> ScriptResult {
    use soyuz_script::ScriptEngine;

    let engine = ScriptEngine::new();
    match engine.compile(code) {
        Ok(()) => ScriptResult {
            success: true,
            error_message: None,
            error_line: None,
        },
        Err(e) => {
            let error_str = e.to_string();
            // Try to extract line number from error message
            let line = extract_line_number(&error_str);
            ScriptResult {
                success: false,
                error_message: Some(error_str),
                error_line: line,
            }
        }
    }
}

fn extract_line_number(error: &str) -> Option<u32> {
    // Rhai errors often contain "line X" or "(line X)"
    if let Some(idx) = error.find("line ") {
        let rest = &error[idx + 5..];
        let num_str: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        num_str.parse().ok()
    } else {
        None
    }
}
