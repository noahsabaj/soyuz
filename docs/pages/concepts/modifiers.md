# Modifiers

Transform the surface of shapes.

## Surface Modifiers

```rhai
sphere(0.5).shell(0.05)
```

```rhai
cube(0.5).round(0.05)
```

```rhai
sphere(0.5).onion(0.05)
```

## Deformations

```rhai
box3(0.5, 1.0, 0.1).twist(2.0)
```

```rhai
box3(1.0, 0.2, 0.2).bend(0.5)
```

## Repetition

```rhai
sphere(0.2).repeat_limited(0.5, 0.5, 0.5, 3.0, 3.0, 3.0)
```

```rhai
sphere(0.2).translate_x(0.5).repeat_polar(6)
```
