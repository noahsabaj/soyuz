// Barrel - A classic procedural asset example
//
// This script creates a hollow barrel with metal bands.
// Run with: soyuz watch examples/barrel.rhai
//       or: soyuz preview --script examples/barrel.rhai

// Main body - a cylinder
let body = cylinder(0.5, 1.2);

// Metal bands around the barrel
let band_top = torus(0.5, 0.08).translate_y(0.5);
let band_bottom = torus(0.5, 0.08).translate_y(-0.5);
let band_middle = torus(0.52, 0.06);

// Combine all parts with smooth blending
let solid = body
    .smooth_union(band_top, 0.05)
    .smooth_union(band_bottom, 0.05)
    .smooth_union(band_middle, 0.03);

// Make it hollow
solid.hollow(0.05)
