//! Embedded WGPU preview that runs as an X11 child window
//!
//! This module provides a preview renderer that can be embedded inside
//! another window (like a Dioxus app) as a native child window.
//!
//! Since winit's event loop must run on the main thread, this is designed
//! to be run as a separate process with a parent window ID passed in.

use crate::camera::Camera;
use crate::raymarcher::Raymarcher;
use soyuz_sdf::SdfOp;
use crate::text_overlay::FpsOverlay;
use glam::Vec3;
use std::sync::Arc;
use std::time::Instant;
use winit::{
    application::ApplicationHandler,
    dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize},
    event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Window, WindowId},
};

#[cfg(target_os = "linux")]
use winit::platform::x11::WindowAttributesExtX11;

/// Configuration for the embedded preview window
#[derive(Debug, Clone)]
pub struct EmbeddedConfig {
    /// Parent window X11 handle (0 means standalone window)
    pub parent_handle: u32,
    /// Initial position relative to parent
    pub x: i32,
    pub y: i32,
    /// Initial size
    pub width: u32,
    pub height: u32,
    /// Window title (only shown when popped out)
    pub title: String,
    /// Whether to show window decorations
    pub decorated: bool,
}

impl Default for EmbeddedConfig {
    fn default() -> Self {
        Self {
            parent_handle: 0,
            x: 0,
            y: 0,
            width: 800,
            height: 600,
            title: "Soyuz Preview".to_string(),
            decorated: false,
        }
    }
}

impl EmbeddedConfig {
    /// Create config for embedded mode (child of another window)
    pub fn embedded(parent_handle: u32, x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            parent_handle,
            x,
            y,
            width,
            height,
            title: "Soyuz Preview".to_string(),
            decorated: false,
        }
    }

    /// Create config for standalone mode (own window)
    pub fn standalone(width: u32, height: u32) -> Self {
        Self {
            parent_handle: 0,
            x: 100,
            y: 100,
            width,
            height,
            title: "Soyuz Preview".to_string(),
            decorated: true,
        }
    }
}

/// Input state for camera control
#[derive(Debug, Default)]
struct InputState {
    mouse_left: bool,
    mouse_right: bool,
    mouse_middle: bool,
    last_mouse_pos: Option<PhysicalPosition<f64>>,
    shift_held: bool,
}

/// Application state for the embedded preview
struct EmbeddedPreviewApp {
    config: EmbeddedConfig,
    sdf: Option<SdfOp>,
    window: Option<Arc<Window>>,
    surface: Option<wgpu::Surface<'static>>,
    surface_config: Option<wgpu::SurfaceConfiguration>,
    device: Option<Arc<wgpu::Device>>,
    queue: Option<Arc<wgpu::Queue>>,
    raymarcher: Option<Raymarcher>,
    fps_overlay: Option<FpsOverlay>,
    camera: Camera,
    input: InputState,
    start_time: Instant,
    instance: wgpu::Instance,
    is_embedded: bool,
}

impl EmbeddedPreviewApp {
    fn new(config: EmbeddedConfig, sdf: Option<SdfOp>) -> Self {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let is_embedded = config.parent_handle != 0;

        Self {
            config,
            sdf,
            window: None,
            surface: None,
            surface_config: None,
            device: None,
            queue: None,
            raymarcher: None,
            fps_overlay: None,
            camera: Camera::default(),
            input: InputState::default(),
            start_time: Instant::now(),
            instance,
            is_embedded,
        }
    }

    fn resize(&mut self, new_size: PhysicalSize<u32>) {
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
        let (Some(surface), Some(raymarcher), Some(config), Some(device), Some(queue)) = (
            &self.surface,
            &self.raymarcher,
            &self.surface_config,
            &self.device,
            &self.queue,
        ) else {
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
                tracing::error!("Surface error: {:?}", e);
                return;
            }
        };

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let time = self.start_time.elapsed().as_secs_f32();
        raymarcher.update_uniforms(
            &self.camera,
            [config.width as f32, config.height as f32],
            time,
        );
        raymarcher.render(&view);

        // Render FPS overlay
        if let Some(fps_overlay) = &mut self.fps_overlay {
            fps_overlay.tick();

            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("FPS Overlay Encoder"),
            });

            fps_overlay.render(
                device,
                queue,
                &mut encoder,
                &view,
                config.width,
                config.height,
            );

            queue.submit(std::iter::once(encoder.finish()));
        }

        output.present();
    }
}

impl ApplicationHandler for EmbeddedPreviewApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        // Build window attributes
        let mut window_attrs = Window::default_attributes()
            .with_title(&self.config.title)
            .with_inner_size(LogicalSize::new(self.config.width, self.config.height))
            .with_position(LogicalPosition::new(self.config.x, self.config.y))
            .with_decorations(self.config.decorated);

        // If we have a parent window handle, embed as child (X11 only)
        #[cfg(target_os = "linux")]
        if self.config.parent_handle != 0 {
            window_attrs = window_attrs.with_embed_parent_window(self.config.parent_handle);
        }

        let window = match event_loop.create_window(window_attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                tracing::error!("Failed to create window: {}", e);
                event_loop.exit();
                return;
            }
        };

        // Create surface
        let surface = match self.instance.create_surface(window.clone()) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("Failed to create surface: {}", e);
                event_loop.exit();
                return;
            }
        };

        // Initialize WGPU
        let (device, queue, format) = match pollster::block_on(async {
            let adapter = self
                .instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::HighPerformance,
                    compatible_surface: Some(&surface),
                    force_fallback_adapter: false,
                })
                .await
                .ok()?;

            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor {
                    label: Some("Soyuz Embedded Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: Default::default(),
                    trace: wgpu::Trace::Off,
                })
                .await
                .ok()?;

            let caps = surface.get_capabilities(&adapter);
            let format = caps
                .formats
                .iter()
                .copied()
                .find(|f| f.is_srgb())
                .unwrap_or(caps.formats[0]);

            Some((Arc::new(device), Arc::new(queue), format))
        }) {
            Some(r) => r,
            None => {
                tracing::error!("Failed to initialize WGPU");
                event_loop.exit();
                return;
            }
        };

        let size = window.inner_size();
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        // Create raymarcher
        let raymarcher = if let Some(sdf) = self.sdf.take() {
            Raymarcher::with_sdf(device.clone(), queue.clone(), format, &sdf)
        } else {
            Raymarcher::new(device.clone(), queue.clone(), format)
        };

        // Create FPS overlay
        let fps_overlay = FpsOverlay::new(&device, &queue, format);

        // Update camera aspect
        self.camera.aspect = size.width as f32 / size.height.max(1) as f32;

        // Store everything
        self.window = Some(window);
        // SAFETY: The surface is tied to the window's lifetime which we're managing
        self.surface = Some(unsafe { std::mem::transmute(surface) });
        self.surface_config = Some(surface_config);
        self.device = Some(device);
        self.queue = Some(queue);
        self.raymarcher = Some(raymarcher);
        self.fps_overlay = Some(fps_overlay);

        tracing::info!("Preview window ready (embedded: {})", self.is_embedded);
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
                            // Only close on Escape if not embedded
                            if !self.is_embedded {
                                event_loop.exit();
                            }
                        }
                        Key::Character(ref c) if c == "r" || c == "R" => {
                            let aspect = self.camera.aspect;
                            self.camera = Camera::default();
                            self.camera.aspect = aspect;
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

/// Run the embedded preview (call this from main thread)
pub fn run_embedded_preview(config: EmbeddedConfig, sdf: Option<SdfOp>) -> anyhow::Result<()> {
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = EmbeddedPreviewApp::new(config, sdf);
    event_loop.run_app(&mut app)?;

    Ok(())
}

/// Controls help text for embedded preview
pub fn embedded_controls_help() -> &'static str {
    r#"
Embedded Preview Controls:
  Left Mouse Drag   - Orbit camera around target
  Right Mouse Drag  - Pan camera
  Middle Mouse Drag - Zoom camera
  Scroll Wheel      - Zoom camera
  Shift + Left Drag - Pan camera (alternative)
  R                 - Reset camera to default
  F                 - Focus on origin
"#
}
