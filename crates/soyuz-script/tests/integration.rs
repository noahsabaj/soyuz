//! Integration tests for script to mesh to export pipeline

// Tests are allowed to use expect/unwrap for cleaner error messages
#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]
// map_or with closure is clearer than is_some_and for extension checking
#![allow(clippy::unnecessary_map_or)]

use soyuz_core::export::MeshExport;
use soyuz_core::mesh::{MeshConfig, SdfToMesh};
use soyuz_core::prelude::Vec3;
use soyuz_script::{CpuSdf, ScriptEngine, Sdf};
use std::path::Path;

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
