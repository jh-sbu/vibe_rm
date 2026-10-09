//! Grass: the landscape textures' grasses (LTEX `GNAM` -> GRAS) scattered
//! over a cell's landscape where those textures show.
//!
//! Candidates lie on a grid (`iMinGrassSize` units apart). At each one the
//! texture showing most picks its grasses; each grass is placed there with its
//! density's chance, when the ground's slope and the water allow, offset by up
//! to its position range, scaled by up to its height range and tinted by the
//! landscape's vertex colour (darkened by up to its colour range). The game's
//! own scattering isn't public; see `known_gaps/grass.md`.

use std::collections::HashMap;

use esp::{FormId, LoadOrder};
use glam::{Vec2, Vec3};

use super::terrain::{CELL_SIZE, Land, VERTS};

/// Units between grass candidates (`iMinGrassSize`).
pub const GRID: f32 = 20.0;
/// Where grass starts fading out and how far it fades (`fGrassStartFadeDistance`
/// at High / Ultra, and a fade range of this engine's choosing).
pub const FADE_START: f32 = 7000.0;
pub const FADE_RANGE: f32 = 1000.0;

/// How strongly grass sways in a wind (units a second): a little in still air,
/// fully in a gale.
pub fn wind_strength(wind: Vec2) -> f32 {
    0.25 + (wind.length() / 1000.0).clamp(0.0, 1.0)
}

/// Waves a second running over grass in a wind. The shader's phase is the
/// integral of this (`Renderer::grass_clock`), so changing winds change the
/// rate smoothly rather than jumping the phase.
pub fn wave_rate(wind: Vec2) -> f32 {
    0.5 + 0.5 * wind_strength(wind)
}

/// `DATA` flags (bit 0, vertex lighting, isn't used).
pub mod flags {
    pub const UNIFORM_SCALING: u8 = 2;
    pub const FIT_TO_SLOPE: u8 = 4;
}

/// A grass (GRAS): its model and how it's scattered (`DATA`).
#[derive(Debug, Clone)]
pub struct Grass {
    pub model: String,
    /// Percent of candidates it's placed at.
    pub density: u8,
    /// The ground's slope it grows on, in degrees.
    pub min_slope: u8,
    pub max_slope: u8,
    /// Units from water, and how they apply ([`Grass::water_ok`]).
    pub water_distance: u16,
    pub water_type: u32,
    pub position_range: f32,
    pub height_range: f32,
    pub color_range: f32,
    /// How long the waves blowing over it are, in units (0: it doesn't sway).
    pub wave_period: f32,
    pub flags: u8,
}

impl Grass {
    pub fn load(lo: &LoadOrder, id: FormId) -> Option<Grass> {
        let rec = lo.get(id)?;
        let model = super::records::model_path(&rec)?;
        let d = rec.get(b"DATA")?;
        if d.len() < 29 {
            return None;
        }
        let f = |i: usize| f32::from_le_bytes(d[i..i + 4].try_into().unwrap());
        Some(Grass {
            model,
            density: d[0],
            min_slope: d[1],
            max_slope: d[2],
            water_distance: u16::from_le_bytes([d[4], d[5]]),
            water_type: u32::from_le_bytes(d[8..12].try_into().unwrap()),
            position_range: f(12),
            height_range: f(16),
            color_range: f(20),
            wave_period: f(24),
            flags: d[28],
        })
    }

    /// Whether it grows at `height` with the water's surface at `water` (none
    /// in the cell: there's no water to be above or below).
    /// Types: 0 above at least, 1 above at most, 2 below at least, 3 below at
    /// most, 4 either at least, 5 either at most, 6 either at most above, 7
    /// either at most below the given distance.
    pub fn water_ok(&self, height: f32, water: Option<f32>) -> bool {
        let n = self.water_distance as f32;
        let Some(w) = water else {
            return matches!(self.water_type, 0 | 4 | 5 | 6 | 7);
        };
        let above = height - w;
        match self.water_type {
            0 => above >= n,
            1 => above >= 0.0 && above <= n,
            2 => -above >= n,
            3 => above <= 0.0 && -above <= n,
            4 => above.abs() >= n,
            5 => above.abs() <= n,
            6 => above <= n,
            7 => -above <= n,
            _ => true,
        }
    }
}

/// The grasses each landscape texture lists, loaded as they're asked for.
#[derive(Default)]
pub struct GrassTypes {
    by_ltex: HashMap<FormId, Vec<usize>>,
    pub grasses: Vec<Grass>,
    index: HashMap<FormId, usize>,
}

impl GrassTypes {
    /// Indices into [`GrassTypes::grasses`] of a landscape texture's grasses.
    pub fn of(&mut self, lo: &LoadOrder, ltex: FormId) -> &[usize] {
        if !self.by_ltex.contains_key(&ltex) {
            let mut v = Vec::new();
            if let Some(rec) = lo.get(ltex) {
                for s in rec.subrecords().filter(|s| s.tag.0 == *b"GNAM") {
                    let id = rec.fid(s.form_id(0));
                    let i = match self.index.get(&id) {
                        Some(&i) => Some(i),
                        None => Grass::load(lo, id).map(|g| {
                            self.grasses.push(g);
                            self.index.insert(id, self.grasses.len() - 1);
                            self.grasses.len() - 1
                        }),
                    };
                    v.extend(i);
                }
            }
            self.by_ltex.insert(ltex, v);
        }
        &self.by_ltex[&ltex]
    }
}

/// One grass placed: where, its up axis (the ground's normal when fitted to
/// the slope), turn about it, scale (across, up), tint and wave period.
#[derive(Debug, Clone, Copy)]
pub struct Blade {
    pub pos: Vec3,
    pub up: Vec3,
    pub yaw: f32,
    pub scale: Vec2,
    pub tint: Vec3,
    pub wave: f32,
}

/// A hash of a candidate and a grass to a value in [0, 1).
fn rand(cell: (i32, i32), i: u32, j: u32, k: u32) -> f32 {
    let mut h = (cell.0 as u32).wrapping_mul(0x9E37_79B1)
        ^ (cell.1 as u32).wrapping_mul(0x85EB_CA77)
        ^ i.wrapping_mul(0xC2B2_AE3D)
        ^ j.wrapping_mul(0x27D4_EB2F)
        ^ k.wrapping_mul(0x1656_67B1);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    (h >> 8) as f32 / (1u32 << 24) as f32
}

/// The landscape's vertex colour at a point of the cell, bilinearly.
fn color_at(land: &Land, p: Vec2) -> Vec3 {
    let l = ((p - land.origin()) / (CELL_SIZE / 32.0)).clamp(Vec2::ZERO, Vec2::splat(31.999));
    let (ix, iy) = (l.x as usize, l.y as usize);
    let (fx, fy) = (l.x - ix as f32, l.y - iy as f32);
    let c = |x: usize, y: usize| land.colors[y * VERTS + x];
    let top = c(ix, iy).lerp(c(ix + 1, iy), fx);
    let bottom = c(ix, iy + 1).lerp(c(ix + 1, iy + 1), fx);
    top.lerp(bottom, fy)
}

/// The ground's normal at a point of the cell, from its heights.
fn normal_at(land: &Land, p: Vec2) -> Vec3 {
    let d = 8.0;
    let lo = land.origin() + Vec2::splat(0.01);
    let hi = land.origin() + Vec2::splat(CELL_SIZE - 0.01);
    let h = |q: Vec2| land.height_at(q.clamp(lo, hi));
    let dx = h(p + Vec2::X * d) - h(p - Vec2::X * d);
    let dy = h(p + Vec2::Y * d) - h(p - Vec2::Y * d);
    Vec3::new(-dx, -dy, 2.0 * d).normalize()
}

/// The grass over a cell's landscape, by grass ([`GrassTypes::grasses`] index).
pub fn scatter(
    lo: &LoadOrder,
    types: &mut GrassTypes,
    land: &Land,
    water: Option<f32>,
) -> HashMap<usize, Vec<Blade>> {
    let mut out: HashMap<usize, Vec<Blade>> = HashMap::new();
    let n = (CELL_SIZE / GRID) as u32;
    let cell = (land.x, land.y);
    let origin = land.origin();
    for j in 0..n {
        for i in 0..n {
            let p = origin + (Vec2::new(i as f32, j as f32) + 0.5) * GRID;
            let Some(ltex) = land.texture_at(p) else {
                continue;
            };
            let grasses = types.of(lo, ltex).to_vec();
            for (k, gi) in grasses.into_iter().enumerate() {
                let g = &types.grasses[gi];
                let r = |s: u32| rand(cell, i, j, k as u32 * 8 + s);
                if r(0) * 100.0 >= g.density as f32 {
                    continue;
                }
                let offset = Vec2::new(r(1) * 2.0 - 1.0, r(2) * 2.0 - 1.0) * g.position_range;
                let q = (p + offset).clamp(origin, origin + Vec2::splat(CELL_SIZE - 0.01));
                let normal = normal_at(land, q);
                let slope = normal.z.clamp(-1.0, 1.0).acos().to_degrees();
                if slope < g.min_slope as f32 || slope > g.max_slope as f32 {
                    continue;
                }
                let z = land.height_at(q);
                if !g.water_ok(z, water) {
                    continue;
                }
                let s = 1.0 + (r(3) * 2.0 - 1.0) * g.height_range;
                let scale = if g.flags & flags::UNIFORM_SCALING != 0 {
                    Vec2::splat(s)
                } else {
                    Vec2::new(1.0, s)
                };
                let up = if g.flags & flags::FIT_TO_SLOPE != 0 {
                    normal
                } else {
                    Vec3::Z
                };
                let tint = color_at(land, q) * (1.0 - r(4) * g.color_range);
                out.entry(gi).or_default().push(Blade {
                    pos: q.extend(z),
                    up,
                    yaw: r(5) * std::f32::consts::TAU,
                    scale,
                    tint,
                    wave: g.wave_period,
                });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grass(water_distance: u16, water_type: u32) -> Grass {
        Grass {
            model: String::new(),
            density: 100,
            min_slope: 0,
            max_slope: 90,
            water_distance,
            water_type,
            position_range: 0.0,
            height_range: 0.0,
            color_range: 0.0,
            wave_period: 0.0,
            flags: 0,
        }
    }

    #[test]
    fn water_types() {
        // Kelp: at least 130 below the surface.
        let kelp = grass(130, 2);
        assert!(kelp.water_ok(-200.0, Some(0.0)));
        assert!(!kelp.water_ok(-100.0, Some(0.0)));
        assert!(!kelp.water_ok(-200.0, None));
        // Field grass: anywhere above the water, or where there's none.
        let field = grass(0, 0);
        assert!(field.water_ok(5.0, Some(0.0)));
        assert!(!field.water_ok(-5.0, Some(0.0)));
        assert!(field.water_ok(-5000.0, None));
    }

    #[test]
    fn rand_is_spread() {
        let v: Vec<f32> = (0..1000).map(|i| rand((3, -7), i, 5, 0)).collect();
        assert!(v.iter().all(|&x| (0.0..1.0).contains(&x)));
        let mean = v.iter().sum::<f32>() / v.len() as f32;
        assert!((mean - 0.5).abs() < 0.05, "{mean}");
    }
}
