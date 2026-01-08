//! API discovery tools for the MCP server
//!
//! Provides tools for discovering available primitives, operations, transforms,
//! and modifiers in the Soyuz scripting API.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A function signature with documentation
#[derive(Debug, Clone, Serialize)]
pub struct FunctionInfo {
    /// Function name
    pub name: &'static str,
    /// Function signature (e.g., "sphere(radius: f32)")
    pub signature: &'static str,
    /// Brief description
    pub description: &'static str,
    /// Example usage
    pub example: &'static str,
}

/// Request for getting detailed documentation for a function
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetDocsRequest {
    /// Name of the function to get documentation for
    pub function_name: String,
}

/// Get all available primitive shapes
#[allow(clippy::too_many_lines)]
pub fn list_primitives() -> Vec<FunctionInfo> {
    vec![
        FunctionInfo {
            name: "sphere",
            signature: "sphere(radius: f64) -> Sdf",
            description: "Creates a sphere centered at origin",
            example: "sphere(0.5)",
        },
        FunctionInfo {
            name: "cube",
            signature: "cube(size: f64) -> Sdf",
            description: "Creates a cube centered at origin",
            example: "cube(1.0)",
        },
        FunctionInfo {
            name: "box3",
            signature: "box3(x: f64, y: f64, z: f64) -> Sdf",
            description: "Creates a box with different dimensions per axis",
            example: "box3(1.0, 0.5, 0.3)",
        },
        FunctionInfo {
            name: "rounded_box",
            signature: "rounded_box(x: f64, y: f64, z: f64, radius: f64) -> Sdf",
            description: "Creates a box with rounded edges",
            example: "rounded_box(1.0, 0.5, 0.3, 0.05)",
        },
        FunctionInfo {
            name: "cylinder",
            signature: "cylinder(radius: f64, height: f64) -> Sdf",
            description: "Creates a cylinder along the Y axis",
            example: "cylinder(0.3, 1.0)",
        },
        FunctionInfo {
            name: "capsule",
            signature: "capsule(radius: f64, height: f64) -> Sdf",
            description: "Creates a capsule (cylinder with hemispherical caps)",
            example: "capsule(0.2, 1.0)",
        },
        FunctionInfo {
            name: "torus",
            signature: "torus(major_radius: f64, minor_radius: f64) -> Sdf",
            description: "Creates a torus (donut shape) in the XZ plane",
            example: "torus(0.5, 0.1)",
        },
        FunctionInfo {
            name: "cone",
            signature: "cone(radius: f64, height: f64) -> Sdf",
            description: "Creates a cone with tip at origin pointing up",
            example: "cone(0.3, 1.0)",
        },
        FunctionInfo {
            name: "plane",
            signature: "plane(nx: f64, ny: f64, nz: f64, offset: f64) -> Sdf",
            description: "Creates an infinite plane with given normal and offset",
            example: "plane(0.0, 1.0, 0.0, 0.0)",
        },
        FunctionInfo {
            name: "ground_plane",
            signature: "ground_plane() -> Sdf",
            description: "Creates a horizontal ground plane at Y=0",
            example: "ground_plane()",
        },
        FunctionInfo {
            name: "ellipsoid",
            signature: "ellipsoid(rx: f64, ry: f64, rz: f64) -> Sdf",
            description: "Creates an ellipsoid with different radii per axis",
            example: "ellipsoid(0.5, 0.3, 0.4)",
        },
        FunctionInfo {
            name: "octahedron",
            signature: "octahedron(size: f64) -> Sdf",
            description: "Creates a regular octahedron",
            example: "octahedron(0.5)",
        },
        FunctionInfo {
            name: "hex_prism",
            signature: "hex_prism(radius: f64, height: f64) -> Sdf",
            description: "Creates a hexagonal prism",
            example: "hex_prism(0.3, 0.5)",
        },
        FunctionInfo {
            name: "tri_prism",
            signature: "tri_prism(width: f64, height: f64) -> Sdf",
            description: "Creates a triangular prism",
            example: "tri_prism(0.5, 0.3)",
        },
        FunctionInfo {
            name: "pyramid",
            signature: "pyramid(height: f64) -> Sdf",
            description: "Creates a square-based pyramid",
            example: "pyramid(1.0)",
        },
        FunctionInfo {
            name: "link",
            signature: "link(length: f64, major_radius: f64, minor_radius: f64) -> Sdf",
            description: "Creates a chain link shape",
            example: "link(0.3, 0.2, 0.05)",
        },
        FunctionInfo {
            name: "extrude_circle",
            signature: "extrude_circle(radius: f64, depth: f64) -> Sdf",
            description: "Extrudes a 2D circle into 3D",
            example: "extrude_circle(0.3, 0.5)",
        },
        FunctionInfo {
            name: "extrude_rect",
            signature: "extrude_rect(width: f64, height: f64, depth: f64) -> Sdf",
            description: "Extrudes a 2D rectangle into 3D",
            example: "extrude_rect(0.5, 0.3, 0.2)",
        },
        FunctionInfo {
            name: "revolve_circle",
            signature: "revolve_circle(radius: f64, offset: f64) -> Sdf",
            description: "Revolves a 2D circle around the Y axis",
            example: "revolve_circle(0.1, 0.5)",
        },
        FunctionInfo {
            name: "revolve_rect",
            signature: "revolve_rect(width: f64, height: f64, offset: f64) -> Sdf",
            description: "Revolves a 2D rectangle around the Y axis",
            example: "revolve_rect(0.2, 0.1, 0.4)",
        },
    ]
}

/// Get all available boolean operations
pub fn list_operations() -> Vec<FunctionInfo> {
    vec![
        FunctionInfo {
            name: "union",
            signature: "sdf.union(other: Sdf) -> Sdf",
            description: "Combines two shapes (logical OR)",
            example: "sphere(0.5).union(cube(0.4))",
        },
        FunctionInfo {
            name: "subtract",
            signature: "sdf.subtract(other: Sdf) -> Sdf",
            description: "Subtracts one shape from another (carves out)",
            example: "sphere(0.5).subtract(cube(0.4))",
        },
        FunctionInfo {
            name: "intersect",
            signature: "sdf.intersect(other: Sdf) -> Sdf",
            description: "Keeps only the overlapping region (logical AND)",
            example: "sphere(0.5).intersect(cube(0.4))",
        },
        FunctionInfo {
            name: "smooth_union",
            signature: "sdf.smooth_union(other: Sdf, k: f64) -> Sdf",
            description: "Blends two shapes together smoothly. k controls blend radius.",
            example: "sphere(0.5).smooth_union(cube(0.4), 0.1)",
        },
        FunctionInfo {
            name: "smooth_subtract",
            signature: "sdf.smooth_subtract(other: Sdf, k: f64) -> Sdf",
            description: "Subtracts with smooth blending at edges",
            example: "sphere(0.5).smooth_subtract(cube(0.4), 0.05)",
        },
        FunctionInfo {
            name: "smooth_intersect",
            signature: "sdf.smooth_intersect(other: Sdf, k: f64) -> Sdf",
            description: "Intersects with smooth blending at edges",
            example: "sphere(0.5).smooth_intersect(cube(0.4), 0.05)",
        },
        FunctionInfo {
            name: "xor",
            signature: "sdf.xor(other: Sdf) -> Sdf",
            description: "Keeps non-overlapping regions (exclusive OR)",
            example: "sphere(0.5).xor(cube(0.4))",
        },
    ]
}

/// Get all available transforms
pub fn list_transforms() -> Vec<FunctionInfo> {
    vec![
        FunctionInfo {
            name: "translate",
            signature: "sdf.translate(x: f64, y: f64, z: f64) -> Sdf",
            description: "Moves the shape by the given offset",
            example: "sphere(0.5).translate(1.0, 0.0, 0.0)",
        },
        FunctionInfo {
            name: "translate_x",
            signature: "sdf.translate_x(x: f64) -> Sdf",
            description: "Moves the shape along the X axis",
            example: "sphere(0.5).translate_x(1.0)",
        },
        FunctionInfo {
            name: "translate_y",
            signature: "sdf.translate_y(y: f64) -> Sdf",
            description: "Moves the shape along the Y axis",
            example: "sphere(0.5).translate_y(1.0)",
        },
        FunctionInfo {
            name: "translate_z",
            signature: "sdf.translate_z(z: f64) -> Sdf",
            description: "Moves the shape along the Z axis",
            example: "sphere(0.5).translate_z(1.0)",
        },
        FunctionInfo {
            name: "rotate_x",
            signature: "sdf.rotate_x(angle: f64) -> Sdf",
            description: "Rotates around the X axis (angle in radians)",
            example: "cube(0.5).rotate_x(deg(45.0))",
        },
        FunctionInfo {
            name: "rotate_y",
            signature: "sdf.rotate_y(angle: f64) -> Sdf",
            description: "Rotates around the Y axis (angle in radians)",
            example: "cube(0.5).rotate_y(deg(45.0))",
        },
        FunctionInfo {
            name: "rotate_z",
            signature: "sdf.rotate_z(angle: f64) -> Sdf",
            description: "Rotates around the Z axis (angle in radians)",
            example: "cube(0.5).rotate_z(deg(45.0))",
        },
        FunctionInfo {
            name: "scale",
            signature: "sdf.scale(factor: f64) -> Sdf",
            description: "Uniformly scales the shape",
            example: "sphere(0.5).scale(2.0)",
        },
        FunctionInfo {
            name: "mirror_x",
            signature: "sdf.mirror_x() -> Sdf",
            description: "Mirrors the shape across the YZ plane",
            example: "sphere(0.5).translate_x(0.5).mirror_x()",
        },
        FunctionInfo {
            name: "mirror_y",
            signature: "sdf.mirror_y() -> Sdf",
            description: "Mirrors the shape across the XZ plane",
            example: "sphere(0.5).translate_y(0.5).mirror_y()",
        },
        FunctionInfo {
            name: "mirror_z",
            signature: "sdf.mirror_z() -> Sdf",
            description: "Mirrors the shape across the XY plane",
            example: "sphere(0.5).translate_z(0.5).mirror_z()",
        },
        FunctionInfo {
            name: "symmetry_x",
            signature: "sdf.symmetry_x() -> Sdf",
            description: "Creates X-axis symmetry (copies shape to both sides)",
            example: "sphere(0.5).translate_x(0.5).symmetry_x()",
        },
        FunctionInfo {
            name: "symmetry_y",
            signature: "sdf.symmetry_y() -> Sdf",
            description: "Creates Y-axis symmetry",
            example: "sphere(0.5).translate_y(0.5).symmetry_y()",
        },
        FunctionInfo {
            name: "symmetry_z",
            signature: "sdf.symmetry_z() -> Sdf",
            description: "Creates Z-axis symmetry",
            example: "sphere(0.5).translate_z(0.5).symmetry_z()",
        },
    ]
}

/// Get all available modifiers
pub fn list_modifiers() -> Vec<FunctionInfo> {
    vec![
        FunctionInfo {
            name: "shell",
            signature: "sdf.shell(thickness: f64) -> Sdf",
            description: "Hollows out the shape, keeping only the surface",
            example: "sphere(0.5).shell(0.05)",
        },
        FunctionInfo {
            name: "hollow",
            signature: "sdf.hollow(thickness: f64) -> Sdf",
            description: "Alias for shell - hollows out the shape",
            example: "sphere(0.5).hollow(0.05)",
        },
        FunctionInfo {
            name: "round",
            signature: "sdf.round(radius: f64) -> Sdf",
            description: "Rounds the edges of the shape",
            example: "cube(0.5).round(0.05)",
        },
        FunctionInfo {
            name: "onion",
            signature: "sdf.onion(thickness: f64) -> Sdf",
            description: "Creates concentric shell layers",
            example: "sphere(0.5).onion(0.05)",
        },
        FunctionInfo {
            name: "elongate",
            signature: "sdf.elongate(x: f64, y: f64, z: f64) -> Sdf",
            description: "Stretches the shape along each axis",
            example: "sphere(0.3).elongate(0.2, 0.0, 0.0)",
        },
        FunctionInfo {
            name: "twist",
            signature: "sdf.twist(amount: f64) -> Sdf",
            description: "Twists the shape around the Y axis",
            example: "box3(0.5, 1.0, 0.1).twist(2.0)",
        },
        FunctionInfo {
            name: "bend",
            signature: "sdf.bend(amount: f64) -> Sdf",
            description: "Bends the shape along the X axis",
            example: "box3(1.0, 0.2, 0.2).bend(0.5)",
        },
        FunctionInfo {
            name: "displace",
            signature: "sdf.displace(amount: f64, frequency: f64) -> Sdf",
            description: "Adds noise-based displacement to the surface",
            example: "sphere(0.5).displace(0.05, 10.0)",
        },
        FunctionInfo {
            name: "repeat",
            signature: "sdf.repeat(sx: f64, sy: f64, sz: f64) -> Sdf",
            description: "Infinitely repeats the shape in a grid pattern",
            example: "sphere(0.2).repeat(1.0, 1.0, 1.0)",
        },
        FunctionInfo {
            name: "repeat_limited",
            signature: "sdf.repeat_limited(sx: f64, sy: f64, sz: f64, cx: f64, cy: f64, cz: f64) -> Sdf",
            description: "Repeats the shape in a limited grid",
            example: "sphere(0.2).repeat_limited(0.5, 0.5, 0.5, 3.0, 3.0, 3.0)",
        },
        FunctionInfo {
            name: "repeat_polar",
            signature: "sdf.repeat_polar(count: i64) -> Sdf",
            description: "Repeats the shape radially around the Y axis",
            example: "sphere(0.2).translate_x(0.5).repeat_polar(6)",
        },
    ]
}

/// Get all available environment functions
pub fn list_environment() -> Vec<FunctionInfo> {
    vec![
        FunctionInfo {
            name: "set_sun_direction",
            signature: "set_sun_direction(x: f64, y: f64, z: f64)",
            description: "Sets the direction of the sun light",
            example: "set_sun_direction(1.0, 1.0, 0.5)",
        },
        FunctionInfo {
            name: "set_sun_color",
            signature: "set_sun_color(r: f64, g: f64, b: f64)",
            description: "Sets the color of the sun light",
            example: "set_sun_color(1.0, 0.9, 0.8)",
        },
        FunctionInfo {
            name: "set_ambient_color",
            signature: "set_ambient_color(r: f64, g: f64, b: f64)",
            description: "Sets the ambient light color",
            example: "set_ambient_color(0.1, 0.1, 0.15)",
        },
        FunctionInfo {
            name: "set_material_color",
            signature: "set_material_color(r: f64, g: f64, b: f64)",
            description: "Sets the base material color",
            example: "set_material_color(0.8, 0.2, 0.1)",
        },
        FunctionInfo {
            name: "set_background_color",
            signature: "set_background_color(r: f64, g: f64, b: f64)",
            description: "Sets the background color",
            example: "set_background_color(0.1, 0.1, 0.1)",
        },
        FunctionInfo {
            name: "set_fog_density",
            signature: "set_fog_density(density: f64)",
            description: "Sets the fog density (0.0 = no fog)",
            example: "set_fog_density(0.02)",
        },
        FunctionInfo {
            name: "env_default",
            signature: "env_default()",
            description: "Resets to default environment preset",
            example: "env_default()",
        },
        FunctionInfo {
            name: "env_studio",
            signature: "env_studio()",
            description: "Clean studio lighting preset",
            example: "env_studio()",
        },
        FunctionInfo {
            name: "env_sunset",
            signature: "env_sunset()",
            description: "Warm sunset lighting preset",
            example: "env_sunset()",
        },
        FunctionInfo {
            name: "env_night",
            signature: "env_night()",
            description: "Cool night/moonlight preset",
            example: "env_night()",
        },
    ]
}

/// Get all available math helpers
pub fn list_math() -> Vec<FunctionInfo> {
    vec![
        FunctionInfo {
            name: "PI",
            signature: "PI() -> f64",
            description: "Returns the value of pi (3.14159...)",
            example: "rotate_y(PI() / 4.0)",
        },
        FunctionInfo {
            name: "TAU",
            signature: "TAU() -> f64",
            description: "Returns 2*pi (6.28318...)",
            example: "rotate_y(TAU() / 8.0)",
        },
        FunctionInfo {
            name: "deg",
            signature: "deg(degrees: f64) -> f64",
            description: "Converts degrees to radians",
            example: "rotate_x(deg(45.0))",
        },
        FunctionInfo {
            name: "rad",
            signature: "rad(radians: f64) -> f64",
            description: "Converts radians to degrees",
            example: "let angle_deg = rad(PI() / 4.0)",
        },
    ]
}

/// Get documentation for a specific function
pub fn get_docs(function_name: &str) -> Option<FunctionInfo> {
    // Search all categories
    let all_functions: Vec<FunctionInfo> = list_primitives()
        .into_iter()
        .chain(list_operations())
        .chain(list_transforms())
        .chain(list_modifiers())
        .chain(list_environment())
        .chain(list_math())
        .collect();

    all_functions
        .into_iter()
        .find(|f| f.name.eq_ignore_ascii_case(function_name))
}
