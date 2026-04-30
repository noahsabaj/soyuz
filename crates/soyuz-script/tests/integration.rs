//! Integration tests for script to mesh to export pipeline

// Tests are allowed to use expect/unwrap for cleaner error messages
#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]
// map_or with closure is clearer than is_some_and for extension checking
#![allow(clippy::unnecessary_map_or)]

use soyuz_core::export::MeshExport;
use soyuz_core::mesh::{MeshConfig, OptimizeConfig, SdfToMesh};
use soyuz_core::prelude::Vec3;
use soyuz_script::{CpuSdf, ScriptEngine, Sdf};
use soyuz_sdf::Environment;
use std::path::Path;

fn assert_vec3_close(actual: [f32; 3], expected: [f32; 3]) {
    for i in 0..3 {
        assert!(
            (actual[i] - expected[i]).abs() < 0.001,
            "component {i} expected {}, got {}",
            expected[i],
            actual[i]
        );
    }
}

#[test]
fn script_to_mesh_pipeline() {
    let script = r#"
        let base = sphere(1.0);
        let hole = cylinder(0.3, 2.0);
        base.subtract(hole)
    "#;

    let engine = ScriptEngine::new();
    let sdf_op = engine
        .eval_to_sdf_op(script)
        .expect("Script should evaluate");

    // Wrap in CpuSdf to get Sdf trait implementation
    let cpu_sdf = CpuSdf::new(sdf_op);

    // Generate mesh with low resolution for fast test
    let config = MeshConfig::default().with_resolution(16);
    let mesh = cpu_sdf.to_mesh(config).expect("Mesh should generate");

    assert!(mesh.vertex_count() > 0, "Mesh should have vertices");
    assert!(mesh.triangle_count() > 0, "Mesh should have triangles");
}

#[test]
fn script_to_mesh_to_export_pipeline() {
    let script = r#"
        // Create a simple shape
        sphere(0.5).smooth_union(cube(0.8), 0.1)
    "#;

    let engine = ScriptEngine::new();
    let sdf_op = engine
        .eval_to_sdf_op(script)
        .expect("Script should evaluate");

    // Wrap and generate mesh
    let cpu_sdf = CpuSdf::new(sdf_op);
    let config = MeshConfig::default().with_resolution(16);
    let mesh = cpu_sdf.to_mesh(config).expect("Mesh should generate");

    assert!(mesh.vertex_count() > 0);
    assert!(mesh.triangle_count() > 0);

    // Export to temp file
    let temp_dir = std::env::temp_dir();
    let temp_path = temp_dir.join("soyuz_test_export.glb");

    mesh.export(&temp_path).expect("Export should succeed");
    assert!(temp_path.exists(), "Exported file should exist");

    // Clean up
    std::fs::remove_file(&temp_path).ok();
}

#[test]
fn all_examples_parse() {
    let engine = ScriptEngine::new();

    // Find examples directory relative to the crate root
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let examples_dir = Path::new(manifest_dir)
        .parent()
        .expect("Should have parent")
        .parent()
        .expect("Should have grandparent")
        .join("examples");

    if !examples_dir.exists() {
        eprintln!(
            "Examples directory not found at {:?}, skipping test",
            examples_dir
        );
        return;
    }

    let mut count = 0;
    for entry in std::fs::read_dir(&examples_dir).expect("Should read examples dir") {
        let path = entry.expect("Should read entry").path();
        if path.extension().map_or(false, |e| e == "rhai") {
            let content = std::fs::read_to_string(&path).expect("Should read file");
            engine
                .compile(&content)
                .unwrap_or_else(|e| panic!("Example {} should parse: {}", path.display(), e));
            count += 1;
        }
    }

    assert!(count > 0, "Should have found at least one example file");
    println!("Successfully parsed {} example files", count);
}

#[test]
fn complex_script_evaluates() {
    let script = r#"
        // Create a gear-like shape
        let body = cylinder(1.0, 0.3);
        let hole = cylinder(0.3, 0.5);
        let tooth = cube(0.3).translate_x(1.1);

        // Create teeth around the gear
        let gear = body.subtract(hole);
        let gear = gear.union(tooth.repeat_polar(8));

        gear
    "#;

    let engine = ScriptEngine::new();
    let sdf_op = engine
        .eval_to_sdf_op(script)
        .expect("Complex script should evaluate");

    // Verify it can be wrapped and evaluated
    let cpu_sdf = CpuSdf::new(sdf_op);

    // Test at a point on the gear body (outside the center hole)
    let distance = cpu_sdf.distance(Vec3::new(0.5, 0.0, 0.0));

    // At x=0.5, should be inside the gear body (between hole and edge)
    assert!(distance < 0.0, "Should be inside gear at x=0.5");
}

#[test]
fn script_with_transforms() {
    // PI() is a function in Rhai, not a constant
    let script = r#"
        sphere(0.5)
            .translate(1.0, 0.0, 0.0)
            .rotate_y(PI() / 4.0)
            .scale(2.0)
    "#;

    let engine = ScriptEngine::new();
    let sdf_op = engine
        .eval_to_sdf_op(script)
        .expect("Transform script should evaluate");

    let cpu_sdf = CpuSdf::new(sdf_op);

    // Generate mesh to ensure transforms work correctly
    let config = MeshConfig::default().with_resolution(16);
    let mesh = cpu_sdf.to_mesh(config).expect("Should generate mesh");

    assert!(mesh.vertex_count() > 0);
}

#[test]
fn registered_advanced_shapes_generate_meshes() {
    let engine = ScriptEngine::new();
    let scripts = [
        ("pyramid", "pyramid(1.0)"),
        ("link", "link(0.35, 0.3, 0.12)"),
        ("extrude_circle", "extrude_circle(0.35, 0.35)"),
        ("extrude_rect", "extrude_rect(0.6, 0.4, 0.3)"),
        (
            "extrude_rounded_rect",
            "extrude_rounded_rect(0.6, 0.4, 0.08, 0.3)",
        ),
        ("revolve_circle", "revolve_circle(0.12, 0.45)"),
        ("revolve_rect", "revolve_rect(0.2, 0.3, 0.45)"),
    ];

    for (name, script) in scripts {
        let sdf_op = engine
            .eval_to_sdf_op(script)
            .unwrap_or_else(|e| panic!("{name} should evaluate: {e}"));
        let cpu_sdf = CpuSdf::new(sdf_op);

        assert!(
            cpu_sdf.distance(Vec3::new(0.1, 0.1, 0.1)).is_finite(),
            "{name} should have a finite CPU distance"
        );

        let mesh = cpu_sdf
            .to_mesh(MeshConfig::default().with_resolution(18))
            .unwrap_or_else(|e| panic!("{name} should generate mesh: {e}"));
        assert!(mesh.vertex_count() > 0, "{name} should have vertices");
        assert!(mesh.triangle_count() > 0, "{name} should have triangles");
    }
}

#[test]
fn script_with_modifiers() {
    let script = r#"
        cube(1.0)
            .round(0.1)
            .hollow(0.05)
    "#;

    let engine = ScriptEngine::new();
    let sdf_op = engine
        .eval_to_sdf_op(script)
        .expect("Modifier script should evaluate");

    let cpu_sdf = CpuSdf::new(sdf_op);

    // At origin, hollow cube should be outside (hollow center)
    let distance = cpu_sdf.distance(Vec3::ZERO);
    assert!(distance > 0.0, "Should be outside hollow cube at origin");
}

#[test]
fn environment_configuration_round_trip() {
    let engine = ScriptEngine::new();
    let script = r#"
        env_sunset();
        set_sun_intensity(1.5);
        set_material_color(1.0, 0.0, 0.0);
        sphere(1.0)
    "#;

    let scene = engine.eval_scene(script).expect("Scene should evaluate");

    // Verify environment was configured
    assert!(
        (scene.environment.sun_intensity - 1.5).abs() < 0.001,
        "Sun intensity should be 1.5, got {}",
        scene.environment.sun_intensity
    );
    assert!(
        (scene.environment.material_color[0] - 1.0).abs() < 0.001,
        "Material red should be 1.0"
    );
    assert!(
        (scene.environment.material_color[1] - 0.0).abs() < 0.001,
        "Material green should be 0.0"
    );
}

#[test]
fn environment_aliases_round_trip() {
    let engine = ScriptEngine::new();
    let scene = engine
        .eval_scene(
            r#"
            env_sunset();
            set_background_color(0.2, 0.3, 0.4);
            sphere(1.0)
        "#,
        )
        .expect("Scene should evaluate");

    assert_vec3_close(scene.environment.sky_horizon, [0.2, 0.3, 0.4]);
    assert_vec3_close(scene.environment.sky_zenith, [0.2, 0.3, 0.4]);

    let default_env = Environment::default();
    let scene = engine
        .eval_scene(
            r#"
            env_sunset();
            env_default();
            sphere(1.0)
        "#,
        )
        .expect("Scene should evaluate");

    assert_vec3_close(scene.environment.sun_direction, default_env.sun_direction);
    assert_vec3_close(scene.environment.sky_horizon, default_env.sky_horizon);
    assert_vec3_close(scene.environment.sky_zenith, default_env.sky_zenith);
}

#[test]
fn multi_format_export() {
    let engine = ScriptEngine::new();
    let sdf_op = engine
        .eval_to_sdf_op("sphere(0.5)")
        .expect("Should evaluate");
    let cpu_sdf = CpuSdf::new(sdf_op);
    let config = MeshConfig::default().with_resolution(16);
    let mesh = cpu_sdf.to_mesh(config).expect("Mesh should generate");

    let temp_dir = std::env::temp_dir();

    // Test STL export
    let stl_path = temp_dir.join("soyuz_test_multi.stl");
    mesh.export(&stl_path).expect("STL export should succeed");
    assert!(stl_path.exists(), "STL file should exist");
    std::fs::remove_file(&stl_path).ok();

    // Test OBJ export
    let obj_path = temp_dir.join("soyuz_test_multi.obj");
    mesh.export(&obj_path).expect("OBJ export should succeed");
    assert!(obj_path.exists(), "OBJ file should exist");
    std::fs::remove_file(&obj_path).ok();
}

#[test]
fn invalid_script_error_handling() {
    let engine = ScriptEngine::new();

    // Syntax error
    assert!(engine.eval_sdf("sphere(").is_err());

    // Script that doesn't return SDF (returns integer)
    assert!(engine.eval_sdf("let x = 42; x").is_err());

    // Script with only a let binding (returns unit, ends with semicolon)
    let result = engine.eval_sdf("let s = sphere(1.0);");
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("HINT") || err_msg.contains(';'),
        "Error should hint about semicolons: {}",
        err_msg
    );
}

#[test]
fn mesh_optimization_pipeline() {
    let engine = ScriptEngine::new();
    let sdf_op = engine
        .eval_to_sdf_op("sphere(1.0)")
        .expect("Should evaluate");
    let cpu_sdf = CpuSdf::new(sdf_op);
    let config = MeshConfig::default().with_resolution(32);
    let mut mesh = cpu_sdf.to_mesh(config).expect("Mesh should generate");

    let original_verts = mesh.vertex_count();
    let original_tris = mesh.triangle_count();

    assert!(original_verts > 0);
    assert!(original_tris > 0);

    // Optimize with welding and decimation
    mesh.optimize(
        &OptimizeConfig::default()
            .with_weld_threshold(0.001)
            .with_target_triangles(original_tris / 2),
    );

    // Should have fewer triangles after optimization
    assert!(
        mesh.triangle_count() <= original_tris,
        "Optimized mesh should have fewer or equal triangles"
    );
    // Should still have geometry
    assert!(mesh.vertex_count() > 0, "Should still have vertices");
    assert!(mesh.triangle_count() > 0, "Should still have triangles");
}
