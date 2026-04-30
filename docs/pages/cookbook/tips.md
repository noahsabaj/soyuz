# Tips & Tricks

Best practices for effective modeling.

## Transform Order Matters

Transforms apply left to right. `translate().rotate()` gives different results than `rotate().translate()`.

## Smooth Blend Values

- `k = 0.01`: barely visible.
- `k = 0.05`: subtle fillet.
- `k = 0.1`: noticeable blend.
- `k = 0.2+`: organic blend.

## Return An SDF

Your script must return an SDF. Do not end the final expression with a semicolon.

```rhai
let shape = sphere(0.5);
shape
```
