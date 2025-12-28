// Sci-Fi Crate - A futuristic cargo container
//
// Demonstrates rounded boxes, shell, and panel details.
// Run with: soyuz watch examples/sci_fi_crate.rhai

// Crate dimensions
let width = 1.0;
let height = 0.8;
let depth = 0.7;
let corner_radius = 0.08;
let wall_thickness = 0.04;

// Main body - rounded box made hollow
let body = rounded_box(width, height, depth, corner_radius)
    .shell(wall_thickness);

// Edge trim - reinforced edges
let edge_h = box3(width + 0.02, 0.06, 0.06);
let edge_v = box3(0.06, height + 0.02, 0.06);
let edge_d = box3(0.06, 0.06, depth + 0.02);

// Place edges at corners
let top_edge = edge_h.translate_y(height / 2.0);
let bottom_edge = edge_h.translate_y(-height / 2.0);
let front_edge = edge_d.translate(0.0, 0.0, depth / 2.0);
let back_edge = edge_d.translate(0.0, 0.0, -depth / 2.0);

// Side handles
let handle = capsule(0.03, 0.15)
    .rotate_z(deg(90.0))
    .translate(width / 2.0 + 0.02, 0.0, 0.0);

// Panel detail - recessed rectangles
let panel = box3(width * 0.6, height * 0.4, 0.02)
    .translate(0.0, 0.0, depth / 2.0 - 0.01);

// Latch detail
let latch = box3(0.15, 0.08, 0.05)
    .translate(0.0, height / 2.0 - 0.15, depth / 2.0);

// Combine everything
body
    .union(top_edge)
    .union(bottom_edge)
    .union(handle)
    .union(handle.mirror_x())
    .subtract(panel)
    .subtract(panel.mirror_z())
    .union(latch)
