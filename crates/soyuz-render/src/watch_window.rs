//! Watch window - Preview with hot reload and error handling
//!
//! Provides a preview window that watches a script file and reloads
//! when changes are detected.

use crate::camera::Camera;
use crate::raymarcher::{Raymarcher, init_with_surface};
use soyuz_sdf::SdfOp;
use glam::Vec3;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::{
    application::ApplicationHandler,
    dpi::{LogicalSize, PhysicalPosition},
    event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Window, WindowId},
};

/// State of the watched script
#[derive(Debug, Clone)]
pub enum ScriptState {
    /// Script loaded successfully
    Ok,
    /// Script has an error
    Error(String),
    /// No script loaded yet
    None,
}

/// Configuration for the watch window
#[derive(Debug, Clone)]
pub struct WatchWindowConfig {
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub script_path: Option<PathBuf>,
}

impl Default for WatchWindowConfig {
    fn default() -> Self {
        Self {
            title: "Soyuz Watch".to_string(),
            width: 1280,
            height: 720,
            script_path: None,
        }
    }
}

/// Callback type for script evaluation
pub type EvalCallback = Box<dyn Fn(&std::path::Path) -> Result<SdfOp, String> + Send>;

/// Input state for camera control
#[derive(Debug, Default)]
struct InputState {
    mouse_left: bool,
    mouse_right: bool,
    mouse_middle: bool,
    last_mouse_pos: Option<PhysicalPosition<f64>>,
    shift_held: bool,
}

/// Application state for the watch window
struct WatchApp<'a> {
    config: WatchWindowConfig,
    window: Option<Arc<Window>>,
    surface: Option<wgpu::Surface<'a>>,
    surface_config: Option<wgpu::SurfaceConfiguration>,
    device: Option<Arc<wgpu::Device>>,
    queue: Option<Arc<wgpu::Queue>>,
    raymarcher: Option<Raymarcher>,
    camera: Camera,
    input: InputState,
    start_time: Instant,
    instance: wgpu::Instance,

    // Watch state
    current_sdf: Option<SdfOp>,
    script_state: ScriptState,
    eval_callback: Option<EvalCallback>,
    last_check_time: Instant,
    check_interval: Duration,
    last_modified: Option<std::time::SystemTime>,
    error_flash_time: Option<Instant>,
}

impl<'a> WatchApp<'a> {
    fn new(config: WatchWindowConfig, eval_callback: Option<EvalCallback>) -> Self {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let now = Instant::now();

        Self {
            config,
            window: None,
            surface: None,
            surface_config: None,
            device: None,
            queue: None,
            raymarcher: None,
            camera: Camera::default(),
            input: InputState::default(),
            start_time: now,
            instance,
            current_sdf: None,
            script_state: ScriptState::None,
            eval_callback,
            last_check_time: now,
            check_interval: Duration::from_millis(100),
            last_modified: None,
            error_flash_time: None,
        }
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            if let (Some(config), Some(surface), Some(device)) =
                (&mut self.surface_config, &self.surface, &self.device)
            {
                config.width = new_size.width;
                config.height = new_size.height;
                surface.configure(device, config);
                self.camera.aspect = new_size.width as f32 / new_size.height as f32;
            }
        }
    }

    fn check_script(&mut self) {
        let Some(path) = &self.config.script_path else {
            return;
        };

        // Check if enough time has passed since last check
        if self.last_check_time.elapsed() < self.check_interval {
            return;
        }
        self.last_check_time = Instant::now();

        // Check file modification time
        let modified = std::fs::metadata(path).and_then(|m| m.modified()).ok();

        if modified != self.last_modified {
            self.last_modified = modified;
            self.reload_script();
        }
    }

    fn reload_script(&mut self) {
        let Some(path) = &self.config.script_path else {
            return;
        };
        let Some(eval) = &self.eval_callback else {
            return;
        };

        println!("\n--- Reloading: {} ---", path.display());

        match eval(path) {
            Ok(sdf) => {
                self.current_sdf = Some(sdf.clone());
                self.script_state = ScriptState::Ok;
                self.error_flash_time = None;

                // Rebuild raymarcher with new SDF
                if let (Some(device), Some(queue), Some(config)) =
                    (&self.device, &self.queue, &self.surface_config)
                {
                    self.raymarcher = Some(Raymarcher::with_sdf(
                        device.clone(),
                        queue.clone(),
                        config.format,
                        &sdf,
                    ));
                }

                // Update window title
                if let Some(window) = &self.window {
                    window.set_title(&format!("{} - OK", self.config.title));
                }

                println!("Script loaded successfully!");
            }
            Err(e) => {
                // Show error in terminal
                eprintln!("\n=== SCRIPT ERROR ===");
                eprintln!("{}", e);
                eprintln!("====================\n");

                self.script_state = ScriptState::Error(e);
                self.error_flash_time = Some(Instant::now());

                // Update window title to show error
                if let Some(window) = &self.window {
                    window.set_title(&format!("{} - ERROR", self.config.title));
                }
            }
        }
    }

    fn handle_mouse_motion(&mut self, position: PhysicalPosition<f64>) {
        if let Some(last_pos) = self.input.last_mouse_pos {
            let dx = (position.x - last_pos.x) as f32 * 0.005;
            let dy = (position.y - last_pos.y) as f32 * 0.005;

            if self.input.mouse_left {
                self.camera.orbit(dx, dy);
            } else if self.input.mouse_right || (self.input.mouse_left && self.input.shift_held) {
                self.camera.pan(-dx * 2.0, dy * 2.0);
            } else if self.input.mouse_middle {
                self.camera.zoom(dy * 5.0);
            }
        }
        self.input.last_mouse_pos = Some(position);
    }

    fn handle_scroll(&mut self, delta: MouseScrollDelta) {
        let scroll = match delta {
            MouseScrollDelta::LineDelta(_, y) => y,
            MouseScrollDelta::PixelDelta(pos) => pos.y as f32 * 0.01,
        };
        self.camera.zoom(scroll * 0.5);
    }

    fn render(&mut self) {
        let (Some(surface), Some(raymarcher), Some(config)) =
            (&self.surface, &self.raymarcher, &self.surface_config)
        else {
            return;
        };

        let output = match surface.get_current_texture() {
            Ok(output) => output,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                if let (Some(device), Some(config)) = (&self.device, &self.surface_config) {
                    surface.configure(device, config);
                }
                return;
            }
            Err(e) => {
                eprintln!("Surface error: {:?}", e);
                return;
            }
        };

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Calculate time with error flash effect
        let mut time = self.start_time.elapsed().as_secs_f32();

        // If there's an error, add a visual indicator by modulating time
        // This causes a subtle "pulse" effect in the shader
        if let Some(flash_start) = self.error_flash_time {
            let flash_elapsed = flash_start.elapsed().as_secs_f32();
            // Rapid oscillation for first second to indicate error
            if flash_elapsed < 1.0 {
                time += (flash_elapsed * 20.0).sin() * 0.5;
            }
        }

        raymarcher.update_uniforms(
            &self.camera,
            [config.width as f32, config.height as f32],
            time,
        );
        raymarcher.render(&view);

        output.present();
    }

    fn reset_camera(&mut self) {
        self.camera = Camera::default();
    }
}

impl ApplicationHandler for WatchApp<'_> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let window_attrs = Window::default_attributes()
            .with_title(&self.config.title)
            .with_inner_size(LogicalSize::new(self.config.width, self.config.height));

        let window = Arc::new(
            event_loop
                .create_window(window_attrs)
                .expect("Failed to create window"),
        );

        let surface = self
            .instance
            .create_surface(window.clone())
            .expect("Failed to create surface");

        let (device, queue, format) =
            pollster::block_on(init_with_surface(&self.instance, &surface));

        let size = window.inner_size();
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        // Initialize raymarcher
        let raymarcher = if let Some(sdf) = &self.current_sdf {
            Raymarcher::with_sdf(device.clone(), queue.clone(), format, sdf)
        } else {
            // Default scene while no script is loaded
            let default_sdf = SdfOp::Sphere { radius: 0.5 };
            Raymarcher::with_sdf(device.clone(), queue.clone(), format, &default_sdf)
        };

        self.camera.aspect = size.width as f32 / size.height as f32;

        self.window = Some(window);
        self.surface = Some(surface);
        self.surface_config = Some(surface_config);
        self.device = Some(device);
        self.queue = Some(queue);
        self.raymarcher = Some(raymarcher);

        // Load initial script
        if self.config.script_path.is_some() {
            self.reload_script();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(new_size) => {
                self.resize(new_size);
            }
            WindowEvent::RedrawRequested => {
                // Check for script changes
                self.check_script();
                self.render();
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let pressed = state == ElementState::Pressed;
                match button {
                    MouseButton::Left => self.input.mouse_left = pressed,
                    MouseButton::Right => self.input.mouse_right = pressed,
                    MouseButton::Middle => self.input.mouse_middle = pressed,
                    _ => {}
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.handle_mouse_motion(position);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                self.handle_scroll(delta);
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.input.shift_held = modifiers.state().shift_key();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed {
                    match event.logical_key {
                        Key::Named(NamedKey::Escape) => {
                            event_loop.exit();
                        }
                        Key::Character(ref c) if c == "r" || c == "R" => {
                            if self.input.shift_held {
                                // Shift+R = force reload script
                                self.reload_script();
                            } else {
                                // R = reset camera
                                self.reset_camera();
                            }
                        }
                        Key::Character(ref c) if c == "f" || c == "F" => {
                            self.camera.target = Vec3::ZERO;
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

/// Run the watch window with a script file
pub fn run_watch_window(
    config: WatchWindowConfig,
    eval_callback: EvalCallback,
) -> anyhow::Result<()> {
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = WatchApp::new(config, Some(eval_callback));
    event_loop.run_app(&mut app)?;

    Ok(())
}

/// Run the watch window with an initial SDF
pub fn run_watch_window_with_sdf(
    config: WatchWindowConfig,
    sdf: SdfOp,
    eval_callback: EvalCallback,
) -> anyhow::Result<()> {
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = WatchApp::new(config, Some(eval_callback));
    app.current_sdf = Some(sdf);
    event_loop.run_app(&mut app)?;

    Ok(())
}

/// Watch controls help text
pub fn watch_controls_help() -> &'static str {
    r#"
Watch Mode Controls:
  Left Mouse Drag   - Orbit camera around target
  Right Mouse Drag  - Pan camera
  Middle Mouse Drag - Zoom camera
  Scroll Wheel      - Zoom camera
  Shift + Left Drag - Pan camera (alternative)
  R                 - Reset camera to default
  Shift + R         - Force reload script
  F                 - Focus on origin
  Escape            - Close preview

The script will automatically reload when saved.
Errors are shown in the terminal and indicated visually.
"#
}
