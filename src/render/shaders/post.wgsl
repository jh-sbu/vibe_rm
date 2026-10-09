// Image space effects over the finished frame (render/post.rs).

struct Post {
    // saturation, brightness, contrast, 1 if the source holds linear values
    cinematic: vec4<f32>,
    tint: vec4<f32>,
    fade: vec4<f32>,
    // blur radius px, double vision offset px, texel size
    blur: vec4<f32>,
    // x: the smallest mip, holding the frame's average
    levels: vec4<f32>,
};

@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var<uniform> post: Post;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> VsOut {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    var o: VsOut;
    o.pos = vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
    o.uv = vec2<f32>(p.x, 1.0 - p.y);
    return o;
}

// A disc of taps around the pixel (radius 1).
const TAPS = array<vec2<f32>, 12>(
    vec2<f32>(-0.326, -0.406), vec2<f32>(-0.840, -0.074), vec2<f32>(-0.696, 0.457),
    vec2<f32>(-0.203, 0.621), vec2<f32>(0.962, -0.195), vec2<f32>(0.473, -0.480),
    vec2<f32>(0.519, 0.767), vec2<f32>(0.185, -0.893), vec2<f32>(0.507, 0.064),
    vec2<f32>(0.896, 0.412), vec2<f32>(-0.322, -0.933), vec2<f32>(-0.792, -0.598),
);

fn blurred(uv: vec2<f32>) -> vec3<f32> {
    let r = post.blur.x * post.blur.zw;
    if (post.blur.x < 0.01) {
        return textureSampleLevel(src, samp, uv, 0.0).rgb;
    }
    var c = textureSampleLevel(src, samp, uv, 0.0).rgb;
    for (var i = 0; i < 12; i++) {
        c += textureSampleLevel(src, samp, uv + TAPS[i] * r, 0.0).rgb;
    }
    return c / 13.0;
}

fn to_display(c: vec3<f32>) -> vec3<f32> {
    return select(c, pow(max(c, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.2)), post.cinematic.w > 0.5);
}

fn from_display(c: vec3<f32>) -> vec3<f32> {
    return select(c, pow(max(c, vec3<f32>(0.0)), vec3<f32>(2.2)), post.cinematic.w > 0.5);
}

// Halving a mip: the bilinear tap between four texels averages them.
@fragment
fn fs_down(in: VsOut) -> @location(0) vec4<f32> {
    return vec4<f32>(textureSampleLevel(src, samp, in.uv, 0.0).rgb, 1.0);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    var c: vec3<f32>;
    if (post.blur.y > 0.01) {
        let d = vec2<f32>(post.blur.y * post.blur.z, 0.0);
        c = 0.5 * (blurred(in.uv - d) + blurred(in.uv + d));
    } else {
        c = blurred(in.uv);
    }
    // Contrast about the frame's average luminance.
    let avg = textureSampleLevel(src, samp, vec2<f32>(0.5), post.levels.x).rgb;
    let pivot = to_display(vec3<f32>(dot(avg, vec3<f32>(0.2125, 0.7154, 0.0721)))).x;
    c = to_display(c);
    let lum = dot(c, vec3<f32>(0.2125, 0.7154, 0.0721));
    c = mix(vec3<f32>(lum), c, post.cinematic.x);
    c = mix(c, lum * post.tint.rgb, post.tint.a);
    c = c * post.cinematic.y;
    c = (c - pivot) * post.cinematic.z + pivot;
    c = clamp(c, vec3<f32>(0.0), vec3<f32>(1.0));
    c = mix(c, post.fade.rgb, post.fade.a);
    return vec4<f32>(from_display(c), 1.0);
}
