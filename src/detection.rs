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
    /// The player's stealth points (100 hidden, 0 found), and seconds before they
    /// may refill after an enemy became alert to them.
    pub(crate) stealth_points: f32,
    regen_wait: f32,
    next_in: f32,
}

impl Default for Detection {
    fn default() -> Self {
        Detection { settings: Default::default(), of_player: HashMap::new(), stealth_points: 100.0, regen_wait: 0.0, next_in: 0.0 }
    }
}

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
}

impl Engine {
    pub(crate) fn detection_settings(&self) -> DetectionSettings {
        *self.detection.settings.get_or_init(|| DetectionSettings::load(&self.lo))
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
        })
    }

    /// The weight of the armor an actor wears.
    fn armor_weight(&self, r: FormId) -> f32 {
        let Some(inv) = self.inventories.get(&r) else { return 0.0 };
        inv.equipped
            .iter()
            .filter(|&&f| self.lo.tag_of(f).is_some_and(|t| t.0 == *b"ARMO"))
            .filter_map(|&f| crate::world::inventory::item_info(&self.lo, f))
            .map(|i| i.weight)
            .sum()
    }

    /// How far detection reaches here: further outdoors.
    fn detection_range(&self, s: &DetectionSettings) -> f32 {
        s.max_distance * if self.outdoors() { s.exterior_distance_mult } else { 1.0 }
    }

    fn outdoors(&self) -> bool {
        matches!(self.location, Location::Exterior { .. })
    }

    /// Whether nothing stands between two points but `to_whom` (looking from
    /// `from_whom`).
    fn clear_line(&self, from: Vec3, to: Vec3, from_whom: FormId, to_whom: FormId) -> bool {
        let d = to - from;
        let dist = d.length().max(1.0);
        match self.physics.raycast_excluding(from, d / dist, (dist - 30.0).max(0.0), from_whom) {
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
        let ambient = env.dalc.map_or(env.ambient, |d| d.iter().copied().sum::<Vec3>() / 6.0);
        let sun_dir = env.sun_dir.normalize_or_zero();
        let sun_lit = !self.outdoors() || (sun_dir.z > 0.0 && self.physics.raycast_excluding(p, sun_dir, SUN_RAY, exclude).is_none());
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
            if self.physics.raycast_excluding(p, dir, (d - 10.0).max(0.0), exclude).is_some() {
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
        let movement = if t.moving { (s.weight_base + s.weight_mult * t.armor_weight) * if t.running { s.running_mult } else { 1.0 } } else { 0.0 };
        let sound = s.sounds_mult * movement * if los { 1.0 } else { s.sound_los_mult };
        let visual = if sees {
            let light = self.light_level_at(t.at - Vec3::Z * 40.0, t.id);
            (s.light_mod + light) * s.light_mult * if self.outdoors() { s.light_exterior_mult } else { 1.0 }
        } else {
            0.0
        };
        let observer_skill = (s.perception_min + (s.perception_max - s.perception_min) * o.sneak / 100.0) * (1.0 + if o.sleeping { s.sleep_bonus } else { 0.0 });
        let target_skill = if t.sneaking { t.sneak * s.skill_mult } else { 0.0 };
        Some((s.base + attenuation * (sound + visual + observer_skill) + (observer_skill - target_skill), sees))
    }

    /// Whether the player or a loaded actor is sneaking.
    pub(crate) fn is_sneaking(&self, r: FormId) -> bool {
        if r == PLAYER_REF { self.player.sneaking } else { self.actor_ref(r).is_some_and(|a| a.is_sneaking()) }
    }

    /// Whether `observer` detects `target` now (`GetDetected`, `IsDetectedBy`).
    pub(crate) fn detects(&self, observer: FormId, target: FormId) -> bool {
        self.detection_value(observer, target).is_some_and(|v| v > 0.0)
    }

    /// Whether `observer`'s last worked out detection of the player is enough
    /// for it to attack: the player's stealth points are gone and it detects
    /// them, or it detects them past the combat threshold.
    pub(crate) fn finds_player(&self, observer: FormId) -> bool {
        let s = self.detection_settings();
        self.detection.of_player.get(&observer).is_some_and(|&v| (v > 0.0 && self.detection.stealth_points <= 0.0) || v > s.combat_threshold)
    }

    /// Work out loaded actors' detection of the player now and then, and drain or
    /// refill the player's stealth points by the most any enemy has. In combat
    /// with anyone, the player has none.
    pub(crate) fn update_detection(&mut self, dt: f32) {
        self.detection.regen_wait = (self.detection.regen_wait - dt).max(0.0);
        self.detection.next_in -= dt;
        if self.detection.next_in > 0.0 {
            return;
        }
        let step = PLAYER_INTERVAL - self.detection.next_in.min(0.0);
        self.detection.next_in = PLAYER_INTERVAL;
        let Some(t) = self.target(PLAYER_REF) else { return };
        let observers: Vec<FormId> = self.cells.values().flat_map(|rt| &rt.actors).map(|a| a.ref_id).collect();
        let mut of_player = HashMap::new();
        let mut most: Option<f32> = None;
        for r in observers {
            let Some(o) = self.observer(r) else { continue };
            let Some((v, _)) = self.detection_between(&o, &t) else { continue };
            of_player.insert(r, v);
            if self.would_attack_player(r) {
                most = Some(most.map_or(v, |m| m.max(v)));
            }
        }
        let s = self.detection_settings();
        let fighting = self.cells.values().flat_map(|rt| &rt.actors).any(|a| !a.dead && a.combat.as_ref().is_some_and(|c| c.target == PLAYER_REF && !c.fleeing));
        let d = &mut self.detection;
        d.of_player = of_player;
        if fighting {
            d.stealth_points = 0.0;
            return;
        }
        match most {
            Some(v) if v > 0.0 => {
                if d.regen_wait <= 0.0 {
                    log::debug!("an enemy is alert to the player ({v:.1})");
                }
                d.stealth_points = (d.stealth_points - v * s.drain_mult * step).max(0.0);
                d.regen_wait = s.alert_wait;
            }
            Some(v) if d.regen_wait <= 0.0 => d.stealth_points = (d.stealth_points + (-v * s.regen_mult).max(s.regen_min) * step).min(100.0),
            Some(_) => {}
            // No enemy near: nobody to hide from.
            None if d.regen_wait <= 0.0 => d.stealth_points = 100.0,
            None => {}
        }
    }

    /// How open the sneak eye is (0 hidden, 1 found): the most any actor
    /// detects the player by against the combat threshold, or how far the
    /// player's stealth points have gone.
    pub fn sneak_eye(&self) -> f32 {
        let s = self.detection_settings();
        let most = self.detection.of_player.values().copied().fold(f32::MIN, f32::max);
        let seen = (most / s.combat_threshold).clamp(0.0, 1.0);
        seen.max(1.0 - self.detection.stealth_points / 100.0)
    }

    /// Console: each loaded actor's detection of the player and what goes into it.
    pub fn describe_detection(&self) -> Vec<String> {
        let s = self.detection_settings();
        let [ambient, sun, lights] = self.light_parts(self.player.position, PLAYER_REF);
        let mut out = vec![format!(
            "player: light {:.0} (ambient {ambient:.0}, sun {sun:.0}, lights {lights:.0}), {}{}{}, armor {:.0}, sneak {:.0}; stealth points {:.0}; range {:.0}",
            self.light_level(PLAYER_REF),
            if self.player.sneaking { "sneaking" } else { "standing" },
            if self.player.moving { ", moving" } else { "" },
            if self.player.running { ", running" } else { "" },
            self.armor_weight(PLAYER_REF),
            self.actor_value(PLAYER_REF, av::SNEAK),
            self.detection.stealth_points,
            self.detection_range(&s),
        )];
        let Some(t) = self.target(PLAYER_REF) else { return out };
        let mut rows: Vec<(f32, String)> = Vec::new();
        for a in self.cells.values().flat_map(|rt| &rt.actors) {
            let Some(o) = self.observer(a.ref_id) else { continue };
            let Some((v, sees)) = self.detection_between(&o, &t) else { continue };
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
                    if self.finds_player(a.ref_id) { ", found" } else { "" }
                ),
            ));
        }
        rows.sort_by(|a, b| b.0.total_cmp(&a.0));
        out.extend(rows.into_iter().map(|r| r.1));
        out
    }
}
