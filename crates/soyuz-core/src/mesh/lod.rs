//! Level of Detail (LOD) generation for meshes

// Builder pattern methods intentionally return Self without #[must_use]
// Sorting f32 values that are guaranteed not to be NaN
#![allow(clippy::return_self_not_must_use)]
#![allow(clippy::unwrap_used)]
#![allow(clippy::missing_panics_doc)]

use super::{Mesh, OptimizeConfig};

/// Configuration for LOD generation
#[derive(Debug, Clone)]
pub struct LodConfig {
    /// LOD levels as (distance, `detail_ratio`) pairs
    /// distance: view distance in meters where this LOD is used
    /// `detail_ratio`: 0.0-1.0, percentage of original triangles to keep
    pub levels: Vec<(f32, f32)>,
    /// Maximum error allowed during decimation
    pub max_error: f32,
    /// Whether to preserve mesh boundaries
    pub preserve_boundaries: bool,
}

impl Default for LodConfig {
    fn default() -> Self {
        Self {
            levels: vec![
                (0.0, 1.0),   // LOD0: full detail
                (10.0, 0.5),  // LOD1: 50% at 10m
                (25.0, 0.25), // LOD2: 25% at 25m
                (50.0, 0.1),  // LOD3: 10% at 50m
            ],
            max_error: 0.05,
            preserve_boundaries: true,
        }
    }
}

impl LodConfig {
    /// Create a simple 2-level LOD config
    pub fn simple() -> Self {
        Self {
            levels: vec![(0.0, 1.0), (20.0, 0.3)],
            ..Default::default()
        }
    }

    /// Create aggressive LOD config for performance
    pub fn aggressive() -> Self {
        Self {
            levels: vec![(0.0, 1.0), (5.0, 0.5), (15.0, 0.2), (30.0, 0.05)],
            max_error: 0.1,
            preserve_boundaries: false,
        }
    }

    /// Add a custom LOD level
    pub fn with_level(mut self, distance: f32, detail: f32) -> Self {
        self.levels.push((distance, detail.clamp(0.01, 1.0)));
        self.levels.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        self
    }

    /// Set maximum error
    pub fn with_max_error(mut self, error: f32) -> Self {
        self.max_error = error;
        self
    }

    /// Set boundary preservation
    pub fn with_preserve_boundaries(mut self, preserve: bool) -> Self {
        self.preserve_boundaries = preserve;
        self
    }
}

/// A set of LOD meshes
#[derive(Debug, Clone)]
pub struct LodMesh {
    /// The LOD levels, sorted by distance (LOD0 first)
    pub levels: Vec<LodLevel>,
}

/// A single LOD level
#[derive(Debug, Clone)]
pub struct LodLevel {
    /// Distance at which this LOD should be used
    pub distance: f32,
    /// The mesh for this LOD level
    pub mesh: Mesh,
    /// Detail ratio (1.0 = full detail)
    pub detail: f32,
}

impl LodMesh {
    /// Get the appropriate LOD mesh for a given distance
    pub fn get_for_distance(&self, distance: f32) -> &Mesh {
        for level in self.levels.iter().rev() {
            if distance >= level.distance {
                return &level.mesh;
            }
        }
        &self.levels[0].mesh
    }

    /// Get the LOD level index for a given distance
    pub fn get_level_for_distance(&self, distance: f32) -> usize {
        for (i, level) in self.levels.iter().enumerate().rev() {
            if distance >= level.distance {
                return i;
            }
        }
        0
    }

    /// Get all meshes
    pub fn meshes(&self) -> impl Iterator<Item = &Mesh> {
        self.levels.iter().map(|l| &l.mesh)
    }

    /// Get LOD count
    pub fn level_count(&self) -> usize {
        self.levels.len()
    }

    /// Total triangle count across all LODs
    pub fn total_triangle_count(&self) -> usize {
        self.levels.iter().map(|l| l.mesh.triangle_count()).sum()
    }
}

impl Mesh {
    /// Generate LOD levels for this mesh
    pub fn generate_lod(&self, config: LodConfig) -> LodMesh {
        let base_triangles = self.triangle_count();
        let mut levels = Vec::with_capacity(config.levels.len());

        for (distance, detail) in config.levels {
            let target_triangles = ((base_triangles as f32) * detail).max(4.0) as usize;

            let mesh = if detail >= 0.99 {
                // LOD0: keep original
                self.clone()
            } else {
                // Create decimated copy
                let mut lod_mesh = self.clone();
                let opt_config = OptimizeConfig {
                    weld_threshold: 0.0001,
                    target_triangles,
                    max_error: config.max_error,
                    preserve_boundaries: config.preserve_boundaries,
                    smooth_angle: std::f32::consts::PI / 4.0,
                };
                lod_mesh.optimize(&opt_config);
                lod_mesh
            };

            levels.push(LodLevel {
                distance,
                mesh,
                detail,
            });
        }

        LodMesh { levels }
    }

    /// Generate LOD with default configuration
    pub fn generate_lod_default(&self) -> LodMesh {
        self.generate_lod(LodConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::Vertex;
    use glam::{Vec2, Vec3};

    fn create_test_mesh() -> Mesh {
        // Create a simple mesh with some triangles
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        // Create a grid of vertices
        let size = 10;
        for z in 0..size {
            for x in 0..size {
                vertices.push(Vertex::new(
                    Vec3::new(x as f32, 0.0, z as f32),
                    Vec3::Y,
                    Vec2::new(x as f32 / size as f32, z as f32 / size as f32),
                ));
            }
        }

        // Create triangles
        for z in 0..size - 1 {
            for x in 0..size - 1 {
                let i = z * size + x;
                indices.push(i as u32);
                indices.push((i + 1) as u32);
                indices.push((i + size) as u32);

                indices.push((i + 1) as u32);
                indices.push((i + size + 1) as u32);
                indices.push((i + size) as u32);
            }
        }

        Mesh { vertices, indices }
    }

    #[test]
    fn test_lod_generation() {
        let mesh = create_test_mesh();
        let lod = mesh.generate_lod_default();

        assert_eq!(lod.level_count(), 4);

        // Each subsequent LOD should have fewer or equal triangles
        let mut prev_count = usize::MAX;
        for level in &lod.levels {
            assert!(level.mesh.triangle_count() <= prev_count);
            prev_count = level.mesh.triangle_count();
        }
    }

    #[test]
    fn test_lod_distance_selection() {
        let mesh = create_test_mesh();
        let lod = mesh.generate_lod_default();

        assert_eq!(lod.get_level_for_distance(0.0), 0);
        assert_eq!(lod.get_level_for_distance(5.0), 0);
        assert_eq!(lod.get_level_for_distance(15.0), 1);
        assert_eq!(lod.get_level_for_distance(30.0), 2);
        assert_eq!(lod.get_level_for_distance(100.0), 3);
    }
}
