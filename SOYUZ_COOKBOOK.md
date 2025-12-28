# Soyuz Cookbook

A complete reference and recipe book for creating 3D models with Soyuz.

Soyuz scripts are written in **Rhai** (a JavaScript-like scripting language) with Soyuz's SDF functions registered. This means you get variables, loops, conditionals, and all the usual programming constructs, plus a library of shape-building functions.

---

## Table of Contents

1. [Quick Start](#quick-start)
2. [Primitives](#primitives)
3. [Boolean Operations](#boolean-operations)
4. [Transforms](#transforms)
5. [Modifiers](#modifiers)
6. [Deformations](#deformations)
7. [Repetition](#repetition)
8. [Math Helpers](#math-helpers)
9. [Environment & Lighting](#environment--lighting)
10. [Recipes](#recipes)

---

## Quick Start

```rhai
// A simple shape - just return an SDF at the end of your script
sphere(0.5)
```

```rhai
// Combine shapes with method chaining
cylinder(0.5, 1.0)
    .union(sphere(0.6))
    .translate_y(0.5)
```

```rhai
// Use variables for parametric models
let radius = 0.5;
let height = 1.0;
cylinder(radius, height)
```

**Important:** Your script must return an SDF (don't end with `;` on the last line).

---

## Primitives

Functions that create basic shapes. All dimensions are in world units.

### `sphere(radius)`
A sphere centered at origin.
```rhai
sphere(0.5)       // radius 0.5
```

### `cube(size)`
A cube centered at origin.
```rhai
cube(1.0)         // 1x1x1 cube
```

### `box3(width, height, depth)`
A rectangular box centered at origin.
```rhai
box3(1.0, 0.5, 0.3)   // width=1, height=0.5, depth=0.3
```

### `rounded_box(width, height, depth, radius)`
A box with rounded edges.
```rhai
rounded_box(1.0, 0.5, 0.3, 0.1)   // corner radius 0.1
```

### `cylinder(radius, height)`
A cylinder centered at origin, extending along Y axis.
```rhai
cylinder(0.4, 1.0)    // radius=0.4, height=1.0
```

### `capsule(radius, height)`
A cylinder with hemispherical caps (pill shape).
```rhai
capsule(0.3, 0.8)     // radius=0.3, total height=0.8
```

### `torus(major_radius, minor_radius)`
A donut shape lying in the XZ plane.
```rhai
torus(0.5, 0.15)      // ring radius=0.5, tube radius=0.15
```

### `cone(radius, height)`
A cone with base at origin, pointing up.
```rhai
cone(0.5, 1.0)        // base radius=0.5, height=1.0
```

### `ellipsoid(rx, ry, rz)`
A stretched sphere.
```rhai
ellipsoid(0.6, 0.4, 0.3)   // different radius per axis
```

### `octahedron(size)`
An 8-faced platonic solid.
```rhai
octahedron(0.6)
```

### `hex_prism(radius, height)`
A hexagonal prism (6-sided column).
```rhai
hex_prism(0.4, 0.8)
```

### `tri_prism(width, height)`
A triangular prism.
```rhai
tri_prism(0.5, 0.8)
```

### `plane(nx, ny, nz, offset)`
An infinite plane defined by normal and offset.
```rhai
plane(0.0, 1.0, 0.0, 0.0)   // horizontal plane at y=0
```

### `ground_plane()`
Shortcut for a horizontal ground plane at y=0.
```rhai
ground_plane()
```

---

## Boolean Operations

Combine shapes together. All boolean operations are methods called on an SDF.

### `.union(other)`
Combine two shapes (add).
```rhai
sphere(0.5).union(cube(0.8))
```

### `.subtract(other)`
Remove one shape from another (cut).
```rhai
cube(1.0).subtract(sphere(0.7))   // cube with spherical hole
```

### `.intersect(other)`
Keep only where both shapes overlap.
```rhai
sphere(0.6).intersect(cube(0.8))  // rounded cube
```

### `.smooth_union(other, k)`
Blend two shapes together smoothly. `k` controls blend radius.
```rhai
sphere(0.4).smooth_union(sphere(0.4).translate_x(0.5), 0.2)
```

### `.smooth_subtract(other, k)`
Smooth subtraction with filleted edge.
```rhai
cube(1.0).smooth_subtract(sphere(0.6), 0.1)
```

### `.smooth_intersect(other, k)`
Smooth intersection with filleted edge.
```rhai
sphere(0.6).smooth_intersect(cube(0.8), 0.1)
```

---

## Transforms

Move, rotate, and scale shapes. All transforms are methods.

### `.translate(x, y, z)`
Move shape by offset.
```rhai
sphere(0.5).translate(1.0, 0.5, 0.0)
```

### `.translate_x(x)` / `.translate_y(y)` / `.translate_z(z)`
Move along a single axis.
```rhai
sphere(0.5).translate_y(1.0)
```

### `.rotate_x(angle)` / `.rotate_y(angle)` / `.rotate_z(angle)`
Rotate around an axis. **Angle is in radians**.
```rhai
box3(1.0, 0.2, 0.5).rotate_z(deg(45.0))   // use deg() to convert degrees
```

### `.scale(factor)`
Uniform scale.
```rhai
sphere(1.0).scale(0.5)   // same as sphere(0.5)
```

### `.mirror_x()` / `.mirror_y()` / `.mirror_z()`
Flip shape across a plane (creates a mirrored copy at negative coordinates).
```rhai
let handle = capsule(0.1, 0.3).translate_x(0.5);
handle.union(handle.mirror_x())   // handles on both sides
```

### `.symmetry_x()` / `.symmetry_y()` / `.symmetry_z()`
Make shape symmetric (folds space, only renders positive side mirrored).
```rhai
sphere(0.5).translate_x(0.3).symmetry_x()   // two spheres
```

---

## Modifiers

Transform the surface of shapes.

### `.shell(thickness)` / `.hollow(thickness)`
Make shape hollow with given wall thickness. (Both do the same thing.)
```rhai
sphere(0.5).shell(0.05)    // hollow sphere
sphere(0.5).hollow(0.05)   // same thing
```

### `.round(radius)`
Round all edges by shrinking then expanding.
```rhai
cube(1.0).round(0.1)   // cube with rounded edges
```

### `.onion(thickness)`
Create concentric shells (like an onion).
```rhai
sphere(0.5).onion(0.1)   // nested spherical shells
```

### `.elongate(x, y, z)`
Stretch shape by inserting flat sections.
```rhai
sphere(0.3).elongate(0.5, 0.0, 0.0)   // pill-like shape
```

---

## Deformations

Warp and bend shapes.

### `.twist(amount)`
Twist shape around Y axis. Amount is twist per unit height.
```rhai
box3(0.3, 2.0, 0.3).twist(2.0)   // twisted column
```

### `.bend(amount)`
Bend shape around Y axis.
```rhai
box3(2.0, 0.2, 0.3).bend(1.0)   // curved beam
```

---

## Repetition

Create patterns by repeating shapes.

### `.repeat(sx, sy, sz)`
Infinite repetition with given spacing. **Use with caution** - creates infinite geometry.
```rhai
sphere(0.2).repeat(1.0, 1.0, 1.0)   // infinite grid of spheres
```

### `.repeat_limited(sx, sy, sz, cx, cy, cz)`
Finite repetition. Spacing (sx,sy,sz) and count (cx,cy,cz) per axis.
```rhai
sphere(0.1).repeat_limited(
    0.3, 0.3, 0.3,   // spacing
    3.0, 3.0, 3.0    // count (3x3x3 grid)
)
```

### `.repeat_polar(count)`
Repeat around Y axis in a circle.
```rhai
box3(0.1, 0.5, 0.1)
    .translate_x(0.5)      // move out from center
    .repeat_polar(8)       // 8 copies in a circle
```

---

## Math Helpers

Constants and conversions for working with angles.

### `PI()`
Returns pi (3.14159...).
```rhai
let quarter_turn = PI() / 2.0;
```

### `TAU()`
Returns tau (2*pi = 6.28318...).
```rhai
let full_rotation = TAU();
```

### `deg(degrees)`
Convert degrees to radians.
```rhai
box3(1.0, 0.2, 0.5).rotate_z(deg(45.0))
```

### `rad(radians)`
Convert radians to degrees (rarely needed).
```rhai
let degrees = rad(PI());   // 180.0
```

---

## Environment & Lighting

Configure the rendering environment. These functions don't return anything - they modify global settings.

### Lighting Presets

Quick presets for common lighting setups:

```rhai
env_studio();    // Neutral studio lighting (default)
env_daylight();  // Bright outdoor daylight
env_sunset();    // Warm orange sunset
env_night();     // Dark blue moonlight
env_clay();      // Soft clay render (no shadows, strong AO)
```

### Sun/Light Settings

```rhai
set_sun_direction(x, y, z);     // Direction TO the sun (will be normalized)
set_sun_color(r, g, b);         // RGB 0-1
set_sun_intensity(intensity);   // Brightness multiplier
set_ambient_color(r, g, b);     // Fill light color
set_ambient_intensity(intensity);
```

### Material Settings

```rhai
set_material_color(r, g, b);        // RGB 0-1
set_material_color_hex("#ff5500");  // Hex color string
set_material_shininess(shininess);  // Specular exponent (higher = shinier)
set_specular_intensity(intensity);  // 0-1
```

### Sky/Background Settings

```rhai
set_sky_horizon(r, g, b);    // Color at horizon
set_sky_zenith(r, g, b);     // Color straight up
set_fog_color(r, g, b);      // Fog color
set_fog_density(density);    // 0 = no fog, higher = more fog
```

### Effect Settings

```rhai
set_ao_enabled(true);           // Ambient occlusion on/off
set_ao_intensity(intensity);    // AO strength
set_shadows_enabled(true);      // Soft shadows on/off
set_shadow_softness(softness);  // Higher = softer shadows
```

### Color Helper

```rhai
let color = rgb_hex("#ff5500");   // Returns [r, g, b] array
```

---

## Recipes

Complete examples demonstrating common patterns.

### Barrel

A hollow barrel with metal bands.

```rhai
// Main body
let body = cylinder(0.5, 1.2);

// Metal bands
let band_top = torus(0.5, 0.08).translate_y(0.5);
let band_bottom = torus(0.5, 0.08).translate_y(-0.5);
let band_middle = torus(0.52, 0.06);

// Combine with smooth blending
let solid = body
    .smooth_union(band_top, 0.05)
    .smooth_union(band_bottom, 0.05)
    .smooth_union(band_middle, 0.03);

// Make hollow
solid.hollow(0.05)
```

### Gear

A mechanical gear with teeth and spoke holes.

```rhai
let teeth_count = 12;
let outer_radius = 1.0;
let inner_radius = 0.3;
let thickness = 0.2;
let tooth_size = 0.15;

// Main body
let body = cylinder(outer_radius - tooth_size, thickness);

// Center hole
let hole = cylinder(inner_radius, thickness + 0.1);

// Single tooth, repeated
let tooth = box3(tooth_size * 2.0, thickness, tooth_size * 1.5)
    .translate(outer_radius - tooth_size * 0.3, 0.0, 0.0);
let teeth = tooth.repeat_polar(teeth_count);

// Spoke holes for lighter look
let spoke = cylinder(0.08, thickness + 0.1)
    .translate((outer_radius + inner_radius) / 2.0, 0.0, 0.0);
let spokes = spoke.repeat_polar(6);

// Combine: body + teeth - hole - spokes
body.union(teeth).subtract(hole).subtract(spokes)
```

### Twisted Column

An architectural column with flutes and twist deformation.

```rhai
let column_radius = 0.4;
let column_height = 2.0;
let twist_amount = 1.5;
let flute_count = 8;
let flute_depth = 0.08;

// Main shaft with flutes
let shaft = cylinder(column_radius, column_height);
let flute = cylinder(flute_depth, column_height + 0.1)
    .translate(column_radius, 0.0, 0.0);
let fluted_shaft = shaft.subtract(flute.repeat_polar(flute_count));

// Apply twist
let twisted = fluted_shaft.twist(twist_amount);

// Base and capital
let base = cylinder(column_radius * 1.3, 0.15)
    .translate_y(-column_height / 2.0 - 0.075);

let capital = cylinder(column_radius * 1.2, 0.12)
    .smooth_union(
        torus(column_radius * 1.1, 0.08).translate_y(0.1),
        0.05
    )
    .translate_y(column_height / 2.0 + 0.06);

twisted.union(base).union(capital)
```

### Sci-Fi Crate

A futuristic cargo container with panel details.

```rhai
let width = 1.0;
let height = 0.8;
let depth = 0.7;
let corner_radius = 0.08;
let wall_thickness = 0.04;

// Hollow rounded box
let body = rounded_box(width, height, depth, corner_radius)
    .shell(wall_thickness);

// Edge reinforcements
let edge_h = box3(width + 0.02, 0.06, 0.06);
let top_edge = edge_h.translate_y(height / 2.0);
let bottom_edge = edge_h.translate_y(-height / 2.0);

// Side handles
let handle = capsule(0.03, 0.15)
    .rotate_z(deg(90.0))
    .translate(width / 2.0 + 0.02, 0.0, 0.0);

// Panel details (recessed)
let panel = box3(width * 0.6, height * 0.4, 0.02)
    .translate(0.0, 0.0, depth / 2.0 - 0.01);

body
    .union(top_edge)
    .union(bottom_edge)
    .union(handle)
    .union(handle.mirror_x())
    .subtract(panel)
    .subtract(panel.mirror_z())
```

### Donut with Frosting

Organic shape blending.

```rhai
let donut = torus(0.5, 0.2);

// Frosting - larger torus with bottom cut off
let frosting_shape = torus(0.5, 0.22);
let cutter = box3(2.0, 0.5, 2.0).translate_y(-0.25);
let frosting = frosting_shape.subtract(cutter);

// Smooth blend
donut.smooth_union(frosting, 0.05)
```

### Lattice Structure

3D repeating pattern bounded by a sphere.

```rhai
let cell_size = 0.5;
let strut_radius = 0.05;

// Struts along each axis
let strut_x = capsule(strut_radius, cell_size * 0.9).rotate_z(deg(90.0));
let strut_y = capsule(strut_radius, cell_size * 0.9);
let strut_z = capsule(strut_radius, cell_size * 0.9).rotate_x(deg(90.0));

// Junction sphere
let junction = sphere(strut_radius * 1.5);

// Unit cell
let unit_cell = strut_x
    .union(strut_y)
    .union(strut_z)
    .union(junction);

// Repeat in 3D (limited)
let lattice = unit_cell.repeat_limited(
    cell_size, cell_size, cell_size,
    3.0, 3.0, 3.0
);

// Bound with sphere
lattice.intersect(sphere(cell_size * 2.5))
```

---

## Tips & Tricks

### Order of Operations
Transforms apply to the shape they're called on. Chain them left to right:
```rhai
// First translate, then rotate
sphere(0.3).translate_x(1.0).rotate_y(deg(45.0))

// vs. First rotate, then translate (different result!)
sphere(0.3).rotate_y(deg(45.0)).translate_x(1.0)
```

### Smooth Blending Values
- `k = 0.01` - barely visible blend
- `k = 0.05` - subtle fillet
- `k = 0.1` - noticeable blend
- `k = 0.2+` - organic, blobby

### Cutting Holes
Make cutting shapes slightly larger than needed:
```rhai
let body = cylinder(0.5, 1.0);
let hole = cylinder(0.2, 1.1);  // slightly taller to ensure clean cut
body.subtract(hole)
```

### Centering
All primitives are centered at origin. Use translate to position:
```rhai
// Stack two cylinders
let bottom = cylinder(0.5, 0.5).translate_y(-0.25);
let top = cylinder(0.3, 0.5).translate_y(0.25);
bottom.union(top)
```

### Rhai Features
You have full Rhai scripting available:
```rhai
// Loops
let result = sphere(0.1);
for i in 0..5 {
    let angle = (i as f64) * TAU() / 5.0;
    let x = 0.5 * cos(angle);
    let z = 0.5 * sin(angle);
    result = result.union(sphere(0.1).translate(x, 0.0, z));
}
result

// Conditionals
let use_smooth = true;
if use_smooth {
    sphere(0.5).smooth_union(cube(0.6), 0.1)
} else {
    sphere(0.5).union(cube(0.6))
}
```

---

*Generated from soyuz-script API*
