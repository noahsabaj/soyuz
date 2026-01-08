//! Minimal preview runner binary
//!
//! This binary is spawned by the main Soyuz Studio app to run previews
//! in a separate process for isolation. It uses the Engine API to load
//! scripts and display the preview window.

use soyuz_engine::{Engine, PreviewOptions};
use std::env;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn main() -> ExitCode {
    // Parse command line args: soyuz-preview --script <path>
    let args: Vec<String> = env::args().collect();

    let script_path = parse_args(&args);
    let Some(path) = script_path else {
        eprintln!("Usage: soyuz-preview --script <path>");
        return ExitCode::FAILURE;
    };

    // Run the preview
    if let Err(e) = run_preview(&path) {
        eprintln!("Preview error: {e}");
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}

fn parse_args(args: &[String]) -> Option<PathBuf> {
    let mut iter = args.iter().skip(1); // Skip program name

    while let Some(arg) = iter.next() {
        if arg == "--script" || arg == "-s" {
            return iter.next().map(PathBuf::from);
        }
    }

    None
}

fn run_preview(path: &Path) -> anyhow::Result<()> {
    let mut engine = Engine::new();

    // Load the script
    engine.load_script(path)?;

    // Open preview window (blocks until closed)
    let options = PreviewOptions::default().with_title("Soyuz Preview");

    engine.preview(options)?;

    Ok(())
}
