//! Detection: who notices whom.
//!
//! One detection value per observer and target, as the Creation Kit wiki's
//! Detection page gives it (Skyrim's, archived; the GECK's for Fallout 3 is the
//! same):
//!
//! ```text
//! value = fSneakBaseValue
//!       + attenuation * (sound + visual + observer skill)
//!       + (observer skill - target skill)
//! attenuation = ((max distance - distance) / max distance) ^ exponent
//! sound  = fSneakSoundsMult * movement * (1, or fSneakSoundLosMult unseen)
//! movement = (fSneakEquippedWeightBase + fSneakEquippedWeightMult * armor
//!            weight) * (fSneakRunningMult running), 0 standing still
//! visual = (fDetectionSneakLightMod + light level) * fSneakLightMult, 0 when
//!          the target can't be seen
//! observer skill = fSneakPerceptionSkillMin + (Max - Min) * Sneak / 100
//! ```
//!
//! Above 0 the target is detected. Against those who would attack them, the
//! player also has stealth points (the CK wiki's Stealth Points page): detection
//! above 0 drains them, below 0 refills them; an enemy attacks once they are
//! gone, or at once when its detection passes `iCombatStealthPointDetectionThreshold`.
//! Crime witnesses and force greets need only detection above 0.
//!
//! Open questions (the target's skill, light level, the view cone...):
//! `known_gaps/detection.md`.

use std::collections::HashMap;

use esp::actor_value as av;
use esp::{FormId, LoadOrder};
use glam::Vec3;

use crate::ai::combat::{gmst_f32, gmst_i32};
use crate::engine::{Engine, Location, PLAYER_REF};

/// How often loaded actors' detection of the player is worked out (seconds).
const PLAYER_INTERVAL: f32 = 0.25;
/// Eye height above the feet (no source; as for finding bodies).
const EYE_HEIGHT: f32 = 110.0;
/// Half the view cone (no source).
const VIEW_HALF_ANGLE: f32 = 95.0;
/// Light level per unit of luminance at a point, and its cap (`GetLightLevel`
/// is documented as 0 to 150; the scale has no source).
const LIGHT_SCALE: f32 = 100.0;
const LIGHT_MAX: f32 = 150.0;
/// How far up to look for something shading a point from the sun.
const SUN_RAY: f32 = 20000.0;

/// The game settings detection is built from. Those Skyrim.esm doesn't carry
/// default to the values UESP and the CK wiki give.
#[derive(Debug, Clone, Copy)]
pub struct DetectionSettings {
    pub base: f32,
    pub max_distance: f32,
    pub exterior_distance_mult: f32,
    /// `fSneakDistanceAttenuationExponent` (2: UESP).
    pub attenuation_exponent: f32,
    pub sounds_mult: f32,
    pub sound_los_mult: f32,
    pub running_mult: f32,
    /// `fSneakEquippedWeightBase` / `Mult` (12, 0.5: UESP).
    pub weight_base: f32,
    pub weight_mult: f32,
    pub light_mod: f32,
    pub light_mult: f32,
    pub light_exterior_mult: f32,
    pub perception_min: f32,
    pub perception_max: f32,
    pub skill_mult: f32,
    pub sleep_bonus: f32,
    /// `iCombatStealthPointDetectionThreshold`.
    pub combat_threshold: f32,
    pub drain_mult: f32,
    pub regen_mult: f32,
    pub regen_min: f32,
    /// `fCombatStealthPointRegenAlertWaitTime` (10: CK wiki).
    pub alert_wait: f32,
}

impl DetectionSettings {
    pub fn load(lo: &LoadOrder) -> Self {
        let f = |n: &str, d: f32| gmst_f32(lo, n, d);
        DetectionSettings {
            base: f("fSneakBaseValue", -15.0),
            max_distance: f("fSneakMaxDistance", 2500.0),
            exterior_distance_mult: f("fSneakExteriorDistanceMult", 2.1),
            attenuation_exponent: f("fSneakDistanceAttenuationExponent", 2.0),
            sounds_mult: f("fSneakSoundsMult", 1.0),
            sound_los_mult: f("fSneakSoundLosMult", 0.3),
            running_mult: f("fSneakRunningMult", 2.0),
            weight_base: f("fSneakEquippedWeightBase", 12.0),
            weight_mult: f("fSneakEquippedWeightMult", 0.5),
            light_mod: f("fDetectionSneakLightMod", 15.0),
            light_mult: f("fSneakLightMult", 0.33),
            light_exterior_mult: f("fSneakLightExteriorMult", 0.5),
            perception_min: f("fSneakPerceptionSkillMin", 0.0),
            perception_max: f("fSneakPerceptionSkillMax", 100.0),
            skill_mult: f("fSneakSkillMult", 0.5),
            sleep_bonus: f("fSneakSleepBonus", 0.0),
            combat_threshold: gmst_i32(lo, "iCombatStealthPointDetectionThreshold", 25) as f32,
            drain_mult: f("fCombatStealthPointDrainMult", 3.0),
            regen_mult: f("fCombatStealthPointRegenMult", 0.2),
            regen_min: f("fCombatStealthPointRegenMin", 5.0),
            alert_wait: f("fCombatStealthPointRegenAlertWaitTime", 10.0),
        }
    }
}

/// What detection keeps between updates.
pub struct Detection {
    settings: std::cell::OnceCell<DetectionSettings>,
    /// Loaded actors' detection of the player (those within range), as last
    /// worked out.
    pub(crate) of_player: HashMap<FormId, f32>,
    /// Stealth points of the player and of actors enemies have been alert to
    /// (100 when missing).
    stealth: HashMap<FormId, Stealth>,
    next_in: f32,
    states_in: f32,
}

impl Default for Detection {
    fn default() -> Self {
        Detection {
            settings: Default::default(),
            of_player: HashMap::new(),
            stealth: HashMap::new(),
            next_in: 0.0,
            states_in: 0.0,
        }
    }
}

/// Someone's stealth points (100 hidden, 0 found), and seconds before they may
/// refill after an enemy became alert to them.
#[derive(Debug, Clone, Copy)]
struct Stealth {
    points: f32,
    regen_wait: f32,
}

impl Stealth {
    /// Drain or refill by the most any enemy detects them by (`None`: no enemy
    /// within reach, nobody to hide from); none while an enemy fights them.
    fn step(&mut self, most: Option<f32>, fighting: bool, s: &DetectionSettings, step: f32) {
        if fighting {
            self.points = 0.0;
            return;
        }
        match most {
            Some(v) if v > 0.0 => {
                self.points = (self.points - v * s.drain_mult * step).max(0.0);
                self.regen_wait = s.alert_wait;
            }
            Some(v) if self.regen_wait <= 0.0 => {
                self.points = (self.points + (-v * s.regen_mult).max(s.regen_min) * step).min(100.0)
            }
            None if self.regen_wait <= 0.0 => self.points = 100.0,
            _ => {}
        }
    }
}

/// An actor searching for someone: alert to them (it almost detected them) or
/// having lost them in a fight. The CK wiki's Detection page.
#[derive(Debug, Clone, Copy)]
pub struct Search {
    pub target: FormId,
    /// Where it searches: where it last noticed them.
    pub at: Vec3,
    pub lost: bool,
    /// Seconds before it looks somewhere else near `at`, and before its next
    /// idle line (`AlertIdle` / `LostIdle`).
    dwell: f32,
    idle_line: f32,
}

/// How often actors' detection states are worked out (seconds; no source).
const STATE_INTERVAL: f32 = 1.0;
/// How far round the spot it is searching a searcher looks, and how long it
/// stays at each place (seconds). No source.
const SEARCH_RADIUS: f32 = 512.0;
const SEARCH_DWELL: (f32, f32) = (3.0, 8.0);

/// Who is looking: a living loaded actor.
struct Observer {
    id: FormId,
    eye: Vec3,
    heading: f32,
    sneak: f32,
    sleeping: bool,
}

/// Who may be noticed: the player or a loaded actor.
struct Target {
    id: FormId,
    /// Where they are looked at (their eyes).
    at: Vec3,
    sneaking: bool,
    moving: bool,
    running: bool,
    armor_weight: f32,
    sneak: f32,
    /// Percent off the observer's skill factor while sneaking (the Stealth
    /// perks: Mod Detection Sneak Skill).
    stealth: f32,
}

impl Engine {
    pub(crate) fn detection_settings(&self) -> DetectionSettings {
        *self
            .detection
            .settings
            .get_or_init(|| DetectionSettings::load(&self.lo))
    }

    fn observer(&self, r: FormId) -> Option<Observer> {
        let a = self.actor_ref(r)?;
        if a.dead || a.bleeding.is_some() || self.is_disabled(r) {
            return None;
        }
        Some(Observer {
            id: r,
            eye: a.pos + Vec3::Z * EYE_HEIGHT * a.scale,
            heading: a.heading,
            sneak: self.actor_value(r, av::SNEAK),
            // In bed, eyes shut.
            sleeping: self.sit_sleep_state(r, true) == 3.0,
        })
    }

    fn target(&self, r: FormId) -> Option<Target> {
        if r == PLAYER_REF {
            return Some(Target {
                id: r,
                at: self.player.eye(),
                sneaking: self.player.sneaking,
                moving: self.player.moving,
                running: self.player.running,
                armor_weight: self.armor_weight(r),
                sneak: self.actor_value(r, av::SNEAK),
                stealth: self.stealth_perks(r),
            });
        }
        let a = self.actor_ref(r)?;
        if self.is_disabled(r) {
            return None;
        }
        let moving = a.speed() > 1.0 && !a.dead;
        Some(Target {
            id: r,
            at: a.pos + Vec3::Z * EYE_HEIGHT * a.scale,
            sneaking: a.is_sneaking(),
            moving,
            running: moving && a.gait() == crate::world::footsteps::Gait::Run,
            armor_weight: self.armor_weight(r),
            sneak: self.actor_value(r, av::SNEAK),
            stealth: self.stealth_perks(r),
        })
    }

    fn stealth_perks(&self, r: FormId) -> f32 {
        self.perk_entry_point(crate::perks::ep::MOD_DETECTION_SNEAK_SKILL, r, &[], 0.0)
    }

    /// The weight of the armor an actor wears.
    fn armor_weight(&self, r: FormId) -> f32 {
        let Some(inv) = self.inventories.get(&r) else {
            return 0.0;
        };
        inv.equipped
            .iter()
            .filter(|&&f| self.lo.tag_of(f).is_some_and(|t| t.0 == *b"ARMO"))
            .filter_map(|&f| crate::world::inventory::item_info(&self.lo, f))
            .map(|i| i.weight)
            .sum()
    }

    /// How far detection reaches here: further outdoors.
    fn detection_range(&self, s: &DetectionSettings) -> f32 {
        s.max_distance
            * if self.outdoors() {
                s.exterior_distance_mult
            } else {
                1.0
            }
    }

    fn outdoors(&self) -> bool {
        matches!(self.location, Location::Exterior { .. })
    }

    /// Whether nothing stands between two points but `to_whom` (looking from
    /// `from_whom`).
    fn clear_line(&self, from: Vec3, to: Vec3, from_whom: FormId, to_whom: FormId) -> bool {
        let d = to - from;
        let dist = d.length().max(1.0);
        match self
            .physics
            .raycast_excluding(from, d / dist, (dist - 30.0).max(0.0), from_whom)
        {
            Some((_, owner)) => owner == Some(to_whom),
            None => true,
        }
    }

    /// The light level at a point (`GetLightLevel`, 0 to 150): the scene's
    /// ambient, its sun (or interior directional light) unless something shades
    /// the point outdoors, and the point lights reaching it with nothing in
    /// between, as the renderer lights a surface facing each.
    pub(crate) fn light_level_at(&self, p: Vec3, exclude: FormId) -> f32 {
        let [ambient, sun, lights] = self.light_parts(p, exclude);
        (ambient + sun + lights).clamp(0.0, LIGHT_MAX)
    }

    /// The light level's ambient, sun and point light parts (unclamped).
    fn light_parts(&self, p: Vec3, exclude: FormId) -> [f32; 3] {
        let level = |c: Vec3| c.dot(Vec3::new(0.2126, 0.7152, 0.0722)) * LIGHT_SCALE;
        let env = &self.scene.env;
        let ambient = env
            .dalc
            .map_or(env.ambient, |d| d.iter().copied().sum::<Vec3>() / 6.0);
        let sun_dir = env.sun_dir.normalize_or_zero();
        let sun_lit = !self.outdoors()
            || (sun_dir.z > 0.0
                && self
                    .physics
                    .raycast_excluding(p, sun_dir, SUN_RAY, exclude)
                    .is_none());
        let sun = if sun_lit { env.sun_color } else { Vec3::ZERO };
        let mut light = Vec3::ZERO;
        for l in &self.scene.lights {
            let at = Vec3::from_slice(&l.pos_radius[..3]);
            let r = l.pos_radius[3];
            let d = at.distance(p);
            if d >= r {
                continue;
            }
            let x = d / r;
            let att = (1.0 - x * x).clamp(0.0, 1.0);
            if att < 0.01 {
                continue;
            }
            let dir = (at - p) / d.max(1.0);
            if self
                .physics
                .raycast_excluding(p, dir, (d - 10.0).max(0.0), exclude)
                .is_some()
            {
                continue;
            }
            light += Vec3::from_slice(&l.color[..3]) * att;
        }
        [level(ambient), level(sun), level(light)]
    }

    /// The light level an actor (or the player) stands in.
    pub(crate) fn light_level(&self, r: FormId) -> f32 {
        let at = if r == PLAYER_REF {
            self.player.position
        } else {
            match self.actor_ref(r) {
                Some(a) => a.pos + Vec3::Z * EYE_HEIGHT * 0.6 * a.scale,
                None => return 0.0,
            }
        };
        self.light_level_at(at, r)
    }

    /// `observer`'s detection value against `target`; `None` when either isn't
    /// a loaded, living actor (or the player as the target), or the target is
    /// beyond detection's reach.
    pub(crate) fn detection_value(&self, observer: FormId, target: FormId) -> Option<f32> {
        if observer == target || observer == PLAYER_REF {
            return None;
        }
        let o = self.observer(observer)?;
        let t = self.target(target)?;
        Some(self.detection_between(&o, &t)?.0)
    }

    /// The detection value and whether `o` sees `t` (light aside).
    fn detection_between(&self, o: &Observer, t: &Target) -> Option<(f32, bool)> {
        let s = self.detection_settings();
        let range = self.detection_range(&s);
        let to = t.at - o.eye;
        let dist = to.length();
        if dist >= range {
            return None;
        }
        let attenuation = ((range - dist) / range).powf(s.attenuation_exponent);
        let los = self.clear_line(o.eye, t.at, o.id, t.id);
        let facing = Vec3::new(o.heading.sin(), o.heading.cos(), 0.0);
        let flat = Vec3::new(to.x, to.y, 0.0).normalize_or_zero();
        let in_view = flat == Vec3::ZERO || facing.dot(flat) >= VIEW_HALF_ANGLE.to_radians().cos();
        let sees = los && in_view && !o.sleeping;
        // The observer's perks for noticing movement and what light shows.
        use crate::perks::ep;
        let about = [Some(t.id)];
        let movement = if t.moving {
            (s.weight_base + s.weight_mult * t.armor_weight)
                * if t.running { s.running_mult } else { 1.0 }
                * self.perk_entry_point(ep::MOD_DETECTION_MOVEMENT, o.id, &about, 1.0)
        } else {
            0.0
        };
        let sound = s.sounds_mult * movement * if los { 1.0 } else { s.sound_los_mult };
        let visual = if sees {
            let light = self.light_level_at(t.at - Vec3::Z * 40.0, t.id);
            (s.light_mod + light)
                * s.light_mult
                * if self.outdoors() {
                    s.light_exterior_mult
                } else {
                    1.0
                }
                * self.perk_entry_point(ep::MOD_DETECTION_LIGHT, o.id, &about, 1.0)
        } else {
            0.0
        };
        // The target's Stealth perks cut the observer's skill factor (UESP).
        let observer_skill = (s.perception_min
            + (s.perception_max - s.perception_min) * o.sneak / 100.0)
            * (1.0 + if o.sleeping { s.sleep_bonus } else { 0.0 })
            * if t.sneaking {
                (1.0 - t.stealth / 100.0).max(0.0)
            } else {
                1.0
            };
        let target_skill = if t.sneaking {
            t.sneak * s.skill_mult
        } else {
            0.0
        };
        Some((
            s.base
                + attenuation * (sound + visual + observer_skill)
                + (observer_skill - target_skill),
            sees,
        ))
    }

    /// Whether the player or a loaded actor is sneaking.
    pub(crate) fn is_sneaking(&self, r: FormId) -> bool {
        if r == PLAYER_REF {
            self.player.sneaking
        } else {
            self.actor_ref(r).is_some_and(|a| a.is_sneaking())
        }
    }

    /// Whether `observer` detects `target` now (`GetDetected`, `IsDetectedBy`).
    pub(crate) fn detects(&self, observer: FormId, target: FormId) -> bool {
        self.detection_value(observer, target)
            .is_some_and(|v| v > 0.0)
    }

    /// Someone's stealth points (100 hidden, 0 found).
    pub(crate) fn stealth_points(&self, r: FormId) -> f32 {
        self.detection.stealth.get(&r).map_or(100.0, |s| s.points)
    }

    /// Whether a detection value against `target` is enough to attack it: its
    /// stealth points are gone and it is detected, or it is detected past the
    /// combat threshold.
    fn enough_to_find(&self, v: f32, target: FormId) -> bool {
        (v > 0.0 && self.stealth_points(target) <= 0.0)
            || v > self.detection_settings().combat_threshold
    }

    /// Whether `observer` finds `target` (to attack it): from the last worked
    /// out detection of the player, else from detection now.
    pub(crate) fn finds(&self, observer: FormId, target: FormId) -> bool {
        let v = if target == PLAYER_REF {
            self.detection.of_player.get(&observer).copied()
        } else {
            self.detection_value(observer, target)
        };
        v.is_some_and(|v| self.enough_to_find(v, target))
    }

    pub(crate) fn finds_player(&self, observer: FormId) -> bool {
        self.finds(observer, PLAYER_REF)
    }

    /// Whether `r` would attack `target` on finding it.
    fn would_attack(&mut self, r: FormId, target: FormId) -> bool {
        if target == PLAYER_REF {
            return self.would_attack_player(r);
        }
        let (Some(a), Some(t)) = (self.actor_ref(r), self.actor_ref(target)) else {
            return false;
        };
        if a.dead || t.dead {
            return false;
        }
        let (stats, theirs) = (a.stats.clone(), t.stats.factions.clone());
        self.hostile_to(&stats, &theirs, false)
    }

    /// Work out loaded actors' detection of the player four times a second and
    /// drain or refill the player's stealth points by the most any enemy has;
    /// once a second, the same for actors as targets, and actors' detection
    /// states (`update_detection_states`).
    pub(crate) fn update_detection(&mut self, dt: f32) {
        for st in self.detection.stealth.values_mut() {
            st.regen_wait = (st.regen_wait - dt).max(0.0);
        }
        self.detection.states_in -= dt;
        if self.detection.states_in <= 0.0 {
            self.detection.states_in = STATE_INTERVAL;
            self.update_detection_states(STATE_INTERVAL);
        }
        self.detection.next_in -= dt;
        if self.detection.next_in > 0.0 {
            return;
        }
        let step = PLAYER_INTERVAL - self.detection.next_in.min(0.0);
        self.detection.next_in = PLAYER_INTERVAL;
        let Some(t) = self.target(PLAYER_REF) else {
            return;
        };
        let observers: Vec<FormId> = self
            .cells
            .values()
            .flat_map(|rt| &rt.actors)
            .map(|a| a.ref_id)
            .collect();
        let mut of_player = HashMap::new();
        let mut most: Option<f32> = None;
        for r in observers {
            let Some(o) = self.observer(r) else { continue };
            let Some((v, _)) = self.detection_between(&o, &t) else {
                continue;
            };
            of_player.insert(r, v);
            if self.would_attack_player(r) {
                most = Some(most.map_or(v, |m| m.max(v)));
            }
        }
        let s = self.detection_settings();
        let fighting = self.fought(PLAYER_REF);
        self.detection.of_player = of_player;
        let st = self.detection.stealth.entry(PLAYER_REF).or_insert(Stealth {
            points: 100.0,
            regen_wait: 0.0,
        });
        if most.is_some_and(|v| v > 0.0) && st.regen_wait <= 0.0 && !fighting {
            log::debug!(
                "an enemy is alert to the player ({:.1})",
                most.unwrap_or_default()
            );
        }
        st.step(most, fighting, &s, step);
        self.update_trespass();
    }

    /// Whether anyone is fighting `r` (not fleeing from it).
    fn fought(&self, r: FormId) -> bool {
        self.cells.values().flat_map(|rt| &rt.actors).any(|a| {
            !a.dead
                && a.combat
                    .as_ref()
                    .is_some_and(|c| c.target == r && !c.fleeing)
        })
    }

    /// Actors' stealth points as targets, and the detection states (CK wiki
    /// "Detection"): a calm actor that would attack someone it detects but
    /// doesn't find yet becomes alert and goes to look where they were; a
    /// fighter whose target has gone undetected for
    /// `fCombatStealthPointRegenDetectedEventWaitTime` loses it and searches
    /// where it last saw it. Searchers attack once they find their target, and
    /// give up once it is undetected with its stealth points full.
    fn update_detection_states(&mut self, step: f32) {
        let s = self.detection_settings();
        let lost_wait = gmst_f32(
            &self.lo,
            "fCombatStealthPointRegenDetectedEventWaitTime",
            10.0,
        );
        let actors: Vec<FormId> = self
            .cells
            .values()
            .flat_map(|rt| &rt.actors)
            .filter(|a| !a.dead && a.bleeding.is_none())
            .map(|a| a.ref_id)
            .collect();
        // Who would attack whom, and by how much they detect them.
        let mut values: HashMap<(FormId, FormId), f32> = HashMap::new();
        for &r in &actors {
            for &t in actors.iter().chain(std::iter::once(&PLAYER_REF)) {
                if t == r || !self.would_attack(r, t) {
                    continue;
                }
                let v = if t == PLAYER_REF {
                    self.detection.of_player.get(&r).copied()
                } else {
                    self.detection_value(r, t)
                };
                if let Some(v) = v {
                    values.insert((r, t), v);
                }
            }
        }
        // Actors' stealth points as targets.
        for &t in &actors {
            let most = values
                .iter()
                .filter(|((_, x), _)| *x == t)
                .map(|(_, &v)| v)
                .fold(None, |m: Option<f32>, v| Some(m.map_or(v, |m| m.max(v))));
            let fighting = self.fought(t);
            if most.is_none() && !fighting && !self.detection.stealth.contains_key(&t) {
                continue;
            }
            let st = self.detection.stealth.entry(t).or_insert(Stealth {
                points: 100.0,
                regen_wait: 0.0,
            });
            st.step(most, fighting, &s, step);
            if st.points >= 100.0 && st.regen_wait <= 0.0 {
                self.detection.stealth.remove(&t);
            }
        }
        self.detection
            .stealth
            .retain(|&t, _| t == PLAYER_REF || actors.contains(&t));
        let position = |e: &Engine, t: FormId| {
            if t == PLAYER_REF {
                Some(e.player.position)
            } else {
                e.actor_ref(t).filter(|a| !a.dead).map(|a| a.pos)
            }
        };
        for &r in &actors {
            let Some(a) = self.actor_ref(r) else { continue };
            let (combat, search) = (a.combat.as_ref().map(|c| (c.target, c.fleeing)), a.search);
            if let Some((t, fleeing)) = combat {
                // Fighting: keep track of the target; lose it once undetected long
                // enough. (Fleeing actors leave by the threat instead.)
                if fleeing {
                    continue;
                }
                let v = if t == PLAYER_REF {
                    self.detection.of_player.get(&r).copied()
                } else {
                    self.detection_value(r, t)
                };
                let seen_at = v.filter(|&v| v > 0.0).and_then(|_| position(self, t));
                let Some(c) = self.actor_mut(r).and_then(|a| a.combat.as_mut()) else {
                    continue;
                };
                if seen_at.is_some() {
                    c.unseen = 0.0;
                    c.last_seen = seen_at;
                } else {
                    c.unseen += step;
                    if c.unseen >= lost_wait {
                        self.lose_target(r);
                    }
                }
            } else if let Some(mut sr) = search {
                let alive = position(self, sr.target);
                if alive.is_none() || !self.would_attack(r, sr.target) {
                    self.calm_down(r, None);
                    continue;
                }
                let v = values.get(&(r, sr.target)).copied();
                if v.is_some_and(|v| self.enough_to_find(v, sr.target)) {
                    // `detect_enemies` starts the fight.
                    continue;
                }
                if v.is_some_and(|v| v > 0.0) {
                    sr.at = alive.unwrap_or(sr.at);
                    sr.dwell = 0.0;
                } else if self.stealth_points(sr.target) >= 100.0 {
                    self.calm_down(r, Some(if sr.lost { b"LOTN" } else { b"ALTN" }));
                    continue;
                }
                sr.dwell -= step;
                sr.idle_line -= step;
                let here = self.actor_ref(r).map_or(sr.at, |a| a.pos);
                let mut go = None;
                if sr.dwell <= 0.0 {
                    let near = here.distance(sr.at) < SEARCH_RADIUS;
                    let mut rolls: Vec<u64> = (0..16).map(|_| self.rand()).collect();
                    go = if near {
                        self.nav
                            .random_point(sr.at, SEARCH_RADIUS, || rolls.pop().unwrap_or(0))
                    } else {
                        None
                    }
                    .or(Some(sr.at));
                    sr.dwell = SEARCH_DWELL.0
                        + (self.rand() % 1000) as f32 / 1000.0 * (SEARCH_DWELL.1 - SEARCH_DWELL.0);
                }
                let line = sr.idle_line <= 0.0;
                if line {
                    sr.idle_line = self.detection_idle_wait();
                }
                if let Some(a) = self.actor_mut(r) {
                    a.search = Some(sr);
                }
                if let Some(to) = go {
                    self.search_towards(r, to);
                }
                if line {
                    self.bark(r, if sr.lost { b"LOIL" } else { b"ALIL" });
                }
            } else {
                // Calm: alert to the enemy it detects most without finding.
                let best = values
                    .iter()
                    .map(|(&(o, t), &v)| (o, t, v))
                    .filter(|&(o, t, v)| o == r && v > 0.0 && !self.enough_to_find(v, t))
                    .max_by(|a, b| a.2.total_cmp(&b.2))
                    .map(|(_, t, _)| t);
                if let Some(t) = best
                    && let Some(at) = position(self, t)
                {
                    self.start_search(r, t, at, false);
                }
            }
        }
    }

    /// Seconds before a searcher's next idle line
    /// (`fCombatDetectionDialogueIdleMin` / `MaxElapsedTime`).
    fn detection_idle_wait(&mut self) -> f32 {
        let lo = gmst_f32(&self.lo, "fCombatDetectionDialogueIdleMinElapsedTime", 10.0);
        let hi = gmst_f32(&self.lo, "fCombatDetectionDialogueIdleMaxElapsedTime", 25.0);
        lo + (self.rand() % 1000) as f32 / 1000.0 * (hi - lo).max(0.0)
    }

    /// `r` starts searching for `target` at `at`: alert, or having lost it in
    /// a fight. Weapons out.
    fn start_search(&mut self, r: FormId, target: FormId, at: Vec3, lost: bool) {
        let idle_line = self.detection_idle_wait();
        let Some(a) = self.actor_mut(r) else { return };
        a.search = Some(Search {
            target,
            at,
            lost,
            dwell: SEARCH_DWELL.1,
            idle_line,
        });
        log::info!("{r} {} {target}", if lost { "lost" } else { "is alert to" });
        self.search_towards(r, at);
        self.draw_weapon(r, true);
        self.bark(r, if lost { b"COLO" } else { b"NOTA" });
    }

    fn search_towards(&mut self, r: FormId, to: Vec3) {
        let Some(key) = self.actor_cells.get(&r).copied() else {
            return;
        };
        if let Some(a) = self
            .cells
            .get_mut(&key)
            .and_then(|rt| rt.actors.iter_mut().find(|a| a.ref_id == r))
        {
            a.search_at(to, &mut self.furniture);
        }
    }

    /// A fighter loses its target (undetected too long, or too far): it stops
    /// fighting and searches where it last saw it, if the target is still
    /// about.
    pub(crate) fn lose_target(&mut self, r: FormId) {
        let Some(c) = self.actor_ref(r).and_then(|a| a.combat.as_ref()) else {
            return;
        };
        let (target, last) = (c.target, c.last_seen);
        let about = target == PLAYER_REF && !self.player_dead()
            || self.actor_ref(target).is_some_and(|t| !t.dead);
        let at = last.or_else(|| {
            if target == PLAYER_REF {
                Some(self.player.position)
            } else {
                self.actor_ref(target).map(|t| t.pos)
            }
        });
        match at.filter(|_| about && self.ai_enabled) {
            Some(at) => {
                self.stop_fighting(r, true);
                self.start_search(r, target, at, true);
            }
            None => self.end_combat(r),
        }
    }

    /// Papyrus `StopCombat`: a searcher gives up without a word.
    pub fn stop_searching(&mut self, r: FormId) {
        if self.actor_ref(r).is_some_and(|a| a.search.is_some()) {
            self.calm_down(r, None);
        }
    }

    /// A searcher gives up: weapons away, back to its packages.
    fn calm_down(&mut self, r: FormId, topic: Option<&[u8; 4]>) {
        let Some(a) = self.actor_mut(r) else { return };
        let target = a.search.map(|s| s.target);
        a.end_search();
        log::info!("{r} stops searching for {target:?}");
        self.draw_weapon(r, false);
        if let Some(t) = topic {
            self.bark(r, t);
        }
    }

    /// How open the sneak eye is (0 hidden, 1 found): the most any actor
    /// detects the player by against the combat threshold, or how far the
    /// player's stealth points have gone.
    pub fn sneak_eye(&self) -> f32 {
        let s = self.detection_settings();
        let most = self
            .detection
            .of_player
            .values()
            .copied()
            .fold(f32::MIN, f32::max);
        let seen = (most / s.combat_threshold).clamp(0.0, 1.0);
        seen.max(1.0 - self.stealth_points(PLAYER_REF) / 100.0)
    }

    /// Console: each loaded actor's detection of the player and what goes into it.
    pub fn describe_detection(&self) -> Vec<String> {
        let s = self.detection_settings();
        let [ambient, sun, lights] = self.light_parts(self.player.position, PLAYER_REF);
        let mut out = vec![format!(
            "player: light {:.0} (ambient {ambient:.0}, sun {sun:.0}, lights {lights:.0}), {}{}{}, armor {:.0}, sneak {:.0}; stealth points {:.0}; range {:.0}",
            self.light_level(PLAYER_REF),
            if self.player.sneaking {
                "sneaking"
            } else {
                "standing"
            },
            if self.player.moving { ", moving" } else { "" },
            if self.player.running { ", running" } else { "" },
            self.armor_weight(PLAYER_REF),
            self.actor_value(PLAYER_REF, av::SNEAK),
            self.stealth_points(PLAYER_REF),
            self.detection_range(&s),
        )];
        let Some(t) = self.target(PLAYER_REF) else {
            return out;
        };
        let mut rows: Vec<(f32, String)> = Vec::new();
        for a in self.cells.values().flat_map(|rt| &rt.actors) {
            let Some(o) = self.observer(a.ref_id) else {
                continue;
            };
            let Some((v, sees)) = self.detection_between(&o, &t) else {
                continue;
            };
            let dist = o.eye.distance(t.at);
            rows.push((
                v,
                format!(
                    "  {} {:>6.1} at {:>5.0}{}{}{}",
                    a.ref_id,
                    v,
                    dist,
                    if sees { ", sees" } else { "" },
                    if o.sleeping { ", asleep" } else { "" },
                    if self.finds_player(a.ref_id) {
                        ", found"
                    } else {
                        ""
                    }
                ),
            ));
        }
        rows.sort_by(|a, b| b.0.total_cmp(&a.0));
        out.extend(rows.into_iter().map(|r| r.1));
        out
    }
}
