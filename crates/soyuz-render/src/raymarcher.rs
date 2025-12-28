//! GPU-based raymarching renderer for SDFs

use crate::camera::Camera;
use crate::environment::{Environment, EnvironmentUniforms};
use crate::shader_gen::{self, SdfOp};
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use std::sync::Arc;
use wgpu::util::DeviceExt;

/// Uniform buffer data sent to the GPU
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct Uniforms {
    pub view_proj: [[f32; 4]; 4],
    pub camera_pos: [f32; 3],
    pub time: f32,
    pub resolution: [f32; 2],
    pub near: f32,
    pub far: f32,
    pub camera_right: [f32; 3],
    pub _pad1: f32,
    pub camera_up: [f32; 3],
    pub _pad2: f32,
    pub camera_forward: [f32; 3],
    pub fov_tan: f32,
}

impl Uniforms {
    pub fn from_camera(camera: &Camera, resolution: [f32; 2], time: f32) -> Self {
        let view = camera.view_matrix();
        let proj = camera.projection_matrix();

        // Extract camera basis vectors from view matrix
        // View matrix transforms world to camera space, so we need the inverse
        let view_inv = view.inverse();
        let right = Vec3::new(view_inv.x_axis.x, view_inv.x_axis.y, view_inv.x_axis.z);
        let up = Vec3::new(view_inv.y_axis.x, view_inv.y_axis.y, view_inv.y_axis.z);
        let forward = -Vec3::new(view_inv.z_axis.x, view_inv.z_axis.y, view_inv.z_axis.z);

        Self {
            view_proj: (proj * view).to_cols_array_2d(),
            camera_pos: camera.position.to_array(),
            time,
            resolution,
            near: camera.near,
            far: camera.far,
            camera_right: right.to_array(),
            _pad1: 0.0,
            camera_up: up.to_array(),
            _pad2: 0.0,
            camera_forward: forward.to_array(),
            fov_tan: (camera.fov * 0.5).tan(),
        }
    }
}

/// Raymarching renderer using WGPU
pub struct Raymarcher {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    env_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    surface_format: wgpu::TextureFormat,
    current_environment: Environment,
}

impl Raymarcher {
    /// Create a new raymarcher with the default scene
    pub fn new(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        let shader_source = shader_gen::get_base_shader();
        Self::with_shader(device, queue, surface_format, shader_source)
    }

    /// Create a raymarcher with a custom SDF
    pub fn with_sdf(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        surface_format: wgpu::TextureFormat,
        sdf: &SdfOp,
    ) -> Self {
        let shader_source = shader_gen::build_shader(sdf);
        Self::with_shader(device, queue, surface_format, &shader_source)
    }

    /// Create a raymarcher with custom shader source
    pub fn with_shader(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        surface_format: wgpu::TextureFormat,
        shader_source: &str,
    ) -> Self {
        Self::with_shader_and_env(
            device,
            queue,
            surface_format,
            shader_source,
            Environment::default(),
        )
    }

    /// Create a raymarcher with custom shader source and environment
    pub fn with_shader_and_env(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        surface_format: wgpu::TextureFormat,
        shader_source: &str,
        environment: Environment,
    ) -> Self {
        // Create shader module
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Raymarching Shader"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        // Create uniform buffer
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Uniform Buffer"),
            contents: bytemuck::cast_slice(&[Uniforms {
                view_proj: Mat4::IDENTITY.to_cols_array_2d(),
                camera_pos: [0.0, 0.0, 3.0],
                time: 0.0,
                resolution: [800.0, 600.0],
                near: 0.1,
                far: 100.0,
                camera_right: [1.0, 0.0, 0.0],
                _pad1: 0.0,
                camera_up: [0.0, 1.0, 0.0],
                _pad2: 0.0,
                camera_forward: [0.0, 0.0, -1.0],
                fov_tan: 0.414, // tan(22.5deg)
            }]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Create environment buffer
        let env_uniforms = EnvironmentUniforms::from(&environment);
        let env_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Environment Uniform Buffer"),
            contents: bytemuck::cast_slice(&[env_uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Create bind group layout with both uniforms and environment
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Uniform Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // Create bind group
        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Uniform Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: env_buffer.as_entire_binding(),
                },
            ],
        });

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create render pipeline
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Raymarching Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        Self {
            device,
            queue,
            pipeline,
            uniform_buffer,
            env_buffer,
            uniform_bind_group,
            surface_format,
            current_environment: environment,
        }
    }

    /// Create a raymarcher with a custom SDF and environment
    pub fn with_sdf_and_env(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        surface_format: wgpu::TextureFormat,
        sdf: &SdfOp,
        environment: Environment,
    ) -> Self {
        let shader_source = shader_gen::build_shader(sdf);
        Self::with_shader_and_env(device, queue, surface_format, &shader_source, environment)
    }

    /// Update uniforms from camera state
    pub fn update_uniforms(&self, camera: &Camera, resolution: [f32; 2], time: f32) {
        let uniforms = Uniforms::from_camera(camera, resolution, time);
        self.queue
            .write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));
    }

    /// Update environment settings
    pub fn update_environment(&mut self, environment: Environment) {
        self.current_environment = environment;
        let env_uniforms = EnvironmentUniforms::from(&self.current_environment);
        self.queue
            .write_buffer(&self.env_buffer, 0, bytemuck::cast_slice(&[env_uniforms]));
    }

    /// Get current environment settings
    pub fn environment(&self) -> &Environment {
        &self.current_environment
    }

    /// Render a frame to the given texture view
    pub fn render(&self, view: &wgpu::TextureView) {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Raymarching Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.1,
                            b: 0.1,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
            render_pass.draw(0..3, 0..1); // Full-screen triangle
        }

        self.queue.submit(std::iter::once(encoder.finish()));
    }

    /// Render to an image buffer (for headless rendering)
    pub fn render_to_image(
        &self,
        width: u32,
        height: u32,
        camera: &Camera,
        time: f32,
    ) -> image::RgbaImage {
        // Create output texture
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Output Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.surface_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Update uniforms
        self.update_uniforms(camera, [width as f32, height as f32], time);

        // Render
        self.render(&view);

        // Create buffer to read back
        let bytes_per_pixel = 4u32;
        let unpadded_bytes_per_row = width * bytes_per_pixel;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_bytes_per_row = (unpadded_bytes_per_row + align - 1) / align * align;
        let buffer_size = (padded_bytes_per_row * height) as u64;

        let output_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Output Buffer"),
            size: buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        // Copy texture to buffer
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Copy Encoder"),
            });

        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &output_buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        self.queue.submit(std::iter::once(encoder.finish()));

        // Read back
        let buffer_slice = output_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            tx.send(result).unwrap();
        });
        let _ = self.device.poll(wgpu::PollType::Wait);
        rx.recv().unwrap().unwrap();

        let data = buffer_slice.get_mapped_range();

        // Convert to image, handling row padding
        let mut img = image::RgbaImage::new(width, height);
        for y in 0..height {
            let row_start = (y * padded_bytes_per_row) as usize;
            let row_end = row_start + (width * bytes_per_pixel) as usize;
            let row_data = &data[row_start..row_end];

            for x in 0..width {
                let pixel_start = (x * bytes_per_pixel) as usize;
                let pixel = &row_data[pixel_start..pixel_start + 4];
                img.put_pixel(x, y, image::Rgba([pixel[0], pixel[1], pixel[2], pixel[3]]));
            }
        }

        drop(data);
        output_buffer.unmap();

        img
    }

    /// Get the surface format
    pub fn surface_format(&self) -> wgpu::TextureFormat {
        self.surface_format
    }

    /// Recreate the pipeline with a new SDF (preserves current environment)
    pub fn set_sdf(&mut self, sdf: &SdfOp) {
        let env = std::mem::take(&mut self.current_environment);
        let shader_source = shader_gen::build_shader(sdf);
        *self = Self::with_shader_and_env(
            self.device.clone(),
            self.queue.clone(),
            self.surface_format,
            &shader_source,
            env,
        );
    }

    /// Recreate the pipeline with a new SDF and environment
    pub fn set_sdf_and_env(&mut self, sdf: &SdfOp, environment: Environment) {
        let shader_source = shader_gen::build_shader(sdf);
        *self = Self::with_shader_and_env(
            self.device.clone(),
            self.queue.clone(),
            self.surface_format,
            &shader_source,
            environment,
        );
    }
}

/// Initialize WGPU for headless rendering (no window)
pub async fn init_headless() -> (Arc<wgpu::Device>, Arc<wgpu::Queue>) {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        ..Default::default()
    });

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        })
        .await
        .expect("Failed to find an appropriate adapter");

    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("Soyuz Device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: Default::default(),
            trace: wgpu::Trace::Off,
        })
        .await
        .expect("Failed to create device");

    (Arc::new(device), Arc::new(queue))
}

/// Initialize WGPU for windowed rendering
pub async fn init_with_surface(
    instance: &wgpu::Instance,
    surface: &wgpu::Surface<'_>,
) -> (Arc<wgpu::Device>, Arc<wgpu::Queue>, wgpu::TextureFormat) {
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(surface),
            force_fallback_adapter: false,
        })
        .await
        .expect("Failed to find an appropriate adapter");

    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("Soyuz Device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: Default::default(),
            trace: wgpu::Trace::Off,
        })
        .await
        .expect("Failed to create device");

    let surface_caps = surface.get_capabilities(&adapter);
    let surface_format = surface_caps
        .formats
        .iter()
        .copied()
        .find(|f| f.is_srgb())
        .unwrap_or(surface_caps.formats[0]);

    (Arc::new(device), Arc::new(queue), surface_format)
}
