# Recipes Overview

Practical modeling recipes and example families.

## Hollow Parts

Start with a solid primitive and apply a shell.

```rhai
sphere(0.5).shell(0.05)
```

## Smooth Assemblies

Blend separate primitives when you want a molded or organic transition.

```rhai
let body = cylinder(0.5, 1.0);
let band = torus(0.5, 0.08).translate_y(0.4);
body.smooth_union(band, 0.05)
```

## Radial Repetition

Build one spoke or tooth, move it away from the center, then repeat around the Y axis.

```rhai
let spoke = box3(0.1, 0.5, 0.05).translate_x(0.5);
spoke.repeat_polar(8)
```
