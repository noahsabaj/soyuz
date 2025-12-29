//! Rhai API for SDF primitives and operations
//!
//! This module provides all SDF functions accessible from Rhai scripts.

use rhai::{Engine, Module};
use soyuz_sdf::SdfOp;

/// SDF node representation for Rhai
/// This is a wrapper that can be passed around in Rhai and converted to SdfOp
#[derive(Debug, Clone)]
pub struct RhaiSdf {
    pub op: SdfOperation,
}

/// All possible SDF operations (mirrors soyuz_sdf::SdfOp)
#[derive(Debug, Clone)]
pub enum SdfOperation {
    // Primitives
    Sphere {
        radius: f64,
    },
    Box3 {
        half_extents: [f64; 3],
    },
    RoundedBox {
        half_extents: [f64; 3],
        radius: f64,
    },
    Cylinder {
        radius: f64,
        half_height: f64,
    },
    Capsule {
        radius: f64,
        half_height: f64,
    },
    Torus {
        major_radius: f64,
        minor_radius: f64,
    },
    Cone {
        radius: f64,
        height: f64,
    },
    Plane {
        normal: [f64; 3],
        offset: f64,
    },
    Ellipsoid {
        radii: [f64; 3],
    },
    Octahedron {
        size: f64,
    },
    HexPrism {
        half_height: f64,
        radius: f64,
    },
    TriPrism {
        size: [f64; 2],
    },

    // Boolean operations
    Union {
        a: Box<SdfOperation>,
        b: Box<SdfOperation>,
    },
    Subtract {
        a: Box<SdfOperation>,
        b: Box<SdfOperation>,
    },
    Intersect {
        a: Box<SdfOperation>,
        b: Box<SdfOperation>,
    },
    SmoothUnion {
        a: Box<SdfOperation>,
        b: Box<SdfOperation>,
        k: f64,
    },
    SmoothSubtract {
        a: Box<SdfOperation>,
        b: Box<SdfOperation>,
        k: f64,
    },
    SmoothIntersect {
        a: Box<SdfOperation>,
        b: Box<SdfOperation>,
        k: f64,
    },

    // Modifiers
    Shell {
        inner: Box<SdfOperation>,
        thickness: f64,
    },
    Round {
        inner: Box<SdfOperation>,
        radius: f64,
    },
    Onion {
        inner: Box<SdfOperation>,
        thickness: f64,
    },
    Elongate {
        inner: Box<SdfOperation>,
        h: [f64; 3],
    },

    // Transforms
    Translate {
        inner: Box<SdfOperation>,
        offset: [f64; 3],
    },
    RotateX {
        inner: Box<SdfOperation>,
        angle: f64,
    },
    RotateY {
        inner: Box<SdfOperation>,
        angle: f64,
    },
    RotateZ {
        inner: Box<SdfOperation>,
        angle: f64,
    },
    Scale {
        inner: Box<SdfOperation>,
        factor: f64,
    },
    MirrorX {
        inner: Box<SdfOperation>,
    },
    MirrorY {
        inner: Box<SdfOperation>,
    },
    MirrorZ {
        inner: Box<SdfOperation>,
    },
    SymmetryX {
        inner: Box<SdfOperation>,
    },
    SymmetryY {
        inner: Box<SdfOperation>,
    },
    SymmetryZ {
        inner: Box<SdfOperation>,
    },

    // Deformations
    Twist {
        inner: Box<SdfOperation>,
        amount: f64,
    },
    Bend {
        inner: Box<SdfOperation>,
        amount: f64,
    },

    // Repetition
    RepeatInfinite {
        inner: Box<SdfOperation>,
        spacing: [f64; 3],
    },
    RepeatLimited {
        inner: Box<SdfOperation>,
        spacing: [f64; 3],
        count: [f64; 3],
    },
    RepeatPolar {
        inner: Box<SdfOperation>,
        count: i64,
    },
}

impl RhaiSdf {
    pub fn new(op: SdfOperation) -> Self {
        Self { op }
    }

    // === Boolean Operations ===

    pub fn union(&mut self, other: RhaiSdf) -> RhaiSdf {
        RhaiSdf::new(SdfOperation::Union {
            a: Box::new(self.op.clone()),
            b: Box::new(other.op),
        })
    }

    pub fn subtract(&mut self, other: RhaiSdf) -> RhaiSdf {
        RhaiSdf::new(SdfOperation::Subtract {
            a: Box::new(self.op.clone()),
            b: Box::new(other.op),
        })
    }

    pub fn intersect(&mut self, other: RhaiSdf) -> RhaiSdf {
        RhaiSdf::new(SdfOperation::Intersect {
            a: Box::new(self.op.clone()),
            b: Box::new(other.op),
        })
    }

    pub fn smooth_union(&mut self, other: RhaiSdf, k: f64) -> RhaiSdf {
        RhaiSdf::new(SdfOperation::SmoothUnion {
            a: Box::new(self.op.clone()),
            b: Box::new(other.op),
            k,
        })
    }

    pub fn smooth_subtract(&mut self, other: RhaiSdf, k: f64) -> RhaiSdf {
        RhaiSdf::new(SdfOperation::SmoothSubtract {
            a: Box::new(self.op.clone()),
            b: Box::new(other.op),
            k,
        })
    }

    pub fn smooth_intersect(&mut self, other: RhaiSdf, k: f64) -> RhaiSdf {
        RhaiSdf::new(SdfOperation::SmoothIntersect {
            a: Box::new(self.op.clone()),
            b: Box::new(other.op),
            k,
        })
    }

    // === Modifiers ===

    pub fn shell(&mut self, thickness: f64) -> RhaiSdf {
        RhaiSdf::new(SdfOperation::Shell {
            inner: Box::new(self.op.clone()),
            thickness,
        })
    }

    pub fn hollow(&mut self, thickness: f64) -> RhaiSdf {
        self.shell(thickness)
    }

    pub fn round(&mut self, radius: f64) -> RhaiSdf {
        RhaiSdf::new(SdfOperation::Round {
            inner: Box::new(self.op.clone()),
            radius,
        })
    }

    pub fn onion(&mut self, thickness: f64) -> RhaiSdf {
        RhaiSdf::new(SdfOperation::Onion {
            inner: Box::new(self.op.clone()),
            thickness,
        })
    }

    pub fn elongate(&mut self, x: f64, y: f64, z: f64) -> RhaiSdf {
        RhaiSdf::new(SdfOperation::Elongate {
            inner: Box::new(self.op.clone()),
            h: [x, y, z],
        })
    }

    // === Transforms ===

    pub fn translate(&mut self, x: f64, y: f64, z: f64) -> RhaiSdf {
        RhaiSdf::new(SdfOperation::Translate {
            inner: Box::new(self.op.clone()),
            offset: [x, y, z],
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
        RhaiSdf::new(SdfOperation::RotateX {
            inner: Box::new(self.op.clone()),
            angle,
        })
    }

    pub fn rotate_y(&mut self, angle: f64) -> RhaiSdf {
        RhaiSdf::new(SdfOperation::RotateY {
            inner: Box::new(self.op.clone()),
            angle,
        })
    }

    pub fn rotate_z(&mut self, angle: f64) -> RhaiSdf {
        RhaiSdf::new(SdfOperation::RotateZ {
            inner: Box::new(self.op.clone()),
            angle,
        })
    }

    pub fn scale(&mut self, factor: f64) -> RhaiSdf {
        RhaiSdf::new(SdfOperation::Scale {
            inner: Box::new(self.op.clone()),
            factor,
        })
    }

    pub fn mirror_x(&mut self) -> RhaiSdf {
        RhaiSdf::new(SdfOperation::MirrorX {
            inner: Box::new(self.op.clone()),
        })
    }

    pub fn mirror_y(&mut self) -> RhaiSdf {
        RhaiSdf::new(SdfOperation::MirrorY {
            inner: Box::new(self.op.clone()),
        })
    }

    pub fn mirror_z(&mut self) -> RhaiSdf {
        RhaiSdf::new(SdfOperation::MirrorZ {
            inner: Box::new(self.op.clone()),
        })
    }

    pub fn symmetry_x(&mut self) -> RhaiSdf {
        RhaiSdf::new(SdfOperation::SymmetryX {
            inner: Box::new(self.op.clone()),
        })
    }

    pub fn symmetry_y(&mut self) -> RhaiSdf {
        RhaiSdf::new(SdfOperation::SymmetryY {
            inner: Box::new(self.op.clone()),
        })
    }

    pub fn symmetry_z(&mut self) -> RhaiSdf {
        RhaiSdf::new(SdfOperation::SymmetryZ {
            inner: Box::new(self.op.clone()),
        })
    }

    // === Deformations ===

    pub fn twist(&mut self, amount: f64) -> RhaiSdf {
        RhaiSdf::new(SdfOperation::Twist {
            inner: Box::new(self.op.clone()),
            amount,
        })
    }

    pub fn bend(&mut self, amount: f64) -> RhaiSdf {
        RhaiSdf::new(SdfOperation::Bend {
            inner: Box::new(self.op.clone()),
            amount,
        })
    }

    // === Repetition ===

    pub fn repeat(&mut self, sx: f64, sy: f64, sz: f64) -> RhaiSdf {
        RhaiSdf::new(SdfOperation::RepeatInfinite {
            inner: Box::new(self.op.clone()),
            spacing: [sx, sy, sz],
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
        RhaiSdf::new(SdfOperation::RepeatLimited {
            inner: Box::new(self.op.clone()),
            spacing: [sx, sy, sz],
            count: [cx, cy, cz],
        })
    }

    pub fn repeat_polar(&mut self, count: i64) -> RhaiSdf {
        RhaiSdf::new(SdfOperation::RepeatPolar {
            inner: Box::new(self.op.clone()),
            count,
        })
    }
}

// === Primitive Constructor Functions ===

pub fn sphere(radius: f64) -> RhaiSdf {
    RhaiSdf::new(SdfOperation::Sphere { radius })
}

pub fn cube(size: f64) -> RhaiSdf {
    let half = size / 2.0;
    RhaiSdf::new(SdfOperation::Box3 {
        half_extents: [half, half, half],
    })
}

pub fn box3(x: f64, y: f64, z: f64) -> RhaiSdf {
    RhaiSdf::new(SdfOperation::Box3 {
        half_extents: [x / 2.0, y / 2.0, z / 2.0],
    })
}

pub fn rounded_box(x: f64, y: f64, z: f64, radius: f64) -> RhaiSdf {
    RhaiSdf::new(SdfOperation::RoundedBox {
        half_extents: [x / 2.0, y / 2.0, z / 2.0],
        radius,
    })
}

pub fn cylinder(radius: f64, height: f64) -> RhaiSdf {
    RhaiSdf::new(SdfOperation::Cylinder {
        radius,
        half_height: height / 2.0,
    })
}

pub fn capsule(radius: f64, height: f64) -> RhaiSdf {
    RhaiSdf::new(SdfOperation::Capsule {
        radius,
        half_height: height / 2.0,
    })
}

pub fn torus(major_radius: f64, minor_radius: f64) -> RhaiSdf {
    RhaiSdf::new(SdfOperation::Torus {
        major_radius,
        minor_radius,
    })
}

pub fn cone(radius: f64, height: f64) -> RhaiSdf {
    RhaiSdf::new(SdfOperation::Cone { radius, height })
}

pub fn plane(nx: f64, ny: f64, nz: f64, offset: f64) -> RhaiSdf {
    RhaiSdf::new(SdfOperation::Plane {
        normal: [nx, ny, nz],
        offset,
    })
}

pub fn ground_plane() -> RhaiSdf {
    plane(0.0, 1.0, 0.0, 0.0)
}

pub fn ellipsoid(rx: f64, ry: f64, rz: f64) -> RhaiSdf {
    RhaiSdf::new(SdfOperation::Ellipsoid {
        radii: [rx, ry, rz],
    })
}

pub fn octahedron(size: f64) -> RhaiSdf {
    RhaiSdf::new(SdfOperation::Octahedron { size })
}

pub fn hex_prism(radius: f64, height: f64) -> RhaiSdf {
    RhaiSdf::new(SdfOperation::HexPrism {
        half_height: height / 2.0,
        radius,
    })
}

pub fn tri_prism(width: f64, height: f64) -> RhaiSdf {
    RhaiSdf::new(SdfOperation::TriPrism {
        size: [width, height],
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

// ============================================================================
// Conversion from SdfOperation to soyuz_sdf::SdfOp
// ============================================================================

impl RhaiSdf {
    /// Convert to renderer-compatible SdfOp
    pub fn to_sdf_op(&self) -> SdfOp {
        convert_to_sdf_op(&self.op)
    }
}

/// Convert SdfOperation (f64) to SdfOp (f32) for the renderer
fn convert_to_sdf_op(op: &SdfOperation) -> SdfOp {
    match op {
        // Primitives
        SdfOperation::Sphere { radius } => SdfOp::Sphere {
            radius: *radius as f32,
        },
        SdfOperation::Box3 { half_extents } => SdfOp::Box {
            half_extents: [
                half_extents[0] as f32,
                half_extents[1] as f32,
                half_extents[2] as f32,
            ],
        },
        SdfOperation::RoundedBox {
            half_extents,
            radius,
        } => SdfOp::RoundedBox {
            half_extents: [
                half_extents[0] as f32,
                half_extents[1] as f32,
                half_extents[2] as f32,
            ],
            radius: *radius as f32,
        },
        SdfOperation::Cylinder {
            radius,
            half_height,
        } => SdfOp::Cylinder {
            radius: *radius as f32,
            half_height: *half_height as f32,
        },
        SdfOperation::Capsule {
            radius,
            half_height,
        } => SdfOp::Capsule {
            radius: *radius as f32,
            half_height: *half_height as f32,
        },
        SdfOperation::Torus {
            major_radius,
            minor_radius,
        } => SdfOp::Torus {
            major_radius: *major_radius as f32,
            minor_radius: *minor_radius as f32,
        },
        SdfOperation::Cone { radius, height } => SdfOp::Cone {
            radius: *radius as f32,
            height: *height as f32,
        },
        SdfOperation::Plane { normal, offset } => SdfOp::Plane {
            normal: [normal[0] as f32, normal[1] as f32, normal[2] as f32],
            offset: *offset as f32,
        },
        SdfOperation::Ellipsoid { radii } => SdfOp::Ellipsoid {
            radii: [radii[0] as f32, radii[1] as f32, radii[2] as f32],
        },
        SdfOperation::Octahedron { size } => SdfOp::Octahedron { size: *size as f32 },
        SdfOperation::HexPrism {
            half_height,
            radius,
        } => SdfOp::HexPrism {
            half_height: *half_height as f32,
            radius: *radius as f32,
        },
        SdfOperation::TriPrism { size } => SdfOp::TriPrism {
            size: [size[0] as f32, size[1] as f32],
        },

        // Boolean operations
        SdfOperation::Union { a, b } => SdfOp::Union {
            a: Box::new(convert_to_sdf_op(a)),
            b: Box::new(convert_to_sdf_op(b)),
        },
        SdfOperation::Subtract { a, b } => SdfOp::Subtract {
            a: Box::new(convert_to_sdf_op(a)),
            b: Box::new(convert_to_sdf_op(b)),
        },
        SdfOperation::Intersect { a, b } => SdfOp::Intersect {
            a: Box::new(convert_to_sdf_op(a)),
            b: Box::new(convert_to_sdf_op(b)),
        },
        SdfOperation::SmoothUnion { a, b, k } => SdfOp::SmoothUnion {
            a: Box::new(convert_to_sdf_op(a)),
            b: Box::new(convert_to_sdf_op(b)),
            k: *k as f32,
        },
        SdfOperation::SmoothSubtract { a, b, k } => SdfOp::SmoothSubtract {
            a: Box::new(convert_to_sdf_op(a)),
            b: Box::new(convert_to_sdf_op(b)),
            k: *k as f32,
        },
        SdfOperation::SmoothIntersect { a, b, k } => SdfOp::SmoothIntersect {
            a: Box::new(convert_to_sdf_op(a)),
            b: Box::new(convert_to_sdf_op(b)),
            k: *k as f32,
        },

        // Modifiers
        SdfOperation::Shell { inner, thickness } => SdfOp::Shell {
            inner: Box::new(convert_to_sdf_op(inner)),
            thickness: *thickness as f32,
        },
        SdfOperation::Round { inner, radius } => SdfOp::Round {
            inner: Box::new(convert_to_sdf_op(inner)),
            radius: *radius as f32,
        },
        SdfOperation::Onion { inner, thickness } => SdfOp::Onion {
            inner: Box::new(convert_to_sdf_op(inner)),
            thickness: *thickness as f32,
        },
        SdfOperation::Elongate { inner, h } => SdfOp::Elongate {
            inner: Box::new(convert_to_sdf_op(inner)),
            h: [h[0] as f32, h[1] as f32, h[2] as f32],
        },

        // Transforms
        SdfOperation::Translate { inner, offset } => SdfOp::Translate {
            inner: Box::new(convert_to_sdf_op(inner)),
            offset: [offset[0] as f32, offset[1] as f32, offset[2] as f32],
        },
        SdfOperation::RotateX { inner, angle } => SdfOp::RotateX {
            inner: Box::new(convert_to_sdf_op(inner)),
            angle: *angle as f32,
        },
        SdfOperation::RotateY { inner, angle } => SdfOp::RotateY {
            inner: Box::new(convert_to_sdf_op(inner)),
            angle: *angle as f32,
        },
        SdfOperation::RotateZ { inner, angle } => SdfOp::RotateZ {
            inner: Box::new(convert_to_sdf_op(inner)),
            angle: *angle as f32,
        },
        SdfOperation::Scale { inner, factor } => SdfOp::Scale {
            inner: Box::new(convert_to_sdf_op(inner)),
            factor: *factor as f32,
        },
        SdfOperation::MirrorX { inner } => SdfOp::Mirror {
            inner: Box::new(convert_to_sdf_op(inner)),
            axis: [1.0, 0.0, 0.0],
        },
        SdfOperation::MirrorY { inner } => SdfOp::Mirror {
            inner: Box::new(convert_to_sdf_op(inner)),
            axis: [0.0, 1.0, 0.0],
        },
        SdfOperation::MirrorZ { inner } => SdfOp::Mirror {
            inner: Box::new(convert_to_sdf_op(inner)),
            axis: [0.0, 0.0, 1.0],
        },
        SdfOperation::SymmetryX { inner } => SdfOp::SymmetryX {
            inner: Box::new(convert_to_sdf_op(inner)),
        },
        SdfOperation::SymmetryY { inner } => SdfOp::SymmetryY {
            inner: Box::new(convert_to_sdf_op(inner)),
        },
        SdfOperation::SymmetryZ { inner } => SdfOp::SymmetryZ {
            inner: Box::new(convert_to_sdf_op(inner)),
        },

        // Deformations
        SdfOperation::Twist { inner, amount } => SdfOp::Twist {
            inner: Box::new(convert_to_sdf_op(inner)),
            amount: *amount as f32,
        },
        SdfOperation::Bend { inner, amount } => SdfOp::Bend {
            inner: Box::new(convert_to_sdf_op(inner)),
            amount: *amount as f32,
        },

        // Repetition
        SdfOperation::RepeatInfinite { inner, spacing } => SdfOp::RepeatInfinite {
            inner: Box::new(convert_to_sdf_op(inner)),
            spacing: [spacing[0] as f32, spacing[1] as f32, spacing[2] as f32],
        },
        SdfOperation::RepeatLimited {
            inner,
            spacing,
            count,
        } => SdfOp::RepeatLimited {
            inner: Box::new(convert_to_sdf_op(inner)),
            spacing: [spacing[0] as f32, spacing[1] as f32, spacing[2] as f32],
            count: [count[0] as f32, count[1] as f32, count[2] as f32],
        },
        SdfOperation::RepeatPolar { inner, count } => SdfOp::RepeatPolar {
            inner: Box::new(convert_to_sdf_op(inner)),
            count: *count as u32,
        },
    }
}
