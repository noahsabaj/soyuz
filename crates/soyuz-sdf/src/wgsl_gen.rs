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

use crate::{ExtrudeProfile, RevolveProfile, SdfOp};

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
            SdfOp::Pyramid { height } => {
                let var = self.next_var();
                writeln!(
                    code,
                    "    let {} = sd_pyramid({}, {:.6});",
                    var, pos_var, height
                )
                .unwrap();
                var
            }
            SdfOp::Link {
                length,
                major_radius,
                minor_radius,
            } => {
                let var = self.next_var();
                writeln!(
                    code,
                    "    let {} = sd_link({}, {:.6}, {:.6}, {:.6});",
                    var, pos_var, length, major_radius, minor_radius
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
            SdfOp::Xor { a, b } => {
                let a_var = self.generate_op(a, pos_var, code);
                let b_var = self.generate_op(b, pos_var, code);
                let var = self.next_var();
                writeln!(code, "    let {} = op_xor({}, {});", var, a_var, b_var).unwrap();
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
                // IQ elongation: evaluate the inner SDF at the clamped position and
                // ADD the negative core term to the DISTANCE (not the position).
                // MUST match cpu_eval.rs Elongate.
                let q_var = self.next_pos_var();
                writeln!(
                    code,
                    "    let {} = abs({}) - vec3<f32>({:.6}, {:.6}, {:.6});",
                    q_var, pos_var, h[0], h[1], h[2]
                )
                .unwrap();
                let clamped = self.next_pos_var();
                writeln!(
                    code,
                    "    let {} = max({}, vec3<f32>(0.0));",
                    clamped, q_var
                )
                .unwrap();
                let inner_var = self.generate_op(inner, &clamped, code);
                let var = self.next_var();
                writeln!(
                    code,
                    "    let {} = {} + min(max({}.x, max({}.y, {}.z)), 0.0);",
                    var, inner_var, q_var, q_var, q_var
                )
                .unwrap();
                var
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
                // Transform the point by R(-angle) so the shape rotates by +angle
                // (CCW, right-hand rule). Coefficients are parenthesized so a
                // negative value can't form "--". MUST match cpu_eval.rs rotations.
                let c = angle.cos();
                let s = angle.sin();
                writeln!(
                    code,
                    "    let {} = vec3<f32>({}.x, ({:.8}) * {}.y + ({:.8}) * {}.z, ({:.8}) * {}.y + ({:.8}) * {}.z);",
                    new_pos, pos_var, c, pos_var, s, pos_var, -s, pos_var, c, pos_var
                ).unwrap();
                self.generate_op(inner, &new_pos, code)
            }
            SdfOp::RotateY { inner, angle } => {
                let new_pos = self.next_pos_var();
                let c = angle.cos();
                let s = angle.sin();
                writeln!(
                    code,
                    "    let {} = vec3<f32>(({:.8}) * {}.x + ({:.8}) * {}.z, {}.y, ({:.8}) * {}.x + ({:.8}) * {}.z);",
                    new_pos, c, pos_var, -s, pos_var, pos_var, s, pos_var, c, pos_var
                ).unwrap();
                self.generate_op(inner, &new_pos, code)
            }
            SdfOp::RotateZ { inner, angle } => {
                let new_pos = self.next_pos_var();
                let c = angle.cos();
                let s = angle.sin();
                writeln!(
                    code,
                    "    let {} = vec3<f32>(({:.8}) * {}.x + ({:.8}) * {}.y, ({:.8}) * {}.x + ({:.8}) * {}.y, {}.z);",
                    new_pos, c, pos_var, s, pos_var, -s, pos_var, c, pos_var, pos_var
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
            SdfOp::Displacement {
                inner,
                amount,
                frequency,
            } => {
                let inner_var = self.generate_op(inner, pos_var, code);
                let var = self.next_var();
                writeln!(
                    code,
                    "    let {} = op_displacement({}, {}, {:.6}, {:.6});",
                    var, inner_var, pos_var, amount, frequency
                )
                .unwrap();
                var
            }

            // 2D-to-3D Operations
            SdfOp::Extrude { profile, depth } => {
                let var = self.next_var();
                match profile {
                    ExtrudeProfile::Circle { radius } => {
                        writeln!(
                            code,
                            "    let {} = op_extrude(sd_circle_2d({}.xy, {:.6}), {}.z, {:.6});",
                            var, pos_var, radius, pos_var, depth
                        )
                        .unwrap();
                    }
                    ExtrudeProfile::Rectangle { width, height } => {
                        writeln!(
                            code,
                            "    let {} = op_extrude(sd_box_2d({}.xy, vec2<f32>({:.6}, {:.6})), {}.z, {:.6});",
                            var, pos_var, width / 2.0, height / 2.0, pos_var, depth
                        )
                        .unwrap();
                    }
                    ExtrudeProfile::RoundedRectangle {
                        width,
                        height,
                        radius,
                    } => {
                        writeln!(
                            code,
                            "    let {} = op_extrude(sd_rounded_box_2d({}.xy, vec2<f32>({:.6}, {:.6}), {:.6}), {}.z, {:.6});",
                            var, pos_var, width / 2.0, height / 2.0, radius, pos_var, depth
                        )
                        .unwrap();
                    }
                }
                var
            }
            SdfOp::Revolve { profile, offset } => {
                let var = self.next_var();
                let p2d = self.next_pos_var();
                writeln!(
                    code,
                    "    let {} = op_revolve({}, {:.6});",
                    p2d, pos_var, offset
                )
                .unwrap();
                match profile {
                    RevolveProfile::Circle { radius } => {
                        writeln!(
                            code,
                            "    let {} = sd_circle_2d({}, {:.6});",
                            var, p2d, radius
                        )
                        .unwrap();
                    }
                    RevolveProfile::Rectangle { width, height } => {
                        writeln!(
                            code,
                            "    let {} = sd_box_2d({}, vec2<f32>({:.6}, {:.6}));",
                            var,
                            p2d,
                            width / 2.0,
                            height / 2.0
                        )
                        .unwrap();
                    }
                }
                var
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

    /// Build a full shader exercising every op whose codegen or WGSL body was
    /// changed and confirm it parses and type-checks with naga (no GPU needed).
    #[test]
    fn generated_shader_validates_with_naga() {
        let sphere = || Arc::new(SdfOp::Sphere { radius: 0.5 });
        let tree = SdfOp::Union {
            a: Arc::new(SdfOp::Union {
                a: Arc::new(SdfOp::Cone {
                    radius: 0.5,
                    height: 1.0,
                }),
                b: Arc::new(SdfOp::Ellipsoid {
                    radii: [0.6, 0.4, 0.5],
                }),
            }),
            b: Arc::new(SdfOp::Union {
                a: Arc::new(SdfOp::RotateZ {
                    inner: Arc::new(SdfOp::RotateY {
                        inner: Arc::new(SdfOp::RotateX {
                            inner: Arc::new(SdfOp::Elongate {
                                inner: sphere(),
                                h: [0.2, 0.0, 0.1],
                            }),
                            angle: 0.5,
                        }),
                        angle: 0.5,
                    }),
                    angle: 0.5,
                }),
                b: Arc::new(SdfOp::Union {
                    a: Arc::new(SdfOp::RepeatInfinite {
                        inner: sphere(),
                        spacing: [1.0, 0.0, 1.0],
                    }),
                    b: Arc::new(SdfOp::HexPrism {
                        half_height: 0.3,
                        radius: 0.4,
                    }),
                }),
            }),
        };

        let shader = crate::build_shader(&tree);
        let module = naga::front::wgsl::parse_str(&shader).unwrap_or_else(|e| {
            panic!(
                "generated WGSL failed to parse:\n{}",
                e.emit_to_string(&shader)
            )
        });
        let mut validator = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        );
        // unwrap_used is allowed in this file; a failure prints the validation error.
        validator.validate(&module).unwrap();
    }
}
