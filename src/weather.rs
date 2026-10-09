//! The weather over time: which weather is on, changing to another, regions'
//! weathers and scripts setting them (`Weather.SetActive`, `ForceActive`...).
//!
//! The sky holds a current weather and, while one comes in, the outgoing one
//! and how far the transition has come (CommonLibSSE's `Sky`: `currentWeather`,
//! `lastWeather`, `currentWeatherPct`, `overrideWeather`). The sky, light, fog
//! and image space mix the two by that. A weather comes in at its DATA "trans
//! delta" (`fWeatherTransAccel` times faster when accelerated). Outdoors, the
//! weathers come from the player's cell's regions (XCLR; the highest priority
//! region with weather data, RDAT type 3, its RDWT list of weather, chance and
//! global), else the climate's (WLST), picked by chance; leaving a region
//! whose weathers don't include the current one brings in one of the new
//! region's, and every few hours the region's weather is rolled again. An
//! override (`abOverride`) holds the weather until `ReleaseOverride`. Indoors
//! the weather goes on but no sky shows. Outdoors the weathers' sounds
//! (SNAM: rain, wind...) loop at their share of the transition, and thunder
//! rolls now and then while it's on. Open questions: `known_gaps/weather.md`.

use std::collections::HashMap;
use std::sync::Arc;

use esp::FormId;

use crate::engine::{Engine, Location};
use crate::render::Environment;
use crate::world::loader;
use crate::world::weather::{self, Climate, SkyState, Weather};

/// Game hours between rolls of a region's weather (made up: the game's
/// interval isn't in its data).
const ROLL_HOURS: (f32, f32) = (2.0, 6.0);
/// Seconds between thunder at a weather's thunder frequency 0 and 255 (made
/// up: the storms have 246, Storm Call's weather 15).
const THUNDER_SECONDS: (f32, f32) = (5.0, 30.0);
/// A weather's trans delta is the thousandths of a transition per game minute
/// (made up: 125, the usual value, takes 8 game minutes).
const TRANS_DELTA_PER_MINUTE: f32 = 1.0 / 1000.0;

#[derive(Default)]
pub struct WeatherState {
    /// The worldspace's climate (kept indoors).
    pub climate: Option<Climate>,
    pub current: Option<Arc<Weather>>,
    /// The weather going out while `current` comes in.
    pub outgoing: Option<Arc<Weather>>,
    /// How far `current` has come in (1: all the way).
    pub pct: f32,
    accelerate: bool,
    /// Set with `abOverride`: no regional changes until `ReleaseOverride`.
    pub overridden: bool,
    /// The cell whose regions gave `list`, and the region.
    region_cell: Option<FormId>,
    pub region: Option<FormId>,
    /// The weathers to pick from: (weather, chance, global giving the chance).
    list: Vec<(FormId, u32, FormId)>,
    /// Game hours until the weather is rolled again.
    next_roll: f32,
    records: HashMap<FormId, Option<Arc<Weather>>>,
    /// The weathers the sky's textures are for (current, outgoing).
    textures: Option<(FormId, Option<FormId>)>,
    /// Under the sky (an exterior).
    pub outdoors: bool,
    /// Weather sounds looping: (weather, sound, voice).
    loops: Vec<(FormId, FormId, crate::audio::VoiceId)>,
    /// Seconds until the next thunder, while there is thunder.
    thunder_in: Option<f32>,
}

/// The regional weather list of a region record, if it has one: (priority,
/// entries).
fn region_weathers(rec: &esp::LoadedRecord<'_>) -> Option<(u8, Vec<(FormId, u32, FormId)>)> {
    let mut found = None;
    let mut in_weather = false;
    for sr in rec.subrecords() {
        match &sr.tag.0 {
            b"RDAT" => {
                in_weather = sr.u32(0) == 3;
                if in_weather {
                    found = Some((sr.data.get(5).copied().unwrap_or(0), Vec::new()));
                }
            }
            b"RDWT" if in_weather => {
                if let Some((_, list)) = &mut found {
                    for e in sr.data.chunks_exact(12) {
                        let id = |o: usize| {
                            rec.fid(FormId(u32::from_le_bytes(e[o..o + 4].try_into().unwrap())))
                        };
                        let chance = u32::from_le_bytes(e[4..8].try_into().unwrap());
                        list.push((id(0), chance, id(8)));
                    }
                }
            }
            _ => {}
        }
    }
    found.filter(|(_, l)| !l.is_empty())
}

impl Engine {
    pub(crate) fn weather_record(&mut self, f: FormId) -> Option<Arc<Weather>> {
        if let Some(w) = self.weather.records.get(&f) {
            return w.clone();
        }
        let w = self
            .lo
            .get(f)
            .filter(|r| r.tag().0 == *b"WTHR")
            .and_then(|_| weather::load_weather(&self.lo, f))
            .map(Arc::new);
        self.weather.records.insert(f, w.clone());
        w
    }

    /// The climate's sunrise and sunset, while there is one.
    pub fn climate(&self) -> Option<&Climate> {
        self.weather.climate.as_ref()
    }

    pub fn current_weather(&self) -> Option<FormId> {
        self.weather.current.as_ref().map(|w| w.id)
    }

    pub fn outgoing_weather(&self) -> Option<FormId> {
        self.weather.outgoing.as_ref().map(|w| w.id)
    }

    /// Going outdoors into a worldspace: its climate, and the weather of the
    /// region at `cell` (kept if it belongs there).
    pub fn setup_weather(&mut self, world: FormId, cell: (i32, i32)) {
        let clmt = self.world_form(world, b"CNAM", 0x10);
        self.weather.climate = Some(
            clmt.and_then(|c| weather::load_climate(&self.lo, c))
                .unwrap_or_default(),
        );
        self.weather.outdoors = true;
        self.weather.region_cell = None;
        self.update_region(world, cell, false);
        if let Some(f) = self
            .forced_weather
            .take()
            .and_then(|w| self.resolve_form(&w))
        {
            self.force_weather(f, true);
        }
        if self.weather.current.is_none() {
            let pick = self.pick_weather();
            if let Some(f) = pick {
                self.force_weather(f, false);
            }
        }
        self.sync_sky_textures();
    }

    /// Going indoors: the weather goes on unseen and unheard.
    pub(crate) fn weather_indoors(&mut self) {
        self.weather.outdoors = false;
        self.renderer.sky.disable();
        self.update_weather_sounds(0.0);
    }

    /// The weathers with their shares of the transition: the outgoing one's,
    /// then the current one's.
    fn weather_weights(&self) -> Vec<(Arc<Weather>, f32)> {
        let ws = &self.weather;
        let mut out = Vec::new();
        if let Some(o) = &ws.outgoing {
            out.push((o.clone(), 1.0 - ws.pct));
        }
        if let Some(c) = &ws.current {
            out.push((c.clone(), if ws.outgoing.is_some() { ws.pct } else { 1.0 }));
        }
        out
    }

    /// Loop the weathers' sounds outdoors at their shares of the transition,
    /// and roll thunder now and then while a weather's thunder is on.
    pub(crate) fn update_weather_sounds(&mut self, dt: f32) {
        let outdoors = self.weather.outdoors && matches!(self.location, Location::Exterior { .. });
        let weights = if outdoors {
            self.weather_weights()
        } else {
            Vec::new()
        };
        let mut wanted: Vec<(FormId, FormId, f32)> = Vec::new();
        let mut thunder: Option<(Arc<Weather>, f32)> = None;
        let pct = self.weather.pct;
        for (i, (w, weight)) in weights.iter().enumerate() {
            let outgoing = i == 0 && weights.len() == 2;
            for &(snd, ty) in &w.sounds {
                if ty == weather::sound_type::THUNDER {
                    let (begin, end, _) = w.thunder;
                    let on = if outgoing {
                        pct < end as f32 / 255.0
                    } else {
                        pct >= begin as f32 / 255.0
                    };
                    if on {
                        thunder = Some((w.clone(), *weight));
                    }
                } else {
                    wanted.push((w.id, snd, *weight));
                }
            }
        }
        let Some(audio) = self.audio.as_ref() else {
            return;
        };
        // Voices stopped from elsewhere (all sounds stop on a cell change) start again.
        self.weather.loops.retain(|&(w, s, v)| {
            let keep = wanted.iter().any(|e| e.0 == w && e.1 == s) && audio.is_playing(v);
            if !keep {
                audio.stop(v);
            }
            keep
        });
        for (w, snd, weight) in wanted {
            let Some(desc) = self.sound_desc(snd) else {
                continue;
            };
            let volume = desc.volume * weight;
            let playing = self
                .weather
                .loops
                .iter()
                .find(|e| e.0 == w && e.1 == snd)
                .map(|e| e.2);
            let audio = self.audio.as_mut().unwrap();
            match playing {
                Some(v) => audio.set_volume(v, volume),
                None => {
                    if let Some(v) =
                        audio.play(&self.vfs, &desc.files[0], volume, true, None, 0.0, 0.0)
                    {
                        log::debug!("weather sound {snd} looping: {}", desc.files[0]);
                        self.weather.loops.push((w, snd, v));
                    }
                }
            }
        }
        let Some((w, weight)) = thunder else {
            self.weather.thunder_in = None;
            return;
        };
        let mean = THUNDER_SECONDS.0
            + (THUNDER_SECONDS.1 - THUNDER_SECONDS.0) * w.thunder.2 as f32 / 255.0;
        let roll = (self.rand() % 1000) as f32 / 1000.0;
        let next = mean * (0.5 + roll);
        // Coming into thunder, the first roll waits.
        let Some(t) = self.weather.thunder_in.map(|t| t - dt) else {
            self.weather.thunder_in = Some(next);
            return;
        };
        if t > 0.0 {
            self.weather.thunder_in = Some(t);
            return;
        }
        self.weather.thunder_in = Some(next);
        let sounds: Vec<FormId> = w
            .sounds
            .iter()
            .filter(|s| s.1 == weather::sound_type::THUNDER)
            .map(|s| s.0)
            .collect();
        let pick = sounds[(self.rand() % sounds.len() as u64) as usize];
        let Some(desc) = self.sound_desc(pick) else {
            return;
        };
        let file = &desc.files[(self.rand() % desc.files.len() as u64) as usize];
        log::debug!("thunder: {file}");
        if let Some(a) = self.audio.as_mut() {
            a.play(&self.vfs, file, desc.volume * weight, false, None, 0.0, 0.0);
        }
    }

    /// The player's cell's regional weathers. When `change` and the current
    /// weather isn't one of them, one of them comes in.
    fn update_region(&mut self, world: FormId, grid: (i32, i32), change: bool) {
        let Some(cell) = self
            .lo
            .world(world)
            .and_then(|w| w.cells.get(&grid).copied())
        else {
            return;
        };
        if self.weather.region_cell == Some(cell) {
            return;
        }
        self.weather.region_cell = Some(cell);
        let regions: Vec<FormId> = self
            .lo
            .get(cell)
            .and_then(|rec| {
                rec.get(b"XCLR").map(|d| {
                    d.chunks_exact(4)
                        .map(|c| rec.fid(FormId(u32::from_le_bytes(c.try_into().unwrap()))))
                        .collect()
                })
            })
            .unwrap_or_default();
        let mut best: Option<(u8, FormId, Vec<(FormId, u32, FormId)>)> = None;
        for r in regions {
            let Some(rec) = self.lo.get(r) else { continue };
            if let Some((priority, list)) = region_weathers(&rec)
                && best.as_ref().is_none_or(|b| priority > b.0)
            {
                best = Some((priority, r, list));
            }
        }
        let (region, list) = match best {
            Some((_, r, l)) => (Some(r), l),
            None => (
                None,
                self.climate()
                    .map(|c| {
                        c.weathers
                            .iter()
                            .map(|&(w, chance)| (w, chance.max(0) as u32, FormId::NULL))
                            .collect()
                    })
                    .unwrap_or_default(),
            ),
        };
        if region == self.weather.region && !self.weather.list.is_empty() {
            return;
        }
        self.weather.region = region;
        self.weather.list = list;
        let belongs = self
            .current_weather()
            .is_some_and(|c| self.weather.list.iter().any(|e| e.0 == c));
        if change && !belongs && !self.weather.overridden {
            let pick = self.pick_weather();
            if let Some(f) = pick {
                self.set_weather(f, false, false);
            }
        }
    }

    /// A weather from the region's (or climate's) list by chance.
    fn pick_weather(&mut self) -> Option<FormId> {
        let list = self.weather.list.clone();
        self.pick_from(list.into_iter())
    }

    fn pick_from(&mut self, list: impl Iterator<Item = (FormId, u32, FormId)>) -> Option<FormId> {
        let chances: Vec<(FormId, u64)> = list
            .map(|(w, chance, global)| {
                let chance = if global.is_null() {
                    chance as f32
                } else {
                    self.global_value(global)
                };
                (w, chance.max(0.0) as u64)
            })
            .filter(|e| e.1 > 0)
            .collect();
        let total: u64 = chances.iter().map(|e| e.1).sum();
        if total == 0 {
            return None;
        }
        let mut roll = self.rand() % total;
        for (w, c) in chances {
            if roll < c {
                return Some(w);
            }
            roll -= c;
        }
        None
    }

    /// `Weather.ForceActive` (console `fw`): the weather on at once.
    pub fn force_weather(&mut self, f: FormId, overriding: bool) {
        let Some(w) = self.weather_record(f) else {
            return;
        };
        log::info!(
            "weather {} forced{}",
            w.editor_id,
            if overriding { " (override)" } else { "" }
        );
        let ws = &mut self.weather;
        ws.current = Some(w);
        ws.outgoing = None;
        ws.pct = 1.0;
        ws.overridden |= overriding;
        self.schedule_weather_roll();
        self.sync_sky_textures();
    }

    /// `Weather.SetActive` (console `sw`): the weather comes in from the
    /// current one (faster when accelerated).
    pub fn set_weather(&mut self, f: FormId, overriding: bool, accelerate: bool) {
        let Some(w) = self.weather_record(f) else {
            return;
        };
        if self.weather.overridden && !overriding {
            log::info!("weather {} not set: overridden", w.editor_id);
            return;
        }
        self.weather.overridden |= overriding;
        if self.current_weather() == Some(f) {
            return;
        }
        log::info!(
            "weather {} coming in{}{}",
            w.editor_id,
            if overriding { " (override)" } else { "" },
            if accelerate { ", accelerated" } else { "" }
        );
        let ws = &mut self.weather;
        let instant = w.trans_delta == 0 || ws.current.is_none();
        ws.outgoing = ws.current.take().filter(|_| !instant);
        ws.current = Some(w);
        ws.pct = if instant { 1.0 } else { 0.0 };
        ws.accelerate = accelerate;
        self.schedule_weather_roll();
        self.sync_sky_textures();
    }

    /// `Weather.ReleaseOverride`: the region's weathers come back.
    pub fn release_weather_override(&mut self) {
        if std::mem::take(&mut self.weather.overridden) {
            log::info!("weather override released");
            self.weather.next_roll = 0.0;
        }
    }

    fn schedule_weather_roll(&mut self) {
        let t = (self.rand() % 1000) as f32 / 1000.0;
        self.weather.next_roll = ROLL_HOURS.0 + (ROLL_HOURS.1 - ROLL_HOURS.0) * t;
    }

    /// `Weather.FindWeather`: one of the region's weathers of a classification
    /// (0 pleasant, 1 cloudy, 2 rainy, 3 snow), by chance.
    pub fn find_weather(&mut self, class: i32) -> Option<FormId> {
        let list: Vec<(FormId, u32, FormId)> = self
            .weather
            .list
            .clone()
            .into_iter()
            .filter(|e| {
                self.weather_record(e.0)
                    .is_some_and(|w| w.classification() == class)
            })
            .collect();
        self.pick_from(list.into_iter())
    }

    /// `IsRaining` / `IsSnowing` (CommonLibSSE's `Sky::IsRaining`): the
    /// incoming weather's precipitation once the transition passes its begin
    /// fade in, the outgoing one's until it passes its end fade out.
    pub fn precipitating(&self, flag: u8) -> bool {
        let ws = &self.weather;
        let incoming = ws
            .current
            .as_ref()
            .is_some_and(|w| w.flags & flag != 0 && (w.precip_fade.0 as f32 / 255.0) < ws.pct);
        let outgoing = ws.outgoing.as_ref().is_some_and(|w| {
            w.flags & flag != 0 && (w.precip_fade.1 as f32 / 255.0) + 0.001 > ws.pct
        });
        incoming || outgoing
    }

    /// `GetWindSpeed`: the weathers' wind speeds (0..1) mixed.
    pub fn wind_speed(&self) -> f32 {
        let ws = &self.weather;
        let cur = ws.current.as_ref().map_or(0.0, |w| w.wind_speed);
        match &ws.outgoing {
            Some(o) => o.wind_speed + (cur - o.wind_speed) * ws.pct,
            None => cur,
        }
    }

    /// `Weather.GetSkyMode`: 1 indoors, 3 under a full sky (0 none).
    pub fn sky_mode(&self) -> i32 {
        match self.location {
            Location::Interior(_) => 1,
            Location::Exterior { .. } => 3,
            Location::Nowhere => 0,
        }
    }

    /// Advance the transition, follow the player's region and roll the
    /// weather now and then.
    pub(crate) fn update_weather(&mut self, game_hours: f32) {
        if self.weather.outgoing.is_some() {
            let delta = self.weather.current.as_ref().map_or(255, |w| w.trans_delta) as f32;
            let accel = if self.weather.accelerate {
                crate::ai::combat::gmst_f32(&self.lo, "fWeatherTransAccel", 16.0)
            } else {
                1.0
            };
            let ws = &mut self.weather;
            ws.pct += delta * TRANS_DELTA_PER_MINUTE * accel * game_hours * 60.0;
            if ws.pct >= 1.0 {
                ws.pct = 1.0;
                ws.outgoing = None;
                ws.accelerate = false;
                self.sync_sky_textures();
            }
        }
        if let Location::Exterior { world, center } = self.location {
            self.update_region(world, center, true);
        }
        self.weather.next_roll -= game_hours;
        if self.weather.next_roll <= 0.0 && !self.weather.overridden {
            self.schedule_weather_roll();
            if self.weather.outgoing.is_none()
                && let Some(f) = self.pick_weather()
            {
                self.set_weather(f, false, false);
            }
        }
    }

    /// Load the weathers' sky textures and hand them to the sky renderer when
    /// they change.
    fn sync_sky_textures(&mut self) {
        if !self.weather.outdoors {
            return;
        }
        let ws = &self.weather;
        let (Some(cur), Some(climate)) = (ws.current.clone(), ws.climate.as_ref()) else {
            self.renderer.sky.disable();
            return;
        };
        let out = ws.outgoing.clone();
        let key = (cur.id, out.as_ref().map(|w| w.id));
        if ws.textures == Some(key) {
            return;
        }
        let sun_tex = climate.sun_texture.clone();
        let layers = |w: &Weather| -> Vec<String> {
            w.clouds.iter().take(4).map(|c| c.texture.clone()).collect()
        };
        let cur_tex = layers(&cur);
        let out_tex = out.as_deref().map(layers).unwrap_or_default();
        let missing: Vec<String> = std::iter::once(&sun_tex)
            .chain(&cur_tex)
            .chain(&out_tex)
            .filter(|t| !self.renderer.textures.contains(t))
            .cloned()
            .collect();
        loader::load_textures(&mut self.renderer, &self.vfs, missing);
        let get = |p: &String| self.renderer.textures.get(p).flatten();
        let sun = get(&sun_tex).unwrap_or_else(|| self.renderer.white.clone());
        let clouds = cur_tex.iter().filter_map(get).collect();
        let outgoing = out_tex.iter().filter_map(get).collect();
        let (dev, sampler, black) = (
            &self.renderer.device,
            &self.renderer.sampler,
            self.renderer.black.clone(),
        );
        self.renderer
            .sky
            .set_textures(dev, sampler, sun, clouds, outgoing, black);
        self.weather.textures = Some(key);
    }

    /// The sky now: the current weather's at this hour, mixed with the
    /// outgoing one's during a transition.
    fn sky_now(&self) -> Option<SkyState> {
        let ws = &self.weather;
        if !ws.outdoors {
            return None;
        }
        let (cur, c) = (ws.current.as_ref()?, ws.climate.as_ref()?);
        let st = weather::evaluate(cur, c, self.hour);
        Some(match &ws.outgoing {
            Some(o) => SkyState::blend(&weather::evaluate(o, c, self.hour), &st, ws.pct),
            None => st,
        })
    }

    /// Evaluate the weather at the current hour into a render environment.
    pub fn sky_environment(&mut self) -> Option<Environment> {
        let st = self.sky_now()?;
        let env = Environment {
            sun_dir: st.light_dir,
            sun_color: st.sunlight,
            ambient: st.ambient,
            fog_near_color: st.fog_near_color,
            fog_far_color: st.fog_far_color,
            fog_near: st.fog_near,
            fog_far: st.fog_far.max(st.fog_near + 1.0),
            fog_power: st.fog_power,
            fog_max: st.fog_max,
            clear_color: st.horizon,
            dalc: Some(st.dalc),
            sky: true,
        };
        self.renderer.sky.set_state(st);
        Some(env)
    }

    /// Console `weather`: the weathers, the transition and the region.
    pub fn describe_weather(&self) -> Vec<String> {
        let ws = &self.weather;
        let name = |w: &Option<Arc<Weather>>| {
            w.as_ref()
                .map_or("none".to_string(), |w| format!("{} {}", w.id, w.editor_id))
        };
        let mut out = vec![format!(
            "current {} (class {}), {:.0}% in{}",
            name(&ws.current),
            ws.current.as_ref().map_or(-1, |w| w.classification()),
            ws.pct * 100.0,
            if ws.overridden { ", overridden" } else { "" }
        )];
        if ws.outgoing.is_some() {
            out.push(format!(
                "outgoing {}{}",
                name(&ws.outgoing),
                if ws.accelerate { ", accelerated" } else { "" }
            ));
        }
        out.push(format!(
            "{:.2}h: raining {} snowing {} wind {:.2}; next roll in {:.1}h",
            self.hour,
            self.precipitating(weather::flags::RAINY),
            self.precipitating(weather::flags::SNOW),
            self.wind_speed(),
            ws.next_roll
        ));
        out.push(format!(
            "{} weathers: {}",
            ws.region.map_or("climate".to_string(), |r| format!(
                "region {r} {}",
                self.lo
                    .get(r)
                    .and_then(|r| r.editor_id())
                    .unwrap_or_default()
            )),
            ws.list
                .iter()
                .map(|(w, c, g)| format!(
                    "{} {}{}",
                    self.lo
                        .get(*w)
                        .and_then(|r| r.editor_id())
                        .unwrap_or_default(),
                    c,
                    if g.is_null() {
                        String::new()
                    } else {
                        format!(" (global {g})")
                    }
                ))
                .collect::<Vec<_>>()
                .join(", ")
        ));
        out
    }

    /// The base image space's weathers: (outgoing, its weight), (current, its weight).
    pub(crate) fn weather_image_spaces(&self) -> Vec<([FormId; 4], f32)> {
        let ws = &self.weather;
        let mut out = Vec::new();
        if let Some(o) = &ws.outgoing {
            out.push((o.image_spaces, 1.0 - ws.pct));
        }
        if let Some(c) = &ws.current {
            out.push((
                c.image_spaces,
                if ws.outgoing.is_some() { ws.pct } else { 1.0 },
            ));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classification_takes_the_first_flag() {
        let mut w = Weather::new(FormId::NULL, String::new());
        assert_eq!(w.classification(), -1);
        w.flags = weather::flags::RAINY | weather::flags::CLOUDY;
        assert_eq!(w.classification(), 1);
        w.flags = weather::flags::SNOW;
        assert_eq!(w.classification(), 3);
    }
}
