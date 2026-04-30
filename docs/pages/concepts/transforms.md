# Transforms

Move, rotate, scale, and mirror shapes.

## Position

```rhai
let shape = sphere(0.5)
    .translate(1.0, 0.0, 0.0)
    .translate_y(0.25);

shape
```

## Rotation

Angles are in radians. Use `deg()` when you want to work in degrees.

```rhai
cube(0.5).rotate_y(deg(45.0))
```

## Scale And Mirror

```rhai
sphere(0.5).scale(2.0)
```

```rhai
sphere(0.5).translate_x(0.5).symmetry_x()
```
