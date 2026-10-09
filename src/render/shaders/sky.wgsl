// Sky: gradient, cloud layers and sun, drawn as a full-screen triangle behind everything.

struct Sky {
    inv_view_proj: mat4x4<f32>,
    cam_pos: vec4<f32>,
    upper: vec4<f32>,
    lower: vec4<f32>,
    horizon: vec4<f32>,
    sun_dir: vec4<f32>,      // xyz, visibility
    sun_color: vec4<f32>,
    cloud_color: array<vec4<f32>, 8>,  // rgb, alpha: incoming layers, then outgoing
    params: vec4<f32>,       // time, wind speed, cloud layer count, stars
    params2: vec4<f32>,      // outgoing weather's layer count
};

@group(0) @binding(0) var<uniform> sky: Sky;
@group(0) @binding(1) var t_sun: texture_2d<f32>;
@group(0) @binding(2) var t_c0: texture_2d<f32>;
@group(0) @binding(3) var t_c1: texture_2d<f32>;
@group(0) @binding(4) var t_c2: texture_2d<f32>;
@group(0) @binding(5) var t_c3: texture_2d<f32>;
@group(0) @binding(6) var t_o0: texture_2d<f32>;
@group(0) @binding(7) var t_o1: texture_2d<f32>;
@group(0) @binding(8) var t_o2: texture_2d<f32>;
@group(0) @binding(9) var t_o3: texture_2d<f32>;
@group(0) @binding(10) var s: sampler;

struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) ndc: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> VOut {
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    let p = uv * 2.0 - 1.0;
    var o: VOut;
    o.clip = vec4<f32>(p, 0.0, 1.0);
    o.ndc = p;
    return o;
}

fn cloud(t: texture_2d<f32>, uv: vec2<f32>, color: vec4<f32>, base: vec3<f32>) -> vec3<f32> {
    let c = textureSample(t, s, uv);
    return mix(base, c.rgb * color.rgb, clamp(c.a * color.a, 0.0, 1.0));
}

@fragment
fn fs_main(in: VOut) -> @location(0) vec4<f32> {
    let p = sky.inv_view_proj * vec4<f32>(in.ndc, 1.0, 1.0);
    let dir = normalize(p.xyz / p.w - sky.cam_pos.xyz);
    let e = dir.z;
    var color: vec3<f32>;
    if (e >= 0.0) {
        color = mix(sky.horizon.rgb, sky.upper.rgb, smoothstep(0.0, 0.5, e));
    } else {
        color = mix(sky.horizon.rgb, sky.lower.rgb, smoothstep(0.0, 0.15, -e));
    }

    // Sun disc and glow.
    let sd = normalize(sky.sun_dir.xyz);
    let cosang = dot(dir, sd);
    let vis = sky.sun_dir.w;
    if (vis > 0.0) {
        let glow = pow(max(cosang, 0.0), 64.0) * 0.35 + pow(max(cosang, 0.0), 1024.0) * 0.8;
        color += sky.sun_color.rgb * glow * vis;
        let size = 0.035;
        if (cosang > 0.99) {
            let right = normalize(cross(sd, vec3<f32>(0.0, 0.0, 1.0)));
            let up = cross(right, sd);
            let uv = vec2<f32>(dot(dir, right), dot(dir, up)) / size * 0.5 + 0.5;
            if (all(uv >= vec2<f32>(0.0)) && all(uv <= vec2<f32>(1.0))) {
                let sc = textureSample(t_sun, s, uv);
                color = mix(color, sc.rgb * sky.sun_color.rgb * 1.5, sc.a * vis);
            }
        }
    }

    // Cloud layers projected onto a plane overhead, scrolling with the wind.
    if (e > 0.0) {
        let n = u32(sky.params.z);
        let scroll = sky.params.x * (0.002 + sky.params.y * 0.01);
        let base_uv = dir.xy / (e + 0.08) * 0.35;
        let fade = smoothstep(0.0, 0.12, e);
        var cc = color;
        // The outgoing weather's layers under the incoming one's.
        let o = u32(sky.params2.x);
        if (o > 0u) { cc = cloud(t_o0, base_uv + vec2<f32>(scroll, 0.0), sky.cloud_color[4], cc); }
        if (o > 1u) { cc = cloud(t_o1, base_uv * 0.8 + vec2<f32>(scroll * 0.7, scroll * 0.2), sky.cloud_color[5], cc); }
        if (o > 2u) { cc = cloud(t_o2, base_uv * 1.3 + vec2<f32>(scroll * 1.2, 0.0), sky.cloud_color[6], cc); }
        if (o > 3u) { cc = cloud(t_o3, base_uv * 0.6 + vec2<f32>(0.0, scroll * 0.5), sky.cloud_color[7], cc); }
        if (n > 0u) { cc = cloud(t_c0, base_uv + vec2<f32>(scroll, 0.0), sky.cloud_color[0], cc); }
        if (n > 1u) { cc = cloud(t_c1, base_uv * 0.8 + vec2<f32>(scroll * 0.7, scroll * 0.2), sky.cloud_color[1], cc); }
        if (n > 2u) { cc = cloud(t_c2, base_uv * 1.3 + vec2<f32>(scroll * 1.2, 0.0), sky.cloud_color[2], cc); }
        if (n > 3u) { cc = cloud(t_c3, base_uv * 0.6 + vec2<f32>(0.0, scroll * 0.5), sky.cloud_color[3], cc); }
        color = mix(color, cc, fade);
    }
    return vec4<f32>(color, 1.0);
}
