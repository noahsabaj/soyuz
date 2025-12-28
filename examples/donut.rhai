// Donut - A simple torus with frosting
//
// A basic example showing smooth unions for organic shapes.
// Run with: soyuz watch examples/donut.rhai

// Donut body
let donut = torus(0.5, 0.2);

// Frosting - a slightly larger torus, cut in half
let frosting_shape = torus(0.5, 0.22);

// Cut the bottom half off the frosting
let cutter = box3(2.0, 0.5, 2.0).translate_y(-0.25);
let frosting = frosting_shape.subtract(cutter);

// Smooth blend the frosting onto the donut
donut.smooth_union(frosting, 0.05)
