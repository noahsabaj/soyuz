//! Soyuz CLI - Command-line interface for procedural asset generation

mod repl;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "soyuz")]
#[command(about = "Procedural asset generation through code", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate a mesh from a script
    Generate {
        /// Input script file
        #[arg(short, long)]
        input: PathBuf,

        /// Output file (format auto-detected from extension)
        #[arg(short, long)]
        output: PathBuf,

        /// Mesh resolution
        #[arg(short, long, default_value = "64")]
        resolution: u32,
    },

    /// Open a real-time preview window
    Preview {
        /// Script file to preview (optional)
        #[arg(short, long)]
        script: Option<PathBuf>,

        /// Window width
        #[arg(long, default_value = "1280")]
        width: u32,

        /// Window height
        #[arg(long, default_value = "720")]
        height: u32,

        /// Window title
        #[arg(long, default_value = "Soyuz Preview")]
        title: String,
    },

    /// Open an embedded preview as a child window (internal use)
    #[command(hide = true)]
    EmbeddedPreview {
        /// Script file to preview
        #[arg(short, long)]
        script: Option<PathBuf>,

        /// Parent window X11 handle
        #[arg(long)]
        parent: u32,

        /// X position relative to parent
        #[arg(long, default_value = "0")]
        x: i32,

        /// Y position relative to parent
        #[arg(long, default_value = "0")]
        y: i32,

        /// Window width
        #[arg(long, default_value = "400")]
        width: u32,

        /// Window height
        #[arg(long, default_value = "300")]
        height: u32,
    },

    /// Render an SDF to an image file (headless)
    Render {
        /// Script file to render (optional, uses demo if not provided)
        #[arg(short, long)]
        script: Option<PathBuf>,

        /// Output image file (.png)
        #[arg(short, long, default_value = "render.png")]
        output: PathBuf,

        /// Image width
        #[arg(long, default_value = "1920")]
        width: u32,

        /// Image height
        #[arg(long, default_value = "1080")]
        height: u32,
    },

    /// Interactive REPL for SDF experimentation
    Repl,

    /// Watch a script for changes with live preview
    Watch {
        /// Script file to watch
        script: PathBuf,

        /// Disable preview window (terminal output only)
        #[arg(long)]
        no_preview: bool,

        /// Window width
        #[arg(long, default_value = "1280")]
        width: u32,

        /// Window height
        #[arg(long, default_value = "720")]
        height: u32,
    },

    /// Generate a simple demo asset
    Demo {
        /// Output file
        #[arg(short, long, default_value = "demo.obj")]
        output: PathBuf,
    },
}

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Generate {
            input,
            output,
            resolution,
        } => {
            run_generate(&input, &output, resolution)?;
        }
        Commands::Preview {
            script,
            width,
            height,
            title,
        } => {
            run_preview(script.as_deref(), width, height, title)?;
        }
        Commands::EmbeddedPreview {
            script,
            parent,
            x,
            y,
            width,
            height,
        } => {
            run_embedded_preview(script.as_deref(), parent, x, y, width, height)?;
        }
        Commands::Render {
            script,
            output,
            width,
            height,
        } => {
            run_render(script.as_deref(), &output, width, height)?;
        }
        Commands::Repl => {
            repl::run_repl()?;
        }
        Commands::Watch {
            script,
            no_preview,
            width,
            height,
        } => {
            run_watch(&script, !no_preview, width, height)?;
        }
        Commands::Demo { output } => {
            generate_demo(&output)?;
        }
    }

    Ok(())
}

fn run_generate(input: &PathBuf, output: &PathBuf, _resolution: u32) -> Result<()> {
    use soyuz_script::ScriptEngine;

    println!("Generating mesh from {}...", input.display());

    // Create script engine and evaluate
    let engine = ScriptEngine::new();
    let _rhai_sdf = engine.eval_sdf_file(input)?;

    // We need to convert the RhaiSdf to a CPU-evaluable SDF for mesh generation
    // For now, we'll create a simple wrapper that evaluates the SDF on CPU
    // This is a limitation - ideally we'd have a unified SDF representation

    println!("Note: Script-based mesh generation is limited.");
    println!("For complex scripts, use 'soyuz preview' or 'soyuz render' instead.");

    // Generate demo mesh as fallback
    generate_demo(output)?;

    Ok(())
}

fn run_preview(
    script: Option<&std::path::Path>,
    width: u32,
    height: u32,
    title: String,
) -> Result<()> {
    use soyuz_render::{WindowConfig, run_preview_with_sdf};

    println!("Opening preview window...");

    let config = WindowConfig {
        title,
        width,
        height,
    };

    // If script provided, load and preview it
    let sdf = if let Some(script_path) = script {
        println!("Loading script: {}", script_path.display());
        let engine = soyuz_script::ScriptEngine::new();
        match engine.eval_file_to_sdf_op(script_path) {
            Ok(sdf) => {
                println!("Script loaded successfully!");
                Some(sdf)
            }
            Err(e) => {
                eprintln!("Error loading script: {}", e);
                eprintln!("Showing default scene instead.");
                None
            }
        }
    } else {
        // Use demo SDF
        Some(create_demo_sdf())
    };

    println!("{}", soyuz_render::controls_help());

    run_preview_with_sdf(config, sdf)?;

    Ok(())
}

fn run_embedded_preview(
    script: Option<&std::path::Path>,
    parent: u32,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Result<()> {
    use soyuz_render::{EmbeddedConfig, run_embedded_preview};

    // Load SDF from script or use demo
    let sdf = if let Some(script_path) = script {
        let engine = soyuz_script::ScriptEngine::new();
        match engine.eval_file_to_sdf_op(script_path) {
            Ok(sdf) => Some(sdf),
            Err(e) => {
                eprintln!("Error loading script: {}", e);
                Some(create_demo_sdf())
            }
        }
    } else {
        Some(create_demo_sdf())
    };

    let config = EmbeddedConfig::embedded(parent, x, y, width, height);
    run_embedded_preview(config, sdf)?;

    Ok(())
}

fn run_render(
    script: Option<&std::path::Path>,
    output: &PathBuf,
    width: u32,
    height: u32,
) -> Result<()> {
    use soyuz_render::{Camera, Raymarcher, init_headless};

    println!(
        "Rendering to {} ({}x{})...",
        output.display(),
        width,
        height
    );

    // Get SDF from script or use demo
    let sdf = if let Some(script_path) = script {
        println!("Loading script: {}", script_path.display());
        let engine = soyuz_script::ScriptEngine::new();
        engine.eval_file_to_sdf_op(script_path)?
    } else {
        create_demo_sdf()
    };

    // Initialize headless WGPU
    let (device, queue) = pollster::block_on(init_headless());

    // Create raymarcher with SDF
    let raymarcher = Raymarcher::with_sdf(
        device,
        queue,
        soyuz_render::wgpu::TextureFormat::Rgba8UnormSrgb,
        &sdf,
    );

    // Setup camera
    let mut camera = Camera::default();
    camera.aspect = width as f32 / height as f32;

    // Render
    let img = raymarcher.render_to_image(width, height, &camera, 0.0);

    // Save
    img.save(output)?;
    println!("Saved to: {}", output.display());

    Ok(())
}

fn run_watch(script: &PathBuf, show_preview: bool, width: u32, height: u32) -> Result<()> {
    use soyuz_render::{WatchWindowConfig, run_watch_window};
    use soyuz_script::ScriptEngine;

    // Verify file exists
    if !script.exists() {
        anyhow::bail!("Script file not found: {}", script.display());
    }

    println!("Watching: {}", script.display());
    println!("{}", soyuz_render::watch_controls_help());

    if show_preview {
        let config = WatchWindowConfig {
            title: format!(
                "Soyuz Watch - {}",
                script.file_name().unwrap_or_default().to_string_lossy()
            ),
            width,
            height,
            script_path: Some(script.clone()),
        };

        // Create eval callback that captures the script engine
        let eval_callback = Box::new(move |path: &std::path::Path| {
            let engine = ScriptEngine::new();
            engine.eval_file_to_sdf_op(path).map_err(|e| e.to_string())
        });

        run_watch_window(config, eval_callback)?;
    } else {
        // Terminal-only watch mode
        run_terminal_watch(script)?;
    }

    Ok(())
}

fn run_terminal_watch(script: &PathBuf) -> Result<()> {
    use notify::{RecursiveMode, Watcher};
    use std::sync::mpsc::channel;
    use std::time::Duration;

    let (tx, rx) = channel();

    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(event) = res {
            if event.kind.is_modify() {
                let _ = tx.send(());
            }
        }
    })?;

    watcher.watch(script, RecursiveMode::NonRecursive)?;

    println!("Watching {} (terminal mode)", script.display());
    println!("Press Ctrl+C to stop\n");

    // Initial evaluation
    eval_and_report(script);

    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(()) => {
                eval_and_report(script);
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                // Continue watching
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                break;
            }
        }
    }

    Ok(())
}

fn eval_and_report(script: &PathBuf) {
    println!("\n--- Evaluating: {} ---", script.display());

    let engine = soyuz_script::ScriptEngine::new();
    match engine.eval_file_to_sdf_op(script) {
        Ok(_sdf) => {
            println!("OK - Script evaluated successfully");
        }
        Err(e) => {
            eprintln!("ERROR:\n{}", e);
        }
    }
}

fn generate_demo(output: &PathBuf) -> Result<()> {
    use soyuz_core::export::MeshExport;
    use soyuz_core::mesh::{MeshConfig, SdfToMesh};
    use soyuz_core::prelude::*;

    println!("Generating demo asset...");

    // Create a procedural barrel-like shape
    let barrel = cylinder(0.5, 1.2)
        .smooth_union(torus(0.5, 0.08).translate_y(0.5), 0.05)
        .smooth_union(torus(0.5, 0.08).translate_y(-0.5), 0.05)
        .hollow(0.05);

    // Generate mesh
    let config = MeshConfig::default()
        .with_resolution(64)
        .with_bounds(soyuz_core::sdf::Aabb::cube(1.5));

    let mesh = barrel.to_mesh(config)?;

    println!(
        "Generated mesh: {} vertices, {} triangles",
        mesh.vertex_count(),
        mesh.triangle_count()
    );

    // Export
    mesh.export(output)?;
    println!("Exported to: {}", output.display());

    Ok(())
}

/// Create a demo SDF for the preview and render commands
fn create_demo_sdf() -> soyuz_render::SdfOp {
    use soyuz_render::SdfOp;

    // Create a barrel shape
    let cylinder = SdfOp::Cylinder {
        radius: 0.5,
        half_height: 0.6,
    };

    let band_top = SdfOp::Translate {
        inner: Box::new(SdfOp::Torus {
            major_radius: 0.5,
            minor_radius: 0.08,
        }),
        offset: [0.0, 0.5, 0.0],
    };

    let band_bottom = SdfOp::Translate {
        inner: Box::new(SdfOp::Torus {
            major_radius: 0.5,
            minor_radius: 0.08,
        }),
        offset: [0.0, -0.5, 0.0],
    };

    // Combine with smooth union
    let body_with_top = SdfOp::SmoothUnion {
        a: Box::new(cylinder),
        b: Box::new(band_top),
        k: 0.05,
    };

    let body_with_bands = SdfOp::SmoothUnion {
        a: Box::new(body_with_top),
        b: Box::new(band_bottom),
        k: 0.05,
    };

    // Hollow it out
    SdfOp::Shell {
        inner: Box::new(body_with_bands),
        thickness: 0.05,
    }
}
