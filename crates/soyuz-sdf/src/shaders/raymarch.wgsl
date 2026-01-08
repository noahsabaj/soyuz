// Soyuz Raymarching Shader
// Renders SDFs in real-time using sphere tracing

// ============================================================================
// Uniforms
// ============================================================================

struct Uniforms {
    view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    time: f32,
    resolution: vec2<f32>,
    near: f32,
    far: f32,
    // Camera basis vectors for ray generation
    camera_right: vec3<f32>,
    _pad1: f32,
    camera_up: vec3<f32>,
    _pad2: f32,
    camera_forward: vec3<f32>,
    fov_tan: f32,
}

// Environment settings for lighting, material, and background
struct EnvironmentUniforms {
    // Lighting
    sun_direction: vec3<f32>,
    sun_intensity: f32,
    sun_color: vec3<f32>,
    ambient_intensity: f32,
    ambient_color: vec3<f32>,
    material_shininess: f32,

    // Material
    material_color: vec3<f32>,
    specular_intensity: f32,

    // Background
    sky_horizon: vec3<f32>,
    fog_density: f32,
    sky_zenith: vec3<f32>,
    ao_intensity: f32,
    fog_color: vec3<f32>,
    shadow_softness: f32,

    // Flags
    ao_enabled: f32,
    shadows_enabled: f32,
    _pad1: f32,
    _pad2: f32,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(0) @binding(1)
var<uniform> env: EnvironmentUniforms;

// ============================================================================
// Vertex Shader - Full screen quad
// ============================================================================

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    // Full screen triangle (more efficient than quad)
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0)
    );

    var out: VertexOutput;
    out.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    out.uv = positions[vertex_index] * 0.5 + 0.5;
    return out;
}

// ============================================================================
// SDF Primitives
// ============================================================================

fn sd_sphere(p: vec3<f32>, r: f32) -> f32 {
    return length(p) - r;
}

fn sd_box(p: vec3<f32>, b: vec3<f32>) -> f32 {
    let q = abs(p) - b;
    return length(max(q, vec3<f32>(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0);
}

fn sd_rounded_box(p: vec3<f32>, b: vec3<f32>, r: f32) -> f32 {
    let q = abs(p) - b + vec3<f32>(r);
    return length(max(q, vec3<f32>(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0) - r;
}

fn sd_cylinder(p: vec3<f32>, r: f32, h: f32) -> f32 {
    let d = vec2<f32>(length(p.xz) - r, abs(p.y) - h);
    return min(max(d.x, d.y), 0.0) + length(max(d, vec2<f32>(0.0)));
}

fn sd_capsule(p: vec3<f32>, r: f32, h: f32) -> f32 {
    let py = clamp(p.y, -h, h);
    return length(p - vec3<f32>(0.0, py, 0.0)) - r;
}

fn sd_torus(p: vec3<f32>, t: vec2<f32>) -> f32 {
    let q = vec2<f32>(length(p.xz) - t.x, p.y);
    return length(q) - t.y;
}

fn sd_cone(p: vec3<f32>, r: f32, h: f32) -> f32 {
    let q = vec2<f32>(length(p.xz), p.y);
    let k1 = vec2<f32>(h, r);
    let k2 = vec2<f32>(h, -r);
    let ca = vec2<f32>(q.x - min(q.x, select(r, 0.0, q.y < 0.0)), abs(q.y) - h);
    let cb = q - k1 + k2 * clamp(dot(k1 - q, k2) / dot(k2, k2), 0.0, 1.0);
    let s = select(1.0, -1.0, cb.x < 0.0 && ca.y < 0.0);
    return s * sqrt(min(dot(ca, ca), dot(cb, cb)));
}

fn sd_plane(p: vec3<f32>, n: vec3<f32>, d: f32) -> f32 {
    return dot(p, n) + d;
}

fn sd_ellipsoid(p: vec3<f32>, r: vec3<f32>) -> f32 {
    let k0 = length(p / r);
    let k1 = length(p / (r * r));
    return k0 * (k0 - 1.0) / k1;
}

fn sd_octahedron(p: vec3<f32>, s: f32) -> f32 {
    let p_abs = abs(p);
    let m = p_abs.x + p_abs.y + p_abs.z - s;

    var q: vec3<f32>;
    if (3.0 * p_abs.x < m) {
        q = p_abs;
    } else if (3.0 * p_abs.y < m) {
        q = vec3<f32>(p_abs.y, p_abs.z, p_abs.x);
    } else if (3.0 * p_abs.z < m) {
        q = vec3<f32>(p_abs.z, p_abs.x, p_abs.y);
    } else {
        return m * 0.57735027;
    }

    let k = clamp(0.5 * (q.z - q.y + s), 0.0, s);
    return length(vec3<f32>(q.x, q.y - s + k, q.z - k));
}

fn sd_hex_prism(p: vec3<f32>, h: vec2<f32>) -> f32 {
    let k = vec3<f32>(-0.8660254, 0.5, 0.57735);
    let p_abs = abs(p);
    var p2 = p_abs.xz - 2.0 * min(dot(vec2<f32>(k.x, k.y), p_abs.xz), 0.0) * vec2<f32>(k.x, k.y);
    let d = vec2<f32>(
        length(p2 - vec2<f32>(clamp(p2.x, -k.z * h.x, k.z * h.x), h.x)) * sign(p2.y - h.x),
        p_abs.y - h.y
    );
    return min(max(d.x, d.y), 0.0) + length(max(d, vec2<f32>(0.0)));
}

fn sd_tri_prism(p: vec3<f32>, h: vec2<f32>) -> f32 {
    let q = abs(p);
    return max(q.z - h.y, max(q.x * 0.866025 + p.y * 0.5, -p.y) - h.x * 0.5);
}

fn sd_pyramid(p: vec3<f32>, h: f32) -> f32 {
    let m2 = h * h + 0.25;
    var p_xz = abs(p.xz);
    if (p_xz.y > p_xz.x) {
        p_xz = p_xz.yx;
    }
    p_xz = p_xz - vec2<f32>(0.5);

    let q = vec3<f32>(p_xz.y, h * p.y - 0.5 * p_xz.x, h * p_xz.x + 0.5 * p.y);
    let s = max(-q.x, 0.0);
    let t = clamp((q.y - 0.5 * p_xz.y) / (m2 + 0.25), 0.0, 1.0);

    let a = m2 * (q.x + s) * (q.x + s) + q.y * q.y;
    let b = m2 * (q.x + 0.5 * t) * (q.x + 0.5 * t) + (q.y - m2 * t) * (q.y - m2 * t);

    let d2 = select(min(a, b), 0.0, min(q.y, -q.x * m2 - q.y * 0.5) > 0.0);
    return sqrt((d2 + q.z * q.z) / m2) * sign(max(q.z, -p.y));
}

fn sd_link(p: vec3<f32>, le: f32, r1: f32, r2: f32) -> f32 {
    let q = vec3<f32>(p.x, max(abs(p.y) - le, 0.0), p.z);
    return length(vec2<f32>(length(q.xy) - r1, q.z)) - r2;
}

// 2D SDF primitives for extrusion and revolution
fn sd_circle_2d(p: vec2<f32>, r: f32) -> f32 {
    return length(p) - r;
}

fn sd_box_2d(p: vec2<f32>, b: vec2<f32>) -> f32 {
    let d = abs(p) - b;
    return length(max(d, vec2<f32>(0.0))) + min(max(d.x, d.y), 0.0);
}

fn sd_rounded_box_2d(p: vec2<f32>, b: vec2<f32>, r: f32) -> f32 {
    let q = abs(p) - b + vec2<f32>(r);
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - r;
}

// Noise function for displacement
fn noise3d(p: vec3<f32>) -> f32 {
    return sin(p.x * 1.0) * sin(p.y * 1.1) * sin(p.z * 0.9) +
           sin(p.x * 2.3) * sin(p.y * 2.1) * sin(p.z * 2.5) * 0.5;
}

// ============================================================================
// SDF Operations
// ============================================================================

fn op_union(d1: f32, d2: f32) -> f32 {
    return min(d1, d2);
}

fn op_subtract(d1: f32, d2: f32) -> f32 {
    return max(d1, -d2);
}

fn op_intersect(d1: f32, d2: f32) -> f32 {
    return max(d1, d2);
}

fn op_smooth_union(d1: f32, d2: f32, k: f32) -> f32 {
    let h = clamp(0.5 + 0.5 * (d2 - d1) / k, 0.0, 1.0);
    return mix(d2, d1, h) - k * h * (1.0 - h);
}

fn op_smooth_subtract(d1: f32, d2: f32, k: f32) -> f32 {
    let h = clamp(0.5 - 0.5 * (d2 + d1) / k, 0.0, 1.0);
    return mix(d1, -d2, h) + k * h * (1.0 - h);
}

fn op_smooth_intersect(d1: f32, d2: f32, k: f32) -> f32 {
    let h = clamp(0.5 - 0.5 * (d2 - d1) / k, 0.0, 1.0);
    return mix(d2, d1, h) + k * h * (1.0 - h);
}

fn op_round(d: f32, r: f32) -> f32 {
    return d - r;
}

fn op_shell(d: f32, thickness: f32) -> f32 {
    return abs(d) - thickness;
}

fn op_onion(d: f32, thickness: f32) -> f32 {
    return abs(d % (thickness * 2.0)) - thickness;
}

// ============================================================================
// Transform Operations
// ============================================================================

fn op_translate(p: vec3<f32>, offset: vec3<f32>) -> vec3<f32> {
    return p - offset;
}

fn op_rotate_x(p: vec3<f32>, angle: f32) -> vec3<f32> {
    let c = cos(angle);
    let s = sin(angle);
    return vec3<f32>(p.x, c * p.y - s * p.z, s * p.y + c * p.z);
}

fn op_rotate_y(p: vec3<f32>, angle: f32) -> vec3<f32> {
    let c = cos(angle);
    let s = sin(angle);
    return vec3<f32>(c * p.x + s * p.z, p.y, -s * p.x + c * p.z);
}

fn op_rotate_z(p: vec3<f32>, angle: f32) -> vec3<f32> {
    let c = cos(angle);
    let s = sin(angle);
    return vec3<f32>(c * p.x - s * p.y, s * p.x + c * p.y, p.z);
}

fn op_scale(p: vec3<f32>, s: f32) -> vec3<f32> {
    return p / s;
}

fn op_symmetry_x(p: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(abs(p.x), p.y, p.z);
}

fn op_symmetry_y(p: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(p.x, abs(p.y), p.z);
}

fn op_symmetry_z(p: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(p.x, p.y, abs(p.z));
}

fn op_symmetry_xyz(p: vec3<f32>) -> vec3<f32> {
    return abs(p);
}

// ============================================================================
// Deformation Operations
// ============================================================================

fn op_twist(p: vec3<f32>, k: f32) -> vec3<f32> {
    let c = cos(k * p.y);
    let s = sin(k * p.y);
    return vec3<f32>(c * p.x - s * p.z, p.y, s * p.x + c * p.z);
}

fn op_bend(p: vec3<f32>, k: f32) -> vec3<f32> {
    let c = cos(k * p.x);
    let s = sin(k * p.x);
    return vec3<f32>(c * p.x - s * p.y, s * p.x + c * p.y, p.z);
}

fn op_elongate(p: vec3<f32>, h: vec3<f32>) -> vec3<f32> {
    let q = abs(p) - h;
    return max(q, vec3<f32>(0.0)) + min(max(q.x, max(q.y, q.z)), 0.0);
}

// ============================================================================
// Repetition Operations
// ============================================================================

fn op_repeat(p: vec3<f32>, c: vec3<f32>) -> vec3<f32> {
    return (p + 0.5 * c) % c - 0.5 * c;
}

fn op_repeat_limited(p: vec3<f32>, c: vec3<f32>, l: vec3<f32>) -> vec3<f32> {
    return p - c * clamp(round(p / c), -l, l);
}

// ============================================================================
// Additional Boolean Operations
// ============================================================================

fn op_xor(d1: f32, d2: f32) -> f32 {
    return max(min(d1, d2), -max(d1, d2));
}

// ============================================================================
// Displacement and Deformation
// ============================================================================

fn op_displacement(d: f32, p: vec3<f32>, amount: f32, freq: f32) -> f32 {
    return d + amount * noise3d(p * freq);
}

// ============================================================================
// 2D-to-3D Operations (Extrude and Revolve)
// ============================================================================

fn op_extrude(d2d: f32, pz: f32, h: f32) -> f32 {
    let w = vec2<f32>(d2d, abs(pz) - h);
    return min(max(w.x, w.y), 0.0) + length(max(w, vec2<f32>(0.0)));
}

fn op_revolve(p: vec3<f32>, offset: f32) -> vec2<f32> {
    return vec2<f32>(length(p.xz) - offset, p.y);
}

// SSOT_FORMULAS_PLACEHOLDER
// Generated formulas from soyuz-math will be injected here

// ============================================================================
// Scene SDF - This is where the user's SDF gets injected
// ============================================================================

// SCENE_SDF_PLACEHOLDER
// Default scene: empty (nothing to render)
fn scene_sdf(p: vec3<f32>) -> f32 {
    // Return large distance = empty scene, just shows background
    return 1000.0;
}

// ============================================================================
// Raymarching
// ============================================================================

const MAX_STEPS: i32 = 128;  // Reduced from 256 - adaptive precision compensates
const MAX_DIST: f32 = 100.0;
const MIN_SURF_DIST: f32 = 0.0001;  // Base precision for close objects
const DIST_SCALE: f32 = 0.0005;     // How much precision degrades with distance

struct RayResult {
    hit: bool,
    dist: f32,
    steps: i32,
    pos: vec3<f32>,
}

// Distance-adaptive surface threshold - far pixels need less precision
fn get_surf_dist(t: f32) -> f32 {
    return MIN_SURF_DIST + DIST_SCALE * t;
}

fn raymarch(ro: vec3<f32>, rd: vec3<f32>) -> RayResult {
    var result: RayResult;
    result.hit = false;
    result.dist = 0.0;
    result.steps = 0;
    result.pos = ro;

    for (var i = 0; i < MAX_STEPS; i++) {
        let p = ro + rd * result.dist;
        let d = scene_sdf(p);

        // Adaptive surface threshold based on distance
        let surf_dist = get_surf_dist(result.dist);
        if (d < surf_dist) {
            result.hit = true;
            result.pos = p;
            result.steps = i;
            return result;
        }

        if (result.dist > MAX_DIST) {
            result.steps = i;
            return result;
        }

        result.dist += d;
    }

    result.steps = MAX_STEPS;
    return result;
}

// ============================================================================
// Normal Calculation
// ============================================================================

fn calc_normal(p: vec3<f32>) -> vec3<f32> {
    // Distance-adaptive epsilon for normal calculation
    // Far surfaces don't need as precise normals
    let dist_from_cam = length(p - uniforms.camera_pos);
    let eps = 0.0001 + 0.0002 * dist_from_cam;
    let e = vec2<f32>(eps, 0.0);
    return normalize(vec3<f32>(
        scene_sdf(p + e.xyy) - scene_sdf(p - e.xyy),
        scene_sdf(p + e.yxy) - scene_sdf(p - e.yxy),
        scene_sdf(p + e.yyx) - scene_sdf(p - e.yyx)
    ));
}

// ============================================================================
// Ambient Occlusion (optimized: 3 samples, unrolled)
// ============================================================================

fn calc_ao(pos: vec3<f32>, nor: vec3<f32>) -> f32 {
    // Unrolled loop with 3 samples for better GPU performance
    let h1 = 0.02;
    let h2 = 0.06;
    let h3 = 0.10;

    let d1 = scene_sdf(pos + h1 * nor);
    let d2 = scene_sdf(pos + h2 * nor);
    let d3 = scene_sdf(pos + h3 * nor);

    var occ = (h1 - d1) * 1.0;
    occ += (h2 - d2) * 0.5;
    occ += (h3 - d3) * 0.25;

    return clamp(1.0 - 4.0 * occ, 0.0, 1.0);
}

// ============================================================================
// Soft Shadows (optimized: reduced steps, adaptive stepping)
// ============================================================================

fn calc_soft_shadow(ro: vec3<f32>, rd: vec3<f32>, mint: f32, maxt: f32, k: f32) -> f32 {
    var res = 1.0;
    var t = mint;
    var ph = 1e10;  // Previous h for improved penumbra

    for (var i = 0; i < 32; i++) {  // Reduced from 64 to 32
        let h = scene_sdf(ro + rd * t);
        if (h < 0.0005) {  // Slightly tighter threshold
            return 0.0;
        }
        // Improved soft shadow calculation
        let y = h * h / (2.0 * ph);
        let d = sqrt(h * h - y * y);
        res = min(res, k * d / max(0.0, t - y));
        ph = h;

        // Adaptive step: take larger steps when far from surfaces
        t += max(h, 0.02);
        if (t > maxt) {
            break;
        }
    }
    return res;
}

// ============================================================================
// Lighting
// ============================================================================

fn get_light(p: vec3<f32>, n: vec3<f32>, rd: vec3<f32>) -> vec3<f32> {
    // Use environment settings
    let sun_dir = normalize(env.sun_direction);
    let sun_color = env.sun_color * env.sun_intensity;
    let ambient = env.ambient_color * env.ambient_intensity;

    // Diffuse
    let diff = max(dot(n, sun_dir), 0.0);

    // Specular (Blinn-Phong)
    let half_vec = normalize(sun_dir - rd);
    let spec = pow(max(dot(n, half_vec), 0.0), env.material_shininess);

    // Shadow (configurable)
    var shadow = 1.0;
    if (env.shadows_enabled > 0.5) {
        shadow = calc_soft_shadow(p + n * 0.002, sun_dir, 0.02, 2.5, env.shadow_softness);
    }

    // Ambient occlusion (configurable)
    var ao = 1.0;
    if (env.ao_enabled > 0.5) {
        ao = calc_ao(p, n);
    }

    // Combine lighting
    var col = ambient * ao;
    col += sun_color * diff * shadow;
    col += sun_color * spec * shadow * env.specular_intensity;

    // Sky reflection (fake)
    let sky_diff = max(dot(n, vec3<f32>(0.0, 1.0, 0.0)), 0.0);
    let sky_color = mix(env.sky_horizon, env.sky_zenith, 0.5);
    col += sky_color * 0.15 * sky_diff * ao;

    return col;
}

// ============================================================================
// Background
// ============================================================================

fn get_background(rd: vec3<f32>) -> vec3<f32> {
    // Gradient sky using environment colors
    let t = 0.5 * (rd.y + 1.0);
    let sky = mix(env.sky_horizon, env.sky_zenith, t);

    // Ground fog using environment settings
    let fog = exp(-10.0 * max(rd.y, 0.0));
    return mix(sky, env.fog_color, fog * 0.3);
}

// ============================================================================
// Fragment Shader
// ============================================================================

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Convert UV to normalized device coordinates (-1 to 1)
    let ndc = in.uv * 2.0 - 1.0;
    let aspect = uniforms.resolution.x / uniforms.resolution.y;

    // Generate ray direction
    let rd = normalize(
        uniforms.camera_forward +
        ndc.x * uniforms.camera_right * uniforms.fov_tan * aspect +
        ndc.y * uniforms.camera_up * uniforms.fov_tan
    );

    let ro = uniforms.camera_pos;

    // Raymarch
    let result = raymarch(ro, rd);

    var col: vec3<f32>;

    if (result.hit) {
        // Calculate normal and lighting
        let n = calc_normal(result.pos);
        col = get_light(result.pos, n, rd);

        // Material color from environment
        col *= env.material_color;

        // Distance fog using environment density
        let fog = exp(-env.fog_density * result.dist * result.dist);
        col = mix(get_background(rd), col, fog);
    } else {
        col = get_background(rd);
    }

    // Gamma correction
    col = pow(col, vec3<f32>(1.0 / 2.2));

    // Vignette
    let vignette = 1.0 - 0.3 * length(ndc);
    col *= vignette;

    return vec4<f32>(col, 1.0);
}
