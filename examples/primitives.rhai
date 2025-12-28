// Primitives Showcase - All available SDF primitives
//
// This shows all the basic shapes you can use.
// Run with: soyuz watch examples/primitives.rhai
//
// Try uncommenting different shapes to see them!

// === BASIC SHAPES ===

// Sphere - radius
// sphere(0.5)

// Cube - size (creates a centered cube)
// cube(1.0)

// Box - width, height, depth (rectangular box)
// box3(1.0, 0.5, 0.3)

// Rounded Box - width, height, depth, corner radius
// rounded_box(1.0, 0.5, 0.3, 0.1)

// Cylinder - radius, height
// cylinder(0.4, 1.0)

// Capsule - radius, height (cylinder with rounded ends)
// capsule(0.3, 0.8)

// Torus - major radius (ring size), minor radius (tube thickness)
// torus(0.5, 0.15)

// Cone - base radius, height
// cone(0.5, 1.0)

// === PLATONIC/GEOMETRIC ===

// Octahedron - size
// octahedron(0.6)

// Hexagonal Prism - radius, height
// hex_prism(0.4, 0.8)

// Triangular Prism - width, height
// tri_prism(0.5, 0.8)

// Ellipsoid - x radius, y radius, z radius
// ellipsoid(0.6, 0.4, 0.3)

// === INFINITE SHAPES ===

// Plane - normal x, y, z, offset from origin
// plane(0.0, 1.0, 0.0, 0.0)  // ground plane

// Ground plane shortcut
// ground_plane()

// === SHOWCASE: Display multiple shapes ===
let shapes = sphere(0.3).translate(-1.5, 0.0, -1.0)
    .union(cube(0.5).translate(0.0, 0.0, -1.0))
    .union(cylinder(0.25, 0.6).translate(1.5, 0.0, -1.0))
    .union(torus(0.35, 0.1).translate(-1.5, 0.0, 0.5))
    .union(capsule(0.2, 0.4).translate(0.0, 0.0, 0.5))
    .union(cone(0.3, 0.6).translate(1.5, 0.0, 0.5))
    .union(octahedron(0.4).translate(-0.75, 0.0, 2.0))
    .union(hex_prism(0.25, 0.5).translate(0.75, 0.0, 2.0));

shapes
