//! Command-line smoke modes for Soyuz Studio.

use soyuz_engine::{EmbeddedConfig, Engine, ExportOptions, PreviewOptions, run_embedded_preview};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "\
Usage:
  soyuz-studio [--fresh]
  soyuz-studio --preview --script <path> [--check-only]
  soyuz-studio --embedded-preview --script <path> --parent <xid> --x <px> --y <px> --width <px> --height <px> [--check-only]
  soyuz-studio --export-check --script <path> --out <path> [--resolution <n>]";
const DEFAULT_EXPORT_RESOLUTION: u32 = 16;

/// Launch mode selected by command-line arguments.
#[derive(Debug, Eq, PartialEq)]
pub enum LaunchMode {
    /// Start the desktop IDE.
    Studio { fresh_start: bool },
    /// Start only the isolated preview window.
    Preview {
        script_path: PathBuf,
        check_only: bool,
    },
    /// Start an isolated native preview embedded into the main Studio window.
    EmbeddedPreview {
        script_path: PathBuf,
        parent_handle: u32,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        check_only: bool,
    },
    /// Export a script through the built Studio artifact for product smoke tests.
    ExportCheck {
        script_path: PathBuf,
        out_path: PathBuf,
        resolution: u32,
    },
}

/// Parse Soyuz Studio command-line arguments.
#[allow(clippy::too_many_lines)]
pub fn parse_launch_mode(args: &[String]) -> Result<LaunchMode, String> {
    let mut fresh_start = false;
    let mut preview = false;
    let mut embedded_preview = false;
    let mut export_check = false;
    let mut check_only = false;
    let mut script_path = None;
    let mut out_path = None;
    let mut resolution = DEFAULT_EXPORT_RESOLUTION;
    let mut parent_handle = None;
    let mut x = None;
    let mut y = None;
    let mut width = None;
    let mut height = None;
    let mut iter = args.iter().skip(1);

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--fresh" => fresh_start = true,
            "--preview" => preview = true,
            "--embedded-preview" => embedded_preview = true,
            "--export-check" => export_check = true,
            "--check-only" => check_only = true,
            "--script" | "-s" => {
                let path = iter
                    .next()
                    .ok_or_else(|| "Missing value for --script".to_string())?;
                script_path = Some(PathBuf::from(path));
            }
            "--out" => {
                let path = iter
                    .next()
                    .ok_or_else(|| "Missing value for --out".to_string())?;
                out_path = Some(PathBuf::from(path));
            }
            "--resolution" => {
                let value = iter
                    .next()
                    .ok_or_else(|| "Missing value for --resolution".to_string())?;
                resolution = parse_resolution(value)?;
            }
            "--parent" => {
                let value = iter
                    .next()
                    .ok_or_else(|| "Missing value for --parent".to_string())?;
                parent_handle = Some(parse_u32(value, "--parent")?);
            }
            "--x" => {
                let value = iter
                    .next()
                    .ok_or_else(|| "Missing value for --x".to_string())?;
                x = Some(parse_i32(value, "--x")?);
            }
            "--y" => {
                let value = iter
                    .next()
                    .ok_or_else(|| "Missing value for --y".to_string())?;
                y = Some(parse_i32(value, "--y")?);
            }
            "--width" => {
                let value = iter
                    .next()
                    .ok_or_else(|| "Missing value for --width".to_string())?;
                width = Some(parse_u32(value, "--width")?);
            }
            "--height" => {
                let value = iter
                    .next()
                    .ok_or_else(|| "Missing value for --height".to_string())?;
                height = Some(parse_u32(value, "--height")?);
            }
            _ => {}
        }
    }

    if [preview, embedded_preview, export_check]
        .into_iter()
        .filter(|enabled| *enabled)
        .count()
        > 1
    {
        Err(
            "Use only one launch mode: --preview, --embedded-preview, or --export-check"
                .to_string(),
        )
    } else if preview {
        let script_path = script_path.ok_or_else(|| USAGE.to_string())?;
        Ok(LaunchMode::Preview {
            script_path,
            check_only,
        })
    } else if embedded_preview {
        let script_path = script_path.ok_or_else(|| USAGE.to_string())?;
        Ok(LaunchMode::EmbeddedPreview {
            script_path,
            parent_handle: parent_handle.ok_or_else(|| USAGE.to_string())?,
            x: x.ok_or_else(|| USAGE.to_string())?,
            y: y.ok_or_else(|| USAGE.to_string())?,
            width: width.ok_or_else(|| USAGE.to_string())?,
            height: height.ok_or_else(|| USAGE.to_string())?,
            check_only,
        })
    } else if export_check {
        if check_only {
            return Err("--check-only only applies to --preview".to_string());
        }
        let script_path = script_path.ok_or_else(|| USAGE.to_string())?;
        let out_path = out_path.ok_or_else(|| USAGE.to_string())?;
        Ok(LaunchMode::ExportCheck {
            script_path,
            out_path,
            resolution,
        })
    } else {
        Ok(LaunchMode::Studio { fresh_start })
    }
}

fn parse_u32(value: &str, flag: &str) -> Result<u32, String> {
    value
        .parse::<u32>()
        .map_err(|_| format!("Invalid {flag} value: {value}"))
}

fn parse_i32(value: &str, flag: &str) -> Result<i32, String> {
    value
        .parse::<i32>()
        .map_err(|_| format!("Invalid {flag} value: {value}"))
}

fn parse_resolution(value: &str) -> Result<u32, String> {
    let resolution = value
        .parse::<u32>()
        .map_err(|_| format!("Invalid --resolution value: {value}"))?;
    if resolution == 0 {
        Err("--resolution must be greater than 0".to_string())
    } else {
        Ok(resolution)
    }
}

/// Run the blocking preview window for a script file.
pub fn run_preview_script(path: &Path, check_only: bool) -> anyhow::Result<()> {
    let mut engine = Engine::new();
    engine.load_script(path)?;
    if check_only {
        return Ok(());
    }
    engine.preview(PreviewOptions::default().with_title("Soyuz Preview"))
}

/// Run preview mode and convert failures into a process exit code.
pub fn run_preview_command(path: &Path, check_only: bool) -> ExitCode {
    match run_preview_script(path, check_only) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Preview error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Run embedded preview mode and convert failures into a process exit code.
pub fn run_embedded_preview_command(
    path: &Path,
    parent_handle: u32,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    check_only: bool,
) -> ExitCode {
    match run_embedded_preview_script(path, parent_handle, x, y, width, height, check_only) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Embedded preview error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run_embedded_preview_script(
    path: &Path,
    parent_handle: u32,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    check_only: bool,
) -> anyhow::Result<()> {
    let mut engine = Engine::new();
    engine.load_script(path)?;
    if check_only {
        return Ok(());
    }

    let sdf = engine.sdf().cloned();
    let config = EmbeddedConfig::embedded(parent_handle, x, y, width, height);
    run_embedded_preview(config, sdf)
}

/// Run export-check mode and convert failures into a process exit code.
pub fn run_export_check_command(path: &Path, out_path: &Path, resolution: u32) -> ExitCode {
    match run_export_check(path, out_path, resolution) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Export check error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run_export_check(path: &Path, out_path: &Path, resolution: u32) -> anyhow::Result<()> {
    let mut engine = Engine::new();
    engine.load_script(path)?;
    engine.export(&ExportOptions::new(out_path).with_resolution(resolution))?;

    let metadata = std::fs::metadata(out_path)?;
    if metadata.len() == 0 {
        anyhow::bail!("Export output is empty: {}", out_path.display());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn parses_studio_mode() {
        assert_eq!(
            parse_launch_mode(&args(&["soyuz-studio"])),
            Ok(LaunchMode::Studio { fresh_start: false })
        );
    }

    #[test]
    fn parses_fresh_studio_mode() {
        assert_eq!(
            parse_launch_mode(&args(&["soyuz-studio", "--fresh"])),
            Ok(LaunchMode::Studio { fresh_start: true })
        );
    }

    #[test]
    fn parses_preview_mode() {
        assert_eq!(
            parse_launch_mode(&args(&[
                "soyuz-studio",
                "--preview",
                "--script",
                "/tmp/preview.rhai"
            ])),
            Ok(LaunchMode::Preview {
                script_path: PathBuf::from("/tmp/preview.rhai"),
                check_only: false,
            })
        );
    }

    #[test]
    fn parses_preview_check_only() {
        assert_eq!(
            parse_launch_mode(&args(&[
                "soyuz-studio",
                "--preview",
                "--script",
                "/tmp/preview.rhai",
                "--check-only",
            ])),
            Ok(LaunchMode::Preview {
                script_path: PathBuf::from("/tmp/preview.rhai"),
                check_only: true,
            })
        );
    }

    #[test]
    fn parses_embedded_preview() {
        assert_eq!(
            parse_launch_mode(&args(&[
                "soyuz-studio",
                "--embedded-preview",
                "--script",
                "/tmp/preview.rhai",
                "--parent",
                "42",
                "--x",
                "10",
                "--y",
                "20",
                "--width",
                "640",
                "--height",
                "480",
            ])),
            Ok(LaunchMode::EmbeddedPreview {
                script_path: PathBuf::from("/tmp/preview.rhai"),
                parent_handle: 42,
                x: 10,
                y: 20,
                width: 640,
                height: 480,
                check_only: false,
            })
        );
    }

    #[test]
    fn parses_export_check() {
        assert_eq!(
            parse_launch_mode(&args(&[
                "soyuz-studio",
                "--export-check",
                "--script",
                "/tmp/preview.rhai",
                "--out",
                "/tmp/preview.stl",
                "--resolution",
                "24",
            ])),
            Ok(LaunchMode::ExportCheck {
                script_path: PathBuf::from("/tmp/preview.rhai"),
                out_path: PathBuf::from("/tmp/preview.stl"),
                resolution: 24,
            })
        );
    }

    #[test]
    fn rejects_preview_mode_without_script() {
        assert_eq!(
            parse_launch_mode(&args(&["soyuz-studio", "--preview"])),
            Err(USAGE.to_string())
        );
    }

    #[test]
    fn rejects_export_check_without_output() {
        assert_eq!(
            parse_launch_mode(&args(&[
                "soyuz-studio",
                "--export-check",
                "--script",
                "/tmp/preview.rhai"
            ])),
            Err(USAGE.to_string())
        );
    }

    #[test]
    fn rejects_invalid_resolution() {
        assert_eq!(
            parse_launch_mode(&args(&[
                "soyuz-studio",
                "--export-check",
                "--script",
                "/tmp/preview.rhai",
                "--out",
                "/tmp/preview.stl",
                "--resolution",
                "zero",
            ])),
            Err("Invalid --resolution value: zero".to_string())
        );
    }
}
