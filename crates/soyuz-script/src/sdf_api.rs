//! Rhai API for SDF primitives and operations
//!
//! This module provides all SDF functions accessible from Rhai scripts.
//!
//! # Precision Notes
//!
//! Rhai scripts use `f64` for numeric literals, but all values are
//! converted to `f32` when constructing SDF operations. This is
//! required for GPU shader compatibility. For most use cases,
//! the precision loss is negligible.

use rhai::{Engine, Module};
use soyuz_sdf::SdfOp;
use std::sync::Arc;

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
            k: k as f32,
        })
    }

    pub fn smooth_subtract(&mut self, other: RhaiSdf, k: f64) -> RhaiSdf {
        RhaiSdf::new(SdfOp::SmoothSubtract {
            a: Arc::clone(&self.op),
            b: other.op,
            k: k as f32,
        })
    }

    pub fn smooth_intersect(&mut self, other: RhaiSdf, k: f64) -> RhaiSdf {
        RhaiSdf::new(SdfOp::SmoothIntersect {
            a: Arc::clone(&self.op),
            b: other.op,
            k: k as f32,
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
        RhaiSdf::new(SdfOp::Onion {
            inner: Arc::clone(&self.op),
            thickness: thickness as f32,
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
        RhaiSdf::new(SdfOp::RepeatPolar {
            inner: Arc::clone(&self.op),
            count: count as u32,
        })
    }
}

// === Primitive Constructor Functions ===

pub fn sphere(radius: f64) -> RhaiSdf {
    RhaiSdf::new(SdfOp::Sphere {
        radius: radius as f32,
    })
}

pub fn cube(size: f64) -> RhaiSdf {
    let half = (size / 2.0) as f32;
    RhaiSdf::new(SdfOp::Box {
        half_extents: [half, half, half],
    })
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
    RhaiSdf::new(SdfOp::Plane {
        normal: [nx as f32, ny as f32, nz as f32],
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
    // Register the RhaiSdf type
    engine
        .register_type_with_name::<RhaiSdf>("Sdf")
        .register_fn("to_string", |sdf: &mut RhaiSdf| format!("{:?}", sdf.op));

    // === Primitive constructors ===
    engine.register_fn("sphere", sphere);
    engine.register_fn("cube", cube);
    engine.register_fn("box3", box3);
    engine.register_fn("rounded_box", rounded_box);
    engine.register_fn("cylinder", cylinder);
    engine.register_fn("capsule", capsule);
    engine.register_fn("torus", torus);
    engine.register_fn("cone", cone);
    engine.register_fn("plane", plane);
    engine.register_fn("ground_plane", ground_plane);
    engine.register_fn("ellipsoid", ellipsoid);
    engine.register_fn("octahedron", octahedron);
    engine.register_fn("hex_prism", hex_prism);
    engine.register_fn("tri_prism", tri_prism);

    // === Boolean operations ===
    engine.register_fn("union", RhaiSdf::union);
    engine.register_fn("subtract", RhaiSdf::subtract);
    engine.register_fn("intersect", RhaiSdf::intersect);
    engine.register_fn("smooth_union", RhaiSdf::smooth_union);
    engine.register_fn("smooth_subtract", RhaiSdf::smooth_subtract);
    engine.register_fn("smooth_intersect", RhaiSdf::smooth_intersect);

    // === Modifiers ===
    engine.register_fn("shell", RhaiSdf::shell);
    engine.register_fn("hollow", RhaiSdf::hollow);
    engine.register_fn("round", RhaiSdf::round);
    engine.register_fn("onion", RhaiSdf::onion);
    engine.register_fn("elongate", RhaiSdf::elongate);

    // === Transforms ===
    engine.register_fn("translate", RhaiSdf::translate);
    engine.register_fn("translate_x", RhaiSdf::translate_x);
    engine.register_fn("translate_y", RhaiSdf::translate_y);
    engine.register_fn("translate_z", RhaiSdf::translate_z);
    engine.register_fn("rotate_x", RhaiSdf::rotate_x);
    engine.register_fn("rotate_y", RhaiSdf::rotate_y);
    engine.register_fn("rotate_z", RhaiSdf::rotate_z);
    engine.register_fn("scale", RhaiSdf::scale);
    engine.register_fn("mirror_x", RhaiSdf::mirror_x);
    engine.register_fn("mirror_y", RhaiSdf::mirror_y);
    engine.register_fn("mirror_z", RhaiSdf::mirror_z);
    engine.register_fn("symmetry_x", RhaiSdf::symmetry_x);
    engine.register_fn("symmetry_y", RhaiSdf::symmetry_y);
    engine.register_fn("symmetry_z", RhaiSdf::symmetry_z);

    // === Deformations ===
    engine.register_fn("twist", RhaiSdf::twist);
    engine.register_fn("bend", RhaiSdf::bend);

    // === Repetition ===
    engine.register_fn("repeat", RhaiSdf::repeat);
    engine.register_fn("repeat_limited", RhaiSdf::repeat_limited);
    engine.register_fn("repeat_polar", RhaiSdf::repeat_polar);

    // === Math helpers ===
    engine.register_fn("PI", pi);
    engine.register_fn("TAU", tau);
    engine.register_fn("deg", deg_to_rad);
    engine.register_fn("rad", rad_to_deg);
}

/// Create a module with all SDF functions (for imports)
/// Note: This module is for advanced use cases. Most users should use register_sdf_api instead.
pub fn create_sdf_module() -> Module {
    // Return empty module - functions are registered directly via register_sdf_api
    Module::new()
}
