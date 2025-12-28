//! Build script for soyuz-math
//!
//! Reads formula specifications from TOML files and generates:
//! - Rust implementations
//! - WGSL shader code
//! - Test cases
//! - Documentation

use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::Path;

use serde::Deserialize;

// ============================================================================
// TOML Schema Definitions
// ============================================================================

#[derive(Debug, Deserialize)]
struct FormulaSpec {
    formula: FormulaInfo,
    codegen: CodegenTemplates,
}

#[derive(Debug, Deserialize)]
struct FormulaInfo {
    name: String,
    category: String,
    description: String,
    verified_date: String,
    params: HashMap<String, ParamInfo>,
    returns: ReturnInfo,
    steps: Vec<Step>,
    pitfalls: Vec<Pitfall>,
    tests: Vec<TestCase>,
}

#[derive(Debug, Deserialize)]
struct ParamInfo {
    #[serde(rename = "type")]
    param_type: String,
    description: String,
}

#[derive(Debug, Deserialize)]
struct ReturnInfo {
    #[serde(rename = "type")]
    return_type: String,
    description: String,
}

#[derive(Debug, Deserialize)]
struct Step {
    name: String,
    expr: String,
    description: String,
}

#[derive(Debug, Deserialize)]
struct Pitfall {
    name: String,
    wrong: String,
    right: String,
    explanation: String,
}

#[derive(Debug, Deserialize)]
struct TestCase {
    name: String,
    input: TestInput,
    expected: Vec<f64>,
    tolerance: f64,
    description: String,
}

#[derive(Debug, Deserialize)]
struct TestInput {
    p: Vec<f64>,
    n: f64,
}

#[derive(Debug, Deserialize)]
struct CodegenTemplates {
    rust: TemplateInfo,
    wgsl: TemplateInfo,
    test: TemplateInfo,
}

#[derive(Debug, Deserialize)]
struct TemplateInfo {
    template: String,
}

// ============================================================================
// Code Generation
// ============================================================================

fn generate_step_docs(steps: &[Step]) -> String {
    steps
        .iter()
        .map(|s| format!("/// - `{}` = {} : {}", s.name, s.expr, s.description))
        .collect::<Vec<_>>()
        .join("\n")
}

fn generate_pitfall_docs(pitfalls: &[Pitfall]) -> String {
    pitfalls
        .iter()
        .map(|p| format!("/// - **{}**: Use `{}` not `{}`", p.name, p.right, p.wrong))
        .collect::<Vec<_>>()
        .join("\n")
}

fn generate_rust_code(spec: &FormulaSpec) -> String {
    let formula = &spec.formula;
    let template = &spec.codegen.rust.template;

    template
        .replace("{name}", &formula.name)
        .replace("{description}", &formula.description)
        .replace("{verified_date}", &formula.verified_date)
        .replace("{step_docs}", &generate_step_docs(&formula.steps))
        .replace("{pitfall_docs}", &generate_pitfall_docs(&formula.pitfalls))
}

fn generate_wgsl_code(spec: &FormulaSpec) -> String {
    let formula = &spec.formula;
    let template = &spec.codegen.wgsl.template;

    template
        .replace("{name}", &formula.name)
        .replace("{description}", &formula.description)
        .replace("{verified_date}", &formula.verified_date)
}

fn generate_test_code(spec: &FormulaSpec) -> String {
    let formula = &spec.formula;
    let template = &spec.codegen.test.template;

    formula
        .tests
        .iter()
        .map(|test| {
            let input_p = format!(
                "{:.6}, {:.6}, {:.6}",
                test.input.p[0], test.input.p[1], test.input.p[2]
            );
            let expected = format!(
                "{:.6}, {:.6}, {:.6}",
                test.expected[0], test.expected[1], test.expected[2]
            );

            template
                .replace("{name}", &formula.name)
                .replace("{test_name}", &test.name)
                .replace("{input_p}", &input_p)
                .replace("{input_n}", &format!("{:.1}_f32", test.input.n))
                .replace("{expected}", &expected)
                .replace("{tolerance}", &format!("{:.6}_f32", test.tolerance))
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn generate_markdown_docs(spec: &FormulaSpec) -> String {
    let formula = &spec.formula;

    let mut doc = String::new();

    doc.push_str(&format!("# {}\n\n", formula.name));
    doc.push_str(&format!("{}\n\n", formula.description));
    doc.push_str(&format!("**Category:** {}\n\n", formula.category));
    doc.push_str(&format!("**Verified:** {}\n\n", formula.verified_date));

    doc.push_str("## Parameters\n\n");
    for (name, info) in &formula.params {
        doc.push_str(&format!(
            "- `{}` ({}): {}\n",
            name, info.param_type, info.description
        ));
    }

    doc.push_str("\n## Returns\n\n");
    doc.push_str(&format!(
        "- `{}`: {}\n\n",
        formula.returns.return_type, formula.returns.description
    ));

    doc.push_str("## Formula Steps\n\n");
    for step in &formula.steps {
        doc.push_str(&format!("1. **{}** = `{}`\n", step.name, step.expr));
        doc.push_str(&format!("   - {}\n\n", step.description));
    }

    doc.push_str("## Pitfalls\n\n");
    for pitfall in &formula.pitfalls {
        doc.push_str(&format!("### {}\n\n", pitfall.name));
        doc.push_str(&format!("- **Wrong:** `{}`\n", pitfall.wrong));
        doc.push_str(&format!("- **Right:** `{}`\n\n", pitfall.right));
        doc.push_str(&format!("{}\n\n", pitfall.explanation.trim()));
    }

    doc.push_str("## Test Vectors\n\n");
    doc.push_str("| Test | Input P | N | Expected | Description |\n");
    doc.push_str("|------|---------|---|----------|-------------|\n");
    for test in &formula.tests {
        doc.push_str(&format!(
            "| {} | ({:.2}, {:.2}, {:.2}) | {} | ({:.2}, {:.2}, {:.2}) | {} |\n",
            test.name,
            test.input.p[0],
            test.input.p[1],
            test.input.p[2],
            test.input.n,
            test.expected[0],
            test.expected[1],
            test.expected[2],
            test.description
        ));
    }

    doc
}

// ============================================================================
// Main Build Script
// ============================================================================

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let out_path = Path::new(&out_dir);

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let formulas_dir = Path::new(&manifest_dir).join("formulas");

    // Collect all generated code
    let mut rust_code = String::new();
    let mut wgsl_code = String::new();
    let mut test_code = String::new();
    let mut docs = String::new();

    // Header for Rust
    rust_code.push_str("// AUTO-GENERATED by build.rs from formula specs\n");
    rust_code.push_str("// DO NOT EDIT - modify the TOML files in formulas/ instead\n\n");
    rust_code.push_str("use glam::Vec3;\n\n");

    // Header for WGSL
    wgsl_code.push_str("// AUTO-GENERATED by build.rs from formula specs\n");
    wgsl_code.push_str("// DO NOT EDIT - modify the TOML files in formulas/ instead\n\n");

    // Header for tests
    test_code.push_str("// AUTO-GENERATED by build.rs from formula specs\n");
    test_code.push_str("// DO NOT EDIT - modify the TOML files in formulas/ instead\n\n");
    test_code.push_str("use glam::Vec3;\n");
    test_code.push_str("use crate::*;\n\n");

    // Header for docs
    docs.push_str("# Soyuz Math Formula Reference\n\n");
    docs.push_str("*Auto-generated from formula specifications*\n\n");
    docs.push_str("---\n\n");

    // Process each formula file
    if formulas_dir.exists() {
        let mut entries: Vec<_> = fs::read_dir(&formulas_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "toml"))
            .collect();

        entries.sort_by_key(|e| e.path());

        for entry in entries {
            let path = entry.path();
            println!("cargo:rerun-if-changed={}", path.display());

            let content = fs::read_to_string(&path).unwrap();
            let spec: FormulaSpec = toml::from_str(&content).unwrap_or_else(|e| {
                panic!("Failed to parse {}: {}", path.display(), e);
            });

            rust_code.push_str(&generate_rust_code(&spec));
            rust_code.push_str("\n\n");

            wgsl_code.push_str(&generate_wgsl_code(&spec));
            wgsl_code.push_str("\n\n");

            test_code.push_str(&generate_test_code(&spec));
            test_code.push_str("\n\n");

            docs.push_str(&generate_markdown_docs(&spec));
            docs.push_str("\n---\n\n");
        }
    }

    // Write output files
    fs::write(out_path.join("formulas.rs"), rust_code).unwrap();
    fs::write(out_path.join("formulas.wgsl"), wgsl_code).unwrap();
    fs::write(out_path.join("tests.rs"), test_code).unwrap();
    fs::write(out_path.join("FORMULAS.md"), docs).unwrap();

    // Rerun if formulas directory changes
    println!("cargo:rerun-if-changed=formulas");
}
