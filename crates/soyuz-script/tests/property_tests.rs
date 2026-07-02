//! Property-based tests for the live SDF pipeline (`SdfOp` evaluated through
//! `CpuSdf` — the exact path the mesher samples).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use proptest::prelude::*;
use soyuz_core::prelude::Vec3;
use soyuz_core::sdf::Sdf;
use soyuz_script::CpuSdf;
use soyuz_sdf::SdfOp;
use std::sync::Arc;

fn sphere(radius: f32) -> SdfOp {
    SdfOp::Sphere { radius }
}

fn dist(op: SdfOp, p: [f32; 3]) -> f32 {
    CpuSdf::new(op).distance(Vec3::from_array(p))
}

/// Sane, non-degenerate parameter ranges: big enough to avoid denormals,
/// small enough to keep f32 tolerances meaningful.
fn radius() -> impl Strategy<Value = f32> {
    0.05f32..5.0
}

fn coord() -> impl Strategy<Value = f32> {
    -8.0f32..8.0
}

proptest! {
    /// A sphere's distance is exactly |p| - r.
    #[test]
    fn sphere_distance_is_exact(r in radius(), x in coord(), y in coord(), z in coord()) {
        let d = dist(sphere(r), [x, y, z]);
        let expected = (x * x + y * y + z * z).sqrt() - r;
        prop_assert!((d - expected).abs() < 1e-4, "d = {d}, expected {expected}");
    }

    /// Union is the pointwise minimum of its operands.
    #[test]
    fn union_is_min(r1 in radius(), r2 in radius(), off in coord(), x in coord(), y in coord(), z in coord()) {
        let a = sphere(r1);
        let b = SdfOp::Translate {
            inner: Arc::new(sphere(r2)),
            offset: [off, 0.0, 0.0],
        };
        let da = dist(a.clone(), [x, y, z]);
        let db = dist(b.clone(), [x, y, z]);
        let du = dist(SdfOp::Union { a: Arc::new(a), b: Arc::new(b) }, [x, y, z]);
        prop_assert!((du - da.min(db)).abs() < 1e-5);
    }

    /// Subtract keeps `a` outside `b`: max(d_a, -d_b).
    #[test]
    fn subtract_is_max_of_negated(r1 in radius(), r2 in radius(), x in coord(), y in coord(), z in coord()) {
        let da = dist(sphere(r1), [x, y, z]);
        let db = dist(sphere(r2), [x, y, z]);
        let ds = dist(
            SdfOp::Subtract { a: Arc::new(sphere(r1)), b: Arc::new(sphere(r2)) },
            [x, y, z],
        );
        prop_assert!((ds - da.max(-db)).abs() < 1e-5);
    }

    /// Translating the SDF is the same as sampling the untranslated SDF at
    /// the untranslated point.
    #[test]
    fn translate_invariance(r in radius(), tx in coord(), ty in coord(), tz in coord(), x in coord(), y in coord(), z in coord()) {
        let translated = SdfOp::Translate {
            inner: Arc::new(sphere(r)),
            offset: [tx, ty, tz],
        };
        let d1 = dist(translated, [x, y, z]);
        let d2 = dist(sphere(r), [x - tx, y - ty, z - tz]);
        prop_assert!((d1 - d2).abs() < 1e-4);
    }

    /// Uniform scaling scales distances by the same factor:
    /// d_scaled(p) = d(p / s) * s.
    #[test]
    fn scale_scales_distance(r in radius(), s in 0.1f32..4.0, x in coord(), y in coord(), z in coord()) {
        let scaled = SdfOp::Scale {
            inner: Arc::new(sphere(r)),
            factor: s,
        };
        let d1 = dist(scaled, [x, y, z]);
        let d2 = dist(sphere(r), [x / s, y / s, z / s]) * s;
        prop_assert!((d1 - d2).abs() < 1e-3, "d1 = {d1}, d2 = {d2}");
    }

    /// The polynomial smooth union never exceeds the hard union and dips at
    /// most k/4 below it (the blend bump's maximum).
    #[test]
    fn smooth_union_bounded_by_hard_union(r1 in radius(), r2 in radius(), k in 0.01f32..0.5, off in coord(), x in coord(), y in coord(), z in coord()) {
        let a = sphere(r1);
        let b = SdfOp::Translate {
            inner: Arc::new(sphere(r2)),
            offset: [0.0, off, 0.0],
        };
        let hard = dist(a.clone(), [x, y, z]).min(dist(b.clone(), [x, y, z]));
        let smooth = dist(
            SdfOp::SmoothUnion { a: Arc::new(a), b: Arc::new(b), k },
            [x, y, z],
        );
        prop_assert!(smooth <= hard + 1e-4, "smooth {smooth} > hard {hard}");
        prop_assert!(smooth >= hard - k / 4.0 - 1e-4, "smooth {smooth} dips more than k/4 below hard {hard}");
    }

    /// Every distance the mesher can sample must be finite — including the
    /// zero-k smooth boolean case, which the script API clamps.
    #[test]
    fn distances_are_finite(r in radius(), k in 0.0f32..0.5, x in coord(), y in coord(), z in coord()) {
        let op = SdfOp::SmoothUnion {
            a: Arc::new(sphere(r)),
            b: Arc::new(SdfOp::Translate {
                inner: Arc::new(sphere(r)),
                offset: [r, 0.0, 0.0],
            }),
            // The script API clamps k away from zero; mirror that here since
            // this test constructs SdfOp directly.
            k: k.max(1e-4),
        };
        let d = dist(op, [x, y, z]);
        prop_assert!(d.is_finite(), "non-finite distance {d}");
    }
}
