// Depth-only passes into the sun's shadow cascades.

struct Shadow {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> sh: Shadow;
@group(0) @binding(2) var<storage, read> palette: array<mat4x4<f32>>;

struct Material {
    uv: vec4<f32>,
    emissive: vec4<f32>,
    specular: vec4<f32>,
    params: vec4<f32>,      // alpha, alpha test threshold (<0 = off), ...
    flags: vec4<u32>,
    falloff: vec4<f32>,
    tint: vec4<f32>,
};

@group(1) @binding(0) var t_diffuse: texture_2d<f32>;
@group(1) @binding(3) var s_main: sampler;
@group(1) @binding(4) var<uniform> mat: Material;

struct VIn {
    @location(0) pos: vec3<f32>,
    @location(4) uv: vec2<f32>,
    @location(6) m0: vec4<f32>,
    @location(7) m1: vec4<f32>,
    @location(8) m2: vec4<f32>,
    @location(9) m3: vec4<f32>,
};

struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_static(v: VIn) -> VOut {
    let model = mat4x4<f32>(v.m0, v.m1, v.m2, v.m3);
    var o: VOut;
    o.clip = sh.view_proj * (model * vec4<f32>(v.pos, 1.0));
    o.uv = v.uv * mat.uv.zw + mat.uv.xy;
    return o;
}

struct SkinIn {
    @location(0) pos: vec3<f32>,
    @location(4) uv: vec2<f32>,
    @location(11) bones: vec4<u32>,
    @location(12) weights: vec4<f32>,
    @location(13) palette_base: u32,
};

@vertex
fn vs_skinned(v: SkinIn) -> VOut {
    let b = v.palette_base;
    let m = palette[b + v.bones.x] * v.weights.x
          + palette[b + v.bones.y] * v.weights.y
          + palette[b + v.bones.z] * v.weights.z
          + palette[b + v.bones.w] * v.weights.w;
    var o: VOut;
    o.clip = sh.view_proj * (m * vec4<f32>(v.pos, 1.0));
    o.uv = v.uv * mat.uv.zw + mat.uv.xy;
    return o;
}

// Alpha-tested surfaces (leaves, grass cards, hair) cast shadows through their cutouts.
@fragment
fn fs_alpha(in: VOut) {
    if (mat.params.y >= 0.0) {
        let a = textureSample(t_diffuse, s_main, in.uv).a * mat.params.x;
        if (a < mat.params.y) {
            discard;
        }
    }
}

@vertex
fn vs_terrain(@location(0) pos: vec3<f32>) -> @builtin(position) vec4<f32> {
    return sh.view_proj * vec4<f32>(pos, 1.0);
}
