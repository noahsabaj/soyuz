# Soyuz

Soyuz is a procedural 3D asset generation framework. You write scripts that describe shapes mathematically, and Soyuz renders them in real-time and exports them as 3D meshes.

```rhai
// This is a complete Soyuz script - it creates a donut
torus(0.5, 0.15)
```

---

## The Core Concept: Signed Distance Fields

Soyuz uses **Signed Distance Fields (SDFs)** to represent 3D shapes. Understanding this mental model is the key to using Soyuz effectively.

An SDF is a function that takes any point in 3D space and returns the **distance to the nearest surface**:
- Positive values = outside the shape
- Negative values = inside the shape
- Zero = exactly on the surface

You never work with vertices or polygons directly. Instead, you:

1. **Create primitives** - Basic shapes like spheres, cubes, cylinders
2. **Combine them** - Union (add), subtract (cut), intersect (overlap)
3. **Transform them** - Translate, rotate, scale, mirror
4. **Modify them** - Round edges, make hollow, twist, bend

The SDF approach means:
- Smooth blending between shapes is trivial
- Boolean operations (cutting holes, combining parts) just work
- Complex organic shapes emerge from simple operations
- Real-time preview via GPU raymarching

---

## Installation

### Prerequisites

- Rust toolchain (1.75+)
- Linux with X11 or Wayland (primary target)

### Build

```bash
git clone https://github.com/noahsabaj/soyuz
cd soyuz

# Build all binaries
cargo build --release

# The binaries are:
# ./target/release/soyuz-studio  (desktop IDE)
# ./target/release/soyuz         (CLI tool)
```

---

## Quick Start

### Option 1: Soyuz Studio (Desktop IDE)

```bash
cargo run --release -p app
```

This opens the full IDE with:
- Code editor with syntax highlighting
- Real-time 3D preview
- File browser
- Export controls

**Your first shape:**
1. The editor starts with an empty script
2. Type: `sphere(0.5)`
3. Press `Ctrl+Enter` to preview
4. A window opens showing your sphere rendered in real-time

### Option 2: Command Line

```bash
# Preview a script file
cargo run --release -p soyuz-cli -- preview --script examples/gear.rhai

# Export to mesh
cargo run --release -p soyuz-cli -- generate --script examples/gear.rhai --output gear.glb

# Interactive REPL
cargo run --release -p soyuz-cli -- repl
```

---

## Writing Scripts

Scripts are written in **Rhai**, a JavaScript-like language. Here's the essential pattern:

```rhai
// Your script must RETURN an SDF (no semicolon on the last line)
sphere(0.5)
```

### Primitives

All primitives are centered at the origin. Dimensions are in world units.

```rhai
sphere(radius)                              // Ball
cube(size)                                  // Box with equal sides
box3(width, height, depth)                  // Rectangular box
cylinder(radius, height)                    // Cylinder along Y axis
capsule(radius, height)                     // Pill shape
torus(major_radius, minor_radius)           // Donut
cone(radius, height)                        // Cone pointing up
```

### Combining Shapes

Use method chaining to combine shapes:

```rhai
// Add shapes together
sphere(0.5).union(cube(0.8))

// Cut one shape from another
cube(1.0).subtract(sphere(0.7))    // Cube with spherical hole

// Keep only the overlap
sphere(0.6).intersect(cube(0.8))   // Rounded cube
```

**Smooth versions** blend shapes organically:

```rhai
// k controls the blend radius (0.05-0.2 are common values)
sphere(0.4).smooth_union(sphere(0.4).translate_x(0.6), 0.15)
```

### Transforms

```rhai
shape.translate(x, y, z)           // Move
shape.translate_x(x)               // Move along one axis
shape.rotate_x(angle)              // Rotate (radians!)
shape.rotate_y(deg(45.0))          // Use deg() for degrees
shape.scale(factor)                // Uniform scale
shape.mirror_x()                   // Mirror across YZ plane
shape.symmetry_x()                 // Fold space (instant symmetry)
```

### Modifiers

```rhai
shape.shell(thickness)             // Make hollow
shape.round(radius)                // Round all edges
shape.twist(amount)                // Twist around Y axis
shape.bend(amount)                 // Bend around Y axis
```

### Repetition

```rhai
shape.repeat_polar(count)          // Repeat in a circle around Y
shape.repeat_limited(              // Finite 3D grid
    sx, sy, sz,                    // Spacing
    cx, cy, cz                     // Count per axis
)
```

---

## Complete Example

Here's a gear - it demonstrates the typical workflow:

```rhai
let teeth_count = 12;
let outer_radius = 1.0;
let inner_radius = 0.3;
let thickness = 0.2;
let tooth_size = 0.15;

// Main body - a cylinder
let body = cylinder(outer_radius - tooth_size, thickness);

// Center hole - subtract this later
let hole = cylinder(inner_radius, thickness + 0.1);

// Single tooth, positioned at the edge
let tooth = box3(tooth_size * 2.0, thickness, tooth_size * 1.5)
    .translate(outer_radius - tooth_size * 0.3, 0.0, 0.0);

// Repeat the tooth around the gear
let teeth = tooth.repeat_polar(teeth_count);

// Spoke holes for visual interest
let spoke = cylinder(0.08, thickness + 0.1)
    .translate((outer_radius + inner_radius) / 2.0, 0.0, 0.0);
let spokes = spoke.repeat_polar(6);

// Final assembly: body + teeth - hole - spokes
body.union(teeth).subtract(hole).subtract(spokes)
```

---

### Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Ctrl+Enter` | Run preview |
| `Ctrl+N` | New tab |
| `Ctrl+W` | Close tab |
| `Ctrl+S` | Save file |
| `Ctrl+O` | Open file |
| `Ctrl+Z` | Undo |
| `Ctrl+Shift+Z` | Redo |
| `Ctrl+\` | Split pane vertically |
| `Ctrl+Shift+\` | Split pane horizontally |

### Export Settings

- **Format**: GLB (binary, recommended), GLTF (JSON + binary), OBJ (legacy)
- **Resolution**: Controls mesh density (32=fast preview, 128=high quality)
- **Optimize**: Reduces vertex count while preserving shape

---

## CLI Reference

```bash
soyuz <COMMAND>

Commands:
  preview    Open a live preview window for a script
  generate   Export a script to a mesh file
  render     Render a script to an image (headless)
  repl       Interactive scripting shell
  watch      Watch a file and auto-refresh preview on changes
  demo       Generate demo assets
```

### Examples

```bash
# Live preview with hot-reload
soyuz watch --script mymodel.rhai

# Export to GLB at high resolution
soyuz generate --script mymodel.rhai --output mymodel.glb --resolution 128

# Render to PNG
soyuz render --script mymodel.rhai --output render.png --width 1920 --height 1080
```

---

## Environment and Lighting

Configure the rendering environment in your script:

```rhai
// Use a preset
env_sunset();       // Warm lighting
env_studio();       // Neutral (default)
env_clay();         // Soft AO look

// Or customize
set_sun_direction(1.0, 1.0, 0.5);
set_sun_color(1.0, 0.95, 0.9);
set_material_color_hex("#ff5500");
set_ao_enabled(true);

// Then define your shape
sphere(0.5)
```

---

## Project Structure

```
soyuz/
  app/                    # Desktop IDE (Dioxus)
  crates/
    soyuz-math/           # Mathematical formulas (generates Rust + WGSL)
    soyuz-core/           # SDF engine, mesh generation, export
    soyuz-render/         # GPU raymarching renderer
    soyuz-script/         # Rhai scripting integration
    soyuz-cli/            # Command-line interface
  examples/               # Sample scripts
  SOYUZ_COOKBOOK.md       # Complete scripting reference
```

---

## Common Patterns

### Making Things Hollow

```rhai
// shell() creates a hollow version with wall thickness
sphere(0.5).shell(0.05)
```

### Cutting Clean Holes

Make the cutting shape slightly larger to ensure clean geometry:

```rhai
let body = cylinder(0.5, 1.0);
let hole = cylinder(0.2, 1.1);  // Taller than body
body.subtract(hole)
```

### Stacking Parts

Primitives are centered at origin. Translate to position:

```rhai
let base = cylinder(0.5, 0.2).translate_y(-0.1);
let top = sphere(0.4).translate_y(0.3);
base.union(top)
```

### Radial Patterns

Position a shape away from center, then repeat around Y:

```rhai
let spoke = box3(0.1, 0.5, 0.05).translate_x(0.5);
spoke.repeat_polar(8)  // 8 spokes in a circle
```

### Smooth Organic Blends

Higher k = more blending:

```rhai
// k=0.05: subtle fillet
// k=0.15: noticeable blend
// k=0.3+: blobby, organic
sphere(0.3).smooth_union(sphere(0.3).translate_x(0.4), 0.15)
```

### Symmetry Shortcut

Instead of building both sides, use symmetry:

```rhai
// Build one side, mirror automatically
let half = sphere(0.3).translate_x(0.5);
half.symmetry_x()  // Creates both sides
```

---

## Troubleshooting

**Preview window is black**
- Check the status bar for script errors
- Ensure your script returns an SDF (no trailing semicolon)

**Mesh has holes or artifacts**
- Increase export resolution
- Check for self-intersecting geometry
- Ensure cutting shapes fully penetrate

**Script doesn't update**
- Press `Ctrl+Enter` to refresh preview
- Check for syntax errors in status bar

**Performance is slow**
- Reduce preview resolution
- Avoid infinite repetition (`repeat()`) - use `repeat_limited()` instead
- Simplify complex smooth blending chains

---

## Further Reading

- **[SOYUZ_COOKBOOK.md](SOYUZ_COOKBOOK.md)** - Complete API reference with all primitives, operations, and recipes
- **[examples/](examples/)** - Working script examples

---

## License

MIT OR Apache-2.0
