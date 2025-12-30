//! Text overlay rendering using glyphon
//!
//! Provides FPS counter and other text overlay functionality.

// Text rendering is infallible in practice
// u32 to i32 cast is safe for reasonable screen sizes
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::expect_used)]

use glyphon::{
    Attrs, Buffer, Cache, Color, Family, FontSystem, Metrics, Shaping, SwashCache, TextArea,
    TextAtlas, TextBounds, TextRenderer, Viewport,
};
use std::collections::VecDeque;
use std::time::Instant;

/// FPS counter that tracks frame timing
pub struct FpsCounter {
    frame_times: VecDeque<Instant>,
    last_frame: Instant,
    fps: f32,
    frame_time_ms: f32,
}

impl Default for FpsCounter {
    fn default() -> Self {
        Self::new()
    }
}

impl FpsCounter {
    /// Create a new FPS counter
    pub fn new() -> Self {
        Self {
            frame_times: VecDeque::with_capacity(120),
            last_frame: Instant::now(),
            fps: 0.0,
            frame_time_ms: 0.0,
        }
    }

    /// Record a new frame and update FPS calculation
    pub fn tick(&mut self) {
        let now = Instant::now();
        let frame_duration = now.duration_since(self.last_frame);
        self.frame_time_ms = frame_duration.as_secs_f32() * 1000.0;
        self.last_frame = now;

        self.frame_times.push_back(now);

        // Keep only frames from the last second
        while let Some(front) = self.frame_times.front() {
            if now.duration_since(*front).as_secs_f32() > 1.0 {
                self.frame_times.pop_front();
            } else {
                break;
            }
        }

        self.fps = self.frame_times.len() as f32;
    }

    /// Get current FPS
    pub fn fps(&self) -> f32 {
        self.fps
    }

    /// Get last frame time in milliseconds
    pub fn frame_time_ms(&self) -> f32 {
        self.frame_time_ms
    }

    /// Get formatted FPS string
    pub fn display_string(&self) -> String {
        format!("{:.0} FPS ({:.1}ms)", self.fps, self.frame_time_ms)
    }
}

/// Text overlay renderer using glyphon
pub struct TextOverlay {
    font_system: FontSystem,
    swash_cache: SwashCache,
    atlas: TextAtlas,
    text_renderer: TextRenderer,
    buffer: Buffer,
    #[allow(dead_code)] // Keep cache alive for atlas lifetime
    cache: Cache,
    viewport: Viewport,
}

impl TextOverlay {
    /// Create a new text overlay renderer
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let mut font_system = FontSystem::new();
        let swash_cache = SwashCache::new();
        let cache = Cache::new(device);
        let mut atlas = TextAtlas::new(device, queue, &cache, format);
        let text_renderer =
            TextRenderer::new(&mut atlas, device, wgpu::MultisampleState::default(), None);
        let viewport = Viewport::new(device, &cache);

        let mut buffer = Buffer::new(&mut font_system, Metrics::new(18.0, 22.0));
        buffer.set_size(&mut font_system, Some(300.0), Some(50.0));

        Self {
            font_system,
            swash_cache,
            atlas,
            text_renderer,
            buffer,
            cache,
            viewport,
        }
    }

    /// Update the text content
    pub fn set_text(&mut self, text: &str) {
        self.buffer.set_text(
            &mut self.font_system,
            text,
            &Attrs::new().family(Family::Monospace),
            Shaping::Advanced,
        );
    }

    /// Render the text overlay
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        width: u32,
        height: u32,
    ) {
        self.buffer.shape_until_scroll(&mut self.font_system, false);

        // Update viewport size
        self.viewport
            .update(queue, glyphon::Resolution { width, height });

        let text_areas = [TextArea {
            buffer: &self.buffer,
            left: 10.0,
            top: 10.0,
            scale: 1.0,
            bounds: TextBounds {
                left: 0,
                top: 0,
                right: width as i32,
                bottom: height as i32,
            },
            default_color: Color::rgb(255, 255, 255),
            custom_glyphs: &[],
        }];

        self.text_renderer
            .prepare(
                device,
                queue,
                &mut self.font_system,
                &mut self.atlas,
                &self.viewport,
                text_areas,
                &mut self.swash_cache,
            )
            .expect("Failed to prepare text");

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Text Overlay Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load, // Don't clear, overlay on top
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            self.text_renderer
                .render(&self.atlas, &self.viewport, &mut pass)
                .expect("Failed to render text");
        }

        self.atlas.trim();
    }
}

/// Combined FPS counter and overlay for easy integration
pub struct FpsOverlay {
    counter: FpsCounter,
    overlay: TextOverlay,
}

impl FpsOverlay {
    /// Create a new FPS overlay
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        Self {
            counter: FpsCounter::new(),
            overlay: TextOverlay::new(device, queue, format),
        }
    }

    /// Update FPS counter (call once per frame, before render)
    pub fn tick(&mut self) {
        self.counter.tick();
        self.overlay.set_text(&self.counter.display_string());
    }

    /// Render the FPS overlay
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        width: u32,
        height: u32,
    ) {
        self.overlay
            .render(device, queue, encoder, view, width, height);
    }

    /// Get current FPS
    pub fn fps(&self) -> f32 {
        self.counter.fps()
    }

    /// Get last frame time in milliseconds
    pub fn frame_time_ms(&self) -> f32 {
        self.counter.frame_time_ms()
    }
}
