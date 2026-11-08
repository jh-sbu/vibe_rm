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
//! sound  = fSneakSoundsMult * (movement + action)
//!          * (1, or fSneakSoundLosMult unseen)
//! movement = (fSneakEquippedWeightBase + fSneakEquippedWeightMult * armor
//!            weight) * (fSneakRunningMult running), 0 standing still
//! action = the sound level of a noise the target just made * fSneakActionMult
//! visual = (fDetectionSneakLightMod + light level) * fSneakLightMult
//!          * (1 + fSneakLightMoveMult moving + fSneakLightRunMult running),
//!          * (100 - observer blindness) / 100 while the target sneaks and
//!          the observer is out of combat; 0 when the target can't be seen
//! observer skill = (fSneakPerceptionSkillMin + (Max - Min) * Sneak / 100)
//!          * (1 + fSneakCombatMod when the target isn't its combat target)
//! ```
//!
//! Above 0 the target is detected. Against those who would attack them, the
//! player also has stealth points (the CK wiki's Stealth Points page): detection
//! above 0 drains them, below 0 refills them; an enemy attacks once they are
//! gone, or at once when its detection passes `iCombatStealthPointDetectionThreshold`
//! (`iCombatStealthPointSneakDetectionThreshold` against someone sneaking).
//! Crime witnesses and force greets need only detection above 0.
//!
//! Noises (UESP Skyrim:Sneak's "Action"; detection events): swinging a weapon
//! (its `VNAM` sound level), loosing an arrow, a blow or arrow landing (the
//! impact's), a script's `CreateDetectionEvent`. Each belongs to the actor who
//! made it and counts for a moment ([`NOISE_TIME`]), heard from where it was
//! made: a calm enemy that notices someone only by a noise elsewhere searches
//! there.
//!
//! Open questions (the target's skill, light level, the view cone...):
//! `known_ambiguities/detection.md`.

use std::collections::HashMap;

use esp::actor_value as av;
use esp::{FormId, LoadOrder};
use glam::Vec3;

use crate::ai::combat::{CombatState, gmst_f32, gmst_i32};
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
/// How long a noise counts towards its owner's detection (seconds; no source:
/// long enough for the once-a-second detection states to hear it).
pub(crate) const NOISE_TIME: f32 = 1.25;

/// The documented blindness AV percentage scales only the sight contribution.
/// Invalid values default to 0 blindness; valid values are constrained to the
/// documented percentage range.
fn visual_blindness_multiplier(blindness: f32, target_sneaking: bool, state: CombatState) -> f32 {
    if !target_sneaking || state != CombatState::None {
        return 1.0;
    }
    let blindness = if blindness.is_finite() {
        blindness.clamp(0.0, 100.0)
    } else {
        0.0
    };
    (100.0 - blindness) / 100.0
}

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
    /// `fSneakActionMult` (2): a noise's sound level's share of the sound.
    pub action_mult: f32,
    /// The sound levels' values, by the `VNAM` / `DATA` enum (Loud, Normal,
    /// Silent, Very Loud): `iSoundLevelLoud` .. Only Silent (10) is in
    /// Skyrim.esm; Skyrim SE read Normal 50, Loud 100, Very Loud 200
    /// (`test-mod autotest21`).
    pub sound_levels: [f32; 4],
    pub running_mult: f32,
    /// `fSneakEquippedWeightBase` / `Mult` (12, 0.5: UESP).
    pub weight_base: f32,
    pub weight_mult: f32,
    pub light_mod: f32,
    pub light_mult: f32,
    pub light_exterior_mult: f32,
    /// `fSneakLightMoveMult` / `RunMult` (0.01, 0.2 in the game: the data
    /// lacks them; `test-mod autotest2`).
    pub light_move_mult: f32,
    pub light_run_mult: f32,
    pub perception_min: f32,
    pub perception_max: f32,
    pub skill_mult: f32,
    pub sleep_bonus: f32,
    /// `fSneakCombatMod` (-0.4 in the game, `test-mod autotest2`).
    pub combat_mod: f32,
    /// `iCombatStealthPointDetectionThreshold`, and the one against someone
    /// sneaking (`iCombatStealthPointSneakDetectionThreshold`, 70 in the
    /// game, `test-mod autotest2`).
    pub combat_threshold: f32,
    pub combat_sneak_threshold: f32,
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
            action_mult: f("fSneakActionMult", 2.0),
            sound_levels: [
                gmst_i32(lo, "iSoundLevelLoud", 100) as f32,
                gmst_i32(lo, "iSoundLevelNormal", 50) as f32,
                gmst_i32(lo, "iSoundLevelSilent", 10) as f32,
                gmst_i32(lo, "iSoundLevelVeryLoud", 200) as f32,
            ],
            running_mult: f("fSneakRunningMult", 2.0),
            weight_base: f("fSneakEquippedWeightBase", 12.0),
            weight_mult: f("fSneakEquippedWeightMult", 0.5),
            light_mod: f("fDetectionSneakLightMod", 15.0),
            light_mult: f("fSneakLightMult", 0.33),
            light_exterior_mult: f("fSneakLightExteriorMult", 0.5),
            light_move_mult: f("fSneakLightMoveMult", 0.01),
            light_run_mult: f("fSneakLightRunMult", 0.2),
            perception_min: f("fSneakPerceptionSkillMin", 0.0),
            perception_max: f("fSneakPerceptionSkillMax", 100.0),
            skill_mult: f("fSneakSkillMult", 0.5),
            sleep_bonus: f("fSneakSleepBonus", 0.0),
            combat_mod: f("fSneakCombatMod", -0.4),
            combat_threshold: gmst_i32(lo, "iCombatStealthPointDetectionThreshold", 25) as f32,
            combat_sneak_threshold: gmst_i32(lo, "iCombatStealthPointSneakDetectionThreshold", 70)
                as f32,
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
    /// `of_player` less those the stealth meter leaves out
    /// (`SetNotShowOnStealthMeter`, while not hostile).
    pub(crate) on_meter: HashMap<FormId, f32>,
    /// Stealth points of the player and of actors enemies have been alert to
    /// (100 when missing).
    stealth: HashMap<FormId, Stealth>,
    /// The noise each actor (or the player) made last, while it counts.
    noises: HashMap<FormId, Noise>,
    next_in: f32,
    states_in: f32,
}

/// A noise someone made (a detection event): its sound level, where (`None`:
/// wherever its owner is, as for a swing), and seconds it still counts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Noise {
    pub level: f32,
    pub at: Option<Vec3>,
    left: f32,
}

impl Default for Detection {
    fn default() -> Self {
        Detection {
            settings: Default::default(),
            of_player: HashMap::new(),
            on_meter: HashMap::new(),
            stealth: HashMap::new(),
            noises: HashMap::new(),
            next_in: 0.0,
            states_in: 0.0,
        }
    }
}

impl Detection {
    pub(crate) fn saved_stealth_points(&self) -> HashMap<FormId, (f32, f32)> {
        self.stealth
            .iter()
            .map(|(&actor, state)| (actor, (state.points, state.regen_wait)))
            .collect()
    }

    /// Replace persisted reservoirs and force the derived observations to be
    /// recomputed on the next detection update.
    pub(crate) fn restore_stealth_points(&mut self, saved: HashMap<FormId, (f32, f32)>) {
        self.stealth = saved
            .into_iter()
            .map(|(actor, (points, regen_wait))| (actor, Stealth { points, regen_wait }))
            .collect();
        self.of_player.clear();
        self.on_meter.clear();
        self.next_in = 0.0;
        self.states_in = 0.0;
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
    /// Seconds since it last detected its target at all, and whether it has
    /// seen the target fleeing while it searched.
    unseen: f32,
    fled: bool,
}

impl Search {
    /// Note a state update: whether the target was detected, and whether it
    /// is fleeing. True when the searcher gives up a lost target that fled,
    /// undetected for `remove_after` seconds
    /// (`fCombatDetectionFleeingLostRemoveTime`; `known_ambiguities/detection.md`).
    fn forgets_fled_target(
        &mut self,
        detected: bool,
        fleeing: bool,
        step: f32,
        remove_after: f32,
    ) -> bool {
        self.fled |= fleeing;
        self.unseen = if detected { 0.0 } else { self.unseen + step };
        self.lost && self.fled && self.unseen >= remove_after
    }
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
    /// Magic: invisible (no visual factor), and the share of movement noise
    /// muffled (`MovementNoiseMult`: Muffle, Muffled Movement, Silence).
    invisible: bool,
    muffle: f32,
}

impl Engine {
    /// The explicit actor flag set by Papyrus `SetAlert`, also queried by
    /// `IsAlerted` and the `GetIsAlerted` condition. The public Skyrim docs do
    /// not establish its relationship to detection's Alert / Combat / Lost
    /// states; see `known_ambiguities/detection.md`.
    pub fn is_alerted(&self, actor: FormId) -> bool {
        self.world_state.alerted.contains(&actor)
    }

    /// Set or clear the explicit alerted flag without inventing a combat target
    /// or interrupting a search. Retained in memory even while its cell unloads.
    pub fn set_alerted(&mut self, actor: FormId, alerted: bool) {
        if !self.is_actor_ref(actor) {
            return;
        }
        if alerted {
            self.world_state.alerted.insert(actor);
            if actor != PLAYER_REF {
                self.draw_weapon(actor, true);
            }
        } else {
            self.world_state.alerted.remove(&actor);
        }
    }

    /// `owner` makes a noise of `level` (an `iSoundLevel*` value, or a
    /// script's 0 to 100) at `at` (`None`: where it is, moving with it). It
    /// counts towards detecting `owner` for [`NOISE_TIME`]; a louder noise
    /// still counting isn't drowned out by a quieter one.
    pub(crate) fn make_noise(&mut self, owner: FormId, level: f32, at: Option<Vec3>) {
        if !(level > 0.0 && level.is_finite()) || !self.is_actor_ref(owner) {
            return;
        }
        let noise = Noise {
            level,
            at,
            left: NOISE_TIME,
        };
        match self.detection.noises.get(&owner) {
            Some(n) if n.level > level => {}
            _ => {
                log::debug!("{owner} makes a noise of {level} at {at:?}");
                self.detection.noises.insert(owner, noise);
            }
        }
    }

    /// The value of a record's sound level (0 Loud, 1 Normal, 2 Silent, 3
    /// Very Loud); `None` for any other.
    pub(crate) fn sound_level(&self, level: u32) -> Option<f32> {
        self.detection_settings()
            .sound_levels
            .get(level as usize)
            .copied()
    }

    /// The sound level of a record's `tag` at byte `at`, as a value.
    fn record_sound_level(&self, rec: FormId, tag: &[u8; 4], at: usize) -> Option<f32> {
        let r = self.lo.get(rec)?;
        let d = r.get(tag)?.get(at..at + 4)?;
        self.sound_level(u32::from_le_bytes(d.try_into().ok()?))
    }

    /// `actor` attacks with `weapon` (bare hands when `None`: the `Unarmed`
    /// weapon's): the weapon's detection sound level (`VNAM`; daggers and
    /// bows Silent, most two-handed weapons Loud), where it is.
    pub(crate) fn weapon_noise(&mut self, actor: FormId, weapon: Option<FormId>) {
        let weapon = weapon.unwrap_or(crate::ai::combat::UNARMED);
        if let Some(level) = self.record_sound_level(weapon, b"VNAM", 0) {
            self.make_noise(actor, level, None);
        }
    }

    /// An impact (`IPCT`) of `owner`'s landing at `at`: its sound level
    /// (`DATA` at 0x10; nearly all Normal).
    pub(crate) fn impact_noise(&mut self, owner: Option<FormId>, ipct: FormId, at: Vec3) {
        if let Some(owner) = owner
            && let Some(level) = self.record_sound_level(ipct, b"DATA", 0x10)
        {
            self.make_noise(owner, level, Some(at));
        }
    }

    /// The noise `r` is making now, if any.
    pub(crate) fn noise_of(&self, r: FormId) -> Option<Noise> {
        self.detection.noises.get(&r).copied()
    }

    /// Noises count down; those done are forgotten.
    fn age_noises(&mut self, dt: f32) {
        self.detection.noises.retain(|_, n| {
            n.left -= dt;
            n.left > 0.0
        });
    }

    /// Where `observer` would look for `target` because of a noise: the
    /// target's noise elsewhere, within reach, while the observer doesn't see
    /// the target itself.
    fn noise_lure(&self, observer: FormId, target: FormId) -> Option<Vec3> {
        let at = self.noise_of(target)?.at?;
        let o = self.observer(observer)?;
        let t = self.target(target)?;
        let s = self.detection_settings();
        if o.eye.distance(at) >= self.detection_range(&s) {
            return None;
        }
        match self.detection_between(&o, &t) {
            Some((_, true)) => None,
            _ => Some(at),
        }
    }

    pub(crate) fn detection_settings(&self) -> DetectionSettings {
        *self
            .detection
            .settings
            .get_or_init(|| DetectionSettings::load(&self.lo))
    }

    fn observer(&self, r: FormId) -> Option<Observer> {
        let a = self.actor_ref(r)?;
        if a.dead || a.exec.bleeding.is_some() || self.is_disabled(r) {
            return None;
        }
        Some(Observer {
            id: r,
            eye: a.exec.pos + Vec3::Z * EYE_HEIGHT * a.scale,
            heading: a.exec.heading,
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
                running: self.player.running && !self.ignores_running(r),
                armor_weight: self.armor_weight(r),
                sneak: self.actor_value(r, av::SNEAK),
                stealth: self.stealth_perks(r),
                invisible: self.actor_value(r, av::INVISIBILITY) > 0.0,
                muffle: self.actor_value(r, av::MOVEMENT_NOISE_MULT).clamp(0.0, 1.0),
            });
        }
        let a = self.actor_ref(r)?;
        if self.is_disabled(r) {
            return None;
        }
        let moving = a.speed() > 1.0 && !a.dead;
        Some(Target {
            id: r,
            at: a.exec.pos + Vec3::Z * EYE_HEIGHT * a.scale,
            sneaking: a.is_sneaking(),
            moving,
            running: moving
                && a.gait() == crate::world::footsteps::Gait::Run
                && !self.ignores_running(r),
            armor_weight: self.armor_weight(r),
            sneak: self.actor_value(r, av::SNEAK),
            stealth: self.stealth_perks(r),
            invisible: self.actor_value(r, av::INVISIBILITY) > 0.0,
            muffle: self.actor_value(r, av::MOVEMENT_NOISE_MULT).clamp(0.0, 1.0),
        })
    }

    /// "Ignore Running During Detection": running counts as moving (no
    /// vanilla perk has it).
    fn ignores_running(&self, r: FormId) -> bool {
        self.perk_flag(crate::perks::ep::IGNORE_RUNNING_DURING_DETECTION, r, &[])
    }

    fn stealth_perks(&self, r: FormId) -> f32 {
        self.perk_entry_point(crate::perks::ep::MOD_DETECTION_SNEAK_SKILL, r, &[], 0.0)
    }

    /// The weight of the armor an actor wears.
    fn armor_weight(&self, r: FormId) -> f32 {
        let Some(inv) = self.inventories.get(&r) else {
            return 0.0;
        };
        inv.equipped()
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

    /// The observer's current blindness affects only sight of a sneaking
    /// target while they have no combat state.
    fn blindness_visual_multiplier(&self, observer: FormId, target_sneaking: bool) -> f32 {
        visual_blindness_multiplier(
            self.actor_value(observer, av::BLINDNESS),
            target_sneaking,
            self.combat_state(observer),
        )
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

    /// Whether `from` has a clear line to `to` (`GetLineOfSight`): actors look
    /// from their eyes, other references from their position. An NPC sees only
    /// within its view cone: one 200 units behind another wasn't in its line of
    /// sight in the game (test-mod autotest22). None when either has no place
    /// in the world.
    pub(crate) fn line_of_sight(&self, from: FormId, to: FormId) -> Option<bool> {
        let a = self.sight_point(from)?;
        let t = self.sight_point(to)?;
        if from != PLAYER_REF
            && let Some(actor) = self.actor_ref(from)
        {
            let facing = Vec3::new(actor.exec.heading.sin(), actor.exec.heading.cos(), 0.0);
            let flat = Vec3::new(t.x - a.x, t.y - a.y, 0.0).normalize_or_zero();
            if flat != Vec3::ZERO && facing.dot(flat) < VIEW_HALF_ANGLE.to_radians().cos() {
                return Some(false);
            }
        }
        Some(self.clear_line(a, t, from, to))
    }

    fn sight_point(&self, r: FormId) -> Option<Vec3> {
        if r == PLAYER_REF {
            return Some(self.player.eye());
        }
        if let Some(a) = self.actor_ref(r) {
            return Some(a.exec.pos + Vec3::Z * EYE_HEIGHT * a.scale);
        }
        self.ref_position(r)
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
                Some(a) => a.exec.pos + Vec3::Z * EYE_HEIGHT * 0.6 * a.scale,
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

    /// The detection value and whether `o` has a geometric view of `t`
    /// (blindness and light aside).
    fn detection_between(&self, o: &Observer, t: &Target) -> Option<(f32, bool)> {
        let s = self.detection_settings();
        let range = self.detection_range(&s);
        let attenuation_at = |d: f32| ((range - d).max(0.0) / range).powf(s.attenuation_exponent);
        let to = t.at - o.eye;
        let dist = to.length();
        // A noise made elsewhere (an arrow landing) is heard from there, as
        // far as detection reaches from it, even beyond the target's reach.
        let noise = self.noise_of(t.id);
        let noise_elsewhere = noise
            .and_then(|n| n.at)
            .filter(|at| o.eye.distance(*at) < range);
        let in_reach = dist < range;
        if !in_reach && noise_elsewhere.is_none() {
            return None;
        }
        let attenuation = attenuation_at(dist);
        let los = in_reach && self.clear_line(o.eye, t.at, o.id, t.id);
        let facing = Vec3::new(o.heading.sin(), o.heading.cos(), 0.0);
        let flat = Vec3::new(to.x, to.y, 0.0).normalize_or_zero();
        let in_view = flat == Vec3::ZERO || facing.dot(flat) >= VIEW_HALF_ANGLE.to_radians().cos();
        let sees = los && in_view && !o.sleeping && !t.invisible;
        // Its noise (UESP's "Action": sound level * fSneakActionMult). One made
        // where the target is joins the movement's sound; one made elsewhere
        // is attenuated, and heard through walls or not, from there. Muffle
        // doesn't quiet it.
        let (action_here, action_elsewhere) = match (noise, noise_elsewhere) {
            (
                Some(Noise {
                    at: None, level, ..
                }),
                _,
            ) => (level * s.action_mult, 0.0),
            (Some(n), Some(p)) => {
                let heard = if self.clear_line(o.eye, p, o.id, FormId::NULL) {
                    1.0
                } else {
                    s.sound_los_mult
                };
                let a = s.sounds_mult * n.level * s.action_mult * heard;
                (0.0, attenuation_at(o.eye.distance(p)) * a)
            }
            _ => (0.0, 0.0),
        };
        // The observer's perks for noticing movement and what light shows.
        use crate::perks::ep;
        let about = [Some(t.id)];
        let movement = if t.moving {
            (s.weight_base + s.weight_mult * t.armor_weight)
                * if t.running { s.running_mult } else { 1.0 }
                * self.perk_entry_point(ep::MOD_DETECTION_MOVEMENT, o.id, &about, 1.0)
                * (1.0 - t.muffle)
        } else {
            0.0
        };
        let sound =
            s.sounds_mult * (movement + action_here) * if los { 1.0 } else { s.sound_los_mult };
        let visual = if sees {
            let light = self.light_level_at(t.at - Vec3::Z * 40.0, t.id);
            (s.light_mod + light)
                * s.light_mult
                * (1.0
                    + if t.moving { s.light_move_mult } else { 0.0 }
                    + if t.running { s.light_run_mult } else { 0.0 })
                * if self.outdoors() {
                    s.light_exterior_mult
                } else {
                    1.0
                }
                * self.perk_entry_point(ep::MOD_DETECTION_LIGHT, o.id, &about, 1.0)
                * self.blindness_visual_multiplier(o.id, t.sneaking)
        } else {
            0.0
        };
        // The target's Stealth perks cut the observer's skill factor (UESP).
        let observer_skill = (s.perception_min
            + (s.perception_max - s.perception_min) * o.sneak / 100.0)
            * (1.0
                + if o.sleeping { s.sleep_bonus } else { 0.0 }
                + if self.combat_target(o.id) == Some(t.id) {
                    0.0
                } else {
                    s.combat_mod
                })
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
                + action_elsewhere
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
        let s = self.detection_settings();
        let threshold = if self.is_sneaking(target) {
            s.combat_sneak_threshold
        } else {
            s.combat_threshold
        };
        (v > 0.0 && self.stealth_points(target) <= 0.0) || v > threshold
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
    pub(crate) fn would_attack(&mut self, r: FormId, target: FormId) -> bool {
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
        self.hostile_to_actor(&stats, target, &theirs)
    }

    /// Work out loaded actors' detection of the player four times a second and
    /// drain or refill the player's stealth points by the most any enemy has;
    /// once a second, the same for actors as targets, and actors' detection
    /// states (`update_detection_states`).
    pub(crate) fn update_detection(&mut self, dt: f32) {
        self.age_noises(dt);
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
        let mut on_meter = HashMap::new();
        let mut most: Option<f32> = None;
        for r in observers {
            let Some(o) = self.observer(r) else { continue };
            let Some((v, _)) = self.detection_between(&o, &t) else {
                continue;
            };
            of_player.insert(r, v);
            let hostile = self.would_attack_player(r);
            if hostile {
                most = Some(most.map_or(v, |m| m.max(v)));
            }
            if hostile || !self.actor_flags.not_on_meter.contains(&r) {
                on_meter.insert(r, v);
            }
        }
        let s = self.detection_settings();
        let fighting = self.fought(PLAYER_REF);
        self.detection.of_player = of_player;
        self.detection.on_meter = on_meter;
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
        self.update_time_to_go();
    }

    /// Whether anyone is fighting `r` (not fleeing from it).
    fn fought(&self, r: FormId) -> bool {
        self.cells.values().flat_map(|rt| &rt.actors).any(|a| {
            !a.dead
                && a.exec
                    .combat
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
        let fled_remove = gmst_f32(&self.lo, "fCombatDetectionFleeingLostRemoveTime", 30.0);
        let actors: Vec<FormId> = self
            .cells
            .values()
            .flat_map(|rt| &rt.actors)
            .filter(|a| !a.dead && a.exec.bleeding.is_none())
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
                e.actor_ref(t).filter(|a| !a.dead).map(|a| a.exec.pos)
            }
        };
        for &r in &actors {
            let Some(a) = self.actor_ref(r) else { continue };
            let (combat, search) = (
                a.exec.combat.as_ref().map(|c| (c.target, c.fleeing)),
                a.exec.search,
            );
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
                let Some(c) = self.actor_mut(r).and_then(|a| a.exec.combat.as_mut()) else {
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
                let fleeing = self.is_fleeing(sr.target);
                if sr.forgets_fled_target(v.is_some_and(|v| v > 0.0), fleeing, step, fled_remove) {
                    log::info!("{r} gives up on {}, who fled", sr.target);
                    self.calm_down(r, Some(b"LOTN"));
                    continue;
                }
                if v.is_some_and(|v| self.enough_to_find(v, sr.target)) {
                    // `detect_enemies` starts the fight.
                    continue;
                }
                if v.is_some_and(|v| v > 0.0) {
                    sr.at = self.noise_lure(r, sr.target).or(alive).unwrap_or(sr.at);
                    sr.dwell = 0.0;
                } else if self.stealth_points(sr.target) >= 100.0 {
                    self.calm_down(r, Some(if sr.lost { b"LOTN" } else { b"ALTN" }));
                    continue;
                }
                sr.dwell -= step;
                sr.idle_line -= step;
                let here = self.actor_ref(r).map_or(sr.at, |a| a.exec.pos);
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
                    a.exec.search = Some(sr);
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
                // Heard only by a noise elsewhere: it looks there.
                if let Some(t) = best
                    && let Some(at) = self.noise_lure(r, t).or_else(|| position(self, t))
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
    pub(crate) fn start_search(&mut self, r: FormId, target: FormId, at: Vec3, lost: bool) {
        let idle_line = self.detection_idle_wait();
        let Some(a) = self.actor_mut(r) else { return };
        a.exec.search = Some(Search {
            target,
            at,
            lost,
            dwell: SEARCH_DWELL.1,
            idle_line,
            unseen: 0.0,
            fled: false,
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
        let Some(c) = self.actor_ref(r).and_then(|a| a.exec.combat.as_ref()) else {
            return;
        };
        let (target, last) = (c.target, c.last_seen);
        let about = target == PLAYER_REF && !self.player_dead()
            || self.actor_ref(target).is_some_and(|t| !t.dead);
        let at = last.or_else(|| {
            if target == PLAYER_REF {
                Some(self.player.position)
            } else {
                self.actor_ref(target).map(|t| t.exec.pos)
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
        if self.actor_ref(r).is_some_and(|a| a.exec.search.is_some()) {
            self.calm_down(r, None);
        }
    }

    /// A searcher gives up: weapons away, back to its packages.
    fn calm_down(&mut self, r: FormId, topic: Option<&[u8; 4]>) {
        let Some(a) = self.actor_mut(r) else { return };
        let target = a.exec.search.map(|s| s.target);
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
            .on_meter
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
        if let Some(n) = self.noise_of(PLAYER_REF) {
            out.push(format!(
                "player's noise: level {:.0} {}, {:.1}s left",
                n.level,
                n.at.map_or("where they are".to_owned(), |p| format!("at {p:.0}")),
                n.left
            ));
        }
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

#[cfg(test)]
mod persistence_tests {
    use super::*;
    use esp::writer::write_plugin;

    struct Fixture(std::path::PathBuf);

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn engine(label: &str) -> (Fixture, Engine) {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "vrm-stealth-save-{}-{label}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir(&dir).unwrap();
        std::fs::write(
            dir.join("Stealth.esp"),
            write_plugin(0, &[], "test", "stealth save tests", 0x8ff, &[]),
        )
        .unwrap();
        let engine = crate::stats::action_tests::headless_engine(&dir, "Stealth.esp");
        (Fixture(dir), engine)
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic plugin only"]
    fn production_save_load_continues_stealth_drain_and_regeneration_wait() {
        const ACTOR: FormId = FormId(0x801);
        let (_source_dir, mut source) = engine("source");
        source.detection.stealth.insert(
            PLAYER_REF,
            Stealth {
                points: 50.0,
                regen_wait: 3.5,
            },
        );
        source.detection.stealth.insert(
            ACTOR,
            Stealth {
                points: 23.25,
                regen_wait: 1.25,
            },
        );
        let bytes = crate::save::state::Snapshot::capture(&source)
            .write()
            .to_bytes();

        let (_target_dir, mut target) = engine("target");
        target.detection.stealth.insert(
            PLAYER_REF,
            Stealth {
                points: 4.0,
                regen_wait: 99.0,
            },
        );
        target.detection.stealth.insert(
            FormId(0x802),
            Stealth {
                points: 12.0,
                regen_wait: 0.0,
            },
        );
        target.detection.of_player.insert(FormId(0x803), 45.0);
        target.detection.next_in = 4.0;
        target.detection.states_in = 2.0;
        crate::save::state::Snapshot::read(
            &crate::save::file::SaveFile::from_bytes(&bytes).unwrap(),
        )
        .unwrap()
        .apply(&mut target)
        .unwrap();

        assert_eq!(
            target.detection.saved_stealth_points(),
            [(PLAYER_REF, (50.0, 3.5)), (ACTOR, (23.25, 1.25)),].into()
        );
        assert!(target.detection.of_player.is_empty());
        assert_eq!(target.detection.next_in, 0.0);
        assert_eq!(target.detection.states_in, 0.0);

        // Continue the existing Stealth::step algorithm from the loaded
        // reservoir; positive detection drains it and restarts the alert wait.
        let mut drained = target.detection.stealth[&PLAYER_REF];
        drained.step(Some(10.0), false, &target.detection_settings(), 0.5);
        assert_eq!(drained.points, 35.0);
        assert_eq!(drained.regen_wait, target.detection_settings().alert_wait);

        // The production update path advances the saved delay without
        // restoring points, then resumes regeneration once it expires.
        target.update_detection(2.0);
        assert_eq!(target.stealth_points(PLAYER_REF), 50.0);
        assert_eq!(target.detection.stealth[&PLAYER_REF].regen_wait, 1.5);
        target.update_detection(1.5);
        assert_eq!(target.stealth_points(PLAYER_REF), 100.0);

        // A legacy file without STPT replaces, rather than retaining, memory.
        let mut legacy = crate::save::file::SaveFile::from_bytes(&bytes).unwrap();
        legacy.sections.retain(|(tag, _)| tag != b"STPT");
        crate::save::state::Snapshot::read(&legacy)
            .unwrap()
            .apply(&mut target)
            .unwrap();
        assert!(target.detection.stealth.is_empty());
        assert!(target.detection.of_player.is_empty());
    }
}

#[cfg(test)]
mod blindness_tests {
    use super::*;

    #[test]
    fn blindness_scales_vision_only_in_the_documented_context() {
        assert_eq!(
            visual_blindness_multiplier(0.0, true, CombatState::None),
            1.0
        );
        assert_eq!(
            visual_blindness_multiplier(40.0, true, CombatState::None),
            0.6
        );
        assert_eq!(
            visual_blindness_multiplier(100.0, true, CombatState::None),
            0.0
        );

        assert_eq!(
            visual_blindness_multiplier(100.0, false, CombatState::None),
            1.0
        );
        assert_eq!(
            visual_blindness_multiplier(100.0, true, CombatState::Fighting),
            1.0
        );
        assert_eq!(
            visual_blindness_multiplier(100.0, true, CombatState::Searching),
            1.0
        );
    }

    #[test]
    fn invalid_blindness_values_default_to_zero_and_finite_values_are_clamped() {
        assert_eq!(
            visual_blindness_multiplier(-10.0, true, CombatState::None),
            1.0
        );
        assert_eq!(
            visual_blindness_multiplier(120.0, true, CombatState::None),
            0.0
        );
        assert_eq!(
            visual_blindness_multiplier(f32::NAN, true, CombatState::None),
            1.0
        );
        assert_eq!(
            visual_blindness_multiplier(f32::INFINITY, true, CombatState::None),
            1.0
        );
    }

    #[test]
    #[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
    fn blindness_changes_the_actual_visual_term_but_not_sound_or_visibility() {
        struct Fixture(std::path::PathBuf);
        impl Drop for Fixture {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }

        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let fixture = Fixture(std::env::temp_dir().join(format!(
            "vrm-detection-blindness-{}-{stamp}",
            std::process::id()
        )));
        std::fs::create_dir(&fixture.0).unwrap();
        let plugin = esp::writer::write_plugin(0, &[], "test", "blindness", 0x900, &[]);
        std::fs::write(fixture.0.join("Blindness.esp"), plugin).unwrap();
        let mut engine = crate::stats::action_tests::headless_engine(&fixture.0, "Blindness.esp");
        let observer = Observer {
            id: PLAYER_REF,
            eye: Vec3::ZERO,
            heading: 0.0,
            sneak: engine.actor_value(PLAYER_REF, av::SNEAK),
            sleeping: false,
        };
        let mut target = Target {
            id: FormId(0x901),
            at: Vec3::new(0.0, 200.0, 0.0),
            sneaking: true,
            moving: true,
            running: false,
            armor_weight: 0.0,
            sneak: 0.0,
            stealth: 0.0,
            invisible: false,
            muffle: 0.0,
        };

        engine.set_actor_value(PLAYER_REF, av::BLINDNESS, 0.0);
        let (unimpaired, sees_unimpaired) = engine.detection_between(&observer, &target).unwrap();
        engine.set_actor_value(PLAYER_REF, av::BLINDNESS, 50.0);
        let (half_blind, sees_half_blind) = engine.detection_between(&observer, &target).unwrap();
        engine.set_actor_value(PLAYER_REF, av::BLINDNESS, 100.0);
        let (fully_blind, sees_fully_blind) = engine.detection_between(&observer, &target).unwrap();

        assert!((half_blind - (unimpaired + fully_blind) * 0.5).abs() < 0.001);
        assert!(unimpaired > half_blind && half_blind > fully_blind);
        assert!(sees_unimpaired && sees_half_blind && sees_fully_blind);

        // At maximum blindness, moving sound still contributes to the actual
        // detection formula because stationary and moving share zero vision.
        target.moving = false;
        let (fully_blind_stationary, sees_stationary) =
            engine.detection_between(&observer, &target).unwrap();
        assert!(fully_blind > fully_blind_stationary);
        assert!(sees_stationary);

        target.moving = true;
        engine.set_actor_value(PLAYER_REF, av::BLINDNESS, 0.0);
        engine.set_actor_value(target.id, av::BLINDNESS, 100.0);
        let (target_blind, sees_target_blind) =
            engine.detection_between(&observer, &target).unwrap();
        assert_eq!(target_blind, unimpaired);
        assert_eq!(sees_target_blind, sees_unimpaired);
    }
}

#[cfg(test)]
mod search_tests {
    use super::*;

    fn search(lost: bool) -> Search {
        Search {
            target: PLAYER_REF,
            at: Vec3::ZERO,
            lost,
            dwell: 0.0,
            idle_line: 0.0,
            unseen: 0.0,
            fled: false,
        }
    }

    #[test]
    fn a_lost_target_that_fled_is_forgotten_after_the_remove_time_unseen() {
        let mut s = search(true);
        assert!(!s.forgets_fled_target(false, true, 10.0, 30.0));
        // It stopped fleeing (safe, out of the fight): it still fled.
        assert!(!s.forgets_fled_target(false, false, 10.0, 30.0));
        assert!(s.forgets_fled_target(false, false, 10.0, 30.0));
    }

    #[test]
    fn detecting_the_target_restarts_the_remove_time() {
        let mut s = search(true);
        assert!(!s.forgets_fled_target(false, true, 25.0, 30.0));
        assert!(!s.forgets_fled_target(true, true, 1.0, 30.0));
        assert!(!s.forgets_fled_target(false, true, 25.0, 30.0));
        assert!(s.forgets_fled_target(false, true, 5.0, 30.0));
    }

    #[test]
    fn targets_that_never_fled_and_alert_searches_keep_the_stealth_rule() {
        let mut s = search(true);
        assert!(!s.forgets_fled_target(false, false, 100.0, 30.0));
        let mut alert = search(false);
        assert!(!alert.forgets_fled_target(false, true, 100.0, 30.0));
    }
}

#[cfg(test)]
mod noise_tests {
    use super::*;
    use esp::writer::{Node, RecordBuilder, write_plugin};

    struct Fixture(std::path::PathBuf);

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    const SWORD: FormId = FormId(0x800);
    const GREATSWORD: FormId = FormId(0x801);
    const IMPACT: FormId = FormId(0x802);
    const OBSERVER: FormId = FormId(0x901);

    /// An engine with a Normal one-handed sword, a Loud greatsword, the
    /// `Unarmed` weapon (Normal) and a Normal impact.
    fn engine() -> (Fixture, Engine) {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "vrm-detection-noise-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir(&dir).unwrap();
        let weapon = |id: FormId, level: u32| {
            RecordBuilder::new(b"WEAP", id).field(b"VNAM", &level.to_le_bytes())
        };
        let mut data = [0u8; 24];
        data[16..20].copy_from_slice(&1u32.to_le_bytes());
        data[20] = 1; // No Decal Data.
        let groups = [
            Node::top(
                b"WEAP",
                vec![
                    weapon(SWORD, 1),
                    weapon(GREATSWORD, 0),
                    weapon(crate::ai::combat::UNARMED, 1),
                ],
            ),
            Node::top(
                b"IPCT",
                vec![RecordBuilder::new(b"IPCT", IMPACT).field(b"DATA", &data)],
            ),
        ];
        std::fs::write(
            dir.join("Noise.esp"),
            write_plugin(0, &[], "test", "noise", 0x900, &groups),
        )
        .unwrap();
        let engine = crate::stats::action_tests::headless_engine(&dir, "Noise.esp");
        (Fixture(dir), engine)
    }

    fn observer() -> Observer {
        Observer {
            id: OBSERVER,
            eye: Vec3::ZERO,
            heading: 0.0,
            sneak: 0.0,
            sleeping: false,
        }
    }

    /// The player standing still `y` units ahead of the observer.
    fn player_at(y: f32) -> Target {
        Target {
            id: PLAYER_REF,
            at: Vec3::new(0.0, y, 0.0),
            sneaking: true,
            moving: false,
            running: false,
            armor_weight: 0.0,
            sneak: 0.0,
            stealth: 0.0,
            invisible: false,
            muffle: 0.0,
        }
    }

    fn attenuation(e: &Engine, d: f32) -> f32 {
        let s = e.detection_settings();
        let range = e.detection_range(&s);
        ((range - d) / range).powf(s.attenuation_exponent)
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic plugin only"]
    fn sound_levels_and_weapon_swings_add_the_action_term() {
        let (_dir, mut e) = engine();
        // The plugin has no iSoundLevel* settings: the values SE read.
        assert_eq!(e.sound_level(0), Some(100.0));
        assert_eq!(e.sound_level(1), Some(50.0));
        assert_eq!(e.sound_level(2), Some(10.0));
        assert_eq!(e.sound_level(3), Some(200.0));
        assert_eq!(e.sound_level(4), None);

        let (o, t) = (observer(), player_at(500.0));
        let (quiet, _) = e.detection_between(&o, &t).unwrap();
        e.weapon_noise(PLAYER_REF, Some(SWORD));
        let (swing, _) = e.detection_between(&o, &t).unwrap();
        // Normal (50) * fSneakActionMult (2), attenuated with the target.
        let expected = attenuation(&e, 500.0) * 50.0 * 2.0;
        assert!((swing - quiet - expected).abs() < 0.01, "{swing} - {quiet}");

        // A louder swing replaces it; a quieter one doesn't drown it out.
        e.weapon_noise(PLAYER_REF, Some(GREATSWORD));
        assert_eq!(e.noise_of(PLAYER_REF).unwrap().level, 100.0);
        e.weapon_noise(PLAYER_REF, None);
        assert_eq!(e.noise_of(PLAYER_REF).unwrap().level, 100.0);

        // Muffle quiets movement, not the noise.
        let mut muffled = player_at(500.0);
        muffled.muffle = 1.0;
        let (loud, _) = e.detection_between(&o, &t).unwrap();
        let (loud_muffled, _) = e.detection_between(&o, &muffled).unwrap();
        assert_eq!(loud, loud_muffled);

        // It counts for a moment, then is gone.
        e.age_noises(NOISE_TIME - 0.1);
        assert!(e.noise_of(PLAYER_REF).is_some());
        e.age_noises(0.2);
        assert!(e.noise_of(PLAYER_REF).is_none());
        let (after, _) = e.detection_between(&o, &t).unwrap();
        assert_eq!(after, quiet);
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic plugin only"]
    fn a_noise_elsewhere_is_heard_from_where_it_was_made() {
        let (_dir, mut e) = engine();
        let o = observer();
        let range = e.detection_range(&e.detection_settings());
        // The player far beyond reach: nothing to detect them by.
        let far = player_at(range * 2.0);
        assert!(e.detection_between(&o, &far).is_none());

        // Their arrow lands 300 units from the observer.
        let at = Vec3::new(300.0, 0.0, 0.0);
        e.impact_noise(Some(PLAYER_REF), IMPACT, at);
        assert_eq!(
            e.noise_of(PLAYER_REF),
            Some(Noise {
                level: 50.0,
                at: Some(at),
                left: NOISE_TIME
            })
        );
        let (v, sees) = e.detection_between(&o, &far).unwrap();
        assert!(!sees);
        let s = e.detection_settings();
        let observer_skill = s.perception_min * (1.0 + s.combat_mod);
        let expected = s.base + attenuation(&e, 300.0) * 50.0 * 2.0 + observer_skill;
        assert!((v - expected).abs() < 0.01, "{v} against {expected}");

        // Beyond reach of the observer too: still nothing.
        e.age_noises(NOISE_TIME);
        e.impact_noise(Some(PLAYER_REF), IMPACT, Vec3::new(range + 10.0, 0.0, 0.0));
        assert!(e.detection_between(&o, &far).is_none());

        // An impact without an owner, or a noise of nobody's, makes none.
        e.age_noises(NOISE_TIME);
        e.impact_noise(None, IMPACT, at);
        e.make_noise(FormId(0x7777), 50.0, Some(at));
        e.make_noise(PLAYER_REF, 0.0, Some(at));
        assert!(e.noise_of(PLAYER_REF).is_none());
        assert!(e.noise_of(FormId(0x7777)).is_none());
    }
}
