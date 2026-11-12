//! Explosions (`EXPL`): what one is, as its record gives it.
//!
//! `DATA` (xEdit's `wbDefinitionsTES5.pas`, UESP's EXPL page, CommonLibSSE's
//! `BGSExplosionData`): light, sound 1, sound 2, impact data set, placed
//! object, spawn projectile (forms), force, damage, radius, image space
//! radius, vertical offset mult (floats), flags, sound level. 273 vanilla
//! records have all 52 bytes; one has 48, without the vertical offset mult
//! (`research/explosions.md`); six shorter ones are of a layout nobody has
//! described, read for their model and enchantment only.
//! `known_ambiguities/explosions.md` holds the choices.

use esp::{FormId, LoadOrder};
use glam::Vec3;
use papyrus::Value;

use esp::actor_value as av;

use crate::ai::combat::gmst_f32;
use crate::engine::{Engine, PLAYER_REF};

/// `DATA` flags.
pub mod flag {
    pub const WORLD_ORIENTATION: u32 = 0x2;
    pub const KNOCK_DOWN_ALWAYS: u32 = 0x4;
    pub const KNOCK_DOWN_BY_FORMULA: u32 = 0x8;
    pub const IGNORE_LOS: u32 = 0x10;
    pub const PUSH_SOURCE_ONLY: u32 = 0x20;
    pub const IGNORE_IMAGE_SPACE_SWAP: u32 = 0x40;
    pub const CHAIN: u32 = 0x80;
    pub const NO_VIBRATION: u32 = 0x100;
}

/// An `EXPL` record as read.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ExplosionData {
    pub id: FormId,
    /// Its model (`MODL`), played as an effect where it goes off.
    pub model: Option<String>,
    /// Applied to what it reaches (`EITM`).
    pub enchantment: FormId,
    /// For the player within `image_space_radius` (`MNAM`).
    pub image_space: FormId,
    pub light: FormId,
    pub sounds: [FormId; 2],
    pub impacts: FormId,
    /// Placed where it goes off (a hazard, an activator, an actor...).
    pub placed: FormId,
    pub spawn_projectile: FormId,
    pub force: f32,
    pub damage: f32,
    /// Reach in game units.
    pub radius: f32,
    pub image_space_radius: f32,
    pub vertical_offset_mult: f32,
    pub flags: u32,
    /// The noise it makes (`SoundLevel`: 0 loud, 1 normal, 2 silent, 3 very loud).
    pub sound_level: u32,
}

impl ExplosionData {
    pub fn load(lo: &LoadOrder, id: FormId) -> Option<ExplosionData> {
        let rec = lo.get(id).filter(|r| r.tag().0 == *b"EXPL")?;
        let form_field = |tag: &[u8; 4]| {
            rec.get(tag)
                .filter(|d| d.len() >= 4)
                .map(|d| u32::from_le_bytes(d[0..4].try_into().unwrap()))
                .filter(|&v| v != 0)
                .map_or(FormId::NULL, |v| rec.fid(FormId(v)))
        };
        let mut x = ExplosionData {
            id,
            model: crate::world::records::model_path(&rec),
            enchantment: form_field(b"EITM"),
            image_space: form_field(b"MNAM"),
            ..Default::default()
        };
        let d = rec.get(b"DATA").unwrap_or_default();
        // The layouts whose order is known: 52 bytes, and 48 without the
        // vertical offset mult.
        if d.len() < 48 {
            return Some(x);
        }
        let u = |o: usize| u32::from_le_bytes(d[o..o + 4].try_into().unwrap());
        let f = |o: usize| f32::from_bits(u(o));
        let form = |o: usize| match u(o) {
            0 => FormId::NULL,
            v => rec.fid(FormId(v)),
        };
        x.light = form(0);
        x.sounds = [form(4), form(8)];
        x.impacts = form(12);
        x.placed = form(16);
        x.spawn_projectile = form(20);
        x.force = f(24);
        x.damage = f(28);
        x.radius = f(32).max(0.0);
        x.image_space_radius = f(36).max(0.0);
        let rest = if d.len() >= 52 {
            x.vertical_offset_mult = f(40);
            44
        } else {
            40
        };
        x.flags = u(rest);
        x.sound_level = u(rest + 4);
        Some(x)
    }

    pub fn has(&self, flag: u32) -> bool {
        self.flags & flag != 0
    }
}

/// The middle of an actor's body at scale 1: what an explosion must reach.
const BODY_CENTRE: f32 = 60.0;
/// How far above where it went off an explosion looks from, so the
/// surface it landed on doesn't hide what stands on it.
const SIGHT_LIFT: f32 = 8.0;
/// Explosions set off while others go off, waiting for the next frame; the
/// most gone off in one frame.
const MAX_PER_FRAME: usize = 32;
/// The fastest a blast sends what it pushes (units a second; no source).
const MAX_PUSH_SPEED: f32 = 700.0;

/// An explosion to set off.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Detonation {
    pub explosion: FormId,
    pub at: Vec3,
    /// Which way what set it off was going (down for one placed).
    pub dir: Vec3,
    /// Who it is blamed on (their actor cause, if they have one).
    pub owner: Option<FormId>,
    /// The reference that went off or set it off (a destructible object, a
    /// script's reference, a caster): spared by it, and not in its way.
    pub source: Option<FormId>,
    /// The spell landing that set it off: its effects and `OnHit`s are the
    /// spell's own (`Engine::spell_lands`).
    pub spell: Option<FormId>,
    /// The weapon and projectile behind it (`OnHit`).
    pub weapon: Option<FormId>,
    pub projectile: Option<FormId>,
}

impl Detonation {
    pub(crate) fn new(explosion: FormId, at: Vec3) -> Self {
        Detonation {
            explosion,
            at,
            dir: -Vec3::Z,
            owner: None,
            source: None,
            spell: None,
            weapon: None,
            projectile: None,
        }
    }
}

/// Explosions waiting to go off.
#[derive(Default)]
pub struct Explosions {
    queue: Vec<Detonation>,
    going_off: bool,
}

/// What an explosion reached: an actor or a reference, where it was struck,
/// and how far from the centre.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Reached {
    pub what: FormId,
    pub at: Vec3,
    pub distance: f32,
}

impl Engine {
    /// Set an explosion off. One set off while another is going off (a
    /// destructible object its damage destroys exploding in turn) goes off
    /// next frame.
    pub(crate) fn explode(&mut self, d: Detonation) {
        if self.explosions.going_off {
            self.explosions.queue.push(d);
            return;
        }
        self.explosions.going_off = true;
        self.go_off(&d);
        self.explosions.going_off = false;
    }

    /// The explosions waiting from last frame go off.
    pub(crate) fn update_explosions(&mut self, _dt: f32) {
        let mut queue = std::mem::take(&mut self.explosions.queue);
        let later = queue.split_off(queue.len().min(MAX_PER_FRAME));
        self.explosions.queue = later;
        for d in queue {
            self.explode(d);
        }
    }

    fn go_off(&mut self, d: &Detonation) {
        let Some(x) = ExplosionData::load(&self.lo, d.explosion) else {
            return;
        };
        let owner = d
            .owner
            .map(|o| self.actor_cause_of(o))
            .filter(|&o| self.is_actor_ref(o));
        log::info!(
            "explosion {} ({}) at {:.0} by {owner:?}: radius {}, damage {}, force {}",
            self.lo
                .get(d.explosion)
                .and_then(|r| r.editor_id())
                .unwrap_or_default(),
            d.explosion,
            d.at,
            x.radius,
            x.damage,
            x.force
        );
        let actors = self.explosion_actors(&x, d);
        let objects = self.explosion_objects(&x, d);
        let hostile_enchantment = self
            .magic_item(x.enchantment)
            .is_some_and(|m| self.item_hostile(&m));
        for r in &actors {
            self.explosion_strikes_actor(&x, d, owner, hostile_enchantment, r.what);
        }
        for r in &objects {
            if x.damage > 0.0 && self.destructible_data(r.what).is_some() {
                self.damage_object_by_blow(r.what, x.damage);
            }
            if d.spell.is_none() {
                self.send_explosion_hit(d, owner, r.what);
            }
        }
        self.explosion_force(&x, d, &actors);
        if let Some(o) = owner
            && let Some(level) = self.sound_level(x.sound_level)
        {
            self.make_noise(o, level, Some(d.at));
        }
    }

    /// The blast's push: actors it knocks down ("Knock Down - Always", or
    /// "By Formula" those left at or below `fKnockdownCurrentHealthThreshold`
    /// percent of their health), and what moves within its radius (corpses
    /// too) pushed away from it, only its source with "Push Explosion Source
    /// Ref Only" (by `fExplosionSourceRefMult`).
    fn explosion_force(&mut self, x: &ExplosionData, d: &Detonation, actors: &[Reached]) {
        let force = x.force * gmst_f32(&self.lo, "fExplosionForceMultLinear", 1.0);
        if x.has(flag::KNOCK_DOWN_ALWAYS | flag::KNOCK_DOWN_BY_FORMULA) {
            let threshold = gmst_f32(&self.lo, "fKnockdownCurrentHealthThreshold", 50.0) / 100.0;
            for r in actors {
                if self.is_dead(r.what) {
                    continue;
                }
                let max = self.av_max(r.what, av::HEALTH);
                let left = self.actor_value(r.what, av::HEALTH);
                if knocks_down(x.flags, left, max, threshold) {
                    let feet = self.ref_position(r.what).unwrap_or(r.at);
                    let push = crate::knock::push_velocity(d.at, feet, force.max(1.0));
                    self.knock_down(r.what, push, crate::knock::KnockCause::Area);
                }
            }
        }
        if force <= 0.0 || x.radius <= 0.0 {
            return;
        }
        let source_only = x.has(flag::PUSH_SOURCE_ONLY);
        let force = if source_only {
            force * gmst_f32(&self.lo, "fExplosionSourceRefMult", 3.0)
        } else {
            force
        };
        let from = d.at + Vec3::Z * SIGHT_LIFT;
        let bodies = self.physics.dynamic_owners_within(d.at, x.radius);
        for (owner, at, mass) in bodies {
            if source_only && Some(owner) != d.source {
                continue;
            }
            // Living actors are knocked down or stand; corpses fly.
            if self.is_actor_ref(owner) && !self.is_dead(owner) {
                continue;
            }
            if !source_only && !self.blast_reaches(x, d, from, at, owner) {
                continue;
            }
            let mut dir = (at - d.at).normalize_or(Vec3::Z);
            dir.z += x.vertical_offset_mult.max(0.0);
            let dir = dir.normalize_or(Vec3::Z);
            // The force as a Havok impulse (kg m/s), no faster than
            // `MAX_PUSH_SPEED` for light things.
            let impulse = (force * crate::physics::METRE).min(mass.max(0.1) * MAX_PUSH_SPEED);
            self.disturb(owner);
            self.physics.apply_impulse(owner, dir * impulse);
        }
    }

    /// An actor in the blast: its damage, its enchantment, and (if not a
    /// spell's) `OnHit` and the fight a blow starts.
    fn explosion_strikes_actor(
        &mut self,
        x: &ExplosionData,
        d: &Detonation,
        owner: Option<FormId>,
        hostile_enchantment: bool,
        target: FormId,
    ) {
        if self.is_ethereal(target) {
            return;
        }
        if x.damage > 0.0 {
            let amount = x.damage * self.difficulty_multiplier(target, false);
            // Its owner's blow: a fight and an assault, as any.
            self.damage_unscaled(target, amount, owner, 0.0);
        }
        if !x.enchantment.is_null() {
            match owner {
                // Its `OnHit` is the explosion's, below.
                Some(o) => {
                    self.apply_item_quietly(x.enchantment, o, target);
                    if hostile_enchantment && o != target && d.spell.is_none() {
                        self.hostile_magic_landed(target, o, x.enchantment, false);
                    }
                }
                None => {
                    self.apply_item(x.enchantment, None, target);
                }
            }
        }
        if d.spell.is_none() {
            self.send_explosion_hit(d, owner, target);
        }
    }

    /// `OnHit` from an explosion: its owner the aggressor, the weapon behind
    /// it (or the explosion) the source, the projectile behind it.
    fn send_explosion_hit(&mut self, d: &Detonation, owner: Option<FormId>, target: FormId) {
        let value = |e: &Engine, f: Option<FormId>| f.map_or(Value::None, |f| e.object_value(f));
        let args = vec![
            value(self, owner),
            value(self, Some(d.weapon.unwrap_or(d.explosion))),
            value(self, d.projectile),
            Value::Bool(false),
            Value::Bool(false),
            Value::Bool(false),
            Value::Bool(false),
        ];
        self.send_script_event(target, "OnHit", args);
    }

    /// Whether the blast at `from` reaches `to` (on `what`): nothing solid
    /// between but `what` itself, unless it ignores line of sight.
    fn blast_reaches(
        &self,
        x: &ExplosionData,
        d: &Detonation,
        from: Vec3,
        to: Vec3,
        what: FormId,
    ) -> bool {
        if x.has(flag::IGNORE_LOS) {
            return true;
        }
        let dist = to.distance(from);
        if dist < 1.0 {
            return true;
        }
        let exclude = d.source.or(d.owner).unwrap_or(FormId::NULL);
        match self
            .physics
            .raycast_excluding(from, (to - from) / dist, dist, exclude)
        {
            Some((_, hit)) => hit == Some(what),
            None => true,
        }
    }

    /// The living actors (the player too) whose body's middle is within the
    /// explosion's radius and in its sight; not its source.
    pub(crate) fn explosion_actors(&self, x: &ExplosionData, d: &Detonation) -> Vec<Reached> {
        if x.radius <= 0.0 {
            return Vec::new();
        }
        let from = d.at + Vec3::Z * SIGHT_LIFT;
        let near = self
            .cells
            .values()
            .flat_map(|rt| &rt.actors)
            .filter(|a| !a.dead)
            .map(|a| (a.ref_id, a.exec.pos + Vec3::Z * BODY_CENTRE * a.scale));
        let player = (!self.player_dead()).then_some((PLAYER_REF, self.player.position));
        let mut out: Vec<Reached> = near
            .chain(player)
            .filter(|&(id, _)| Some(id) != d.source)
            .filter_map(|(what, at)| {
                let distance = at.distance(d.at);
                (distance <= x.radius && self.blast_reaches(x, d, from, at, what))
                    .then_some(Reached { what, at, distance })
            })
            .collect();
        out.sort_by(|a, b| a.distance.total_cmp(&b.distance));
        out
    }

    /// The loaded references other than actors within the radius and in
    /// sight that it can affect: destructible ones, and those whose scripts
    /// hear `OnHit`; not its source.
    pub(crate) fn explosion_objects(&self, x: &ExplosionData, d: &Detonation) -> Vec<Reached> {
        if x.radius <= 0.0 {
            return Vec::new();
        }
        let from = d.at + Vec3::Z * SIGHT_LIFT;
        let mut out: Vec<Reached> = self
            .cells
            .values()
            .flat_map(|rt| &rt.refs)
            .copied()
            .filter(|&r| Some(r) != d.source && !self.is_actor_ref(r))
            .filter_map(|r| {
                let at = self.ref_position(r)?;
                let distance = at.distance(d.at);
                (distance <= x.radius).then_some(Reached {
                    what: r,
                    at,
                    distance,
                })
            })
            .filter(|r| {
                self.destructible_data(r.what).is_some()
                    || self.vm.handles(papyrus::ObjectId::Form(r.what.0), "OnHit")
            })
            .filter(|r| {
                let at = self.physics.owner_center(r.what).unwrap_or(r.at);
                self.blast_reaches(x, d, from, at, r.what)
            })
            .collect();
        out.sort_by(|a, b| a.distance.total_cmp(&b.distance));
        out
    }
}

/// Whether an explosion of these flags knocks down an actor left with
/// `left` of `max` health: always, or by formula at or below `threshold`
/// of it (`fKnockdownCurrentHealthThreshold`, a percentage, as a fraction).
fn knocks_down(flags: u32, left: f32, max: f32, threshold: f32) -> bool {
    flags & flag::KNOCK_DOWN_ALWAYS != 0
        || flags & flag::KNOCK_DOWN_BY_FORMULA != 0 && max > 0.0 && left / max <= threshold
}

#[cfg(test)]
#[path = "explosions_tests.rs"]
mod tests;

#[cfg(test)]
mod record_tests {
    use super::*;
    use esp::writer::{Node, RecordBuilder, write_plugin};

    fn data(len: usize) -> Vec<u8> {
        let mut d = Vec::new();
        for form in [0x901u32, 0x902, 0x903, 0x904, 0x905, 0x906] {
            d.extend_from_slice(&form.to_le_bytes());
        }
        for v in [240.0f32, 5.0, 500.0, 1024.0] {
            d.extend_from_slice(&v.to_le_bytes());
        }
        if len == 52 {
            d.extend_from_slice(&0.25f32.to_le_bytes());
        }
        d.extend_from_slice(&0x141u32.to_le_bytes());
        d.extend_from_slice(&2u32.to_le_bytes());
        d.truncate(len);
        d
    }

    fn load(records: Vec<RecordBuilder>) -> (std::path::PathBuf, LoadOrder) {
        let dir = std::env::temp_dir().join(format!(
            "vrm-expl-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&dir).unwrap();
        let nodes = [Node::top(b"EXPL", records)];
        std::fs::write(
            dir.join("Expl.esp"),
            write_plugin(0, &[], "test", "explosions", 0x910, &nodes),
        )
        .unwrap();
        let lo = LoadOrder::load(&dir, &["Expl.esp".into()]).unwrap();
        (dir, lo)
    }

    #[test]
    fn reads_the_full_and_the_short_layouts() {
        let (dir, lo) = load(vec![
            RecordBuilder::new(b"EXPL", FormId(0x800))
                .zstring(b"MODL", "Effects\\FXGasTrapBlast.nif")
                .u32(b"EITM", 0x907)
                .u32(b"MNAM", 0x908)
                .field(b"DATA", &data(52)),
            RecordBuilder::new(b"EXPL", FormId(0x801)).field(b"DATA", &data(48)),
            RecordBuilder::new(b"EXPL", FormId(0x802))
                .u32(b"EITM", 0x907)
                .field(b"DATA", &data(44)),
        ]);
        let full = ExplosionData::load(&lo, FormId(0x800)).unwrap();
        assert_eq!(
            full.model.as_deref(),
            Some("meshes/effects/fxgastrapblast.nif")
        );
        assert_eq!(full.enchantment, FormId(0x907));
        assert_eq!(full.image_space, FormId(0x908));
        assert_eq!(full.light, FormId(0x901));
        assert_eq!(full.sounds, [FormId(0x902), FormId(0x903)]);
        assert_eq!(full.impacts, FormId(0x904));
        assert_eq!(full.placed, FormId(0x905));
        assert_eq!(full.spawn_projectile, FormId(0x906));
        assert_eq!(
            (
                full.force,
                full.damage,
                full.radius,
                full.image_space_radius
            ),
            (240.0, 5.0, 500.0, 1024.0)
        );
        assert_eq!(full.vertical_offset_mult, 0.25);
        assert_eq!((full.flags, full.sound_level), (0x141, 2));
        assert!(full.has(flag::IGNORE_IMAGE_SPACE_SWAP) && !full.has(flag::IGNORE_LOS));

        // TrapPoisonGasExplosion01's 48 bytes: no vertical offset mult.
        let short = ExplosionData::load(&lo, FormId(0x801)).unwrap();
        assert_eq!(short.vertical_offset_mult, 0.0);
        assert_eq!((short.flags, short.sound_level), (0x141, 2));
        assert_eq!(short.radius, 500.0);

        // An unknown layout keeps only what lies outside DATA.
        let old = ExplosionData::load(&lo, FormId(0x802)).unwrap();
        assert_eq!(old.enchantment, FormId(0x907));
        assert_eq!((old.radius, old.damage, old.flags), (0.0, 0.0, 0));
        assert!(ExplosionData::load(&lo, FormId(0x803)).is_none());
        let _ = std::fs::remove_dir_all(dir);
    }
}
