// Forward shader for Creation Engine style objects (BSLightingShaderProperty /
// BSEffectShaderProperty). Lighting is done in gamma space, like the original.

struct Frame {
    view_proj: mat4x4<f32>,
    cam_pos: vec4<f32>,
    sun_dir: vec4<f32>,     // xyz: direction *towards* the light
    sun_color: vec4<f32>,
    ambient: vec4<f32>,
    fog_near_color: vec4<f32>,
    fog_far_color: vec4<f32>,
    fog: vec4<f32>,         // near, far, power, max
    misc: vec4<f32>,        // time, light count, exterior flag, grass wave clock (cycles)
    amb: array<vec4<f32>, 6>, // directional ambient X+ X- Y+ Y- Z+ Z-; amb[0].w > 0.5 enables
    lod_clip: vec4<f32>,    // xy min, xy max of the loaded full-detail area
    shadow_vp: array<mat4x4<f32>, 4>, // sun shadow cascades
    shadow_splits: vec4<f32>,         // view depth where each cascade ends
    shadow_params: vec4<f32>,         // enabled, texel size (uv), ...
    cam_fwd: vec4<f32>,
    wind: vec4<f32>,        // wind velocity xy (units a second), grass fade start, fade range
};

struct Light {
    pos_radius: vec4<f32>,
    color: vec4<f32>,
};

@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var<storage, read> lights: array<Light>;
@group(0) @binding(2) var<storage, read> palette: array<mat4x4<f32>>;

@group(0) @binding(3) var shadow_map: texture_depth_2d_array;
@group(0) @binding(4) var shadow_sampler: sampler_comparison;

// How much of the sun reaches a point (1 lit, 0 in shadow): the cascade for its view
// depth, sampled 3x3 a little off the surface along its normal.
fn sun_shadow(world_pos: vec3<f32>, normal: vec3<f32>) -> f32 {
    if (frame.shadow_params.x < 0.5) {
        return 1.0;
    }
    let depth = dot(world_pos - frame.cam_pos.xyz, frame.cam_fwd.xyz);
    let splits = frame.shadow_splits;
    if (depth > splits.w) {
        return 1.0;
    }
    var c = 0u;
    if (depth > splits.x) { c = 1u; }
    if (depth > splits.y) { c = 2u; }
    if (depth > splits.z) { c = 3u; }
    let vp = frame.shadow_vp[c];
    // The cascade's half-width in world units, from its projection's scale.
    let radius = 1.0 / length(vec3<f32>(vp[0].x, vp[1].x, vp[2].x));
    let texel = 2.0 * radius * frame.shadow_params.y;
    let p = vp * vec4<f32>(world_pos + normal * texel * 1.5, 1.0);
    let uv = vec2<f32>(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5);
    if (uv.x <= 0.0 || uv.x >= 1.0 || uv.y <= 0.0 || uv.y >= 1.0 || p.z >= 1.0) {
        return 1.0;
    }
    let t = frame.shadow_params.y;
    var lit = 0.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            lit += textureSampleCompareLevel(shadow_map, shadow_sampler, uv + vec2<f32>(f32(x), f32(y)) * t, c, p.z);
        }
    }
    lit /= 9.0;
    // Fade out towards the end of the last cascade.
    return mix(lit, 1.0, smoothstep(splits.w * 0.85, splits.w, depth));
}

struct Material {
    uv: vec4<f32>,          // offset.xy, scale.zw
    emissive: vec4<f32>,    // rgb, multiple
    specular: vec4<f32>,    // rgb * strength, glossiness
    params: vec4<f32>,      // alpha, alpha test threshold (<0 = off), has normal map, has glow map
    flags: vec4<u32>,       // shader flags 1, shader flags 2, kind (0 lit, 1 effect), unused
    falloff: vec4<f32>,     // effect: start angle, stop angle, start opacity, stop opacity (cosines)
    tint: vec4<f32>,        // skin / hair tint
    billboard: vec4<f32>,   // a billboard's model-space origin, mode + 1 (0: none)
    env: vec4<f32>,         // environment map scale, has cube map, has environment mask
};

@group(1) @binding(0) var t_diffuse: texture_2d<f32>;
@group(1) @binding(1) var t_normal: texture_2d<f32>;
@group(1) @binding(2) var t_glow: texture_2d<f32>;
@group(1) @binding(3) var s_main: sampler;
@group(1) @binding(4) var<uniform> mat: Material;
@group(1) @binding(5) var t_env: texture_cube<f32>;
@group(1) @binding(6) var t_env_mask: texture_2d<f32>;

struct VIn {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tangent: vec3<f32>,
    @location(3) bitangent: vec3<f32>,
    @location(4) uv: vec2<f32>,
    @location(5) color: vec4<f32>,
    @location(6) m0: vec4<f32>,
    @location(7) m1: vec4<f32>,
    @location(8) m2: vec4<f32>,
    @location(9) m3: vec4<f32>,
    @location(10) light_idx: vec4<u32>,
    @location(15) tint: vec4<f32>,
    @location(16) params: vec4<f32>,
};

struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tangent: vec3<f32>,
    @location(3) bitangent: vec3<f32>,
    @location(4) uv: vec2<f32>,
    @location(5) color: vec4<f32>,
    @location(6) @interpolate(flat) light_idx: vec4<u32>,
    // Model-to-world rotation, for model-space normal maps.
    @location(7) mx: vec3<f32>,
    @location(8) my: vec3<f32>,
    @location(9) mz: vec3<f32>,
    // The instance's tint (colour, opacity).
    @location(10) tint: vec4<f32>,
    // How much fog it takes (0 for the sky's models).
    @location(11) fog: f32,
    // Opacity before the alpha test (grass fading with distance).
    @location(12) fade: f32,
};

// A billboard's axes in the world.
// Modes 1, 5 and 9 turn about the up axis only (5 about the model's own);
// 3 and 4 face the camera's position, the others its view direction.
fn billboard_axes(model: mat4x4<f32>, pivot: vec3<f32>, mode: u32) -> mat3x3<f32> {
    var up = vec3<f32>(0.0, 0.0, 1.0);
    if (mode == 5u) {
        up = normalize(model[2].xyz);
    }
    var toward = -frame.cam_fwd.xyz;
    if (mode == 3u || mode == 4u || mode == 1u || mode == 5u || mode == 9u) {
        toward = normalize(frame.cam_pos.xyz - pivot);
    }
    if (mode == 1u || mode == 5u || mode == 9u) {
        toward = toward - up * dot(toward, up);
        if (length(toward) < 0.0001) {
            toward = vec3<f32>(0.0, -1.0, 0.0);
        }
        toward = normalize(toward);
    } else {
        var right = cross(toward, up);
        if (length(right) < 0.0001) {
            right = vec3<f32>(1.0, 0.0, 0.0);
        }
        up = normalize(cross(normalize(right), toward));
    }
    let across = normalize(cross(up, toward));
    // A billboard's own x is across, y up and z towards the camera (its
    // mesh lies in its xy plane; nodes rest turned so that y is the model's z).
    return mat3x3<f32>(across, up, toward);
}

@vertex
fn vs_main(v: VIn) -> VOut {
    var model = mat4x4<f32>(v.m0, v.m1, v.m2, v.m3);
    if (mat.billboard.w > 0.5) {
        let pivot = (model * vec4<f32>(mat.billboard.xyz, 1.0)).xyz;
        let scale = length(v.m0.xyz);
        let axes = billboard_axes(model, pivot, u32(mat.billboard.w - 1.0)) * scale;
        model = mat4x4<f32>(
            vec4<f32>(axes[0], 0.0),
            vec4<f32>(axes[1], 0.0),
            vec4<f32>(axes[2], 0.0),
            vec4<f32>(pivot, 1.0),
        );
    }
    let world = model * vec4<f32>(v.pos, 1.0);
    let m3 = mat3x3<f32>(model[0].xyz, model[1].xyz, model[2].xyz);
    var o: VOut;
    o.clip = frame.view_proj * world;
    o.world_pos = world.xyz;
    o.normal = m3 * v.normal;
    o.tangent = m3 * v.tangent;
    o.bitangent = m3 * v.bitangent;
    // Model-space normal maps: their meshes carry the shape's axes in the
    // model as tangent and bitangent (see `build_mesh`).
    o.mx = m3 * v.tangent;
    o.my = m3 * v.bitangent;
    o.mz = m3 * cross(v.tangent, v.bitangent);
    o.uv = v.uv * mat.uv.zw + mat.uv.xy;
    o.color = v.color;
    o.light_idx = v.light_idx;
    o.tint = v.tint;
    o.fog = v.params.x;
    o.fade = 1.0;
    return o;
}

struct GrassIn {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tangent: vec3<f32>,
    @location(3) bitangent: vec3<f32>,
    @location(4) uv: vec2<f32>,
    @location(5) color: vec4<f32>,
    @location(6) pos_scale: vec4<f32>,  // position, scale across
    @location(7) up_yaw: vec4<f32>,     // up axis, turn about it
    @location(8) tint_wave: vec4<f32>,  // tint, wave period (units; 0: still)
    @location(9) scale_up: vec4<f32>,   // scale up
};

// Grass: each blade stood on the ground along its up axis, turned and
// scaled, its top swaying in waves running with the wind (vertex alpha is how
// far each vertex sways), fading out beyond the grass fade distance.
@vertex
fn vs_grass(v: GrassIn) -> VOut {
    let up = normalize(v.up_yaw.xyz);
    // The rotation taking +Z to `up` (about Z x up).
    let k = vec3<f32>(-up.y, up.x, 0.0);
    let f = 1.0 / (1.0 + up.z);
    let tilt = mat3x3<f32>(
        vec3<f32>(1.0 - k.y * k.y * f, k.x * k.y * f, -k.y),
        vec3<f32>(k.x * k.y * f, 1.0 - k.x * k.x * f, k.x),
        up,
    );
    let cy = cos(v.up_yaw.w);
    let sy = sin(v.up_yaw.w);
    let turn = mat3x3<f32>(vec3<f32>(cy, sy, 0.0), vec3<f32>(-sy, cy, 0.0), vec3<f32>(0.0, 0.0, 1.0));
    let m3 = tilt * turn;
    let local = vec3<f32>(v.pos.xy * v.pos_scale.w, v.pos.z * v.scale_up.x);
    var world = v.pos_scale.xyz + m3 * local;
    let wave = v.tint_wave.w;
    if (wave > 0.0) {
        let wind = frame.wind.xy;
        let speed = length(wind);
        var dir = vec2<f32>(1.0, 0.0);
        if (speed > 1.0) {
            dir = wind / speed;
        }
        let strength = 0.25 + clamp(speed / 1000.0, 0.0, 1.0);
        // The waves run along a fixed axis: following the wind's direction,
        // a slight turn would sweep the far-off origin's waves past at speed.
        let along = dot(v.pos_scale.xy, vec2<f32>(0.6, 0.8)) / wave;
        let phase = (fract(along) - fract(frame.misc.w)) * 6.2831853;
        let bend = v.color.a * strength * 10.0 * (0.6 + 0.4 * sin(phase));
        world += vec3<f32>(dir * bend, 0.0);
    }
    var o: VOut;
    o.clip = frame.view_proj * vec4<f32>(world, 1.0);
    o.world_pos = world;
    o.normal = m3 * v.normal;
    o.tangent = m3 * v.tangent;
    o.bitangent = m3 * v.bitangent;
    o.mx = m3[0];
    o.my = m3[1];
    o.mz = m3[2];
    o.uv = v.uv * mat.uv.zw + mat.uv.xy;
    o.color = v.color;
    o.light_idx = vec4<u32>(0xFFFFFFFFu);
    o.tint = vec4<f32>(v.tint_wave.rgb, 1.0);
    o.fog = 1.0;
    let dist = distance(world, frame.cam_pos.xyz);
    o.fade = 1.0 - clamp((dist - frame.wind.z) / max(frame.wind.w, 1.0), 0.0, 1.0);
    return o;
}

struct SkinIn {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tangent: vec3<f32>,
    @location(3) bitangent: vec3<f32>,
    @location(4) uv: vec2<f32>,
    @location(5) color: vec4<f32>,
    @location(11) bones: vec4<u32>,
    @location(12) weights: vec4<f32>,
    @location(13) palette_base: u32,
    @location(14) light_idx: vec4<u32>,
};

@vertex
fn vs_skinned(v: SkinIn) -> VOut {
    let b = v.palette_base;
    let m = palette[b + v.bones.x] * v.weights.x
          + palette[b + v.bones.y] * v.weights.y
          + palette[b + v.bones.z] * v.weights.z
          + palette[b + v.bones.w] * v.weights.w;
    let world = m * vec4<f32>(v.pos, 1.0);
    var o: VOut;
    o.clip = frame.view_proj * world;
    o.world_pos = world.xyz;
    o.normal = (m * vec4<f32>(v.normal, 0.0)).xyz;
    o.tangent = (m * vec4<f32>(v.tangent, 0.0)).xyz;
    o.bitangent = (m * vec4<f32>(v.bitangent, 0.0)).xyz;
    o.uv = v.uv * mat.uv.zw + mat.uv.xy;
    o.color = v.color;
    o.light_idx = v.light_idx;
    o.mx = m[0].xyz;
    o.my = m[1].xyz;
    o.mz = m[2].xyz;
    o.tint = vec4<f32>(1.0);
    o.fog = 1.0;
    o.fade = 1.0;
    return o;
}

const SF1_VERTEX_ALPHA: u32 = 8u;
const SF1_GREYSCALE_TO_PALETTE_COLOR: u32 = 16u;
const SF1_GREYSCALE_TO_PALETTE_ALPHA: u32 = 32u;
const SF1_USE_FALLOFF: u32 = 64u;
const SF1_MODEL_SPACE_NORMALS: u32 = 4096u;
const SF2_VERTEX_COLORS: u32 = 32u;
const SF2_SOFT_LIGHTING: u32 = 33554432u;
const SF2_BACK_LIGHTING: u32 = 134217728u;
const SF2_TREE_ANIM: u32 = 536870912u;

fn ambient(n: vec3<f32>) -> vec3<f32> {
    if (frame.amb[0].w < 0.5) {
        return frame.ambient.rgb;
    }
    let n2 = n * n;
    let x = select(frame.amb[1].rgb, frame.amb[0].rgb, n.x >= 0.0);
    let y = select(frame.amb[3].rgb, frame.amb[2].rgb, n.y >= 0.0);
    let z = select(frame.amb[5].rgb, frame.amb[4].rgb, n.z >= 0.0);
    return x * n2.x + y * n2.y + z * n2.z;
}

fn apply_fog(color: vec3<f32>, world_pos: vec3<f32>) -> vec3<f32> {
    let dist = distance(world_pos, frame.cam_pos.xyz);
    let range = max(frame.fog.y - frame.fog.x, 1.0);
    var f = clamp((dist - frame.fog.x) / range, 0.0, 1.0);
    f = pow(f, max(frame.fog.z, 0.0001)) * frame.fog.w;
    let fog_color = mix(frame.fog_near_color.rgb, frame.fog_far_color.rgb, f);
    return mix(color, fog_color, f);
}

fn light_index(idx: vec4<u32>, i: u32) -> u32 {
    let word = idx[i / 2u];
    return select(word & 0xFFFFu, word >> 16u, (i & 1u) == 1u);
}

@fragment
fn fs_main(in: VOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    if (mat.flags.z == 2u) {
        let c = frame.lod_clip;
        if (in.world_pos.x > c.x && in.world_pos.x < c.z && in.world_pos.y > c.y && in.world_pos.y < c.w) {
            discard;
        }
    }
    let base = textureSample(t_diffuse, s_main, in.uv);
    var albedo = base.rgb;
    var alpha = base.a * mat.params.x;
    let flags1 = mat.flags.x;
    let flags2 = mat.flags.y;
    if ((flags2 & SF2_VERTEX_COLORS) != 0u) {
        albedo *= in.color.rgb;
    }
    let shader_type = mat.flags.w;
    if (shader_type == 5u || shader_type == 6u) {
        albedo *= mat.tint.rgb;
    }
    if (shader_type == 4u && mat.params.w > 1.5) {
        // FaceGen tint mask, applied as an overlay.
        let tint = textureSample(t_glow, s_main, in.uv).rgb;
        albedo = clamp(albedo * tint * 2.0, vec3<f32>(0.0), vec3<f32>(1.0));
    }
    // Trees keep their wind sway weights in vertex alpha, not opacity.
    if ((flags1 & SF1_VERTEX_ALPHA) != 0u && (flags2 & SF2_TREE_ANIM) == 0u) {
        alpha *= in.color.a;
    }

    if (mat.flags.z == 1u) {
        // Effect shader: unlit, emissive tinted. The glow slot holds the greyscale palette.
        var c = base.rgb;
        var a = base.a;
        // The palette is looked up clamped: the sampler repeats, and a
        // greyscale of 0 would blend in the palette's far end.
        let pal = 0.5 / vec2<f32>(textureDimensions(t_glow));
        if ((flags1 & SF1_GREYSCALE_TO_PALETTE_COLOR) != 0u && mat.params.w > 0.5) {
            c = textureSample(t_glow, s_main, clamp(vec2<f32>(base.g, in.color.r), pal, 1.0 - pal)).rgb;
        }
        if ((flags1 & SF1_GREYSCALE_TO_PALETTE_ALPHA) != 0u && mat.params.w > 0.5) {
            a = textureSample(t_glow, s_main, clamp(vec2<f32>(base.a, in.color.a), pal, 1.0 - pal)).a;
        }
        c *= mat.emissive.rgb * mat.emissive.w;
        a *= mat.params.x;
        if ((flags2 & SF2_VERTEX_COLORS) != 0u) {
            c *= in.color.rgb;
        }
        if ((flags1 & SF1_VERTEX_ALPHA) != 0u) {
            a *= in.color.a;
        }
        if ((flags1 & SF1_USE_FALLOFF) != 0u) {
            let v = normalize(frame.cam_pos.xyz - in.world_pos);
            let ndv = abs(dot(normalize(in.normal), v));
            let f = mat.falloff;
            let t = clamp((ndv - f.y) / max(f.x - f.y, 0.0001), 0.0, 1.0);
            a *= mix(f.w, f.z, t);
        }
        if (mat.params.y >= 0.0 && a < mat.params.y) {
            discard;
        }
        c *= in.tint.rgb;
        a *= in.tint.a;
        return vec4<f32>(mix(c, apply_fog(c, in.world_pos), in.fog), clamp(a, 0.0, 1.0));
    }

    alpha *= in.fade;
    if (mat.params.y >= 0.0 && alpha < mat.params.y) {
        discard;
    }

    var n = normalize(in.normal);
    var spec_mask = 0.0;
    if (mat.params.z > 0.5) {
        let nt = textureSample(t_normal, s_main, in.uv);
        let tn = nt.xyz * 2.0 - 1.0;
        if ((flags1 & SF1_MODEL_SPACE_NORMALS) != 0u) {
            // Model-space normal maps store X/Y/Z in R/B/G order in Skyrim.
            let msn = vec3<f32>(tn.x, tn.z, tn.y);
            n = normalize(mat3x3<f32>(in.mx, in.my, in.mz) * msn);
        } else {
            n = normalize(tn.x * normalize(in.bitangent) + tn.y * normalize(in.tangent) + tn.z * n);
            spec_mask = nt.a;
        }
    }
    if (!front) {
        n = -n;
    }

    let view_dir = normalize(frame.cam_pos.xyz - in.world_pos);
    var diffuse = ambient(n);
    var specular = vec3<f32>(0.0);
    let gloss = max(mat.specular.w, 1.0);

    // Directional (sun / interior directional)
    let l = normalize(frame.sun_dir.xyz);
    let ndl = dot(n, l);
    var sun = 1.0;
    if (ndl > 0.0) {
        var gn = normalize(in.normal);
        if (!front) {
            gn = -gn;
        }
        sun = sun_shadow(in.world_pos, gn);
    }
    diffuse += frame.sun_color.rgb * max(ndl, 0.0) * sun;
    if (ndl > 0.0) {
        let h = normalize(l + view_dir);
        specular += frame.sun_color.rgb * pow(max(dot(n, h), 0.0), gloss) * spec_mask * sun;
    }

    for (var i = 0u; i < 8u; i++) {
        let li = light_index(in.light_idx, i);
        if (li == 0xFFFFu) {
            break;
        }
        let light = lights[li];
        let d = light.pos_radius.xyz - in.world_pos;
        let dist = length(d);
        let r = light.pos_radius.w;
        if (dist >= r) {
            continue;
        }
        let ld = d / max(dist, 0.001);
        let x = dist / r;
        let att = clamp(1.0 - x * x, 0.0, 1.0);
        let nl = max(dot(n, ld), 0.0);
        diffuse += light.color.rgb * nl * att;
        if (nl > 0.0) {
            let h = normalize(ld + view_dir);
            specular += light.color.rgb * pow(max(dot(n, h), 0.0), gloss) * spec_mask * att;
        }
    }

    // Environment mapping: the cube map (authored with the world's axes, the
    // sky on +Z) seen along the reflected view, masked by the environment
    // mask or else the normal map's specular mask, lit like the surface.
    var env = vec3<f32>(0.0);
    if (mat.env.y > 0.5) {
        var mask = spec_mask;
        if (mat.env.z > 0.5) {
            mask = textureSample(t_env_mask, s_main, in.uv).r;
        }
        let r = reflect(-view_dir, n);
        env = textureSample(t_env, s_main, r).rgb * mask * mat.env.x * diffuse;
    }

    var emit = mat.emissive.rgb * mat.emissive.w;
    if (mat.params.w > 0.5 && mat.params.w < 1.5) {
        emit *= textureSample(t_glow, s_main, in.uv).rgb;
    }
    diffuse += emit;
    let color = albedo * diffuse + specular * mat.specular.rgb + env;
    return vec4<f32>(apply_fog(color * in.tint.rgb, in.world_pos), alpha * in.tint.a);
}
