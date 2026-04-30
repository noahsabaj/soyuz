//! Property-based tests for SDF invariants

// Property tests use generated values that may be unused in assertions
#![allow(clippy::unwrap_used)]

use glam::Vec3;
use proptest::prelude::*;
use soyuz_core::mesh::{MeshConfig, SdfToMesh};
use soyuz_core::sdf::Sdf;
use soyuz_core::sdf::operations::Union;
use soyuz_core::sdf::primitives::*;

proptest! {
    #[test]
    fn sphere_negative_at_origin(radius in 0.01f32..100.0) {
        let s = sphere(radius);
        prop_assert!(
            s.distance(Vec3::ZERO) < 0.0,
            "Sphere(r={}) should be negative at origin, got {}",
            radius,
            s.distance(Vec3::ZERO)
        );
    }

    #[test]
    fn sphere_positive_far_away(radius in 0.01f32..10.0) {
        let s = sphere(radius);
        let far = Vec3::new(radius * 3.0, 0.0, 0.0);
        prop_assert!(
            s.distance(far) > 0.0,
            "Sphere(r={}) should be positive at 3r",
            radius
        );
    }

    #[test]
    fn cube_negative_at_origin(size in 0.01f32..100.0) {
        let c = cube(size);
        prop_assert!(c.distance(Vec3::ZERO) < 0.0);
    }

    #[test]
    fn union_le_min(
        r1 in 0.1f32..5.0,
        r2 in 0.1f32..5.0,
        x in -10.0f32..10.0,
        y in -10.0f32..10.0,
        z in -10.0f32..10.0,
    ) {
        let a = sphere(r1);
        let b = sphere(r2);
        let u = Union::new(a, b);
        let p = Vec3::new(x, y, z);

        let union_d = u.distance(p);
        let min_d = a.distance(p).min(b.distance(p));

        prop_assert!(
            union_d <= min_d + 1e-6,
            "union(a,b) should be <= min(a,b): {} vs {}",
            union_d,
            min_d
        );
    }

    #[test]
    fn mesh_valid_indices(resolution in 8u32..24) {
        let s = sphere(1.0);
        let config = MeshConfig::default().with_resolution(resolution);
        let mesh = s.to_mesh(config).unwrap();

        let vertex_count = mesh.vertex_count() as u32;
        for &idx in &mesh.indices {
            prop_assert!(
                idx < vertex_count,
                "Index {} out of bounds (vertex_count={})",
                idx,
                vertex_count
            );
        }
    }

    #[test]
    fn bounds_contain_surface_points(radius in 0.1f32..5.0) {
        let s = sphere(radius);
        let bounds = s.bounds();

        // Surface points on each axis should be within bounds
        for surface_point in [
            Vec3::new(radius, 0.0, 0.0),
            Vec3::new(0.0, radius, 0.0),
            Vec3::new(0.0, 0.0, radius),
        ] {
            prop_assert!(bounds.min.x <= surface_point.x && surface_point.x <= bounds.max.x);
            prop_assert!(bounds.min.y <= surface_point.y && surface_point.y <= bounds.max.y);
            prop_assert!(bounds.min.z <= surface_point.z && surface_point.z <= bounds.max.z);
        }
    }
}
