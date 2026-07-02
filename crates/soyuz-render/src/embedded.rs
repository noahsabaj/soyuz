//! Embedded WGPU preview that runs as an X11 child window
//!
//! This module provides a preview renderer that can be embedded inside
//! another window (like a Dioxus app) as a native child window.
//!
//! Since winit's event loop must run on the main thread, this is designed
//! to be run as a separate process with a parent window ID passed in.

// Input state tracks multiple mouse buttons independently
// Collapsible if is clearer as two separate conditions
// Let-else pattern is less readable for complex async init
// Raw strings are clearer without unnecessary hashes
// GPU initialization patterns are clearer with match
// Closures are clearer than method references in this context
// Default trait access is consistent with wgpu patterns
#![allow(clippy::struct_excessive_bools)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::manual_let_else)]
#![allow(clippy::needless_raw_string_hashes)]
#![allow(clippy::single_match_else)]
#![allow(clippy::redundant_closure_for_method_calls)]
#![allow(clippy::default_trait_access)]

use crate::camera_controller::CameraController;
use crate::raymarcher::Raymarcher;
use crate::text_overlay::FpsOverlay;
use soyuz_sdf::SdfOp;
use std::sync::Arc;
use std::time::Instant;
use winit::{
    application::ApplicationHandler,
    dpi::{PhysicalPosition, PhysicalSize},
    event::{ElementState, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Window, WindowId},
};

#[cfg(target_os = "linux")]
use winit::platform::x11::WindowAttributesExtX11;

/// A surface tied to its window, ensuring correct lifetime management.
///
/// The surface is created from the window and holds a reference to it.
/// This struct ensures the window outlives the surface by storing them together.
/// A custom `Drop` impl explicitly drops the surface before the window, rather
/// than relying on field declaration order.
struct WindowedSurface {
    /// Wrapped in `ManuallyDrop` so the custom `Drop` impl can drop it
    /// before the window.
    surface: std::mem::ManuallyDrop<wgpu::Surface<'static>>,
    /// The window that owns the surface's underlying handle
    #[allow(dead_code)]
    window: Arc<Window>,
}

impl WindowedSurface {
    /// Create a new windowed surface.
    ///
    /// # Safety
    /// This is safe because:
    /// 1. The surface is created from the window
    /// 2. The window is stored in the same struct
    /// 3. The custom `Drop` impl explicitly drops the surface before the window
    fn new(
        instance: &wgpu::Instance,
        window: Arc<Window>,
    ) -> Result<Self, wgpu::CreateSurfaceError> {
        let surface = instance.create_surface(window.clone())?;
        // SAFETY: The surface is created from `window`, and `window` is stored
        // in this struct. The custom `Drop` impl explicitly drops the surface
        // before the window, guaranteeing the window outlives the surface.
        #[allow(unsafe_code)]
        let surface =
            unsafe { std::mem::transmute::<wgpu::Surface<'_>, wgpu::Surface<'static>>(surface) };
        Ok(Self {
            surface: std::mem::ManuallyDrop::new(surface),
            window,
        })
    }

    fn surface(&self) -> &wgpu::Surface<'static> {
        &self.surface
    }

    fn window(&self) -> &Arc<Window> {
        &self.window
    }
}

impl Drop for WindowedSurface {
    fn drop(&mut self) {
        // SAFETY: Drop the surface first, while the window handle is still alive.
        // ManuallyDrop makes this ordering explicit rather than relying on field declaration order.
        #[allow(unsafe_code)]
        unsafe {
            std::mem::ManuallyDrop::drop(&mut self.surface);
        }
    }
}

/// Configuration for the embedded preview window
#[derive(Debug, Clone)]
pub struct EmbeddedConfig {
    /// Parent window X11 handle (0 means standalone window)
    pub parent_handle: u32,
    /// Initial position relative to parent, in physical (device) pixels
    pub x: i32,
    pub y: i32,
    /// Initial size, in physical (device) pixels
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

/// Application state for the embedded preview
struct EmbeddedPreviewApp {
    config: EmbeddedConfig,
    sdf: Option<SdfOp>,
    windowed_surface: Option<WindowedSurface>,
    surface_config: Option<wgpu::SurfaceConfiguration>,
    device: Option<Arc<wgpu::Device>>,
    queue: Option<Arc<wgpu::Queue>>,
    raymarcher: Option<Raymarcher>,
    fps_overlay: Option<FpsOverlay>,
    controller: CameraController,
    start_time: Instant,
    /// Created in `resumed` together with the surface: the backend set is
    /// chosen there (Vulkan first, GL fallback), and a surface is only valid
    /// with adapters from its own instance.
    instance: Option<wgpu::Instance>,
    is_embedded: bool,
    /// One-shot flag so the first successful present leaves a breadcrumb on
    /// stderr; diagnosing "embedded preview stays blank" needs to know whether
    /// any frame was ever presented.
    first_frame_logged: bool,
}

/// Wall-clock UTC `HH:MM:SS.mmm` for breadcrumbs. Matches the studio's
/// tracing timestamps (UTC) so logs from multiple preview processes and the
/// studio itself can be correlated on one timeline.
fn wallclock() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    format!(
        "{:02}:{:02}:{:02}.{:03}Z",
        (secs / 3600) % 24,
        (secs / 60) % 60,
        secs % 60,
        now.subsec_millis()
    )
}

/// Stderr breadcrumb for the embedded child's startup sequence. The child
/// inherits the studio's stderr, so these land in the launching terminal/log
/// and make presentation stalls diagnosable in the field. PID and wall-clock
/// disambiguate interleaved output when several children overlap.
macro_rules! preview_log {
    ($self:expr, $($arg:tt)*) => {
        eprintln!(
            "[soyuz-preview pid {} @{} +{:>4}ms] {}",
            std::process::id(),
            wallclock(),
            $self.start_time.elapsed().as_millis(),
            format_args!($($arg)*)
        )
    };
}

impl EmbeddedPreviewApp {
    fn new(config: EmbeddedConfig, sdf: Option<SdfOp>) -> Self {
        let is_embedded = config.parent_handle != 0;

        Self {
            config,
            sdf,
            windowed_surface: None,
            surface_config: None,
            device: None,
            queue: None,
            raymarcher: None,
            fps_overlay: None,
            controller: CameraController::new(),
            start_time: Instant::now(),
            instance: None,
            is_embedded,
            first_frame_logged: false,
        }
    }

    /// Create an instance limited to `backends`, a surface on it, and a
    /// matching adapter. Split per-backend because a surface only pairs with
    /// adapters from its own instance: the Vulkan-first attempt must not
    /// touch GL at all — EGL/GLVND initialization inside `request_adapter`
    /// is the classic multi-second stall on a cold driver stack (first GPU
    /// client after boot), and it is pure waste on any box with Vulkan.
    fn init_gpu(
        &self,
        window: &Arc<Window>,
        backends: wgpu::Backends,
    ) -> Option<(wgpu::Instance, WindowedSurface, wgpu::Adapter)> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends,
            ..Default::default()
        });

        let windowed_surface = match WindowedSurface::new(&instance, window.clone()) {
            Ok(ws) => ws,
            Err(e) => {
                preview_log!(self, "surface creation FAILED ({backends:?}): {e}");
                return None;
            }
        };
        preview_log!(self, "surface created ({backends:?})");

        preview_log!(self, "requesting adapter ({backends:?})...");
        let adapter =
            match pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(windowed_surface.surface()),
                force_fallback_adapter: false,
            })) {
                Ok(adapter) => adapter,
                Err(e) => {
                    preview_log!(self, "no adapter for {backends:?}: {e}");
                    return None;
                }
            };
        preview_log!(
            self,
            "adapter: {} ({:?})",
            adapter.get_info().name,
            adapter.get_info().backend
        );

        Some((instance, windowed_surface, adapter))
    }

    fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            if let (Some(config), Some(ws), Some(device)) = (
                &mut self.surface_config,
                &self.windowed_surface,
                &self.device,
            ) {
                config.width = new_size.width;
                config.height = new_size.height;
                ws.surface().configure(device, config);
                self.controller.camera.aspect = new_size.width as f32 / new_size.height as f32;
            }
        }
    }

    fn render(&mut self) {
        let (Some(ws), Some(raymarcher), Some(config), Some(device), Some(queue)) = (
            &self.windowed_surface,
            &self.raymarcher,
            &self.surface_config,
            &self.device,
            &self.queue,
        ) else {
            return;
        };

        let surface = ws.surface();
        let output = match surface.get_current_texture() {
            Ok(output) => output,
            Err(e @ (wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated)) => {
                if !self.first_frame_logged {
                    preview_log!(self, "surface {e:?} before first frame; reconfiguring");
                }
                if let (Some(device), Some(config)) = (&self.device, &self.surface_config) {
                    surface.configure(device, config);
                }
                return;
            }
            Err(e) => {
                preview_log!(self, "surface error: {e:?}");
                tracing::error!("Surface error: {:?}", e);
                return;
            }
        };

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let time = self.start_time.elapsed().as_secs_f32();
        raymarcher.update_uniforms(
            &self.controller.camera,
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
        if !self.first_frame_logged {
            self.first_frame_logged = true;
            preview_log!(self, "first frame presented");
            // Embedded windows were created hidden; map only now that real
            // content exists, so a slow GPU bring-up never parks an unpainted
            // window over the studio. Ask for another redraw immediately: X11
            // discards presents to unmapped windows, so the first post-map
            // frame is what actually reaches the screen.
            if self.is_embedded {
                ws.window().set_visible(true);
                ws.window().request_redraw();
                preview_log!(self, "embedded window mapped");
            }
        }
    }
}

impl ApplicationHandler for EmbeddedPreviewApp {
    #[allow(clippy::too_many_lines)] // One linear window/GPU init sequence with breadcrumbs
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.windowed_surface.is_some() {
            return;
        }

        // Build window attributes. The rect arrives in physical pixels
        // (captured as CSS px x devicePixelRatio on the studio side), so apply
        // it as Physical* — letting winit re-scale a Logical rect would double
        // the display scale on fractional-DPI setups.
        //
        // Embedded windows start hidden and are mapped only after the first
        // frame is presented: GPU bring-up can stall for seconds on a cold
        // driver, and an already-mapped-but-never-painted X11 window sits as
        // a white hole over the studio UI for that whole time.
        let mut window_attrs = Window::default_attributes()
            .with_title(&self.config.title)
            .with_inner_size(PhysicalSize::new(self.config.width, self.config.height))
            .with_position(PhysicalPosition::new(self.config.x, self.config.y))
            .with_decorations(self.config.decorated)
            .with_visible(!self.is_embedded);

        // If we have a parent window handle, embed as child (X11 only)
        #[cfg(target_os = "linux")]
        if self.config.parent_handle != 0 {
            window_attrs = window_attrs.with_embed_parent_window(self.config.parent_handle);
        }

        let window = match event_loop.create_window(window_attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                preview_log!(self, "window creation FAILED: {e}");
                tracing::error!("Failed to create window: {}", e);
                event_loop.exit();
                return;
            }
        };
        preview_log!(
            self,
            "window created (embedded: {}, inner: {:?}, scale: {})",
            self.is_embedded,
            window.inner_size(),
            window.scale_factor()
        );

        // Any exposure between mapping and the next present shows the X11
        // window background; match the studio's preview pane color so that
        // moment is invisible instead of server-default white.
        #[cfg(target_os = "linux")]
        if self.is_embedded {
            set_x11_background_pixel(&window, crate::theme_generated::PREVIEW_CANVAS_BG_X11);
        }

        // Initialize WGPU: Vulkan (PRIMARY) first, GL only as a fallback.
        let (instance, windowed_surface, adapter) =
            match self.init_gpu(&window, wgpu::Backends::PRIMARY).or_else(|| {
                preview_log!(self, "no PRIMARY (Vulkan) adapter; trying GL fallback");
                self.init_gpu(&window, wgpu::Backends::GL)
            }) {
                Some(parts) => parts,
                None => {
                    preview_log!(self, "WGPU init FAILED (no adapter on any backend)");
                    tracing::error!("Failed to initialize WGPU");
                    event_loop.exit();
                    return;
                }
            };

        let surface = windowed_surface.surface();

        preview_log!(self, "requesting device...");
        let (device, queue) =
            match pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                label: Some("Soyuz Embedded Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: Default::default(),
                trace: wgpu::Trace::Off,
            })) {
                Ok((device, queue)) => (Arc::new(device), Arc::new(queue)),
                Err(e) => {
                    preview_log!(self, "device request FAILED: {e}");
                    tracing::error!("Failed to get WGPU device: {e}");
                    event_loop.exit();
                    return;
                }
            };

        let caps = surface.get_capabilities(&adapter);
        // Prefer sRGB; fall back to the first format. Don't index `formats`
        // eagerly — it can be empty.
        let format = match caps
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .or_else(|| caps.formats.first().copied())
        {
            Some(format) => format,
            None => {
                preview_log!(self, "WGPU init FAILED (surface reports no formats)");
                tracing::error!("Failed to initialize WGPU: no surface formats");
                event_loop.exit();
                return;
            }
        };
        preview_log!(self, "device ready, format {format:?}");

        let size = windowed_surface.window().inner_size();
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
        self.controller.camera.aspect = size.width as f32 / size.height.max(1) as f32;

        // Store everything
        self.instance = Some(instance);
        self.windowed_surface = Some(windowed_surface);
        self.surface_config = Some(surface_config);
        self.device = Some(device);
        self.queue = Some(queue);
        self.raymarcher = Some(raymarcher);
        self.fps_overlay = Some(fps_overlay);

        preview_log!(self, "renderer ready ({}x{})", size.width, size.height);
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
                if let Some(ws) = &self.windowed_surface {
                    ws.window().request_redraw();
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                self.controller.handle_mouse_button(state, button);
            }
            WindowEvent::Focused(true) if self.is_embedded => {
                // Diagnostics only: a docked child should never receive X11
                // keyboard focus. The studio heals focus from its side; the
                // child deliberately does not touch focus (an unexpected
                // SetInputFocus from here confuses the WM's focus tracking).
                preview_log!(self, "focus gained (unexpected for a docked child)");
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.controller.handle_mouse_motion(position);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                self.controller.handle_scroll(delta);
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.controller.handle_modifiers(&modifiers);
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                match event.logical_key {
                    // Only close on Escape if not embedded.
                    Key::Named(NamedKey::Escape) if !self.is_embedded => {
                        event_loop.exit();
                    }
                    Key::Character(ref c) if c == "r" || c == "R" => {
                        self.controller.reset_camera();
                    }
                    Key::Character(ref c) if c == "f" || c == "F" => {
                        self.controller.focus_origin();
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(ws) = &self.windowed_surface {
            ws.window().request_redraw();
        }
    }
}

/// Set the X11 window background pixel, shown whenever the server exposes the
/// window before the renderer's next present (initial map, resizes). Without
/// it those moments flash server-default white over the studio UI.
#[cfg(target_os = "linux")]
fn set_x11_background_pixel(window: &Window, color: u32) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{ChangeWindowAttributesAux, ConnectionExt};

    let xid = window.window_handle().ok().and_then(|h| match h.as_raw() {
        RawWindowHandle::Xlib(h) => u32::try_from(h.window).ok(),
        RawWindowHandle::Xcb(h) => Some(h.window.get()),
        _ => None,
    });
    let Some(xid) = xid else {
        return;
    };
    let Ok((conn, _)) = x11rb::connect(None) else {
        return;
    };
    let _ = conn.change_window_attributes(
        xid,
        &ChangeWindowAttributesAux::new().background_pixel(color),
    );
    let _ = conn.flush();
}

/// Run the embedded preview (call this from main thread)
pub fn run_embedded_preview(config: EmbeddedConfig, sdf: Option<SdfOp>) -> anyhow::Result<()> {
    // X11 embedding requires an X11 event loop, but winit's backend
    // auto-selection prefers Wayland whenever WAYLAND_DISPLAY is set — even
    // when the studio window we must embed into is X11 (e.g. GDK_BACKEND=x11
    // under XWayland). On Wayland the embed parent and position are silently
    // ignored and the "docked" preview becomes a floating top-level window,
    // so force the X11 backend whenever a parent handle was supplied.
    #[cfg(target_os = "linux")]
    let event_loop = if config.parent_handle != 0 {
        use winit::platform::x11::EventLoopBuilderExtX11;
        EventLoop::builder().with_x11().build()?
    } else {
        EventLoop::new()?
    };
    #[cfg(not(target_os = "linux"))]
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
