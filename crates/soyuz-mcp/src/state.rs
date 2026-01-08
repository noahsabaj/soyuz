//! Scene state management for the MCP server
//!
//! Since the Rhai engine contains non-Send types (Rc<RefCell<...>>), we use
//! a channel-based architecture where the engine runs in a dedicated thread
//! and tool calls communicate via message passing.

use std::thread;

use anyhow::{anyhow, Result};
use image::ImageEncoder;
use soyuz_core::export::MeshExport;
use soyuz_core::mesh::{MeshConfig, OptimizeConfig, SdfToMesh};
use soyuz_core::sdf::Sdf;
use soyuz_engine::{Engine, ExportFormat};
use soyuz_render::{Raymarcher, init_headless};
use soyuz_script::CpuSdf;
use soyuz_sdf::{Environment, build_shader};
use tokio::sync::{mpsc, oneshot};

use crate::camera::CameraAngle;

/// Commands sent to the engine thread
enum Command {
    RunScript {
        code: String,
        respond: oneshot::Sender<Result<SceneInfo>>,
    },
    CompileScript {
        code: String,
        respond: oneshot::Sender<Result<()>>,
    },
    Render {
        angle: CameraAngle,
        width: u32,
        height: u32,
        respond: oneshot::Sender<Result<Vec<u8>>>,
    },
    ExportMesh {
        format: ExportFormat,
        resolution: u32,
        optimize: bool,
        respond: oneshot::Sender<Result<ExportInfo>>,
    },
    GetWgsl {
        respond: oneshot::Sender<Result<String>>,
    },
    GetSceneInfo {
        respond: oneshot::Sender<SceneInfo>,
    },
    ClearScene {
        respond: oneshot::Sender<()>,
    },
}

/// Handle to the engine thread
///
/// This is Send + Sync and can be cloned and shared between tasks.
#[derive(Clone)]
pub struct SoyuzState {
    sender: mpsc::UnboundedSender<Command>,
}

impl SoyuzState {
    /// Create a new state instance with initialized GPU
    ///
    /// This spawns a dedicated thread for the Rhai engine and GPU operations.
    ///
    /// # Errors
    /// Returns an error if GPU initialization fails.
    #[allow(clippy::too_many_lines)]
    pub async fn new() -> Result<Self> {
        // Initialize GPU first (this is async)
        let (device, queue) = init_headless().await?;

        // Create channel for commands
        let (tx, mut rx) = mpsc::unbounded_channel::<Command>();

        // Spawn dedicated thread for engine operations
        thread::spawn(move || {
            let mut engine = Engine::new();
            let mut raymarcher: Option<Raymarcher> = None;

            // Process commands
            while let Some(cmd) = rx.blocking_recv() {
                match cmd {
                    Command::RunScript { code, respond } => {
                        let result = engine.run_script(&code).map(|scene| {
                            // Create raymarcher for the new scene
                            let rm = Raymarcher::with_sdf_and_env(
                                device.clone(),
                                queue.clone(),
                                wgpu::TextureFormat::Rgba8UnormSrgb,
                                &scene.sdf,
                                scene.environment.clone(),
                            );
                            raymarcher = Some(rm);

                            // Get scene info
                            let cpu_sdf = CpuSdf::new(scene.sdf.clone());
                            let bounds = cpu_sdf.bounds();

                            SceneInfo {
                                loaded: true,
                                bounds_min: bounds.min.to_array(),
                                bounds_max: bounds.max.to_array(),
                                bounds_size: bounds.size().to_array(),
                                environment: Some(EnvironmentInfo::from(&scene.environment)),
                            }
                        });
                        let _ = respond.send(result.map_err(|e| anyhow!(e.to_string())));
                    }

                    Command::CompileScript { code, respond } => {
                        let result = engine.compile(&code).map_err(|e| anyhow!(e.to_string()));
                        let _ = respond.send(result);
                    }

                    Command::Render {
                        angle,
                        width,
                        height,
                        respond,
                    } => {
                        let result = (|| -> Result<Vec<u8>> {
                            let rm = raymarcher
                                .as_ref()
                                .ok_or_else(|| anyhow!("No scene loaded"))?;

                            let scene = engine.scene().ok_or_else(|| anyhow!("No scene loaded"))?;

                            // Get bounds for camera positioning
                            let cpu_sdf = CpuSdf::new(scene.sdf.clone());
                            let bounds = cpu_sdf.bounds();
                            let center = bounds.center();
                            let size = bounds.size().max_element();

                            // Create camera
                            let mut camera = angle.to_camera(center, size);
                            camera.aspect = width as f32 / height as f32;

                            // Render
                            let image = rm.render_to_image(width, height, &camera, 0.0)?;

                            // Encode as PNG
                            let mut png_bytes = Vec::new();
                            let encoder = image::codecs::png::PngEncoder::new(&mut png_bytes);
                            encoder.write_image(
                                image.as_raw(),
                                image.width(),
                                image.height(),
                                image::ExtendedColorType::Rgba8,
                            )?;

                            Ok(png_bytes)
                        })();
                        let _ = respond.send(result);
                    }

                    Command::ExportMesh {
                        format,
                        resolution,
                        optimize,
                        respond,
                    } => {
                        let result = (|| -> Result<ExportInfo> {
                            let scene = engine.scene().ok_or_else(|| anyhow!("No scene loaded"))?;

                            // Create CPU SDF
                            let cpu_sdf = CpuSdf::new(scene.sdf.clone());
                            let bounds = cpu_sdf.bounds();

                            // Generate mesh
                            let config = MeshConfig::default()
                                .with_resolution(resolution)
                                .with_bounds(bounds);

                            let mut mesh = cpu_sdf.to_mesh(config)?;

                            if optimize {
                                mesh.optimize(&OptimizeConfig::default());
                            }

                            let vertex_count = mesh.vertex_count();
                            let triangle_count = mesh.triangle_count();

                            // Export to temp file
                            let temp_dir = std::env::temp_dir();
                            let temp_path = temp_dir.join(format!(
                                "soyuz_export_{}.{}",
                                std::process::id(),
                                format.extension()
                            ));

                            mesh.export(&temp_path)?;

                            let bytes = std::fs::read(&temp_path)?;
                            let _ = std::fs::remove_file(&temp_path);

                            Ok(ExportInfo {
                                format,
                                bytes,
                                vertex_count,
                                triangle_count,
                            })
                        })();
                        let _ = respond.send(result);
                    }

                    Command::GetWgsl { respond } => {
                        let result = engine
                            .scene()
                            .map(|scene| build_shader(&scene.sdf))
                            .ok_or_else(|| anyhow!("No scene loaded"));
                        let _ = respond.send(result);
                    }

                    Command::GetSceneInfo { respond } => {
                        let info = if let Some(scene) = engine.scene() {
                            let cpu_sdf = CpuSdf::new(scene.sdf.clone());
                            let bounds = cpu_sdf.bounds();

                            SceneInfo {
                                loaded: true,
                                bounds_min: bounds.min.to_array(),
                                bounds_max: bounds.max.to_array(),
                                bounds_size: bounds.size().to_array(),
                                environment: Some(EnvironmentInfo::from(&scene.environment)),
                            }
                        } else {
                            SceneInfo {
                                loaded: false,
                                bounds_min: [0.0; 3],
                                bounds_max: [0.0; 3],
                                bounds_size: [0.0; 3],
                                environment: None,
                            }
                        };
                        let _ = respond.send(info);
                    }

                    Command::ClearScene { respond } => {
                        engine.clear_scene();
                        raymarcher = None;
                        let _ = respond.send(());
                    }
                }
            }
        });

        Ok(Self { sender: tx })
    }

    /// Execute a Rhai script and update the current scene
    pub async fn run_script(&self, code: &str) -> Result<SceneInfo> {
        let (tx, rx) = oneshot::channel();
        self.sender.send(Command::RunScript {
            code: code.to_string(),
            respond: tx,
        })?;
        rx.await?
    }

    /// Compile a script without executing (syntax check)
    pub async fn compile_script(&self, code: &str) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.sender.send(Command::CompileScript {
            code: code.to_string(),
            respond: tx,
        })?;
        rx.await?
    }

    /// Render the current scene to a PNG image
    pub async fn render(&self, angle: CameraAngle, width: u32, height: u32) -> Result<Vec<u8>> {
        let (tx, rx) = oneshot::channel();
        self.sender.send(Command::Render {
            angle,
            width,
            height,
            respond: tx,
        })?;
        rx.await?
    }

    /// Export the current scene to a mesh file format
    pub async fn export_mesh(
        &self,
        format: ExportFormat,
        resolution: u32,
        optimize: bool,
    ) -> Result<ExportInfo> {
        let (tx, rx) = oneshot::channel();
        self.sender.send(Command::ExportMesh {
            format,
            resolution,
            optimize,
            respond: tx,
        })?;
        rx.await?
    }

    /// Get the WGSL shader code for the current scene
    pub async fn get_wgsl(&self) -> Result<String> {
        let (tx, rx) = oneshot::channel();
        self.sender.send(Command::GetWgsl { respond: tx })?;
        rx.await?
    }

    /// Get information about the current scene
    pub async fn scene_info(&self) -> SceneInfo {
        let (tx, rx) = oneshot::channel();
        let _ = self.sender.send(Command::GetSceneInfo { respond: tx });
        rx.await.unwrap_or(SceneInfo {
            loaded: false,
            bounds_min: [0.0; 3],
            bounds_max: [0.0; 3],
            bounds_size: [0.0; 3],
            environment: None,
        })
    }

    /// Clear the current scene
    pub async fn clear_scene(&self) {
        let (tx, rx) = oneshot::channel();
        let _ = self.sender.send(Command::ClearScene { respond: tx });
        let _ = rx.await;
    }
}

/// Information about the current scene
#[derive(Debug, Clone)]
pub struct SceneInfo {
    /// Whether a scene is currently loaded
    pub loaded: bool,
    /// Minimum bounds of the scene
    pub bounds_min: [f32; 3],
    /// Maximum bounds of the scene
    pub bounds_max: [f32; 3],
    /// Size of the scene bounds
    pub bounds_size: [f32; 3],
    /// Environment settings
    pub environment: Option<EnvironmentInfo>,
}

impl std::fmt::Display for SceneInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.loaded {
            write!(
                f,
                "Scene loaded. Bounds: [{:.2}, {:.2}, {:.2}] to [{:.2}, {:.2}, {:.2}]",
                self.bounds_min[0], self.bounds_min[1], self.bounds_min[2],
                self.bounds_max[0], self.bounds_max[1], self.bounds_max[2]
            )
        } else {
            write!(f, "No scene loaded")
        }
    }
}

/// Information about environment settings
#[derive(Debug, Clone, serde::Serialize)]
pub struct EnvironmentInfo {
    pub sun_direction: [f32; 3],
    pub sun_color: [f32; 3],
    pub ambient_color: [f32; 3],
    pub material_color: [f32; 3],
}

impl From<&Environment> for EnvironmentInfo {
    fn from(env: &Environment) -> Self {
        Self {
            sun_direction: env.sun_direction,
            sun_color: env.sun_color,
            ambient_color: env.ambient_color,
            material_color: env.material_color,
        }
    }
}

/// Result of a mesh export operation
#[derive(Debug)]
pub struct ExportInfo {
    /// Export format used
    pub format: ExportFormat,
    /// Raw bytes of the exported file
    pub bytes: Vec<u8>,
    /// Number of vertices in the mesh
    pub vertex_count: usize,
    /// Number of triangles in the mesh
    pub triangle_count: usize,
}

impl std::fmt::Display for ExportInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Exported {} ({} vertices, {} triangles, {} bytes)",
            self.format.extension(),
            self.vertex_count,
            self.triangle_count,
            self.bytes.len()
        )
    }
}
