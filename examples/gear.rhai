// Gear - A mechanical gear with teeth
//
// Demonstrates polar repetition and boolean operations.
// Run with: soyuz watch examples/gear.rhai

// Parameters - try changing these!
let teeth_count = 12;
let outer_radius = 1.0;
let inner_radius = 0.3;
let thickness = 0.2;
let tooth_size = 0.15;

// Main gear body - a cylinder
let body = cylinder(outer_radius - tooth_size, thickness);

// Center hole
let hole = cylinder(inner_radius, thickness + 0.1);

// A single tooth
let tooth = box3(tooth_size * 2.0, thickness, tooth_size * 1.5)
    .translate(outer_radius - tooth_size * 0.3, 0.0, 0.0);

// Repeat teeth around the gear
let teeth = tooth.repeat_polar(teeth_count);

// Spoke holes for a lighter look
let spoke = cylinder(0.08, thickness + 0.1)
    .translate((outer_radius + inner_radius) / 2.0, 0.0, 0.0);
let spokes = spoke.repeat_polar(6);

// Combine: body + teeth - hole - spokes
body.union(teeth).subtract(hole).subtract(spokes)
