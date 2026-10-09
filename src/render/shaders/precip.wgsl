// Rain and snow particles in a box around the camera (see render/precip.rs).

struct Frame {
    view_proj: mat4x4<f32>,
    cam_pos: vec4<f32>,
    sun_dir: vec4<f32>,
    sun_color: vec4<f32>,
    ambient: vec4<f32>,
    fog_near_color: vec4<f32>,
    fog_far_color: vec4<f32>,
    fog: vec4<f32>,
    misc: vec4<f32>,
};

struct Precip {
    shape: vec4<f32>,    // size x, size y, box size, snow
    motion: vec4<f32>,   // fall so far, turn so far (degrees), start rotation range, gravity velocity
    offsets: vec4<f32>,  // centre offset min, max, subtextures x, y
    color: vec4<f32>,
    wind: vec4<f32>,     // drift so far (wrapped to the box) x, y; wind velocity x, y
};

@group(0) @binding(0) var<uniform> frame: Frame;
@group(1) @binding(0) var<uniform> p: Precip;
@group(1) @binding(1) var t_particle: texture_2d<f32>;
@group(1) @binding(2) var s: sampler;

struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) alpha: f32,
};

fn hash(x: u32) -> u32 {
    var v = x * 747796405u + 2891336453u;
    v = ((v >> ((v >> 28u) + 4u)) ^ v) * 277803737u;
    return (v >> 22u) ^ v;
}

fn rand(x: u32) -> f32 {
    return f32(hash(x) & 0xffffffu) / 16777216.0;
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0),
    );
    let corner = corners[vi];
    let seed = ii * 8u;
    let b = p.shape.z;
    let cam = frame.cam_pos.xyz;
    // A fixed place in the world falling at the gravity velocity, wrapped
    // into the box centred on the camera.
    var pos = vec3<f32>(rand(seed), rand(seed + 1u), rand(seed + 2u)) * b;
    pos.z -= p.motion.x;
    pos.x += p.wind.x;
    pos.y += p.wind.y;
    let rel = pos - cam;
    let w = rel - b * floor(rel / b) - 0.5 * b;
    var c = cam + w;
    let snow = p.shape.w > 0.5;
    if (snow) {
        // Turning about the falling centre.
        let r = mix(p.offsets.x, p.offsets.y, rand(seed + 3u));
        let a = radians(rand(seed + 4u) * p.motion.z + p.motion.y);
        c += vec3<f32>(cos(a), sin(a), 0.0) * r;
    }
    let to_cam = cam - c;
    let d = length(to_cam);
    let view = to_cam / max(d, 0.001);
    // Rain streaks along its fall, slanted by the wind; snow faces the camera.
    var up = vec3<f32>(0.0, 0.0, 1.0);
    if (!snow) {
        up = normalize(vec3<f32>(-p.wind.z, -p.wind.w, max(p.motion.w, 1.0)));
    }
    var right = cross(up, view);
    if (length(right) < 0.001) {
        right = vec3<f32>(1.0, 0.0, 0.0);
    }
    right = normalize(right);
    if (snow) {
        up = cross(view, right);
    }
    let world = c + right * corner.x * p.shape.x * 0.5 + up * corner.y * p.shape.y * 0.5;

    let nx = max(u32(p.offsets.z), 1u);
    let ny = max(u32(p.offsets.w), 1u);
    let sub = hash(seed + 5u) % (nx * ny);
    let cell = vec2<f32>(f32(sub % nx), f32(sub / nx));
    let uv = (cell + vec2<f32>(corner.x * 0.5 + 0.5, 0.5 - corner.y * 0.5)) / vec2<f32>(f32(nx), f32(ny));

    // Faded out towards the box's edges (where they wrap) and right at the eye.
    let edge = 1.0 - smoothstep(0.6, 1.0, length(w) / (0.5 * b));
    let near = smoothstep(8.0, 48.0, d);

    var o: VOut;
    o.clip = frame.view_proj * vec4<f32>(world, 1.0);
    o.world_pos = world;
    o.uv = uv;
    o.alpha = edge * near;
    return o;
}

fn apply_fog(color: vec3<f32>, world_pos: vec3<f32>) -> vec3<f32> {
    let dist = distance(world_pos, frame.cam_pos.xyz);
    let range = max(frame.fog.y - frame.fog.x, 1.0);
    var f = clamp((dist - frame.fog.x) / range, 0.0, 1.0);
    f = pow(f, max(frame.fog.z, 0.0001)) * frame.fog.w;
    let fog_color = mix(frame.fog_near_color.rgb, frame.fog_far_color.rgb, f);
    return mix(color, fog_color, f);
}

@fragment
fn fs_main(in: VOut) -> @location(0) vec4<f32> {
    // Rain's streaks are a texel or two wide: mipmaps would average them away.
    // Its texture is dark and faint (alpha under 0.4); drawn as it is the rain
    // can't be seen, so it takes the light's colour with its alpha doubled
    // (made up: the game's particle shader isn't public).
    var t: vec4<f32>;
    if (p.shape.w > 0.5) {
        t = textureSample(t_particle, s, in.uv);
    } else {
        t = textureSampleLevel(t_particle, s, in.uv, 0.0);
        t = vec4<f32>(vec3<f32>(1.0), min(t.a * 2.0, 1.0));
    }
    let a = t.a * in.alpha * p.color.a;
    if (a < 0.004) {
        discard;
    }
    return vec4<f32>(apply_fog(t.rgb * p.color.rgb, in.world_pos), a);
}
