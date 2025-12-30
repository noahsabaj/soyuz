//! Window management with winit for real-time preview

// Raw strings are clearer without unnecessary hashes
// Input state tracks multiple mouse buttons independently
// Lifetime annotation is clearer explicit
// Collapsible if is clearer as two separate conditions
// Format inlining not always clearer
#![allow(clippy::needless_raw_string_hashes)]
#![allow(clippy::struct_excessive_bools)]
#![allow(clippy::elidable_lifetime_names)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::uninlined_format_args)]

use crate::camera::Camera;
use crate::raymarcher::{Raymarcher, init_with_surface};
use crate::text_overlay::FpsOverlay;
use glam::Vec3;
use soyuz_sdf::SdfOp;
use std::sync::Arc;
use std::time::Instant;
use winit::{
    application::ApplicationHandler,
    dpi::{LogicalSize, PhysicalPosition},
    event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Window, WindowId},
};

/// Configuration for the preview window
#[derive(Debug, Clone)]
pub struct WindowConfig {
    pub title: String,
    pub width: u32,
    pub height: u32,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            title: "Soyuz Preview".to_string(),
            width: 1280,
            height: 720,
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

/// Application state for the preview window
struct PreviewApp<'a> {
    config: WindowConfig,
    sdf: Option<SdfOp>,
    window: Option<Arc<Window>>,
    surface: Option<wgpu::Surface<'a>>,
    surface_config: Option<wgpu::SurfaceConfiguration>,
    device: Option<Arc<wgpu::Device>>,
    queue: Option<Arc<wgpu::Queue>>,
    raymarcher: Option<Raymarcher>,
    fps_overlay: Option<FpsOverlay>,
    camera: Camera,
    input: InputState,
    start_time: Instant,
    instance: wgpu::Instance,
}

impl<'a> PreviewApp<'a> {
    fn new(config: WindowConfig, sdf: Option<SdfOp>) -> Self {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

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

                // Update camera aspect ratio
                self.camera.aspect = new_size.width as f32 / new_size.height as f32;
            }
        }
    }

    fn handle_mouse_motion(&mut self, position: PhysicalPosition<f64>) {
        if let Some(last_pos) = self.input.last_mouse_pos {
            let dx = (position.x - last_pos.x) as f32 * 0.005;
            let dy = (position.y - last_pos.y) as f32 * 0.005;

            if self.input.mouse_left {
                // Orbit camera
                self.camera.orbit(dx, dy);
            } else if self.input.mouse_right || (self.input.mouse_left && self.input.shift_held) {
                // Pan camera
                self.camera.pan(-dx * 2.0, dy * 2.0);
            } else if self.input.mouse_middle {
                // Zoom camera
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
                // Reconfigure surface
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

    fn reset_camera(&mut self) {
        self.camera = Camera::default();
    }
}

impl ApplicationHandler for PreviewApp<'_> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        // Create window
        let window_attrs = Window::default_attributes()
            .with_title(&self.config.title)
            .with_inner_size(LogicalSize::new(self.config.width, self.config.height));

        let window = match event_loop.create_window(window_attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                eprintln!("Failed to create window: {}", e);
                event_loop.exit();
                return;
            }
        };

        // Create surface
        let surface = match self.instance.create_surface(window.clone()) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to create surface: {}", e);
                event_loop.exit();
                return;
            }
        };

        // Initialize WGPU
        let (device, queue, format) =
            match pollster::block_on(init_with_surface(&self.instance, &surface)) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("Failed to initialize GPU: {}", e);
                    event_loop.exit();
                    return;
                }
            };

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

        // Create raymarcher
        let raymarcher = if let Some(sdf) = &self.sdf {
            Raymarcher::with_sdf(device.clone(), queue.clone(), format, sdf)
        } else {
            Raymarcher::new(device.clone(), queue.clone(), format)
        };

        // Create FPS overlay
        let fps_overlay = FpsOverlay::new(&device, &queue, format);

        // Update camera aspect
        self.camera.aspect = size.width as f32 / size.height as f32;

        self.window = Some(window);
        self.surface = Some(surface);
        self.surface_config = Some(surface_config);
        self.device = Some(device);
        self.queue = Some(queue);
        self.raymarcher = Some(raymarcher);
        self.fps_overlay = Some(fps_overlay);
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
                            event_loop.exit();
                        }
                        Key::Character(ref c) if c == "r" || c == "R" => {
                            self.reset_camera();
                        }
                        Key::Character(ref c) if c == "f" || c == "F" => {
                            // Focus on origin
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

/// Run the preview window with the default scene
pub fn run_preview(config: WindowConfig) -> anyhow::Result<()> {
    run_preview_with_sdf(config, None)
}

/// Run the preview window with a custom SDF
pub fn run_preview_with_sdf(config: WindowConfig, sdf: Option<SdfOp>) -> anyhow::Result<()> {
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = PreviewApp::new(config, sdf);
    event_loop.run_app(&mut app)?;

    Ok(())
}

/// Preview controls help text
pub fn controls_help() -> &'static str {
    r#"
Preview Controls:
  Left Mouse Drag   - Orbit camera around target
  Right Mouse Drag  - Pan camera
  Middle Mouse Drag - Zoom camera
  Scroll Wheel      - Zoom camera
  Shift + Left Drag - Pan camera (alternative)
  R                 - Reset camera to default
  F                 - Focus on origin
  Escape            - Close preview
"#
}
