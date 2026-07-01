//! Mesh optimization: vertex welding, decimation, and normal smoothing

// Builder pattern methods intentionally return Self without #[must_use]
// Edge collapse algorithm is a complex single unit
#![allow(clippy::return_self_not_must_use)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::map_entry)] // Performance is fine and clearer this way
#![allow(clippy::unused_self)] // API consistency

use super::{Mesh, Vertex};
use glam::Vec3;
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::collections::HashMap;

/// Configuration for mesh optimization
#[derive(Debug, Clone)]
pub struct OptimizeConfig {
    /// Threshold for welding vertices (in meters)
    pub weld_threshold: f32,
    /// Target triangle count for decimation (0 = no decimation)
    pub target_triangles: usize,
    /// Maximum error allowed during decimation
    pub max_error: f32,
    /// Whether to preserve boundaries
    pub preserve_boundaries: bool,
    /// Angle threshold for smoothing normals (radians)
    pub smooth_angle: f32,
}

impl Default for OptimizeConfig {
    fn default() -> Self {
        Self {
            weld_threshold: 0.0001,
            target_triangles: 0,
            max_error: 0.01,
            preserve_boundaries: true,
            smooth_angle: std::f32::consts::PI / 4.0, // 45 degrees
        }
    }
}

impl OptimizeConfig {
    pub fn with_weld_threshold(mut self, threshold: f32) -> Self {
        self.weld_threshold = threshold;
        self
    }

    pub fn with_target_triangles(mut self, count: usize) -> Self {
        self.target_triangles = count;
        self
    }

    pub fn with_max_error(mut self, error: f32) -> Self {
        self.max_error = error;
        self
    }

    pub fn with_smooth_angle(mut self, angle: f32) -> Self {
        self.smooth_angle = angle;
        self
    }
}

impl Mesh {
    /// Optimize the mesh with the given configuration
    pub fn optimize(&mut self, config: &OptimizeConfig) {
        // Step 1: Weld duplicate vertices
        if config.weld_threshold > 0.0 {
            self.weld_vertices(config.weld_threshold);
        }

        // Step 2: Decimate if target is set
        if config.target_triangles > 0 && self.triangle_count() > config.target_triangles {
            self.decimate(
                config.target_triangles,
                config.max_error,
                config.preserve_boundaries,
            );
        }

        // Step 3: Recalculate normals with smoothing
        self.smooth_normals(config.smooth_angle);
    }

    /// Weld vertices that are closer than the threshold
    pub fn weld_vertices(&mut self, threshold: f32) {
        if self.vertices.is_empty() {
            return;
        }

        let threshold_sq = threshold * threshold;

        // Build spatial hash for fast lookups
        let cell_size = threshold * 2.0;
        let mut spatial_hash: HashMap<(i32, i32, i32), Vec<usize>> = HashMap::new();

        for (i, v) in self.vertices.iter().enumerate() {
            let key = (
                (v.position[0] / cell_size).floor() as i32,
                (v.position[1] / cell_size).floor() as i32,
                (v.position[2] / cell_size).floor() as i32,
            );
            spatial_hash.entry(key).or_default().push(i);
        }

        // Map from old index to new index
        let mut index_map: Vec<u32> = (0..self.vertices.len() as u32).collect();
        let mut new_vertices: Vec<Vertex> = Vec::with_capacity(self.vertices.len());
        let mut vertex_remap: Vec<Option<u32>> = vec![None; self.vertices.len()];

        for i in 0..self.vertices.len() {
            if vertex_remap[i].is_some() {
                continue;
            }

            let v = &self.vertices[i];
            let pos = Vec3::from_array(v.position);
            let key = (
                (v.position[0] / cell_size).floor() as i32,
                (v.position[1] / cell_size).floor() as i32,
                (v.position[2] / cell_size).floor() as i32,
            );

            // Check for existing vertex to weld to
            let mut found_match = None;
            'outer: for dx in -1..=1 {
                for dy in -1..=1 {
                    for dz in -1..=1 {
                        let neighbor_key = (key.0 + dx, key.1 + dy, key.2 + dz);
                        if let Some(indices) = spatial_hash.get(&neighbor_key) {
                            for &j in indices {
                                if j >= i {
                                    continue;
                                }
                                if let Some(new_idx) = vertex_remap[j] {
                                    let other_pos =
                                        Vec3::from_array(new_vertices[new_idx as usize].position);
                                    if pos.distance_squared(other_pos) < threshold_sq {
                                        found_match = Some(new_idx);
                                        break 'outer;
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if let Some(new_idx) = found_match {
                vertex_remap[i] = Some(new_idx);
                index_map[i] = new_idx;
            } else {
                let new_idx = new_vertices.len() as u32;
                new_vertices.push(*v);
                vertex_remap[i] = Some(new_idx);
                index_map[i] = new_idx;
            }
        }

        // Remap indices
        for idx in &mut self.indices {
            *idx = index_map[*idx as usize];
        }

        // Remove degenerate triangles
        let mut new_indices = Vec::with_capacity(self.indices.len());
        for tri in self.indices.chunks(3) {
            if tri[0] != tri[1] && tri[1] != tri[2] && tri[2] != tri[0] {
                new_indices.extend_from_slice(tri);
            }
        }

        self.vertices = new_vertices;
        self.indices = new_indices;
    }

    /// Decimate the mesh by greedy edge collapse.
    ///
    /// The collapse cost is a heuristic (edge length, weighted up for boundary
    /// edges and high-curvature regions) — not a true quadric error metric.
    /// Collapsed vertices are moved to the edge midpoint.
    pub fn decimate(&mut self, target_triangles: usize, max_error: f32, preserve_boundaries: bool) {
        if self.triangle_count() <= target_triangles {
            return;
        }

        // Build adjacency information
        let mut vertex_triangles: Vec<Vec<usize>> = vec![Vec::new(); self.vertices.len()];
        for (tri_idx, tri) in self.indices.chunks(3).enumerate() {
            for &v in tri {
                vertex_triangles[v as usize].push(tri_idx);
            }
        }

        // Build edge list with collapse cost and vertex-edge adjacency
        let mut edges: Vec<Edge> = Vec::new();
        let mut edge_set: HashMap<(u32, u32), usize> = HashMap::new();
        let mut vertex_edges: Vec<Vec<usize>> = vec![Vec::new(); self.vertices.len()];
        let mut heap: BinaryHeap<HeapEntry> = BinaryHeap::new();

        for tri in self.indices.chunks(3) {
            for i in 0..3 {
                let v0 = tri[i].min(tri[(i + 1) % 3]);
                let v1 = tri[i].max(tri[(i + 1) % 3]);
                let key = (v0, v1);

                if !edge_set.contains_key(&key) {
                    let cost = self.compute_edge_collapse_cost(
                        v0,
                        v1,
                        &vertex_triangles,
                        preserve_boundaries,
                    );
                    let edge_idx = edges.len();
                    edges.push(Edge {
                        v0,
                        v1,
                        cost,
                        collapsed: false,
                        generation: 0,
                    });
                    edge_set.insert(key, edge_idx);
                    vertex_edges[v0 as usize].push(edge_idx);
                    vertex_edges[v1 as usize].push(edge_idx);
                    heap.push(HeapEntry {
                        cost,
                        edge_idx,
                        generation: 0,
                    });
                }
            }
        }

        // Collapse edges until we reach target
        let mut current_triangles = self.triangle_count();
        let mut collapsed_vertices: Vec<Option<u32>> = vec![None; self.vertices.len()];

        while current_triangles > target_triangles {
            // Pop best edge from heap
            let entry = loop {
                let Some(entry) = heap.pop() else {
                    break None;
                };
                let edge = &edges[entry.edge_idx];
                // Skip stale entries and already-collapsed edges
                if edge.collapsed || entry.generation != edge.generation || entry.cost > max_error {
                    continue;
                }
                break Some(entry);
            };

            let Some(entry) = entry else {
                break; // No more edges below error threshold
            };

            let edge_idx = entry.edge_idx;
            let edge = &edges[edge_idx];
            let v0 = self.resolve_vertex(edge.v0, &collapsed_vertices);
            let v1 = self.resolve_vertex(edge.v1, &collapsed_vertices);

            if v0 == v1 {
                edges[edge_idx].collapsed = true;
                continue;
            }

            // Collapse v1 into v0
            collapsed_vertices[v1 as usize] = Some(v0);
            edges[edge_idx].collapsed = true;

            // Update vertex position to midpoint
            let p0 = Vec3::from_array(self.vertices[v0 as usize].position);
            let p1 = Vec3::from_array(self.vertices[v1 as usize].position);
            let mid = (p0 + p1) * 0.5;
            self.vertices[v0 as usize].position = mid.to_array();

            // Count triangles that will be removed (only check triangles adjacent to v0/v1)
            let mut removed = 0;
            let mut checked_tris = Vec::new();
            for &t in vertex_triangles[v0 as usize]
                .iter()
                .chain(vertex_triangles[v1 as usize].iter())
            {
                if checked_tris.contains(&t) {
                    continue;
                }
                checked_tris.push(t);
                let tri = &self.indices[t * 3..t * 3 + 3];
                let t0 = self.resolve_vertex(tri[0], &collapsed_vertices);
                let t1 = self.resolve_vertex(tri[1], &collapsed_vertices);
                let t2 = self.resolve_vertex(tri[2], &collapsed_vertices);
                if t0 == t1 || t1 == t2 || t2 == t0 {
                    removed += 1;
                }
            }
            current_triangles -= removed;

            // Update affected neighbor edges' costs using vertex_edges adjacency
            let neighbor_edge_indices: Vec<usize> = vertex_edges[v0 as usize]
                .iter()
                .chain(vertex_edges[v1 as usize].iter())
                .copied()
                .collect();

            for &ei in &neighbor_edge_indices {
                if edges[ei].collapsed || ei == edge_idx {
                    continue;
                }
                let ev0 = self.resolve_vertex(edges[ei].v0, &collapsed_vertices);
                let ev1 = self.resolve_vertex(edges[ei].v1, &collapsed_vertices);
                if ev0 == ev1 {
                    edges[ei].collapsed = true;
                    continue;
                }
                let new_cost = self.compute_edge_collapse_cost(
                    ev0,
                    ev1,
                    &vertex_triangles,
                    preserve_boundaries,
                );
                edges[ei].cost = new_cost;
                edges[ei].generation += 1;
                heap.push(HeapEntry {
                    cost: new_cost,
                    edge_idx: ei,
                    generation: edges[ei].generation,
                });
            }
        }

        // Apply collapsed vertices to indices
        let new_indices: Vec<u32> = self
            .indices
            .iter()
            .map(|&idx| self.resolve_vertex(idx, &collapsed_vertices))
            .collect();
        self.indices = new_indices;

        // Remove degenerate triangles and rebuild
        let mut new_indices = Vec::with_capacity(self.indices.len());
        for tri in self.indices.chunks(3) {
            if tri[0] != tri[1] && tri[1] != tri[2] && tri[2] != tri[0] {
                new_indices.extend_from_slice(tri);
            }
        }
        self.indices = new_indices;

        // Remove unused vertices
        self.remove_unused_vertices();
    }

    fn resolve_vertex(&self, v: u32, collapsed: &[Option<u32>]) -> u32 {
        let mut current = v;
        while let Some(next) = collapsed[current as usize] {
            current = next;
        }
        current
    }

    fn compute_edge_collapse_cost(
        &self,
        v0: u32,
        v1: u32,
        vertex_triangles: &[Vec<usize>],
        preserve_boundaries: bool,
    ) -> f32 {
        let p0 = Vec3::from_array(self.vertices[v0 as usize].position);
        let p1 = Vec3::from_array(self.vertices[v1 as usize].position);

        // Base cost is edge length
        let mut cost = p0.distance(p1);

        // Penalize boundary edges if preserving
        if preserve_boundaries {
            let shared_tris: usize = vertex_triangles[v0 as usize]
                .iter()
                .filter(|&&t| vertex_triangles[v1 as usize].contains(&t))
                .count();

            if shared_tris == 1 {
                // This is a boundary edge
                cost *= 10.0;
            }
        }

        // Penalize high-curvature regions (large normal differences)
        let n0 = Vec3::from_array(self.vertices[v0 as usize].normal);
        let n1 = Vec3::from_array(self.vertices[v1 as usize].normal);
        let normal_diff = 1.0 - n0.dot(n1).max(0.0);
        cost *= 1.0 + normal_diff * 2.0;

        cost
    }

    /// Remove vertices not referenced by any triangle
    pub fn remove_unused_vertices(&mut self) {
        let mut used = vec![false; self.vertices.len()];
        for &idx in &self.indices {
            used[idx as usize] = true;
        }

        let mut new_vertices = Vec::new();
        let mut index_map = vec![0u32; self.vertices.len()];

        for (i, v) in self.vertices.iter().enumerate() {
            if used[i] {
                index_map[i] = new_vertices.len() as u32;
                new_vertices.push(*v);
            }
        }

        for idx in &mut self.indices {
            *idx = index_map[*idx as usize];
        }

        self.vertices = new_vertices;
    }

    /// Smooth normals based on angle threshold
    pub fn smooth_normals(&mut self, angle_threshold: f32) {
        let cos_threshold = angle_threshold.cos();

        // Build adjacency: for each vertex, find all triangles using it
        let mut vertex_triangles: Vec<Vec<usize>> = vec![Vec::new(); self.vertices.len()];
        for (tri_idx, tri) in self.indices.chunks(3).enumerate() {
            for &v in tri {
                vertex_triangles[v as usize].push(tri_idx);
            }
        }

        // Compute face normals
        let mut face_normals: Vec<Vec3> = Vec::with_capacity(self.indices.len() / 3);
        for tri in self.indices.chunks(3) {
            let p0 = Vec3::from_array(self.vertices[tri[0] as usize].position);
            let p1 = Vec3::from_array(self.vertices[tri[1] as usize].position);
            let p2 = Vec3::from_array(self.vertices[tri[2] as usize].position);
            let normal = (p1 - p0).cross(p2 - p0).normalize_or_zero();
            face_normals.push(normal);
        }

        // For each vertex, average normals of adjacent faces within angle threshold
        for (v_idx, v) in self.vertices.iter_mut().enumerate() {
            let triangles = &vertex_triangles[v_idx];
            if triangles.is_empty() {
                continue;
            }

            // Use the first triangle's normal as reference
            let ref_normal = face_normals[triangles[0]];
            let mut sum = Vec3::ZERO;
            let mut count = 0;

            for &tri_idx in triangles {
                let face_normal = face_normals[tri_idx];
                if ref_normal.dot(face_normal) >= cos_threshold {
                    sum += face_normal;
                    count += 1;
                }
            }

            if count > 0 {
                v.normal = (sum / count as f32).normalize_or_zero().to_array();
            }
        }
    }
}

#[derive(Debug, Clone)]
struct Edge {
    v0: u32,
    v1: u32,
    cost: f32,
    collapsed: bool,
    generation: u32,
}

struct HeapEntry {
    cost: f32,
    edge_idx: usize,
    generation: u32,
}

impl PartialEq for HeapEntry {
    fn eq(&self, other: &Self) -> bool {
        self.cost == other.cost
    }
}
impl Eq for HeapEntry {}

impl PartialOrd for HeapEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for HeapEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse: BinaryHeap is max-heap, we want min-cost first
        other
            .cost
            .partial_cmp(&self.cost)
            .unwrap_or(Ordering::Equal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec2;

    #[test]
    fn test_weld_vertices() {
        let mut mesh = Mesh {
            vertices: vec![
                Vertex::new(Vec3::new(0.0, 0.0, 0.0), Vec3::Y, Vec2::ZERO),
                Vertex::new(Vec3::new(0.00001, 0.0, 0.0), Vec3::Y, Vec2::ZERO), // Should weld
                Vertex::new(Vec3::new(1.0, 0.0, 0.0), Vec3::Y, Vec2::ZERO),
            ],
            indices: vec![0, 1, 2],
        };

        mesh.weld_vertices(0.001);

        assert_eq!(mesh.vertices.len(), 2); // Two unique vertices
    }

    #[test]
    fn test_remove_unused() {
        let mut mesh = Mesh {
            vertices: vec![
                Vertex::new(Vec3::new(0.0, 0.0, 0.0), Vec3::Y, Vec2::ZERO),
                Vertex::new(Vec3::new(1.0, 0.0, 0.0), Vec3::Y, Vec2::ZERO),
                Vertex::new(Vec3::new(0.0, 1.0, 0.0), Vec3::Y, Vec2::ZERO),
                Vertex::new(Vec3::new(99.0, 99.0, 99.0), Vec3::Y, Vec2::ZERO), // Unused
            ],
            indices: vec![0, 1, 2],
        };

        mesh.remove_unused_vertices();

        assert_eq!(mesh.vertices.len(), 3);
    }
}
