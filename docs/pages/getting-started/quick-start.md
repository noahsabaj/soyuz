# Quick Start

Create your first 3D model in a few minutes.

## The Simplest Script

A Soyuz script is a Rhai file that returns an SDF as its final expression.

```rhai
sphere(0.5)
```

This creates a sphere with radius `0.5`, centered at the origin.

## Combining Shapes

Use method chaining to compose primitives.

```rhai
// Union: combine two shapes
sphere(0.5).union(cube(0.7))
```

```rhai
// Subtract: cut one shape from another
cube(1.0).subtract(sphere(0.8))
```

```rhai
// Intersect: keep only the overlap
sphere(0.6).intersect(cube(0.8))
```

## Transforms

Move, rotate, and scale shapes before combining them.

```rhai
let part = sphere(0.3)
    .translate(1.0, 0.0, 0.0)
    .rotate_y(deg(45.0))
    .scale(1.5);

part
```

## Variables And Logic

Rhai supports variables and normal script structure.

```rhai
let radius = 0.4;
let height = 1.0;

let body = cylinder(radius, height);
let hollow = cylinder(radius - 0.05, height - 0.1)
    .translate_y(0.05);
let handle = torus(0.15, 0.04)
    .rotate_x(deg(90.0))
    .translate(radius + 0.1, 0.0, 0.0);

body.subtract(hollow).union(handle)
```

## Preview And Export

In Soyuz Studio, open or refresh Preview to render the active script. Open Export to choose the mesh format and resolution.
