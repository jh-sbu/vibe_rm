//! Image space modifiers (IMAD) on the player's view, screen fades
//! (`Game.FadeOutGame`) and camera shakes (`Game.ShakeCamera`).
//!
//! An IMAD's DNAM holds whether it animates, its duration and its keys' counts
//! (CommonLibSSE's `ImageSpaceModifierData`); its curves are subrecords of
//! (time, value) keys, time 0..1 over the duration: `<i>IAD` the multiplier
//! and `<0x40 + i>IAD` the addend of value `i` (saturation 17, brightness 18,
//! contrast 19), `BNAM` blur, `VNAM` double vision, and colour curves of
//! (time, r, g, b, amount) keys: `TNAM` tint, `NAM3` fade. Each value is the
//! base image space's times the multiplier plus the addend.
//!
//! The base image space (IMGS: CommonLibSSE's `ImageSpaceBaseData`) is the
//! interior cell's (XCIM, else `DefaultImageSpaceInterior`) or outdoors the
//! weather's for the time of day (IMSP: sunrise, day, sunset, night, blended
//! like its colours, and with the outgoing weather's in a transition). Its cinematic values (CNAM) and tint (TNAM) are drawn;
//! the HDR ones (HNAM) are kept but there is no HDR to drive.
//! Open questions: `known_gaps/image-space.md`.

use std::collections::HashMap;
use std::sync::Arc;

use esp::FormId;
use glam::Vec3;

use crate::engine::Engine;
use crate::render::post::PostEffect;

/// (time 0..1, value) keys.
#[derive(Debug, Clone, Default)]
struct Curve(Vec<(f32, f32)>);

impl Curve {
    fn parse(d: &[u8]) -> Self {
        let f = |o: usize| f32::from_le_bytes(d[o..o + 4].try_into().unwrap());
        Curve(
            d.chunks_exact(8)
                .enumerate()
                .map(|(i, _)| (f(i * 8), f(i * 8 + 4)))
                .collect(),
        )
    }

    fn at(&self, t: f32, default: f32) -> f32 {
        let k = &self.0;
        match k.iter().position(|&(kt, _)| kt > t) {
            None => k.last().map_or(default, |p| p.1),
            Some(0) => k[0].1,
            Some(i) => {
                let ((t0, v0), (t1, v1)) = (k[i - 1], k[i]);
                v0 + (v1 - v0) * ((t - t0) / (t1 - t0).max(1e-6))
            }
        }
    }
}

/// (time 0..1, [r, g, b, amount]) keys.
#[derive(Debug, Clone, Default)]
struct ColorCurve(Vec<(f32, [f32; 4])>);

impl ColorCurve {
    fn parse(d: &[u8]) -> Self {
        let f = |o: usize| f32::from_le_bytes(d[o..o + 4].try_into().unwrap());
        ColorCurve(
            d.chunks_exact(20)
                .enumerate()
                .map(|(i, _)| {
                    let o = i * 20;
                    (f(o), [f(o + 4), f(o + 8), f(o + 12), f(o + 16)])
                })
                .collect(),
        )
    }

    fn at(&self, t: f32) -> [f32; 4] {
        let k = &self.0;
        match k.iter().position(|&(kt, _)| kt > t) {
            None => k.last().map_or([0.0; 4], |p| p.1),
            Some(0) => k[0].1,
            Some(i) => {
                let ((t0, a), (t1, b)) = (k[i - 1], k[i]);
                let s = (t - t0) / (t1 - t0).max(1e-6);
                std::array::from_fn(|j| a[j] + (b[j] - a[j]) * s)
            }
        }
    }
}

/// The cinematic values a modifier drives, by curve index.
const SATURATION: u8 = 17;
const BRIGHTNESS: u8 = 18;
const CONTRAST: u8 = 19;

/// An image space modifier's record.
#[derive(Debug, Default)]
pub struct Imad {
    pub editor_id: String,
    /// Plays its curves over `duration` and ends; otherwise holds its first
    /// keys (the static values: the rest of the curve goes back to neutral)
    /// until removed.
    pub animatable: bool,
    pub duration: f32,
    /// (multiplier, addend) of saturation, brightness, contrast.
    cinematic: [(Curve, Curve); 3],
    tint: ColorCurve,
    fade: ColorCurve,
    blur: Curve,
    double_vision: Curve,
}

impl Imad {
    fn parse(rec: &esp::LoadedRecord<'_>) -> Self {
        let mut m = Imad {
            editor_id: rec.editor_id().unwrap_or_default(),
            ..Default::default()
        };
        if let Some(d) = rec.get(b"DNAM").filter(|d| d.len() >= 8) {
            m.animatable = d[0] != 0;
            m.duration = f32::from_le_bytes(d[4..8].try_into().unwrap());
        }
        for sr in rec.subrecords() {
            let tag = sr.tag.0;
            match &tag {
                b"TNAM" => m.tint = ColorCurve::parse(sr.data),
                b"NAM3" => m.fade = ColorCurve::parse(sr.data),
                b"BNAM" => m.blur = Curve::parse(sr.data),
                b"VNAM" => m.double_vision = Curve::parse(sr.data),
                [i, b'I', b'A', b'D'] => {
                    let (index, add) = if *i >= 0x40 {
                        (i - 0x40, true)
                    } else {
                        (*i, false)
                    };
                    let slot = match index {
                        SATURATION => 0,
                        BRIGHTNESS => 1,
                        CONTRAST => 2,
                        _ => continue,
                    };
                    let c = Curve::parse(sr.data);
                    if add {
                        m.cinematic[slot].1 = c;
                    } else {
                        m.cinematic[slot].0 = c;
                    }
                }
                _ => {}
            }
        }
        m
    }
}

/// `DefaultImageSpaceInterior`: interiors without an image space of their own.
const DEFAULT_INTERIOR: FormId = FormId(0x160);

/// An image space's record (IMGS), or several blended.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Imgs {
    /// HNAM: eye adapt speed, bloom blur radius, bloom threshold, bloom scale,
    /// receive bloom threshold, white, sunlight scale, sky scale, eye adapt
    /// strength.
    pub hdr: [f32; 9],
    /// CNAM: saturation, brightness, contrast.
    pub cinematic: [f32; 3],
    /// TNAM: amount, then the colour.
    pub tint: [f32; 4],
}

impl Default for Imgs {
    fn default() -> Self {
        Imgs {
            hdr: [0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 0.0],
            cinematic: [1.0; 3],
            tint: [0.0; 4],
        }
    }
}

impl Imgs {
    fn parse(rec: &esp::LoadedRecord<'_>) -> Self {
        fn fill<const N: usize>(out: &mut [f32; N], d: Option<&[u8]>) {
            let Some(d) = d else { return };
            for (v, b) in out.iter_mut().zip(d.chunks_exact(4)) {
                *v = f32::from_le_bytes(b.try_into().unwrap());
            }
        }
        let mut m = Imgs::default();
        fill(&mut m.hdr, rec.get(b"HNAM"));
        fill(&mut m.cinematic, rec.get(b"CNAM"));
        fill(&mut m.tint, rec.get(b"TNAM"));
        m
    }

    /// Image spaces mixed by weight (weights summing to 1). Tints mix as
    /// colour times amount, so one without a tint fades another's out.
    fn blend(parts: &[(Imgs, f32)]) -> Imgs {
        let mut m = Imgs {
            hdr: [0.0; 9],
            cinematic: [0.0; 3],
            tint: [0.0; 4],
        };
        for (p, w) in parts {
            for i in 0..9 {
                m.hdr[i] += p.hdr[i] * w;
            }
            for i in 0..3 {
                m.cinematic[i] += p.cinematic[i] * w;
                m.tint[i + 1] += p.tint[i + 1] * p.tint[0] * w;
            }
            m.tint[0] += p.tint[0] * w;
        }
        if m.tint[0] > 1e-4 {
            for i in 1..4 {
                m.tint[i] /= m.tint[0];
            }
        }
        m
    }
}

/// A modifier applied to the view.
#[derive(Debug, Clone)]
struct Active {
    imad: FormId,
    /// Seconds since applied.
    t: f32,
    strength: f32,
    /// Fading in over this many seconds (`ApplyCrossFade`).
    fade_in: f32,
    /// Fading out: (seconds since it began, over how long).
    fade_out: Option<(f32, f32)>,
    /// The cross-fade modifier (there is one at most).
    cross_fade: bool,
}

impl Active {
    fn weight(&self) -> f32 {
        let mut w = self.strength;
        if self.fade_in > 0.0 {
            w *= (self.t / self.fade_in).min(1.0);
        }
        if let Some((t, d)) = self.fade_out {
            w *= 1.0 - (t / d.max(1e-3)).min(1.0);
        }
        w
    }
}

/// `Game.FadeOutGame`: fading to (or from) black or white.
#[derive(Debug, Clone, Copy)]
struct GameFade {
    out: bool,
    color: [f32; 3],
    delay: f32,
    duration: f32,
    t: f32,
}

impl GameFade {
    fn progress(&self) -> f32 {
        ((self.t - self.delay) / self.duration.max(1e-3)).clamp(0.0, 1.0)
    }

    fn amount(&self) -> f32 {
        if self.out {
            self.progress()
        } else {
            1.0 - self.progress()
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Shake {
    strength: f32,
    duration: f32,
    t: f32,
}

/// How long a shake given no duration lasts (the game's default isn't in its
/// data).
const DEFAULT_SHAKE_SECONDS: f32 = 1.0;
/// A shake from a source fades out to nothing this far away (units).
const SHAKE_RANGE: f32 = 4096.0;

#[derive(Default)]
pub struct ImageSpace {
    records: HashMap<FormId, Option<Arc<Imad>>>,
    image_spaces: HashMap<FormId, Option<Imgs>>,
    /// The interior's image space (XCIM); outdoors it follows the weather.
    pub interior: Option<FormId>,
    /// The base image space this frame.
    pub base: Imgs,
    active: Vec<Active>,
    fade: Option<GameFade>,
    shakes: Vec<Shake>,
    /// The view turn the shake added last frame (yaw, pitch), taken back first.
    shake_turn: (f32, f32),
    shake_time: f32,
}

impl Engine {
    fn imad(&mut self, f: FormId) -> Option<Arc<Imad>> {
        if let Some(m) = self.imagespace.records.get(&f) {
            return m.clone();
        }
        let m = self
            .lo
            .get(f)
            .filter(|r| r.tag().0 == *b"IMAD")
            .map(|r| Arc::new(Imad::parse(&r)));
        self.imagespace.records.insert(f, m.clone());
        m
    }

    fn imgs(&mut self, f: FormId) -> Option<Imgs> {
        if let Some(m) = self.imagespace.image_spaces.get(&f) {
            return *m;
        }
        let m = self
            .lo
            .get(f)
            .filter(|r| r.tag().0 == *b"IMGS")
            .map(|r| Imgs::parse(&r));
        self.imagespace.image_spaces.insert(f, m);
        m
    }

    /// Entering an interior: its image space from now on (null for the
    /// default one).
    pub(crate) fn set_interior_image_space(&mut self, f: Option<FormId>) {
        self.imagespace.interior = f.map(|f| if f.is_null() { DEFAULT_INTERIOR } else { f });
    }

    /// The interior's image space, or the weather's at this hour.
    fn base_image_space(&mut self) -> Imgs {
        if let Some(f) = self.imagespace.interior {
            return self
                .imgs(f)
                .or_else(|| self.imgs(DEFAULT_INTERIOR))
                .unwrap_or_default();
        }
        let Some(c) = self.climate() else {
            return Imgs::default();
        };
        let tw = crate::world::weather::time_weights(c, self.hour);
        let mut parts: Vec<(Imgs, f32)> = Vec::new();
        for (slots, weight) in self.weather_image_spaces() {
            for t in (0..4).filter(|&t| tw[t] > 0.0) {
                let m = self.imgs(slots[t]).unwrap_or_default();
                parts.push((m, tw[t] * weight));
            }
        }
        Imgs::blend(&parts)
    }

    /// `ImageSpaceModifier.Apply`: put a modifier on the view at `strength`
    /// (again from the start if it's on already).
    pub fn apply_imod(&mut self, f: FormId, strength: f32) {
        let Some(m) = self.imad(f) else { return };
        log::info!("image space modifier {} on ({strength})", m.editor_id);
        let is = &mut self.imagespace;
        is.active.retain(|a| a.imad != f);
        is.active.push(Active {
            imad: f,
            t: 0.0,
            strength,
            fade_in: 0.0,
            fade_out: None,
            cross_fade: false,
        });
    }

    /// `ImageSpaceModifier.ApplyCrossFade`: the modifier fades in over `secs`
    /// while the previous cross-fade one fades out.
    pub fn apply_imod_cross_fade(&mut self, f: FormId, secs: f32) {
        let Some(m) = self.imad(f) else { return };
        log::info!(
            "image space modifier {} cross-fading in over {secs}s",
            m.editor_id
        );
        self.remove_imod_cross_fade(secs);
        let is = &mut self.imagespace;
        is.active.retain(|a| a.imad != f);
        is.active.push(Active {
            imad: f,
            t: 0.0,
            strength: 1.0,
            fade_in: secs.max(0.0),
            fade_out: None,
            cross_fade: true,
        });
    }

    /// `ImageSpaceModifier.RemoveCrossFade`: the cross-fade modifier fades out.
    pub fn remove_imod_cross_fade(&mut self, secs: f32) {
        for a in &mut self.imagespace.active {
            if a.cross_fade && a.fade_out.is_none() {
                a.fade_out = Some((0.0, secs.max(0.0)));
            }
        }
        self.imagespace
            .active
            .retain(|a| a.fade_out.is_none_or(|(_, d)| d > 0.0));
    }

    /// `ImageSpaceModifier.Remove`.
    pub fn remove_imod(&mut self, f: FormId) {
        self.imagespace.active.retain(|a| a.imad != f);
    }

    /// `Game.FadeOutGame(abFadingOut, abBlackFade, afSecsBeforeFade,
    /// afFadeDuration)`: fade to black (or white) after a delay and stay faded;
    /// fading in, the screen starts faded and clears.
    pub fn fade_out_game(&mut self, out: bool, black: bool, delay: f32, duration: f32) {
        log::info!(
            "fading {} {} ({delay}s, then {duration}s)",
            if out { "out to" } else { "in from" },
            if black { "black" } else { "white" }
        );
        self.imagespace.fade = Some(GameFade {
            out,
            color: if black { [0.0; 3] } else { [1.0; 3] },
            delay: delay.max(0.0),
            duration: duration.max(0.0),
            t: 0.0,
        });
    }

    /// `Game.ShakeCamera(akSource, afStrength, afDuration)`: a shake from a
    /// source is weaker the farther the player is from it.
    pub fn shake_camera(&mut self, source: Option<FormId>, strength: f32, duration: f32) {
        let mut strength = strength.clamp(0.0, 1.0);
        if let Some(p) = source.and_then(|s| self.ref_position(s)) {
            strength *= (1.0 - p.distance(self.player.position) / SHAKE_RANGE).max(0.0);
        }
        if strength <= 0.0 {
            return;
        }
        let duration = if duration > 0.0 {
            duration
        } else {
            DEFAULT_SHAKE_SECONDS
        };
        self.imagespace.shakes.push(Shake {
            strength,
            duration,
            t: 0.0,
        });
    }

    /// Advance the modifiers, fades and shakes by `dt` and set the frame's
    /// effects.
    pub(crate) fn update_imagespace(&mut self, dt: f32) {
        let base = self.base_image_space();
        self.imagespace.base = base;
        let mut fx = PostEffect {
            saturation: base.cinematic[0],
            brightness: base.cinematic[1],
            contrast: base.cinematic[2],
            ..Default::default()
        };
        let mut active = std::mem::take(&mut self.imagespace.active);
        active.retain_mut(|a| {
            a.t += dt;
            if let Some((t, _)) = &mut a.fade_out {
                *t += dt;
            }
            a.fade_out.is_none_or(|(t, d)| t < d)
        });
        let mut keep = Vec::with_capacity(active.len());
        let mut tint: Vec<[f32; 4]> =
            vec![[base.tint[1], base.tint[2], base.tint[3], base.tint[0]]];
        let mut fades: Vec<[f32; 4]> = Vec::new();
        for a in active {
            let Some(m) = self.imad(a.imad) else { continue };
            let s = a.weight();
            let t = if !m.animatable {
                0.0
            } else if m.duration > 0.0 {
                (a.t / m.duration).min(1.0)
            } else {
                1.0
            };
            let values = [&mut fx.saturation, &mut fx.brightness, &mut fx.contrast];
            for (v, (mult, add)) in values.into_iter().zip(&m.cinematic) {
                let mult = 1.0 + (mult.at(t, 1.0) - 1.0) * s;
                *v = *v * mult + add.at(t, 0.0) * s;
            }
            let mut c = m.tint.at(t);
            c[3] *= s;
            tint.push(c);
            let mut c = m.fade.at(t);
            c[3] *= s;
            fades.push(c);
            fx.blur = fx.blur.max(m.blur.at(t, 0.0) * s);
            fx.double_vision = fx.double_vision.max(m.double_vision.at(t, 0.0) * s);
            if !(m.animatable && a.t >= m.duration) {
                keep.push(a);
            }
        }
        self.imagespace.active = keep;
        if let Some(f) = &mut self.imagespace.fade {
            f.t += dt;
            fades.push([f.color[0], f.color[1], f.color[2], f.amount()]);
            if !f.out && f.progress() >= 1.0 {
                self.imagespace.fade = None;
            }
        }
        fx.tint = layer(&tint, [1.0, 1.0, 1.0, 0.0]);
        fx.fade = layer(&fades, [0.0; 4]);
        self.scene.post = fx;
        self.update_shake(dt);
    }

    /// Take back last frame's shake and turn the view by this frame's.
    fn update_shake(&mut self, dt: f32) {
        let is = &mut self.imagespace;
        self.camera.yaw -= is.shake_turn.0;
        self.camera.pitch -= is.shake_turn.1;
        is.shake_turn = (0.0, 0.0);
        is.shake_time += dt;
        is.shakes.retain_mut(|s| {
            s.t += dt;
            s.t < s.duration
        });
        let amp = is
            .shakes
            .iter()
            .map(|s| s.strength * (1.0 - s.t / s.duration))
            .fold(0.0f32, f32::max);
        if amp <= 0.0 {
            return;
        }
        // Up to about 3 degrees at full strength, wobbling a few times a second.
        let t = is.shake_time;
        let a = amp * 0.05;
        let yaw = a * ((t * 23.0).sin() * 0.6 + (t * 37.0).sin() * 0.4);
        let pitch = a * ((t * 29.0).sin() * 0.6 + (t * 41.0 + 1.3).sin() * 0.4);
        is.shake_turn = (yaw, pitch);
        self.camera.yaw += yaw;
        self.camera.pitch += pitch;
        self.camera.position += Vec3::new(0.0, 0.0, amp * 2.0 * (t * 31.0).sin());
    }

    /// Console `imods`: the modifiers on the view, fades and shakes.
    pub fn describe_imagespace(&self) -> Vec<String> {
        let is = &self.imagespace;
        let mut out: Vec<String> = is
            .active
            .iter()
            .map(|a| {
                let m = is.records.get(&a.imad).cloned().flatten();
                format!(
                    "{} {}: {:.2}s of {:.2}s{}, weight {:.2}{}",
                    a.imad,
                    m.as_ref().map_or("?", |m| m.editor_id.as_str()),
                    a.t,
                    m.as_ref().map_or(0.0, |m| m.duration),
                    if m.as_ref().is_some_and(|m| m.animatable) {
                        ""
                    } else {
                        " (held)"
                    },
                    a.weight(),
                    if a.cross_fade { ", cross-fade" } else { "" }
                )
            })
            .collect();
        if let Some(f) = &is.fade {
            out.push(format!(
                "game fade {}: {:.2}s, amount {:.2}",
                if f.out { "out" } else { "in" },
                f.t,
                f.amount()
            ));
        }
        for s in &is.shakes {
            out.push(format!(
                "shake {:.2}: {:.2}s of {:.2}s",
                s.strength, s.t, s.duration
            ));
        }
        let b = &is.base;
        out.push(format!(
            "base: saturation {:.2} brightness {:.2} contrast {:.2} tint {:?} hdr {:?}{}",
            b.cinematic[0],
            b.cinematic[1],
            b.cinematic[2],
            b.tint,
            b.hdr,
            match is.interior {
                Some(f) => format!(" (interior {f})"),
                None => " (weather)".into(),
            }
        ));
        let p = &self.scene.post;
        out.push(format!(
            "frame: saturation {:.2} brightness {:.2} contrast {:.2} tint {:?} fade {:?} blur {:.2} double vision {:.2}",
            p.saturation, p.brightness, p.contrast, p.tint, p.fade, p.blur, p.double_vision
        ));
        out
    }
}

/// Colours laid one over another (each `[r, g, b, amount]`, in order) as one.
fn layer(layers: &[[f32; 4]], none: [f32; 4]) -> [f32; 4] {
    let mut rgb = [0.0f32; 3];
    let mut clear = 1.0f32;
    for l in layers {
        let a = l[3].clamp(0.0, 1.0);
        for j in 0..3 {
            rgb[j] = rgb[j] * (1.0 - a) + l[j] * a;
        }
        clear *= 1.0 - a;
    }
    let amount = 1.0 - clear;
    if amount < 1e-4 {
        return none;
    }
    [rgb[0] / amount, rgb[1] / amount, rgb[2] / amount, amount]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curves_interpolate_and_hold_their_ends() {
        let c = Curve(vec![(0.0, 0.0), (0.5, 1.0), (1.0, 1.0)]);
        assert_eq!(c.at(-1.0, 9.0), 0.0);
        assert!((c.at(0.25, 9.0) - 0.5).abs() < 1e-6);
        assert_eq!(c.at(2.0, 9.0), 1.0);
        assert_eq!(Curve::default().at(0.5, 9.0), 9.0);
    }

    #[test]
    fn blended_image_spaces_fade_tints_by_amount() {
        let a = Imgs {
            tint: [0.5, 1.0, 0.0, 0.0],
            cinematic: [2.0, 1.0, 1.0],
            ..Default::default()
        };
        let m = Imgs::blend(&[(a, 0.5), (Imgs::default(), 0.5)]);
        assert!((m.cinematic[0] - 1.5).abs() < 1e-6);
        assert!((m.tint[0] - 0.25).abs() < 1e-6);
        assert!((m.tint[1] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn layered_colours_compose_like_one_over_another() {
        // Black at half, then white at half: a quarter black, half white.
        let l = layer(&[[0.0, 0.0, 0.0, 0.5], [1.0, 1.0, 1.0, 0.5]], [0.0; 4]);
        assert!((l[3] - 0.75).abs() < 1e-6);
        assert!((l[0] - 0.5 / 0.75).abs() < 1e-6);
        assert_eq!(layer(&[], [1.0, 1.0, 1.0, 0.0]), [1.0, 1.0, 1.0, 0.0]);
    }
}
