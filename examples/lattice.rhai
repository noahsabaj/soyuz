// Lattice Structure - A 3D repeating pattern
//
// Demonstrates infinite repetition and symmetry.
// Run with: soyuz watch examples/lattice.rhai

// Unit cell parameters
let cell_size = 0.5;
let strut_radius = 0.05;

// Create a single strut along each axis
let strut_x = capsule(strut_radius, cell_size * 0.9)
    .rotate_z(deg(90.0));
let strut_y = capsule(strut_radius, cell_size * 0.9);
let strut_z = capsule(strut_radius, cell_size * 0.9)
    .rotate_x(deg(90.0));

// Junction sphere at intersections
let junction = sphere(strut_radius * 1.5);

// Combine into a unit cell
let unit_cell = strut_x
    .union(strut_y)
    .union(strut_z)
    .union(junction);

// Repeat in 3D space (limited to avoid infinite geometry)
let lattice = unit_cell.repeat_limited(
    cell_size, cell_size, cell_size,  // spacing
    3.0, 3.0, 3.0                      // count in each direction
);

// Optional: bound the lattice with a sphere
let bounding_sphere = sphere(cell_size * 2.5);
lattice.intersect(bounding_sphere)
