//! Mesh generation from SDFs using Marching Cubes
//!
//! Uses Rayon for parallel processing of voxel grids.

mod lod;
mod marching_cubes;
mod optimize;

use crate::Result;
use crate::sdf::{Aabb, Sdf};
use glam::{Vec2, Vec3};
use rayon::prelude::*;

pub use lod::{LodConfig, LodLevel, LodMesh};
pub use marching_cubes::EDGE_TABLE;
pub use marching_cubes::TRI_TABLE;
pub use optimize::OptimizeConfig;

/// A vertex with position, normal, and UV coordinates
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}

impl Vertex {
    pub fn new(position: Vec3, normal: Vec3, uv: Vec2) -> Self {
        Self {
            position: position.to_array(),
            normal: normal.to_array(),
            uv: uv.to_array(),
        }
    }
}

/// A triangle mesh
#[derive(Debug, Clone, Default)]
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

impl Mesh {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get number of triangles
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    /// Get number of vertices
    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    /// Calculate face normals and smooth them
    pub fn recalculate_normals(&mut self) {
        // Reset normals
        for v in &mut self.vertices {
            v.normal = [0.0, 0.0, 0.0];
        }

        // Accumulate face normals
        for tri in self.indices.chunks(3) {
            let i0 = tri[0] as usize;
            let i1 = tri[1] as usize;
            let i2 = tri[2] as usize;

            let p0 = Vec3::from_array(self.vertices[i0].position);
            let p1 = Vec3::from_array(self.vertices[i1].position);
            let p2 = Vec3::from_array(self.vertices[i2].position);

            let edge1 = p1 - p0;
            let edge2 = p2 - p0;
            let face_normal = edge1.cross(edge2);

            // Add to each vertex (will be normalized later)
            for &i in &[i0, i1, i2] {
                self.vertices[i].normal[0] += face_normal.x;
                self.vertices[i].normal[1] += face_normal.y;
                self.vertices[i].normal[2] += face_normal.z;
            }
        }

        // Normalize
        for v in &mut self.vertices {
            let n = Vec3::from_array(v.normal);
            let normalized = n.normalize_or_zero();
            v.normal = normalized.to_array();
        }
    }

    /// Generate simple UV coordinates (planar projection)
    pub fn generate_uvs_planar(&mut self, axis: Vec3, scale: f32) {
        let up = if axis.y.abs() > 0.9 { Vec3::X } else { Vec3::Y };
        let right = axis.cross(up).normalize();
        let forward = right.cross(axis).normalize();

        for v in &mut self.vertices {
            let p = Vec3::from_array(v.position);
            v.uv = [p.dot(right) * scale, p.dot(forward) * scale];
        }
    }

    /// Generate triplanar UV coordinates
    pub fn generate_uvs_triplanar(&mut self, scale: f32) {
        for v in &mut self.vertices {
            let p = Vec3::from_array(v.position);
            let n = Vec3::from_array(v.normal).abs();

            // Blend based on normal direction
            let uv_x = Vec2::new(p.y, p.z) * scale;
            let uv_y = Vec2::new(p.x, p.z) * scale;
            let uv_z = Vec2::new(p.x, p.y) * scale;

            let uv = uv_x * n.x + uv_y * n.y + uv_z * n.z;
            v.uv = uv.to_array();
        }
    }

    /// Generate box projection UVs (6 faces)
    ///
    /// Projects each triangle onto the axis-aligned face most aligned with its normal.
    /// Results in cleaner UVs than triplanar for box-like shapes.
    pub fn generate_uvs_box(&mut self, scale: f32) {
        // Process each triangle
        for tri in self.indices.chunks(3) {
            let i0 = tri[0] as usize;
            let i1 = tri[1] as usize;
            let i2 = tri[2] as usize;

            // Calculate face normal
            let p0 = Vec3::from_array(self.vertices[i0].position);
            let p1 = Vec3::from_array(self.vertices[i1].position);
            let p2 = Vec3::from_array(self.vertices[i2].position);

            let face_normal = (p1 - p0).cross(p2 - p0).normalize_or_zero();
            let abs_normal = face_normal.abs();

            // Determine dominant axis
            let (u_axis, v_axis) = if abs_normal.x >= abs_normal.y && abs_normal.x >= abs_normal.z {
                // X-facing: project onto YZ plane
                (Vec3::Y, Vec3::Z)
            } else if abs_normal.y >= abs_normal.z {
                // Y-facing: project onto XZ plane
                (Vec3::X, Vec3::Z)
            } else {
                // Z-facing: project onto XY plane
                (Vec3::X, Vec3::Y)
            };

            // Project vertices
            for &idx in &[i0, i1, i2] {
                let p = Vec3::from_array(self.vertices[idx].position);
                self.vertices[idx].uv = [p.dot(u_axis) * scale, p.dot(v_axis) * scale];
            }
        }
    }

    /// Generate cylindrical UV coordinates
    ///
    /// Projects UVs as if wrapping a cylinder around the Y axis.
    pub fn generate_uvs_cylindrical(&mut self, scale: f32) {
        for v in &mut self.vertices {
            let p = Vec3::from_array(v.position);
            let u = (p.x.atan2(p.z) / std::f32::consts::TAU + 0.5) * scale;
            let v_coord = p.y * scale;
            v.uv = [u, v_coord];
        }
    }

    /// Generate spherical UV coordinates
    ///
    /// Projects UVs as if wrapping a sphere.
    pub fn generate_uvs_spherical(&mut self, scale: f32) {
        for v in &mut self.vertices {
            let p = Vec3::from_array(v.position).normalize_or_zero();
            let u = (p.x.atan2(p.z) / std::f32::consts::TAU + 0.5) * scale;
            let v_coord = (p.y.asin() / std::f32::consts::PI + 0.5) * scale;
            v.uv = [u, v_coord];
        }
    }

    /// Automatically choose the best UV projection based on mesh bounds
    pub fn generate_uvs_auto(&mut self, scale: f32) {
        // Calculate bounding box
        let mut min = Vec3::splat(f32::MAX);
        let mut max = Vec3::splat(f32::MIN);

        for v in &self.vertices {
            let p = Vec3::from_array(v.position);
            min = min.min(p);
            max = max.max(p);
        }

        let size = max - min;
        let aspect_xz = size.x.max(size.z) / size.y.max(0.001);

        // Choose projection based on shape
        if aspect_xz > 2.0 {
            // Flat/wide shape: use planar from above
            self.generate_uvs_planar(Vec3::Y, scale);
        } else if aspect_xz < 0.5 {
            // Tall shape: use cylindrical
            self.generate_uvs_cylindrical(scale);
        } else {
            // Roughly cubic: use triplanar
            self.generate_uvs_triplanar(scale);
        }
    }
}

/// Configuration for mesh generation
#[derive(Debug, Clone)]
pub struct MeshConfig {
    /// Grid resolution (number of cells along each axis)
    pub resolution: u32,
    /// Bounding box to sample within
    pub bounds: Aabb,
    /// ISO level (distance value for surface extraction)
    pub iso_level: f32,
    /// Whether to compute normals from the SDF gradient
    pub compute_normals: bool,
}

impl Default for MeshConfig {
    fn default() -> Self {
        Self {
            resolution: 64,
            bounds: Aabb::cube(2.0),
            iso_level: 0.0,
            compute_normals: true,
        }
    }
}

impl MeshConfig {
    pub fn with_resolution(mut self, resolution: u32) -> Self {
        self.resolution = resolution;
        self
    }

    pub fn with_bounds(mut self, bounds: Aabb) -> Self {
        self.bounds = bounds;
        self
    }

    pub fn with_iso_level(mut self, iso_level: f32) -> Self {
        self.iso_level = iso_level;
        self
    }
}

/// Extension trait to generate meshes from SDFs
pub trait SdfToMesh: Sdf + Sync {
    /// Generate a mesh from this SDF
    ///
    /// Uses Rayon for parallel mesh generation across all CPU cores.
    fn to_mesh(&self, config: MeshConfig) -> Result<Mesh> {
        generate_mesh(self, config)
    }

    /// Generate mesh with default config
    fn to_mesh_default(&self) -> Result<Mesh> {
        self.to_mesh(MeshConfig::default())
    }
}

impl<T: Sdf + Sync> SdfToMesh for T {}

/// Generate a mesh from an SDF using marching cubes
///
/// Uses Rayon for parallel processing:
/// - Parallel SDF sampling to build the distance field
/// - Parallel cell processing for marching cubes triangulation
pub fn generate_mesh<S: Sdf + ?Sized + Sync>(sdf: &S, config: MeshConfig) -> Result<Mesh> {
    let res = config.resolution;
    let bounds = config.bounds;
    let size = bounds.size();
    let step = size / res as f32;
    let grid_size = (res + 1) as usize;

    // === Phase 1: Parallel SDF sampling ===
    // Create all grid point indices
    let total_points = grid_size * grid_size * grid_size;
    let values: Vec<f32> = (0..total_points)
        .into_par_iter()
        .map(|idx| {
            let x = idx % grid_size;
            let y = (idx / grid_size) % grid_size;
            let z = idx / (grid_size * grid_size);
            let p = bounds.min + Vec3::new(x as f32, y as f32, z as f32) * step;
            sdf.distance(p)
        })
        .collect();

    // === Phase 2: Parallel marching cubes ===
    // Process cells in parallel, each cell produces local triangles
    let total_cells = (res * res * res) as usize;

    let cell_results: Vec<CellTriangles> = (0..total_cells)
        .into_par_iter()
        .filter_map(|cell_idx| {
            let x = (cell_idx % res as usize) as u32;
            let y = ((cell_idx / res as usize) % res as usize) as u32;
            let z = (cell_idx / (res * res) as usize) as u32;

            process_cell(x, y, z, &values, grid_size, &bounds, step, &config, sdf)
        })
        .collect();

    // === Phase 3: Merge results ===
    let mut mesh = Mesh::new();

    // Pre-calculate total size for efficiency
    let total_verts: usize = cell_results.iter().map(|c| c.vertices.len()).sum();
    let total_indices: usize = cell_results.iter().map(|c| c.indices.len()).sum();

    mesh.vertices.reserve(total_verts);
    mesh.indices.reserve(total_indices);

    for cell in cell_results {
        let base_idx = mesh.vertices.len() as u32;
        mesh.vertices.extend(cell.vertices);
        mesh.indices
            .extend(cell.indices.iter().map(|&i| i + base_idx));
    }

    // Generate UVs
    mesh.generate_uvs_triplanar(1.0);

    Ok(mesh)
}

/// Triangles generated by a single cell
struct CellTriangles {
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
}

/// Process a single marching cubes cell
fn process_cell<S: Sdf + ?Sized>(
    x: u32,
    y: u32,
    z: u32,
    values: &[f32],
    grid_size: usize,
    bounds: &Aabb,
    step: Vec3,
    config: &MeshConfig,
    sdf: &S,
) -> Option<CellTriangles> {
    let corner_positions = [
        bounds.min + Vec3::new(x as f32, y as f32, z as f32) * step,
        bounds.min + Vec3::new((x + 1) as f32, y as f32, z as f32) * step,
        bounds.min + Vec3::new((x + 1) as f32, (y + 1) as f32, z as f32) * step,
        bounds.min + Vec3::new(x as f32, (y + 1) as f32, z as f32) * step,
        bounds.min + Vec3::new(x as f32, y as f32, (z + 1) as f32) * step,
        bounds.min + Vec3::new((x + 1) as f32, y as f32, (z + 1) as f32) * step,
        bounds.min + Vec3::new((x + 1) as f32, (y + 1) as f32, (z + 1) as f32) * step,
        bounds.min + Vec3::new(x as f32, (y + 1) as f32, (z + 1) as f32) * step,
    ];

    let corner_values = [
        values[(z as usize * grid_size + y as usize) * grid_size + x as usize],
        values[(z as usize * grid_size + y as usize) * grid_size + (x + 1) as usize],
        values[(z as usize * grid_size + (y + 1) as usize) * grid_size + (x + 1) as usize],
        values[(z as usize * grid_size + (y + 1) as usize) * grid_size + x as usize],
        values[((z + 1) as usize * grid_size + y as usize) * grid_size + x as usize],
        values[((z + 1) as usize * grid_size + y as usize) * grid_size + (x + 1) as usize],
        values[((z + 1) as usize * grid_size + (y + 1) as usize) * grid_size + (x + 1) as usize],
        values[((z + 1) as usize * grid_size + (y + 1) as usize) * grid_size + x as usize],
    ];

    // Determine cube index
    let mut cube_index = 0u8;
    for i in 0..8 {
        if corner_values[i] < config.iso_level {
            cube_index |= 1 << i;
        }
    }

    // Skip if entirely inside or outside
    if cube_index == 0 || cube_index == 255 {
        return None;
    }

    // Get edge flags
    let edge_flags = EDGE_TABLE[cube_index as usize];

    // Interpolate vertices on edges
    let mut edge_vertices = [Vec3::ZERO; 12];

    for edge in 0..12 {
        if (edge_flags & (1 << edge)) != 0 {
            let (v0, v1) = EDGE_CORNERS[edge];
            let p0 = corner_positions[v0];
            let p1 = corner_positions[v1];
            let val0 = corner_values[v0];
            let val1 = corner_values[v1];

            let t = if (val1 - val0).abs() > 0.00001 {
                (config.iso_level - val0) / (val1 - val0)
            } else {
                0.5
            };

            edge_vertices[edge] = p0.lerp(p1, t);
        }
    }

    // Generate triangles for this cell
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    let tri_indices = &TRI_TABLE[cube_index as usize];
    let mut i = 0;
    while tri_indices[i] != -1 {
        let base_idx = vertices.len() as u32;

        for j in 0..3 {
            let edge_idx = tri_indices[i + j] as usize;
            let pos = edge_vertices[edge_idx];

            // Compute normal from SDF gradient
            let normal = if config.compute_normals {
                compute_gradient(sdf, pos, 0.001)
            } else {
                Vec3::Y
            };

            vertices.push(Vertex::new(pos, normal, Vec2::ZERO));
        }

        // Wind triangles correctly
        indices.push(base_idx);
        indices.push(base_idx + 1);
        indices.push(base_idx + 2);

        i += 3;
    }

    Some(CellTriangles { vertices, indices })
}

/// Compute the gradient (normal) of an SDF at a point
fn compute_gradient<S: Sdf + ?Sized>(sdf: &S, p: Vec3, eps: f32) -> Vec3 {
    let dx = sdf.distance(p + Vec3::X * eps) - sdf.distance(p - Vec3::X * eps);
    let dy = sdf.distance(p + Vec3::Y * eps) - sdf.distance(p - Vec3::Y * eps);
    let dz = sdf.distance(p + Vec3::Z * eps) - sdf.distance(p - Vec3::Z * eps);
    Vec3::new(dx, dy, dz).normalize_or_zero()
}

/// Edge to corner vertex mapping for marching cubes
const EDGE_CORNERS: [(usize, usize); 12] = [
    (0, 1),
    (1, 2),
    (2, 3),
    (3, 0),
    (4, 5),
    (5, 6),
    (6, 7),
    (7, 4),
    (0, 4),
    (1, 5),
    (2, 6),
    (3, 7),
];
