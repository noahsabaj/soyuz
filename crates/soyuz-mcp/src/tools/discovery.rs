//! API discovery tools for the MCP server
//!
//! Discovery is backed by the shared Soyuz API manifest so MCP responses,
//! website documentation, and runtime compatibility aliases stay aligned.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use soyuz_script::api_generated::API_MANIFEST_JSON;
use std::sync::OnceLock;

/// A function signature with documentation
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FunctionInfo {
    /// Function name
    pub name: String,
    /// Function signature (e.g., "sphere(radius: f64)")
    pub signature: String,
    /// Brief description
    pub description: String,
    /// Example usage
    pub example: String,
    /// API kind: constructor, method, function, preset, or alias
    pub kind: String,
    /// Canonical function for aliases
    #[serde(rename = "aliasOf", skip_serializing_if = "Option::is_none")]
    pub alias_of: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApiManifest {
    categories: Vec<ApiCategory>,
}

#[derive(Debug, Deserialize)]
struct ApiCategory {
    id: String,
    #[allow(dead_code)]
    title: String,
    functions: Vec<FunctionInfo>,
}

/// Request for getting detailed documentation for a function
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetDocsRequest {
    /// Name of the function to get documentation for
    pub function_name: String,
}

fn api_manifest() -> &'static ApiManifest {
    static MANIFEST: OnceLock<ApiManifest> = OnceLock::new();
    MANIFEST.get_or_init(|| match serde_json::from_str(API_MANIFEST_JSON) {
        Ok(manifest) => manifest,
        Err(err) => panic!("SOYUZ_API.json should be valid: {err}"),
    })
}

fn list_category(id: &str) -> Vec<FunctionInfo> {
    api_manifest()
        .categories
        .iter()
        .find(|category| category.id == id)
        .map(|category| category.functions.clone())
        .unwrap_or_default()
}

/// Get all available primitive shapes
pub fn list_primitives() -> Vec<FunctionInfo> {
    list_category("primitives")
}

/// Get all available boolean operations
pub fn list_operations() -> Vec<FunctionInfo> {
    list_category("operations")
}

/// Get all available transforms
pub fn list_transforms() -> Vec<FunctionInfo> {
    list_category("transforms")
}

/// Get all available modifiers
pub fn list_modifiers() -> Vec<FunctionInfo> {
    list_category("modifiers")
}

/// Get all available environment functions
pub fn list_environment() -> Vec<FunctionInfo> {
    list_category("environment")
}

/// Get all available math helpers
pub fn list_math() -> Vec<FunctionInfo> {
    list_category("math")
}

/// Get documentation for a specific function
pub fn get_docs(function_name: &str) -> Option<FunctionInfo> {
    api_manifest()
        .categories
        .iter()
        .flat_map(|category| category.functions.iter())
        .find(|function| function.name == function_name)
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_lists_registered_advanced_shapes() {
        let primitives = list_primitives();
        for name in [
            "pyramid",
            "link",
            "extrude_circle",
            "extrude_rect",
            "extrude_rounded_rect",
            "revolve_circle",
            "revolve_rect",
        ] {
            assert!(
                primitives.iter().any(|function| function.name == name),
                "missing primitive {name}"
            );
        }
    }

    #[test]
    fn manifest_lists_registered_environment_api_and_aliases() {
        let environment = list_environment();
        for name in [
            "set_sun_intensity",
            "set_ambient_intensity",
            "set_material_color_hex",
            "set_sky_horizon",
            "set_sky_zenith",
            "set_background_color",
            "env_default",
            "env_daylight",
            "env_clay",
        ] {
            assert!(
                environment.iter().any(|function| function.name == name),
                "missing environment function {name}"
            );
        }
    }

    #[test]
    fn get_docs_searches_all_manifest_categories() {
        let Some(displace) = get_docs("displace") else {
            panic!("displace docs should exist");
        };
        assert_eq!(displace.kind, "method");

        let Some(env_default) = get_docs("env_default") else {
            panic!("env_default docs should exist");
        };
        assert_eq!(
            env_default.alias_of.as_deref(),
            Some("Environment::default")
        );
    }
}
