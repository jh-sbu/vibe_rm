//! Weather (WTHR) and climate (CLMT) evaluation: sky colours, fog, sun and clouds over the day.

use esp::{FormId, LoadOrder};
use glam::{Vec2, Vec3};

use super::records::rgb;

pub mod color {
    pub const SKY_UPPER: usize = 0;
    pub const FOG_NEAR: usize = 1;
    pub const AMBIENT: usize = 3;
    pub const SUNLIGHT: usize = 4;
    pub const SUN: usize = 5;
    pub const STARS: usize = 6;
    pub const SKY_LOWER: usize = 7;
    pub const HORIZON: usize = 8;
    pub const EFFECT_LIGHTING: usize = 9;
    pub const CLOUD_LOD_DIFFUSE: usize = 10;
    pub const CLOUD_LOD_AMBIENT: usize = 11;
    pub const FOG_FAR: usize = 12;
    pub const SKY_STATICS: usize = 13;
    pub const WATER_MULTIPLIER: usize = 14;
    pub const SUN_GLARE: usize = 15;
    pub const MOON_GLARE: usize = 16;
    pub const COUNT: usize = 17;
}

/// Index of a time-of-day slot in weather colour tables.
pub const SUNRISE: usize = 0;
pub const DAY: usize = 1;
pub const SUNSET: usize = 2;
pub const NIGHT: usize = 3;

#[derive(Debug, Clone, Default)]
pub struct CloudLayer {
    pub texture: String,
    pub colors: [Vec3; 4],
    pub alphas: [f32; 4],
    /// QNAM / RNAM: how fast it drifts along x and y (-0.1..0.1).
    pub speed: Vec2,
}

/// A cloud layer at one moment.
#[derive(Debug, Clone)]
pub struct CloudState {
    pub texture: String,
    pub color: Vec3,
    pub alpha: f32,
    pub speed: Vec2,
}

/// A cloud speed byte (QNAM, RNAM) as xEdit reads it: 127 still, 0..254
/// from -0.1 to 0.1.
fn cloud_speed(b: u8) -> f32 {
    (b as f32 - 127.0) / 1270.0
}

#[derive(Debug, Clone)]
pub struct Weather {
    pub id: FormId,
    pub editor_id: String,
    pub colors: [[Vec3; 4]; color::COUNT],
    /// day near, day far, night near, night far, day power, night power, day max, night max
    pub fog: [f32; 8],
    pub clouds: Vec<CloudLayer>,
    /// Directional ambient per time of day: X+, X-, Y+, Y-, Z+, Z-.
    pub dalc: [[Vec3; 6]; 4],
    pub wind_speed: f32,
    /// IMSP: the image space per time of day (sunrise, day, sunset, night).
    pub image_spaces: [FormId; 4],
    /// DATA flags: pleasant, cloudy, rainy, snow ([`flags`]).
    pub flags: u8,
    /// DATA "trans delta": how fast it comes in (0: at once).
    pub trans_delta: u8,
    /// Precipitation's begin fade in and end fade out (0..255 of a transition).
    pub precip_fade: (u8, u8),
    /// Thunder and lightning's begin fade in, end fade out and frequency.
    pub thunder: (u8, u8, u8),
    /// SNAM: (sound, [`sound_type`]).
    pub sounds: Vec<(FormId, u32)>,
    /// DATA lightning colour.
    pub lightning_color: Vec3,
    /// MNAM: the precipitation (SPGD) drawn while it rains or snows.
    pub precipitation: FormId,
    /// TNAM: the sky statics (placed cloud statics) shown in this weather.
    pub sky_statics: Vec<FormId>,
}

/// Shader particle geometry (SPGD): the rain or snow a weather draws. DATA
/// follows UESP's layout (CommonLibSSE's `BGSShaderParticleGeometryData`
/// settings): gravity velocity, rotation velocity, particle size X / Y,
/// center offset min / max, initial rotation range, subtextures X / Y,
/// type, box size, particle density.
#[derive(Debug, Clone, PartialEq)]
pub struct Precipitation {
    pub id: FormId,
    /// Falling speed (units a second).
    pub gravity: f32,
    /// Degrees a second each particle turns about its falling centre.
    pub rotation_velocity: f32,
    pub size: (f32, f32),
    /// How far from its centre a particle turns (min, max).
    pub center_offset: (f32, f32),
    /// The spread of particles' starting angles (degrees).
    pub start_rotation: f32,
    /// The texture's grid of subtextures (columns, rows).
    pub subtextures: (u32, u32),
    pub snow: bool,
    /// The edge of the box of particles around the camera.
    pub box_size: f32,
    pub density: f32,
    pub texture: String,
}

pub fn load_precipitation(lo: &LoadOrder, id: FormId) -> Option<Precipitation> {
    let rec = lo.get(id).filter(|r| r.tag().0 == *b"SPGD")?;
    let data = rec.get(b"DATA")?;
    // DustParticles and FogParticles stop before the box size and density.
    if data.len() < 48 {
        return None;
    }
    let f = |i: usize| f32::from_le_bytes(data[i * 4..i * 4 + 4].try_into().unwrap());
    let u = |i: usize| u32::from_le_bytes(data[i * 4..i * 4 + 4].try_into().unwrap());
    Some(Precipitation {
        id,
        gravity: f(0),
        rotation_velocity: f(1),
        size: (f(2), f(3)),
        center_offset: (f(4), f(5)),
        start_rotation: f(6),
        subtextures: (u(7).max(1), u(8).max(1)),
        snow: u(9) == 1,
        box_size: u(10) as f32,
        density: f(11),
        texture: super::records::texture_path(&esp::decode_zstring(rec.get(b"ICON")?)),
    })
}

/// SNAM sound types (CommonLibSSE's `TESWeather::SoundType`).
pub mod sound_type {
    pub const THUNDER: u32 = 3;
}

/// Weather DATA flags (CommonLibSSE's `TESWeather::WeatherDataFlag`).
pub mod flags {
    pub const PLEASANT: u8 = 1 << 0;
    pub const CLOUDY: u8 = 1 << 1;
    pub const RAINY: u8 = 1 << 2;
    pub const SNOW: u8 = 1 << 3;
}

impl Weather {
    /// `Weather.GetClassification`: 0 pleasant, 1 cloudy, 2 rainy, 3 snow, -1 none.
    pub fn classification(&self) -> i32 {
        [flags::PLEASANT, flags::CLOUDY, flags::RAINY, flags::SNOW]
            .iter()
            .position(|&f| self.flags & f != 0)
            .map_or(-1, |i| i as i32)
    }
}

#[derive(Debug, Clone)]
pub struct Climate {
    pub weathers: Vec<(FormId, i32)>,
    pub sun_texture: String,
    pub sun_glare_texture: String,
    /// Sunrise begin/end, sunset begin/end in hours.
    pub sunrise: (f32, f32),
    pub sunset: (f32, f32),
}

impl Default for Climate {
    fn default() -> Self {
        Climate {
            weathers: Vec::new(),
            sun_texture: "textures/sky/sun.dds".into(),
            sun_glare_texture: "textures/sky/sunglare.dds".into(),
            sunrise: (5.5, 10.0),
            sunset: (16.0, 20.5),
        }
    }
}

pub fn load_climate(lo: &LoadOrder, id: FormId) -> Option<Climate> {
    let rec = lo.get(id)?;
    let mut c = Climate::default();
    for sr in rec.subrecords() {
        match &sr.tag.0 {
            b"WLST" => {
                for e in sr.data.chunks_exact(12) {
                    let w = rec.fid(FormId(u32::from_le_bytes(e[0..4].try_into().unwrap())));
                    let chance = i32::from_le_bytes(e[4..8].try_into().unwrap());
                    c.weathers.push((w, chance));
                }
            }
            b"FNAM" => c.sun_texture = super::records::texture_path(&sr.zstring()),
            b"GNAM" => c.sun_glare_texture = super::records::texture_path(&sr.zstring()),
            b"TNAM" if sr.data.len() >= 4 => {
                let h = |b: u8| b as f32 / 6.0;
                c.sunrise = (h(sr.data[0]), h(sr.data[1]));
                c.sunset = (h(sr.data[2]), h(sr.data[3]));
            }
            _ => {}
        }
    }
    Some(c)
}

const CLOUD_TAGS: [&[u8; 4]; 32] = [
    b"00TX", b"10TX", b"20TX", b"30TX", b"40TX", b"50TX", b"60TX", b"70TX", b"80TX", b"90TX",
    b":0TX", b";0TX", b"<0TX", b"=0TX", b">0TX", b"?0TX", b"@0TX", b"A0TX", b"B0TX", b"C0TX",
    b"D0TX", b"E0TX", b"F0TX", b"G0TX", b"H0TX", b"I0TX", b"J0TX", b"K0TX", b"L0TX", b"M0TX",
    b"N0TX", b"O0TX",
];

impl Weather {
    /// A weather with nothing set.
    pub fn new(id: FormId, editor_id: String) -> Self {
        Weather {
            id,
            editor_id,
            colors: [[Vec3::splat(0.5); 4]; color::COUNT],
            fog: [0.0, 80000.0, 0.0, 40000.0, 1.0, 1.0, 1.0, 1.0],
            clouds: Vec::new(),
            dalc: [[Vec3::splat(0.3); 6]; 4],
            wind_speed: 0.0,
            image_spaces: [FormId::NULL; 4],
            flags: 0,
            trans_delta: 0,
            precip_fade: (0, 0),
            thunder: (0, 0, 0),
            sounds: Vec::new(),
            lightning_color: Vec3::ONE,
            precipitation: FormId::NULL,
            sky_statics: Vec::new(),
        }
    }
}

pub fn load_weather(lo: &LoadOrder, id: FormId) -> Option<Weather> {
    let rec = lo.get(id)?;
    let mut w = Weather::new(id, rec.editor_id().unwrap_or_default());
    let mut layer_tex: Vec<Option<String>> = vec![None; 32];
    let mut pnam: Option<Vec<u8>> = None;
    let mut jnam: Option<Vec<u8>> = None;
    let mut speeds: (Option<Vec<u8>>, Option<Vec<u8>>) = (None, None);
    let mut disabled = 0u32;
    let mut dalc_i = 0;
    for sr in rec.subrecords() {
        if let Some(i) = CLOUD_TAGS.iter().position(|t| **t == sr.tag.0) {
            let s = sr.zstring();
            if !s.is_empty() {
                layer_tex[i] = Some(super::records::texture_path(&s));
            }
            continue;
        }
        match &sr.tag.0 {
            b"NAM0" => {
                let n = (sr.data.len() / 16).min(color::COUNT);
                for c in 0..n {
                    for t in 0..4 {
                        w.colors[c][t] = rgb(sr.data, (c * 4 + t) * 4);
                    }
                }
            }
            b"FNAM" => {
                for i in 0..8.min(sr.data.len() / 4) {
                    w.fog[i] = sr.f32(i * 4);
                }
            }
            b"PNAM" => pnam = Some(sr.data.to_vec()),
            b"JNAM" => jnam = Some(sr.data.to_vec()),
            b"QNAM" => speeds.0 = Some(sr.data.to_vec()),
            b"RNAM" => speeds.1 = Some(sr.data.to_vec()),
            b"NAM1" => disabled = sr.u32(0),
            b"DATA" if sr.data.len() >= 12 => {
                w.wind_speed = sr.u8(0) as f32 / 255.0;
                w.trans_delta = sr.u8(3);
                w.precip_fade = (sr.u8(6), sr.u8(7));
                w.thunder = (sr.u8(8), sr.u8(9), sr.u8(10));
                w.flags = sr.u8(11);
                if sr.data.len() >= 15 {
                    w.lightning_color =
                        Vec3::new(sr.u8(12) as f32, sr.u8(13) as f32, sr.u8(14) as f32) / 255.0;
                }
            }
            b"SNAM" if sr.data.len() >= 8 => {
                w.sounds.push((rec.fid(sr.form_id(0)), sr.u32(4)));
            }
            b"MNAM" => w.precipitation = rec.fid(sr.form_id(0)),
            b"TNAM" if sr.data.len() >= 4 => w.sky_statics.push(rec.fid(sr.form_id(0))),
            b"IMSP" => {
                for (t, f) in w
                    .image_spaces
                    .iter_mut()
                    .enumerate()
                    .take(sr.data.len() / 4)
                {
                    *f = rec.fid(sr.form_id(t * 4));
                }
            }
            b"DALC" if dalc_i < 4 => {
                for a in 0..6 {
                    w.dalc[dalc_i][a] = rgb(sr.data, a * 4);
                }
                dalc_i += 1;
            }
            _ => {}
        }
    }
    for (i, tex) in layer_tex.into_iter().enumerate() {
        let Some(texture) = tex else { continue };
        if disabled & (1 << i) != 0 {
            continue;
        }
        let speed = |s: &Option<Vec<u8>>| {
            s.as_ref()
                .and_then(|s| s.get(i))
                .map_or(0.0, |&b| cloud_speed(b))
        };
        let mut layer = CloudLayer {
            texture,
            speed: Vec2::new(speed(&speeds.0), speed(&speeds.1)),
            ..Default::default()
        };
        for t in 0..4 {
            if let Some(p) = &pnam {
                layer.colors[t] = rgb(p, (i * 4 + t) * 4);
            }
            layer.alphas[t] = jnam
                .as_ref()
                .and_then(|j| j.get((i * 4 + t) * 4..(i * 4 + t) * 4 + 4))
                .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
                .unwrap_or(1.0);
        }
        w.clouds.push(layer);
    }
    Some(w)
}

/// Fully evaluated sky state for one moment.
#[derive(Debug, Clone)]
pub struct SkyState {
    pub sky_upper: Vec3,
    pub sky_lower: Vec3,
    pub horizon: Vec3,
    pub fog_near_color: Vec3,
    pub fog_far_color: Vec3,
    pub fog_near: f32,
    pub fog_far: f32,
    pub fog_power: f32,
    pub fog_max: f32,
    pub sunlight: Vec3,
    pub sun_color: Vec3,
    pub ambient: Vec3,
    /// Lighting for effects (the precipitation's colour).
    pub effect_lighting: Vec3,
    /// The colour sky statics are drawn in.
    pub sky_statics: Vec3,
    pub dalc: [Vec3; 6],
    pub sun_dir: Vec3,
    pub light_dir: Vec3,
    pub sun_visible: f32,
    pub stars: f32,
    pub clouds: Vec<CloudState>,
    /// The outgoing weather's layers fading out under them in a transition.
    pub outgoing_clouds: Vec<CloudState>,
}

impl SkyState {
    /// Part way (`t`) from the outgoing weather's sky `a` to the incoming `b`:
    /// colours and fog mix, `a`'s clouds fade out as `b`'s fade in.
    pub fn blend(a: &SkyState, b: &SkyState, t: f32) -> SkyState {
        let t = t.clamp(0.0, 1.0);
        let v = |x: Vec3, y: Vec3| x.lerp(y, t);
        let f = |x: f32, y: f32| x + (y - x) * t;
        let fade = |c: &[CloudState], k: f32| -> Vec<CloudState> {
            c.iter()
                .map(|l| CloudState {
                    alpha: l.alpha * k,
                    ..l.clone()
                })
                .collect()
        };
        SkyState {
            sky_upper: v(a.sky_upper, b.sky_upper),
            sky_lower: v(a.sky_lower, b.sky_lower),
            horizon: v(a.horizon, b.horizon),
            fog_near_color: v(a.fog_near_color, b.fog_near_color),
            fog_far_color: v(a.fog_far_color, b.fog_far_color),
            fog_near: f(a.fog_near, b.fog_near),
            fog_far: f(a.fog_far, b.fog_far),
            fog_power: f(a.fog_power, b.fog_power),
            fog_max: f(a.fog_max, b.fog_max),
            sunlight: v(a.sunlight, b.sunlight),
            sun_color: v(a.sun_color, b.sun_color),
            ambient: v(a.ambient, b.ambient),
            effect_lighting: v(a.effect_lighting, b.effect_lighting),
            sky_statics: v(a.sky_statics, b.sky_statics),
            dalc: std::array::from_fn(|i| v(a.dalc[i], b.dalc[i])),
            sun_dir: b.sun_dir,
            light_dir: b.light_dir,
            sun_visible: b.sun_visible,
            stars: b.stars,
            clouds: fade(&b.clouds, t),
            outgoing_clouds: fade(&a.clouds, 1.0 - t),
        }
    }
}

/// Blend weights over the 4 time-of-day slots for an hour of the day.
pub fn time_weights(c: &Climate, hour: f32) -> [f32; 4] {
    let mut w = [0.0; 4];
    let (rb, re) = c.sunrise;
    let (sb, se) = c.sunset;
    let rm = (rb + re) * 0.5;
    let sm = (sb + se) * 0.5;
    let lerp2 = |w: &mut [f32; 4], a: usize, b: usize, t: f32| {
        let t = t.clamp(0.0, 1.0);
        w[a] = 1.0 - t;
        w[b] = t;
    };
    if hour < rb || hour >= se {
        w[NIGHT] = 1.0;
    } else if hour < rm {
        lerp2(&mut w, NIGHT, SUNRISE, (hour - rb) / (rm - rb));
    } else if hour < re {
        lerp2(&mut w, SUNRISE, DAY, (hour - rm) / (re - rm));
    } else if hour < sb {
        w[DAY] = 1.0;
    } else if hour < sm {
        lerp2(&mut w, DAY, SUNSET, (hour - sb) / (sm - sb));
    } else {
        lerp2(&mut w, SUNSET, NIGHT, (hour - sm) / (se - sm));
    }
    w
}

pub fn evaluate(w: &Weather, c: &Climate, hour: f32) -> SkyState {
    let tw = time_weights(c, hour);
    let col = |i: usize| -> Vec3 { (0..4).map(|t| w.colors[i][t] * tw[t]).sum() };
    let night = tw[NIGHT];
    let day_factor = 1.0 - night;
    let mix = |d: f32, n: f32| d * day_factor + n * night;

    // Sun path: rises in the east at sunrise mid, sets in the west at sunset mid.
    let rise = (c.sunrise.0 + c.sunrise.1) * 0.5;
    let set = (c.sunset.0 + c.sunset.1) * 0.5;
    let t = (hour - rise) / (set - rise);
    let ang = t * std::f32::consts::PI;
    let sun_dir = Vec3::new(ang.cos(), -0.25 * ang.sin(), ang.sin()).normalize();
    // At night the moons light the world from roughly the opposite path.
    let light_dir = if sun_dir.z > 0.0 {
        sun_dir
    } else {
        let mt = ((hour - set).rem_euclid(24.0)) / (24.0 - (set - rise));
        let ma = mt * std::f32::consts::PI;
        Vec3::new(ma.cos(), 0.25 * ma.sin(), ma.sin().max(0.2)).normalize()
    };
    let mut dalc = [Vec3::ZERO; 6];
    for (a, d) in dalc.iter_mut().enumerate() {
        *d = (0..4).map(|t| w.dalc[t][a] * tw[t]).sum();
    }
    let clouds = w
        .clouds
        .iter()
        .map(|l| {
            let color: Vec3 = (0..4).map(|t| l.colors[t] * tw[t]).sum();
            let alpha: f32 = (0..4).map(|t| l.alphas[t] * tw[t]).sum();
            CloudState {
                texture: l.texture.clone(),
                color,
                alpha,
                speed: l.speed,
            }
        })
        .collect();
    SkyState {
        sky_upper: col(color::SKY_UPPER),
        sky_lower: col(color::SKY_LOWER),
        horizon: col(color::HORIZON),
        fog_near_color: col(color::FOG_NEAR),
        fog_far_color: col(color::FOG_FAR),
        fog_near: mix(w.fog[0], w.fog[2]),
        fog_far: mix(w.fog[1], w.fog[3]),
        fog_power: mix(w.fog[4], w.fog[5]).max(0.01),
        fog_max: mix(w.fog[6], w.fog[7]),
        sunlight: col(color::SUNLIGHT),
        sun_color: col(color::SUN),
        ambient: col(color::AMBIENT),
        effect_lighting: col(color::EFFECT_LIGHTING),
        sky_statics: col(color::SKY_STATICS),
        dalc,
        sun_dir,
        light_dir,
        sun_visible: (sun_dir.z * 8.0 + 0.5).clamp(0.0, 1.0),
        stars: night,
        clouds,
        outgoing_clouds: Vec::new(),
    }
}
