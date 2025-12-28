// ============================================
// SEMI-AUTOMATIC PISTOL
// A detailed handgun model
// ============================================

// === PARAMETERS ===
let grip_width = 0.12;
let grip_depth = 0.25;
let grip_height = 0.45;
let grip_angle = deg(15.0);  // Ergonomic angle

let frame_length = 0.55;
let frame_height = 0.18;
let frame_depth = 0.22;

let slide_length = 0.65;
let slide_height = 0.15;
let slide_depth = 0.20;

let barrel_radius = 0.04;
let barrel_length = 0.12;

// === GRIP ===
// Main grip body with slight taper
let grip_base = rounded_box(grip_width, grip_height, grip_depth, 0.02)
    .translate_y(-grip_height / 2.0)
    .rotate_z(-grip_angle);

// Grip texturing - vertical lines
let grip_groove = box3(0.005, grip_height * 0.6, 0.005)
    .translate(grip_width / 2.0 + 0.001, -grip_height * 0.4, 0.0)
    .rotate_z(-grip_angle);
let grip_grooves = grip_groove.repeat_limited(0.0, 0.0, 0.02, 1.0, 1.0, 8.0);

let grip = grip_base.subtract(grip_grooves).subtract(grip_grooves.mirror_x());

// Grip back curve (palm swell)
let palm_swell = ellipsoid(0.08, grip_height * 0.4, 0.08)
    .translate(-grip_width / 2.0 - 0.02, -grip_height * 0.4, 0.0)
    .rotate_z(-grip_angle);

let grip_complete = grip.smooth_union(palm_swell, 0.03);

// === FRAME ===
// Main frame body
let frame_body = rounded_box(frame_length, frame_height, frame_depth, 0.015)
    .translate(frame_length / 2.0 - grip_width / 2.0, frame_height / 2.0, 0.0);

// Trigger guard
let guard_outer = torus(0.06, 0.015)
    .rotate_x(deg(90.0))
    .translate(0.08, -0.02, 0.0);
let guard_inner = torus(0.045, 0.02)
    .rotate_x(deg(90.0))
    .translate(0.08, -0.02, 0.0);
let guard_cutter = box3(0.15, 0.08, 0.1)
    .translate(0.08, 0.02, 0.0);
let trigger_guard = guard_outer
    .subtract(guard_inner)
    .subtract(guard_cutter);

// Trigger
let trigger = box3(0.015, 0.04, 0.025)
    .round(0.005)
    .translate(0.08, -0.01, 0.0);

// Beaver tail (grip safety area)
let beaver_tail = ellipsoid(0.04, 0.03, 0.08)
    .translate(-0.05, frame_height * 0.8, 0.0);

// Dust cover / rail section
let rail = box3(0.15, 0.025, frame_depth + 0.01)
    .translate(0.18, frame_height - 0.01, 0.0);
let rail_groove = box3(0.14, 0.008, 0.005)
    .translate(0.18, frame_height + 0.005, frame_depth / 2.0 - 0.02);
let rail_section = rail.subtract(rail_groove).subtract(rail_groove.mirror_z());

let frame = frame_body
    .union(trigger_guard)
    .union(trigger)
    .smooth_union(beaver_tail, 0.02)
    .union(rail_section);

// === SLIDE ===
// Main slide body
let slide_body = rounded_box(slide_length, slide_height, slide_depth, 0.01)
    .translate(slide_length / 2.0 - grip_width / 2.0 + 0.02,
               frame_height + slide_height / 2.0 + 0.005, 0.0);

// Slide serrations (rear)
let serration = box3(0.004, slide_height * 0.6, slide_depth + 0.01)
    .translate(-grip_width / 2.0 + 0.04, frame_height + slide_height / 2.0, 0.0);
let rear_serrations = serration.repeat_limited(0.012, 0.0, 0.0, 8.0, 1.0, 1.0);

// Slide serrations (front)
let front_serration = box3(0.004, slide_height * 0.5, slide_depth + 0.01)
    .translate(slide_length - grip_width / 2.0 - 0.05,
               frame_height + slide_height / 2.0 + 0.02, 0.0);
let front_serrations = front_serration.repeat_limited(0.012, 0.0, 0.0, 5.0, 1.0, 1.0);

// Ejection port
let ejection_port = box3(0.08, slide_height * 0.4, 0.06)
    .translate(0.15, frame_height + slide_height * 0.8, slide_depth / 2.0 - 0.01);

// Front sight
let front_sight = box3(0.008, 0.025, 0.015)
    .translate(slide_length - grip_width / 2.0 - 0.02,
               frame_height + slide_height + 0.01, 0.0);

// Rear sight
let rear_sight_body = box3(0.025, 0.02, slide_depth * 0.6)
    .translate(0.0, frame_height + slide_height + 0.008, 0.0);
let rear_sight_notch = box3(0.015, 0.015, 0.008)
    .translate(0.0, frame_height + slide_height + 0.02, 0.0);
let rear_sight = rear_sight_body.subtract(rear_sight_notch);

let slide = slide_body
    .subtract(rear_serrations)
    .subtract(front_serrations)
    .subtract(ejection_port)
    .union(front_sight)
    .union(rear_sight);

// === BARREL ===
let barrel_outer = cylinder(barrel_radius, barrel_length)
    .rotate_z(deg(90.0))
    .translate(slide_length - grip_width / 2.0 + barrel_length / 2.0,
               frame_height + slide_height / 2.0 + 0.02, 0.0);

let barrel_inner = cylinder(barrel_radius * 0.6, barrel_length + 0.02)
    .rotate_z(deg(90.0))
    .translate(slide_length - grip_width / 2.0 + barrel_length / 2.0,
               frame_height + slide_height / 2.0 + 0.02, 0.0);

let barrel = barrel_outer.subtract(barrel_inner);

// === MAGAZINE WELL ===
let mag_well = box3(grip_width * 0.7, grip_height * 0.8, grip_depth * 0.7)
    .translate_y(-grip_height * 0.5)
    .rotate_z(-grip_angle);

// Magazine baseplate (visible at bottom)
let mag_baseplate = rounded_box(grip_width * 0.75, 0.02, grip_depth * 0.75, 0.005)
    .translate_y(-grip_height - 0.01)
    .rotate_z(-grip_angle);

// === HAMMER ===
let hammer = box3(0.015, 0.04, 0.03)
    .round(0.005)
    .rotate_z(deg(-20.0))
    .translate(-grip_width / 2.0 - 0.01, frame_height + 0.02, 0.0);

// === ASSEMBLY ===
// Environment setup
env_studio();
set_material_color(0.15, 0.15, 0.15);  // Dark gunmetal
set_material_shininess(80.0);
set_specular_intensity(0.4);

// Combine all parts
grip_complete
    .union(frame)
    .union(slide)
    .union(barrel)
    .subtract(mag_well)
    .union(mag_baseplate)
    .union(hammer)
    .translate_y(0.25)  // Center vertically
