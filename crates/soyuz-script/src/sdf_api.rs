//! Rhai API for SDF primitives and operations
//!
//! This module provides all SDF functions accessible from Rhai scripts.
//!
//! # Precision Notes
//!
//! Floating-point script literals are `f64`, converted to `f32` when
//! constructing SDF operations (required for GPU shader compatibility). For
//! most use cases the precision loss is negligible.
//!
//! # Numeric literals must be floats
//!
//! Rhai does not auto-convert integer literals (`1`, which is `i64`) to the
//! `f64` these functions expect, so write `sphere(0.5)`, `cylinder(1.0, 2.0)`,
//! `box3(1.0, 1.0, 1.0)` etc. — `cylinder(1, 2)` raises a "function not found"
//! error. As a convenience, `sphere(..)` and `cube(..)` also accept integers.

use rhai::Engine;
use soyuz_sdf::{ExtrudeProfile, RevolveProfile, SdfOp};
use std::sync::Arc;

/// Smooth boolean blends divide by `k`, so `k = 0` goes NaN exactly where both
/// surfaces are equidistant (the blend seam). Clamp script-provided `k` to this
/// minimum so a zero (or negative) `k` degrades to an effectively hard boolean.
const MIN_SMOOTH_K: f32 = 1e-4;

/// SDF node representation for Rhai
///
/// This wrapper holds an `Arc<SdfOp>` for efficient cloning (O(1) reference
/// count increment instead of O(n) deep clone) and thread-safe sharing.
#[derive(Debug, Clone)]
pub struct RhaiSdf {
    /// The underlying SDF operation tree
    pub op: Arc<SdfOp>,
}

impl RhaiSdf {
    /// Create a new RhaiSdf from an SdfOp
    pub fn new(op: SdfOp) -> Self {
        Self { op: Arc::new(op) }
    }

    /// Create a new RhaiSdf from an Arc<SdfOp>
    pub fn from_arc(op: Arc<SdfOp>) -> Self {
        Self { op }
    }

    /// Get a reference to the underlying SdfOp
    pub fn as_sdf_op(&self) -> &SdfOp {
        &self.op
    }

    /// Clone the inner SdfOp (for when you need ownership)
    pub fn to_sdf_op(&self) -> SdfOp {
        (*self.op).clone()
    }

    // === Boolean Operations ===

    pub fn union(&mut self, other: RhaiSdf) -> RhaiSdf {
        RhaiSdf::new(SdfOp::Union {
            a: Arc::clone(&self.op),
            b: other.op,
        })
    }

    pub fn subtract(&mut self, other: RhaiSdf) -> RhaiSdf {
        RhaiSdf::new(SdfOp::Subtract {
            a: Arc::clone(&self.op),
            b: other.op,
        })
    }

    pub fn intersect(&mut self, other: RhaiSdf) -> RhaiSdf {
        RhaiSdf::new(SdfOp::Intersect {
            a: Arc::clone(&self.op),
            b: other.op,
        })
    }

    pub fn smooth_union(&mut self, other: RhaiSdf, k: f64) -> RhaiSdf {
        RhaiSdf::new(SdfOp::SmoothUnion {
            a: Arc::clone(&self.op),
            b: other.op,
            k: (k as f32).max(MIN_SMOOTH_K),
        })
    }

    pub fn smooth_subtract(&mut self, other: RhaiSdf, k: f64) -> RhaiSdf {
        RhaiSdf::new(SdfOp::SmoothSubtract {
            a: Arc::clone(&self.op),
            b: other.op,
            k: (k as f32).max(MIN_SMOOTH_K),
        })
    }

    pub fn smooth_intersect(&mut self, other: RhaiSdf, k: f64) -> RhaiSdf {
        RhaiSdf::new(SdfOp::SmoothIntersect {
            a: Arc::clone(&self.op),
            b: other.op,
            k: (k as f32).max(MIN_SMOOTH_K),
        })
    }

    pub fn xor(&mut self, other: RhaiSdf) -> RhaiSdf {
        RhaiSdf::new(SdfOp::Xor {
            a: Arc::clone(&self.op),
            b: other.op,
        })
    }

    // === Modifiers ===

    pub fn shell(&mut self, thickness: f64) -> RhaiSdf {
        RhaiSdf::new(SdfOp::Shell {
            inner: Arc::clone(&self.op),
            thickness: thickness as f32,
        })
    }

    pub fn hollow(&mut self, thickness: f64) -> RhaiSdf {
        self.shell(thickness)
    }

    pub fn round(&mut self, radius: f64) -> RhaiSdf {
        RhaiSdf::new(SdfOp::Round {
            inner: Arc::clone(&self.op),
            radius: radius as f32,
        })
    }

    pub fn onion(&mut self, thickness: f64) -> RhaiSdf {
        // Onion takes `d % (2*thickness)`; a zero thickness divides by zero (NaN).
        let thickness = (thickness as f32).max(1e-4);
        RhaiSdf::new(SdfOp::Onion {
            inner: Arc::clone(&self.op),
            thickness,
        })
    }

    pub fn elongate(&mut self, x: f64, y: f64, z: f64) -> RhaiSdf {
        RhaiSdf::new(SdfOp::Elongate {
            inner: Arc::clone(&self.op),
            h: [x as f32, y as f32, z as f32],
        })
    }

    // === Transforms ===

    pub fn translate(&mut self, x: f64, y: f64, z: f64) -> RhaiSdf {
        RhaiSdf::new(SdfOp::Translate {
            inner: Arc::clone(&self.op),
            offset: [x as f32, y as f32, z as f32],
        })
    }

    pub fn translate_x(&mut self, x: f64) -> RhaiSdf {
        self.translate(x, 0.0, 0.0)
    }

    pub fn translate_y(&mut self, y: f64) -> RhaiSdf {
        self.translate(0.0, y, 0.0)
    }

    pub fn translate_z(&mut self, z: f64) -> RhaiSdf {
        self.translate(0.0, 0.0, z)
    }

    pub fn rotate_x(&mut self, angle: f64) -> RhaiSdf {
        RhaiSdf::new(SdfOp::RotateX {
            inner: Arc::clone(&self.op),
            angle: angle as f32,
        })
    }

    pub fn rotate_y(&mut self, angle: f64) -> RhaiSdf {
        RhaiSdf::new(SdfOp::RotateY {
            inner: Arc::clone(&self.op),
            angle: angle as f32,
        })
    }

    pub fn rotate_z(&mut self, angle: f64) -> RhaiSdf {
        RhaiSdf::new(SdfOp::RotateZ {
            inner: Arc::clone(&self.op),
            angle: angle as f32,
        })
    }

    pub fn scale(&mut self, factor: f64) -> RhaiSdf {
        RhaiSdf::new(SdfOp::Scale {
            inner: Arc::clone(&self.op),
            factor: factor as f32,
        })
    }

    pub fn mirror_x(&mut self) -> RhaiSdf {
        RhaiSdf::new(SdfOp::Mirror {
            inner: Arc::clone(&self.op),
            axis: [1.0, 0.0, 0.0],
        })
    }

    pub fn mirror_y(&mut self) -> RhaiSdf {
        RhaiSdf::new(SdfOp::Mirror {
            inner: Arc::clone(&self.op),
            axis: [0.0, 1.0, 0.0],
        })
    }

    pub fn mirror_z(&mut self) -> RhaiSdf {
        RhaiSdf::new(SdfOp::Mirror {
            inner: Arc::clone(&self.op),
            axis: [0.0, 0.0, 1.0],
        })
    }

    pub fn symmetry_x(&mut self) -> RhaiSdf {
        RhaiSdf::new(SdfOp::SymmetryX {
            inner: Arc::clone(&self.op),
        })
    }

    pub fn symmetry_y(&mut self) -> RhaiSdf {
        RhaiSdf::new(SdfOp::SymmetryY {
            inner: Arc::clone(&self.op),
        })
    }

    pub fn symmetry_z(&mut self) -> RhaiSdf {
        RhaiSdf::new(SdfOp::SymmetryZ {
            inner: Arc::clone(&self.op),
        })
    }

    // === Deformations ===

    pub fn twist(&mut self, amount: f64) -> RhaiSdf {
        RhaiSdf::new(SdfOp::Twist {
            inner: Arc::clone(&self.op),
            amount: amount as f32,
        })
    }

    pub fn bend(&mut self, amount: f64) -> RhaiSdf {
        RhaiSdf::new(SdfOp::Bend {
            inner: Arc::clone(&self.op),
            amount: amount as f32,
        })
    }

    pub fn displace(&mut self, amount: f64, frequency: f64) -> RhaiSdf {
        RhaiSdf::new(SdfOp::Displacement {
            inner: Arc::clone(&self.op),
            amount: amount as f32,
            frequency: frequency as f32,
        })
    }

    // === Repetition ===

    pub fn repeat(&mut self, sx: f64, sy: f64, sz: f64) -> RhaiSdf {
        RhaiSdf::new(SdfOp::RepeatInfinite {
            inner: Arc::clone(&self.op),
            spacing: [sx as f32, sy as f32, sz as f32],
        })
    }

    pub fn repeat_limited(
        &mut self,
        sx: f64,
        sy: f64,
        sz: f64,
        cx: f64,
        cy: f64,
        cz: f64,
    ) -> RhaiSdf {
        RhaiSdf::new(SdfOp::RepeatLimited {
            inner: Arc::clone(&self.op),
            spacing: [sx as f32, sy as f32, sz as f32],
            count: [cx as f32, cy as f32, cz as f32],
        })
    }

    pub fn repeat_polar(&mut self, count: i64) -> RhaiSdf {
        // Clamp to a sane range: <=0 would divide by zero (NaN) and a negative
        // i64 would wrap to a ~4.3e9 u32. 1024 angular sectors is already extreme.
        let count = count.clamp(1, 1024) as u32;
        RhaiSdf::new(SdfOp::RepeatPolar {
            inner: Arc::clone(&self.op),
            count,
        })
    }
}

// === Primitive Constructor Functions ===

pub fn sphere(radius: f64) -> RhaiSdf {
    RhaiSdf::new(SdfOp::Sphere {
        radius: radius as f32,
    })
}

/// Integer overload for sphere - allows `sphere(1)` instead of requiring `sphere(1.0)`
pub fn sphere_int(radius: i64) -> RhaiSdf {
    sphere(radius as f64)
}

pub fn cube(size: f64) -> RhaiSdf {
    let half = (size / 2.0) as f32;
    RhaiSdf::new(SdfOp::Box {
        half_extents: [half, half, half],
    })
}

/// Integer overload for cube - allows `cube(1)` instead of requiring `cube(1.0)`
pub fn cube_int(size: i64) -> RhaiSdf {
    cube(size as f64)
}

pub fn box3(x: f64, y: f64, z: f64) -> RhaiSdf {
    RhaiSdf::new(SdfOp::Box {
        half_extents: [(x / 2.0) as f32, (y / 2.0) as f32, (z / 2.0) as f32],
    })
}

pub fn rounded_box(x: f64, y: f64, z: f64, radius: f64) -> RhaiSdf {
    RhaiSdf::new(SdfOp::RoundedBox {
        half_extents: [(x / 2.0) as f32, (y / 2.0) as f32, (z / 2.0) as f32],
        radius: radius as f32,
    })
}

pub fn cylinder(radius: f64, height: f64) -> RhaiSdf {
    RhaiSdf::new(SdfOp::Cylinder {
        radius: radius as f32,
        half_height: (height / 2.0) as f32,
    })
}

pub fn capsule(radius: f64, height: f64) -> RhaiSdf {
    RhaiSdf::new(SdfOp::Capsule {
        radius: radius as f32,
        half_height: (height / 2.0) as f32,
    })
}

pub fn torus(major_radius: f64, minor_radius: f64) -> RhaiSdf {
    RhaiSdf::new(SdfOp::Torus {
        major_radius: major_radius as f32,
        minor_radius: minor_radius as f32,
    })
}

pub fn cone(radius: f64, height: f64) -> RhaiSdf {
    RhaiSdf::new(SdfOp::Cone {
        radius: radius as f32,
        height: height as f32,
    })
}

pub fn plane(nx: f64, ny: f64, nz: f64, offset: f64) -> RhaiSdf {
    // Normalize at construction so the stored normal is unit-length; the CPU and
    // GPU plane evaluators both use it raw (and so must agree). Fall back to +Y
    // for a degenerate zero normal.
    let len = (nx * nx + ny * ny + nz * nz).sqrt();
    let normal = if len > 1e-12 {
        [(nx / len) as f32, (ny / len) as f32, (nz / len) as f32]
    } else {
        [0.0, 1.0, 0.0]
    };
    RhaiSdf::new(SdfOp::Plane {
        normal,
        offset: offset as f32,
    })
}

pub fn ground_plane() -> RhaiSdf {
    plane(0.0, 1.0, 0.0, 0.0)
}

pub fn ellipsoid(rx: f64, ry: f64, rz: f64) -> RhaiSdf {
    RhaiSdf::new(SdfOp::Ellipsoid {
        radii: [rx as f32, ry as f32, rz as f32],
    })
}

pub fn octahedron(size: f64) -> RhaiSdf {
    RhaiSdf::new(SdfOp::Octahedron { size: size as f32 })
}

pub fn hex_prism(radius: f64, height: f64) -> RhaiSdf {
    RhaiSdf::new(SdfOp::HexPrism {
        half_height: (height / 2.0) as f32,
        radius: radius as f32,
    })
}

pub fn tri_prism(width: f64, height: f64) -> RhaiSdf {
    RhaiSdf::new(SdfOp::TriPrism {
        size: [width as f32, height as f32],
    })
}

pub fn pyramid(height: f64) -> RhaiSdf {
    RhaiSdf::new(SdfOp::Pyramid {
        height: height as f32,
    })
}

pub fn link(length: f64, major_radius: f64, minor_radius: f64) -> RhaiSdf {
    RhaiSdf::new(SdfOp::Link {
        length: length as f32,
        major_radius: major_radius as f32,
        minor_radius: minor_radius as f32,
    })
}

// === 2D-to-3D Operations ===

pub fn extrude_circle(radius: f64, depth: f64) -> RhaiSdf {
    RhaiSdf::new(SdfOp::Extrude {
        profile: ExtrudeProfile::Circle {
            radius: radius as f32,
        },
        depth: depth as f32,
    })
}

pub fn extrude_rect(width: f64, height: f64, depth: f64) -> RhaiSdf {
    RhaiSdf::new(SdfOp::Extrude {
        profile: ExtrudeProfile::Rectangle {
            width: width as f32,
            height: height as f32,
        },
        depth: depth as f32,
    })
}

pub fn extrude_rounded_rect(width: f64, height: f64, radius: f64, depth: f64) -> RhaiSdf {
    RhaiSdf::new(SdfOp::Extrude {
        profile: ExtrudeProfile::RoundedRectangle {
            width: width as f32,
            height: height as f32,
            radius: radius as f32,
        },
        depth: depth as f32,
    })
}

pub fn revolve_circle(radius: f64, offset: f64) -> RhaiSdf {
    RhaiSdf::new(SdfOp::Revolve {
        profile: RevolveProfile::Circle {
            radius: radius as f32,
        },
        offset: offset as f32,
    })
}

pub fn revolve_rect(width: f64, height: f64, offset: f64) -> RhaiSdf {
    RhaiSdf::new(SdfOp::Revolve {
        profile: RevolveProfile::Rectangle {
            width: width as f32,
            height: height as f32,
        },
        offset: offset as f32,
    })
}

// === Math constants ===

pub fn pi() -> f64 {
    std::f64::consts::PI
}

pub fn tau() -> f64 {
    std::f64::consts::TAU
}

pub fn deg_to_rad(deg: f64) -> f64 {
    deg * std::f64::consts::PI / 180.0
}

pub fn rad_to_deg(rad: f64) -> f64 {
    rad * 180.0 / std::f64::consts::PI
}

/// Register all SDF functions with a Rhai engine
pub fn register_sdf_api(engine: &mut Engine) {
    crate::api_generated::register_sdf_api_generated(engine);
}
