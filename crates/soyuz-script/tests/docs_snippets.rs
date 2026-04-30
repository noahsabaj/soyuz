//! Validation for canonical Soyuz documentation examples.

// Tests use unwrap/expect to keep failures focused on the broken snippet.
#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]

use serde_json::Value;
use soyuz_script::{ScriptEngine, api_generated::API_MANIFEST_JSON};
use std::path::{Path, PathBuf};

#[test]
fn api_manifest_examples_execute() {
    let manifest: Value =
        serde_json::from_str(API_MANIFEST_JSON).expect("generated API manifest is valid JSON");
    let engine = ScriptEngine::new();

    for category in manifest["categories"].as_array().expect("categories array") {
        let category_id = category["id"].as_str().expect("category id");
        for function in category["functions"].as_array().expect("functions array") {
            let name = function["name"].as_str().expect("function name");
            let example = function["example"].as_str().expect("function example");
            assert_script_executes(
                &engine,
                example,
                &format!("API example {category_id}.{name}"),
            );
        }
    }
}

#[test]
fn canonical_markdown_rhai_fences_execute() {
    let root = workspace_root();
    let docs_root = root.join("docs/pages");
    let mut markdown_files = Vec::new();
    collect_markdown_files(&docs_root, &mut markdown_files);
    assert!(
        !markdown_files.is_empty(),
        "expected canonical docs markdown files under {}",
        docs_root.display()
    );

    let engine = ScriptEngine::new();
    for path in markdown_files {
        let markdown = std::fs::read_to_string(&path).unwrap();
        for (index, snippet) in rhai_fences(&markdown).into_iter().enumerate() {
            assert_script_executes(
                &engine,
                &snippet,
                &format!("{} Rhai fence {}", path.display(), index + 1),
            );
        }
    }
}

fn assert_script_executes(engine: &ScriptEngine, script: &str, label: &str) {
    if engine.eval_sdf(script).is_ok() {
        return;
    }

    engine
        .run(script)
        .unwrap_or_else(|error| panic!("{label} should execute:\n{script}\n\n{error}"));
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("soyuz-script crate should be inside crates")
        .parent()
        .expect("crates should be inside workspace root")
        .to_path_buf()
}

fn collect_markdown_files(dir: &Path, output: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect_markdown_files(&path, output);
        } else if path.extension().is_some_and(|extension| extension == "md") {
            output.push(path);
        }
    }
}

fn rhai_fences(markdown: &str) -> Vec<String> {
    let mut snippets = Vec::new();
    let mut current = Vec::new();
    let mut in_rhai = false;

    for line in markdown.lines() {
        if let Some(info) = line.strip_prefix("```") {
            if in_rhai {
                snippets.push(current.join("\n"));
                current.clear();
                in_rhai = false;
                continue;
            }

            let info = info.trim();
            let is_runnable_rhai = info.split_whitespace().next() == Some("rhai")
                && !info.contains("ignore")
                && !info.contains("no_run");
            in_rhai = is_runnable_rhai;
            continue;
        }

        if in_rhai {
            current.push(line);
        }
    }

    snippets
}
