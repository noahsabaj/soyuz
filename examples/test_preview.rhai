// Test script for previewing in Soyuz Studio
// Demonstrates: primitives, smooth union, transforms, and environment

// Set up a nice studio environment
env_studio();
set_material_color(0.8, 0.6, 0.4);

// Create a simple sculptural form
let base = cylinder(0.8, 0.3)
    .translate_y(-0.5);

let body = sphere(0.6)
    .scale(1.2);

let top = torus(0.3, 0.1)
    .translate_y(0.6);

// Combine with smooth blending
base
    .smooth_union(body, 0.15)
    .smooth_union(top, 0.1)
