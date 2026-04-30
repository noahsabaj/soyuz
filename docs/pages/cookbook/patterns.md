# Common Patterns

Reusable techniques for everyday modeling.

## Making Things Hollow

```rhai
sphere(0.5).hollow(0.05)
```

## Cutting Clean Holes

Make the cutter slightly larger than the target so the exported mesh has clean openings.

```rhai
let body = cylinder(0.5, 1.0);
let hole = cylinder(0.2, 1.1);
body.subtract(hole)
```

## Radial Patterns

```rhai
let tooth = cube(0.2).translate_x(0.8);
tooth.repeat_polar(12)
```

## Symmetry Shortcut

```rhai
let half = sphere(0.3).translate_x(0.5);
half.symmetry_x()
```
