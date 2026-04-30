# Boolean Operations

Combine shapes with union, subtract, and intersect.

## Basic Operations

```rhai
// Union combines two shapes
sphere(0.5).union(cube(0.7))
```

```rhai
// Subtract cuts one shape from another
cube(1.0).subtract(sphere(0.8))
```

```rhai
// Intersect keeps only the overlap
sphere(0.6).intersect(cube(0.8))
```

## Smooth Operations

Smooth operations add a blend radius `k` for more organic transitions.

```rhai
let a = sphere(0.5);
let b = cube(0.4);
a.smooth_union(b, 0.1)
```
