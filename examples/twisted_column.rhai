// Twisted Column - An architectural column with twist deformation
//
// Demonstrates the twist deformation and smooth blending.
// Run with: soyuz watch examples/twisted_column.rhai

// Column parameters
let column_radius = 0.4;
let column_height = 2.0;
let twist_amount = 1.5;  // Try changing this!
let flute_count = 8;
let flute_depth = 0.08;

// Main column shaft
let shaft = cylinder(column_radius, column_height);

// Flutes (vertical grooves) - using repeated subtraction
let flute = cylinder(flute_depth, column_height + 0.1)
    .translate(column_radius, 0.0, 0.0);
let flutes = flute.repeat_polar(flute_count);

// Apply flutes to shaft
let fluted_shaft = shaft.subtract(flutes);

// Apply twist deformation
let twisted = fluted_shaft.twist(twist_amount);

// Base and capital (decorative top/bottom)
let base = cylinder(column_radius * 1.3, 0.15)
    .translate_y(-column_height / 2.0 - 0.075);

let capital = cylinder(column_radius * 1.2, 0.12)
    .smooth_union(
        torus(column_radius * 1.1, 0.08).translate_y(0.1),
        0.05
    )
    .translate_y(column_height / 2.0 + 0.06);

// Combine all parts
twisted.union(base).union(capital)
