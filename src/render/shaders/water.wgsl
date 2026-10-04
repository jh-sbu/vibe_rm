// Simple water: scrolling normal maps, fresnel blend between water colour and reflection colour.

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
    amb: array<vec4<f32>, 6>,
};

struct Water {
    shallow: vec4<f32>,
    deep: vec4<f32>,
    reflection: vec4<f32>,
    params: vec4<f32>,   // fresnel, reflectivity, sun specular power, unused
};

@group(0) @binding(0) var<uniform> frame: Frame;
@group(1) @binding(0) var t_noise: texture_2d<f32>;
@group(1) @binding(1) var s: sampler;
@group(1) @binding(2) var<uniform> water: Water;

struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
};

@vertex
fn vs_main(@location(0) pos: vec3<f32>) -> VOut {
    var o: VOut;
    o.clip = frame.view_proj * vec4<f32>(pos, 1.0);
    o.world_pos = pos;
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
    let t = frame.misc.x;
    let uv = in.world_pos.xy / 2048.0;
    let n1 = textureSample(t_noise, s, uv + vec2<f32>(t * 0.010, t * 0.004)).xyz * 2.0 - 1.0;
    let n2 = textureSample(t_noise, s, uv * 2.7 + vec2<f32>(-t * 0.006, t * 0.011)).xyz * 2.0 - 1.0;
    let n = normalize(vec3<f32>(n1.xy + n2.xy, 6.0));
    let v = normalize(frame.cam_pos.xyz - in.world_pos);
    let ndv = clamp(dot(n, v), 0.0, 1.0);
    let fresnel = clamp(pow(1.0 - ndv, 4.0) * 0.9 + 0.08, 0.0, 1.0);
    // Reflect the horizon / fog colour as a cheap sky reflection.
    let sky = mix(frame.fog_far_color.rgb, water.reflection.rgb, 0.5);
    var color = mix(mix(water.shallow.rgb, water.deep.rgb, 0.6), sky, fresnel);
    let l = normalize(frame.sun_dir.xyz);
    let h = normalize(l + v);
    color += frame.sun_color.rgb * pow(max(dot(n, h), 0.0), max(water.params.z, 16.0)) * 0.8;
    let alpha = mix(0.75, 0.95, fresnel);
    return vec4<f32>(apply_fog(color, in.world_pos), alpha);
}
