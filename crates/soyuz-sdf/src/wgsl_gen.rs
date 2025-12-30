//! SDF to WGSL code generator
//!
//! Converts Rust SDF types to WGSL shader code for GPU raymarching.

// String writing is infallible, so .unwrap() is safe here
// Format args inlining is not always more readable for shader code generation
// The generate_op function is large because each SDF operation is a separate case
#![allow(clippy::unwrap_used)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::too_many_lines)]

use std::fmt::Write;

use crate::SdfOp;

/// Generate WGSL code for an SDF operation tree
pub struct WgslGenerator {
    var_counter: usize,
}

impl WgslGenerator {
    pub fn new() -> Self {
        Self { var_counter: 0 }
    }

    fn next_var(&mut self) -> String {
        let var = format!("d{}", self.var_counter);
        self.var_counter += 1;
        var
    }

    fn next_pos_var(&mut self) -> String {
        let var = format!("p{}", self.var_counter);
        self.var_counter += 1;
        var
    }

    /// Generate the complete `scene_sdf` function
    pub fn generate(&mut self, sdf: &SdfOp) -> String {
        self.var_counter = 0;
        let mut code = String::new();

        writeln!(code, "fn scene_sdf(p: vec3<f32>) -> f32 {{").unwrap();

        let result = self.generate_op(sdf, "p", &mut code);

        writeln!(code, "    return {};", result).unwrap();
        writeln!(code, "}}").unwrap();

        code
    }

    /// Generate code for a single SDF operation, returns the variable name containing the result
    fn generate_op(&mut self, op: &SdfOp, pos_var: &str, code: &mut String) -> String {
        match op {
            // Primitives
            SdfOp::Sphere { radius } => {
                let var = self.next_var();
                writeln!(
                    code,
                    "    let {} = sd_sphere({}, {:.6});",
                    var, pos_var, radius
                )
                .unwrap();
                var
            }
            SdfOp::Box { half_extents } => {
                let var = self.next_var();
                writeln!(
                    code,
                    "    let {} = sd_box({}, vec3<f32>({:.6}, {:.6}, {:.6}));",
                    var, pos_var, half_extents[0], half_extents[1], half_extents[2]
                )
                .unwrap();
                var
            }
            SdfOp::RoundedBox {
                half_extents,
                radius,
            } => {
                let var = self.next_var();
                writeln!(
                    code,
                    "    let {} = sd_rounded_box({}, vec3<f32>({:.6}, {:.6}, {:.6}), {:.6});",
                    var, pos_var, half_extents[0], half_extents[1], half_extents[2], radius
                )
                .unwrap();
                var
            }
            SdfOp::Cylinder {
                radius,
                half_height,
            } => {
                let var = self.next_var();
                writeln!(
                    code,
                    "    let {} = sd_cylinder({}, {:.6}, {:.6});",
                    var, pos_var, radius, half_height
                )
                .unwrap();
                var
            }
            SdfOp::Capsule {
                radius,
                half_height,
            } => {
                let var = self.next_var();
                writeln!(
                    code,
                    "    let {} = sd_capsule({}, {:.6}, {:.6});",
                    var, pos_var, radius, half_height
                )
                .unwrap();
                var
            }
            SdfOp::Torus {
                major_radius,
                minor_radius,
            } => {
                let var = self.next_var();
                writeln!(
                    code,
                    "    let {} = sd_torus({}, vec2<f32>({:.6}, {:.6}));",
                    var, pos_var, major_radius, minor_radius
                )
                .unwrap();
                var
            }
            SdfOp::Cone { radius, height } => {
                let var = self.next_var();
                writeln!(
                    code,
                    "    let {} = sd_cone({}, {:.6}, {:.6});",
                    var, pos_var, radius, height
                )
                .unwrap();
                var
            }
            SdfOp::Plane { normal, offset } => {
                let var = self.next_var();
                writeln!(
                    code,
                    "    let {} = sd_plane({}, vec3<f32>({:.6}, {:.6}, {:.6}), {:.6});",
                    var, pos_var, normal[0], normal[1], normal[2], offset
                )
                .unwrap();
                var
            }
            SdfOp::Ellipsoid { radii } => {
                let var = self.next_var();
                writeln!(
                    code,
                    "    let {} = sd_ellipsoid({}, vec3<f32>({:.6}, {:.6}, {:.6}));",
                    var, pos_var, radii[0], radii[1], radii[2]
                )
                .unwrap();
                var
            }
            SdfOp::Octahedron { size } => {
                let var = self.next_var();
                writeln!(
                    code,
                    "    let {} = sd_octahedron({}, {:.6});",
                    var, pos_var, size
                )
                .unwrap();
                var
            }
            SdfOp::HexPrism {
                half_height,
                radius,
            } => {
                let var = self.next_var();
                writeln!(
                    code,
                    "    let {} = sd_hex_prism({}, vec2<f32>({:.6}, {:.6}));",
                    var, pos_var, radius, half_height
                )
                .unwrap();
                var
            }
            SdfOp::TriPrism { size } => {
                let var = self.next_var();
                writeln!(
                    code,
                    "    let {} = sd_tri_prism({}, vec2<f32>({:.6}, {:.6}));",
                    var, pos_var, size[0], size[1]
                )
                .unwrap();
                var
            }

            // Boolean operations
            SdfOp::Union { a, b } => {
                let a_var = self.generate_op(a, pos_var, code);
                let b_var = self.generate_op(b, pos_var, code);
                let var = self.next_var();
                writeln!(code, "    let {} = op_union({}, {});", var, a_var, b_var).unwrap();
                var
            }
            SdfOp::Subtract { a, b } => {
                let a_var = self.generate_op(a, pos_var, code);
                let b_var = self.generate_op(b, pos_var, code);
                let var = self.next_var();
                writeln!(code, "    let {} = op_subtract({}, {});", var, a_var, b_var).unwrap();
                var
            }
            SdfOp::Intersect { a, b } => {
                let a_var = self.generate_op(a, pos_var, code);
                let b_var = self.generate_op(b, pos_var, code);
                let var = self.next_var();
                writeln!(
                    code,
                    "    let {} = op_intersect({}, {});",
                    var, a_var, b_var
                )
                .unwrap();
                var
            }
            SdfOp::SmoothUnion { a, b, k } => {
                let a_var = self.generate_op(a, pos_var, code);
                let b_var = self.generate_op(b, pos_var, code);
                let var = self.next_var();
                writeln!(
                    code,
                    "    let {} = op_smooth_union({}, {}, {:.6});",
                    var, a_var, b_var, k
                )
                .unwrap();
                var
            }
            SdfOp::SmoothSubtract { a, b, k } => {
                let a_var = self.generate_op(a, pos_var, code);
                let b_var = self.generate_op(b, pos_var, code);
                let var = self.next_var();
                writeln!(
                    code,
                    "    let {} = op_smooth_subtract({}, {}, {:.6});",
                    var, a_var, b_var, k
                )
                .unwrap();
                var
            }
            SdfOp::SmoothIntersect { a, b, k } => {
                let a_var = self.generate_op(a, pos_var, code);
                let b_var = self.generate_op(b, pos_var, code);
                let var = self.next_var();
                writeln!(
                    code,
                    "    let {} = op_smooth_intersect({}, {}, {:.6});",
                    var, a_var, b_var, k
                )
                .unwrap();
                var
            }

            // Modifiers
            SdfOp::Shell { inner, thickness } => {
                let inner_var = self.generate_op(inner, pos_var, code);
                let var = self.next_var();
                writeln!(
                    code,
                    "    let {} = op_shell({}, {:.6});",
                    var, inner_var, thickness
                )
                .unwrap();
                var
            }
            SdfOp::Round { inner, radius } => {
                let inner_var = self.generate_op(inner, pos_var, code);
                let var = self.next_var();
                writeln!(
                    code,
                    "    let {} = op_round({}, {:.6});",
                    var, inner_var, radius
                )
                .unwrap();
                var
            }
            SdfOp::Onion { inner, thickness } => {
                let inner_var = self.generate_op(inner, pos_var, code);
                let var = self.next_var();
                writeln!(
                    code,
                    "    let {} = op_onion({}, {:.6});",
                    var, inner_var, thickness
                )
                .unwrap();
                var
            }
            SdfOp::Elongate { inner, h } => {
                let new_pos = self.next_pos_var();
                writeln!(
                    code,
                    "    let {} = op_elongate({}, vec3<f32>({:.6}, {:.6}, {:.6}));",
                    new_pos, pos_var, h[0], h[1], h[2]
                )
                .unwrap();
                self.generate_op(inner, &new_pos, code)
            }

            // Transforms
            SdfOp::Translate { inner, offset } => {
                let new_pos = self.next_pos_var();
                writeln!(
                    code,
                    "    let {} = op_translate({}, vec3<f32>({:.6}, {:.6}, {:.6}));",
                    new_pos, pos_var, offset[0], offset[1], offset[2]
                )
                .unwrap();
                self.generate_op(inner, &new_pos, code)
            }
            SdfOp::RotateX { inner, angle } => {
                let new_pos = self.next_pos_var();
                // Pre-compute sin/cos at code generation time for better GPU performance
                let c = angle.cos();
                let s = angle.sin();
                writeln!(
                    code,
                    "    let {} = vec3<f32>({}.x, {:.8} * {}.y - {:.8} * {}.z, {:.8} * {}.y + {:.8} * {}.z);",
                    new_pos, pos_var, c, pos_var, s, pos_var, s, pos_var, c, pos_var
                ).unwrap();
                self.generate_op(inner, &new_pos, code)
            }
            SdfOp::RotateY { inner, angle } => {
                let new_pos = self.next_pos_var();
                let c = angle.cos();
                let s = angle.sin();
                writeln!(
                    code,
                    "    let {} = vec3<f32>({:.8} * {}.x + {:.8} * {}.z, {}.y, -{:.8} * {}.x + {:.8} * {}.z);",
                    new_pos, c, pos_var, s, pos_var, pos_var, s, pos_var, c, pos_var
                ).unwrap();
                self.generate_op(inner, &new_pos, code)
            }
            SdfOp::RotateZ { inner, angle } => {
                let new_pos = self.next_pos_var();
                let c = angle.cos();
                let s = angle.sin();
                writeln!(
                    code,
                    "    let {} = vec3<f32>({:.8} * {}.x - {:.8} * {}.y, {:.8} * {}.x + {:.8} * {}.y, {}.z);",
                    new_pos, c, pos_var, s, pos_var, s, pos_var, c, pos_var, pos_var
                ).unwrap();
                self.generate_op(inner, &new_pos, code)
            }
            SdfOp::Scale { inner, factor } => {
                let new_pos = self.next_pos_var();
                writeln!(
                    code,
                    "    let {} = op_scale({}, {:.6});",
                    new_pos, pos_var, factor
                )
                .unwrap();
                let inner_var = self.generate_op(inner, &new_pos, code);
                let var = self.next_var();
                writeln!(code, "    let {} = {} * {:.6};", var, inner_var, factor).unwrap();
                var
            }
            SdfOp::Mirror { inner, axis } => {
                // Simple mirror using abs on the appropriate axis
                let new_pos = self.next_pos_var();
                if axis[0].abs() > 0.5 {
                    writeln!(code, "    let {} = op_symmetry_x({});", new_pos, pos_var).unwrap();
                } else if axis[1].abs() > 0.5 {
                    writeln!(code, "    let {} = op_symmetry_y({});", new_pos, pos_var).unwrap();
                } else {
                    writeln!(code, "    let {} = op_symmetry_z({});", new_pos, pos_var).unwrap();
                }
                self.generate_op(inner, &new_pos, code)
            }
            SdfOp::SymmetryX { inner } => {
                let new_pos = self.next_pos_var();
                writeln!(code, "    let {} = op_symmetry_x({});", new_pos, pos_var).unwrap();
                self.generate_op(inner, &new_pos, code)
            }
            SdfOp::SymmetryY { inner } => {
                let new_pos = self.next_pos_var();
                writeln!(code, "    let {} = op_symmetry_y({});", new_pos, pos_var).unwrap();
                self.generate_op(inner, &new_pos, code)
            }
            SdfOp::SymmetryZ { inner } => {
                let new_pos = self.next_pos_var();
                writeln!(code, "    let {} = op_symmetry_z({});", new_pos, pos_var).unwrap();
                self.generate_op(inner, &new_pos, code)
            }

            // Deformations
            SdfOp::Twist { inner, amount } => {
                let new_pos = self.next_pos_var();
                writeln!(
                    code,
                    "    let {} = op_twist({}, {:.6});",
                    new_pos, pos_var, amount
                )
                .unwrap();
                self.generate_op(inner, &new_pos, code)
            }
            SdfOp::Bend { inner, amount } => {
                let new_pos = self.next_pos_var();
                writeln!(
                    code,
                    "    let {} = op_bend({}, {:.6});",
                    new_pos, pos_var, amount
                )
                .unwrap();
                self.generate_op(inner, &new_pos, code)
            }

            // Repetition
            SdfOp::RepeatInfinite { inner, spacing } => {
                let new_pos = self.next_pos_var();
                writeln!(
                    code,
                    "    let {} = op_repeat({}, vec3<f32>({:.6}, {:.6}, {:.6}));",
                    new_pos, pos_var, spacing[0], spacing[1], spacing[2]
                )
                .unwrap();
                self.generate_op(inner, &new_pos, code)
            }
            SdfOp::RepeatLimited {
                inner,
                spacing,
                count,
            } => {
                let new_pos = self.next_pos_var();
                writeln!(
                    code,
                    "    let {} = op_repeat_limited({}, vec3<f32>({:.6}, {:.6}, {:.6}), vec3<f32>({:.6}, {:.6}, {:.6}));",
                    new_pos, pos_var,
                    spacing[0], spacing[1], spacing[2],
                    count[0], count[1], count[2]
                ).unwrap();
                self.generate_op(inner, &new_pos, code)
            }
            SdfOp::RepeatPolar { inner, count } => {
                let new_pos = self.next_pos_var();
                writeln!(
                    code,
                    "    let {} = op_repeat_polar({}, {:.1});",
                    new_pos, pos_var, *count as f32
                )
                .unwrap();
                self.generate_op(inner, &new_pos, code)
            }
        }
    }
}

impl Default for WgslGenerator {
    fn default() -> Self {
        Self::new()
    }
}

/// Get the base shader code (everything except the `scene_sdf` function)
pub fn get_base_shader() -> &'static str {
    include_str!("shaders/raymarch.wgsl")
}

/// Replace the `scene_sdf` function in the base shader with custom code
pub fn inject_scene_sdf(base_shader: &str, scene_sdf_code: &str) -> String {
    // Find the default scene_sdf function and replace it
    let marker = "// SCENE_SDF_PLACEHOLDER";
    if let Some(pos) = base_shader.find(marker) {
        // Find the end of the default scene_sdf function
        let start = pos;
        // Find the closing brace of the function
        if let Some(func_start) = base_shader[start..].find("fn scene_sdf") {
            let func_start = start + func_start;
            // Count braces to find end of function
            let mut brace_count = 0;
            let mut func_end = func_start;
            let mut found_open = false;

            for (i, c) in base_shader[func_start..].char_indices() {
                if c == '{' {
                    brace_count += 1;
                    found_open = true;
                } else if c == '}' {
                    brace_count -= 1;
                    if found_open && brace_count == 0 {
                        func_end = func_start + i + 1;
                        break;
                    }
                }
            }

            // Replace the function
            let mut result = String::new();
            result.push_str(&base_shader[..start]);
            result.push_str(scene_sdf_code);
            result.push_str(&base_shader[func_end..]);
            return result;
        }
    }

    // If we can't find the marker, just append at the end (fallback)
    format!("{}\n{}", base_shader, scene_sdf_code)
}

/// Build a complete shader from an SDF operation tree
pub fn build_shader(sdf: &SdfOp) -> String {
    let mut generator = WgslGenerator::new();
    let scene_code = generator.generate(sdf);
    let base = get_base_shader();

    // Inject SSOT formulas from soyuz-math
    let with_formulas = inject_ssot_formulas(base);

    // Inject the scene SDF
    inject_scene_sdf(&with_formulas, &scene_code)
}

/// Inject SSOT formulas from soyuz-math into the shader
fn inject_ssot_formulas(shader: &str) -> String {
    let marker = "// SSOT_FORMULAS_PLACEHOLDER";
    let formulas = soyuz_math::get_wgsl_code();

    if let Some(pos) = shader.find(marker) {
        let mut result = String::new();
        result.push_str(&shader[..pos]);
        result.push_str(formulas);
        result.push_str(&shader[pos + marker.len()..]);
        result
    } else {
        // Fallback: just prepend if marker not found
        format!("{}\n{}", formulas, shader)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_simple_sphere() {
        let sdf = SdfOp::Sphere { radius: 1.0 };
        let mut generator = WgslGenerator::new();
        let code = generator.generate(&sdf);
        assert!(code.contains("sd_sphere"));
        assert!(code.contains("1.0"));
    }

    #[test]
    fn test_union() {
        let sdf = SdfOp::Union {
            a: Arc::new(SdfOp::Sphere { radius: 1.0 }),
            b: Arc::new(SdfOp::Box {
                half_extents: [0.5, 0.5, 0.5],
            }),
        };
        let mut generator = WgslGenerator::new();
        let code = generator.generate(&sdf);
        assert!(code.contains("sd_sphere"));
        assert!(code.contains("sd_box"));
        assert!(code.contains("op_union"));
    }

    #[test]
    fn test_transform() {
        let sdf = SdfOp::Translate {
            inner: Arc::new(SdfOp::Sphere { radius: 1.0 }),
            offset: [1.0, 2.0, 3.0],
        };
        let mut generator = WgslGenerator::new();
        let code = generator.generate(&sdf);
        assert!(code.contains("op_translate"));
        assert!(code.contains("1.0"));
        assert!(code.contains("2.0"));
        assert!(code.contains("3.0"));
    }
}
