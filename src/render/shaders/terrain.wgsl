// Landscape: up to 8 texture layers per quadrant blended by per-vertex opacity.

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

@group(0) @binding(0) var<uniform> frame: Frame;

@group(1) @binding(0) var d0: texture_2d<f32>;
@group(1) @binding(1) var d1: texture_2d<f32>;
@group(1) @binding(2) var d2: texture_2d<f32>;
@group(1) @binding(3) var d3: texture_2d<f32>;
@group(1) @binding(4) var d4: texture_2d<f32>;
@group(1) @binding(5) var d5: texture_2d<f32>;
@group(1) @binding(6) var d6: texture_2d<f32>;
@group(1) @binding(7) var d7: texture_2d<f32>;
@group(1) @binding(8) var n0: texture_2d<f32>;
@group(1) @binding(9) var n1: texture_2d<f32>;
@group(1) @binding(10) var n2: texture_2d<f32>;
@group(1) @binding(11) var n3: texture_2d<f32>;
@group(1) @binding(12) var n4: texture_2d<f32>;
@group(1) @binding(13) var n5: texture_2d<f32>;
@group(1) @binding(14) var n6: texture_2d<f32>;
@group(1) @binding(15) var n7: texture_2d<f32>;
@group(1) @binding(16) var s: sampler;

struct VIn {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
    @location(3) uv: vec2<f32>,
    @location(4) w0: vec4<f32>,
    @location(5) w1: vec4<f32>,
};

struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
    @location(3) uv: vec2<f32>,
    @location(4) w0: vec4<f32>,
    @location(5) w1: vec4<f32>,
};

@vertex
fn vs_main(v: VIn) -> VOut {
    var o: VOut;
    o.clip = frame.view_proj * vec4<f32>(v.pos, 1.0);
    o.world_pos = v.pos;
    o.normal = v.normal;
    o.color = v.color;
    o.uv = v.uv;
    o.w0 = v.w0;
    o.w1 = v.w1;
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
    let uv = in.uv;
    var c = textureSample(d0, s, uv).rgb;
    var nm = textureSample(n0, s, uv).xyz;
    c = mix(c, textureSample(d1, s, uv).rgb, in.w0.x);
    nm = mix(nm, textureSample(n1, s, uv).xyz, in.w0.x);
    c = mix(c, textureSample(d2, s, uv).rgb, in.w0.y);
    nm = mix(nm, textureSample(n2, s, uv).xyz, in.w0.y);
    c = mix(c, textureSample(d3, s, uv).rgb, in.w0.z);
    nm = mix(nm, textureSample(n3, s, uv).xyz, in.w0.z);
    c = mix(c, textureSample(d4, s, uv).rgb, in.w0.w);
    nm = mix(nm, textureSample(n4, s, uv).xyz, in.w0.w);
    c = mix(c, textureSample(d5, s, uv).rgb, in.w1.x);
    nm = mix(nm, textureSample(n5, s, uv).xyz, in.w1.x);
    c = mix(c, textureSample(d6, s, uv).rgb, in.w1.y);
    nm = mix(nm, textureSample(n6, s, uv).xyz, in.w1.y);
    c = mix(c, textureSample(d7, s, uv).rgb, in.w1.z);
    nm = mix(nm, textureSample(n7, s, uv).xyz, in.w1.z);

    // Terrain tangent space aligned with world X/Y.
    let N = normalize(in.normal);
    let T = normalize(cross(vec3<f32>(0.0, 1.0, 0.0), N));
    let B = cross(N, T);
    let tn = nm * 2.0 - 1.0;
    let n = normalize(tn.x * T + tn.y * B + tn.z * N);

    let l = normalize(frame.sun_dir.xyz);
    let diffuse = frame.ambient.rgb + frame.sun_color.rgb * max(dot(n, l), 0.0);
    let color = c * in.color * diffuse;
    return vec4<f32>(apply_fog(color, in.world_pos), 1.0);
}
