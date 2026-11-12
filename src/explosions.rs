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
/// The shortest an explosion's light lasts (seconds; no source).
const MIN_FLASH: f32 = 0.5;
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
    /// Its reference in the world (`PlaceAtMe`'s); one is made if none.
    pub reference: Option<FormId>,
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
            reference: None,
        }
    }
}

/// Explosions waiting to go off, and the lights of those going off.
#[derive(Default)]
pub struct Explosions {
    queue: Vec<Detonation>,
    going_off: bool,
    lights: Vec<Flash>,
    /// Explosions' references, and the seconds each has left.
    refs: Vec<(FormId, f32)>,
    /// Spells waiting where they landed to go off (runes).
    pub(crate) runes: Vec<Rune>,
}

/// A spell whose projectile has an alternate trigger (`PROJ` flag 0x4: the
/// runes), lying where it landed until an enemy of its caster comes within
/// its proximity, its timer runs out or another spell lands near it.
pub(crate) struct Rune {
    pub caster: FormId,
    pub spell: FormId,
    pub dual: bool,
    pub at: Vec3,
    pub dir: Vec3,
    /// Reach of its trigger (`PROJ` `DATA` 0x1C).
    pub proximity: f32,
    /// Seconds until it goes off by itself (`DATA` 0x20; none for 0).
    pub timer: Option<f32>,
    place: Option<crate::ai::schedule::Place>,
    /// Its projectile's effect, kept playing while it waits.
    pub fx: Option<crate::spell_fx::effect::EffectInstance>,
}

/// An explosion's light: fading out over its life.
#[derive(Clone)]
struct Flash {
    light: crate::render::GpuLight,
    age: f32,
    life: f32,
}

/// What an explosion reached: an actor or a reference, where it was struck,
/// and how far from the centre.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Reached {
    pub what: FormId,
    pub at: Vec3,
    pub distance: f32,
    /// Nothing solid between (or the explosion ignores line of sight): it
    /// takes the blast; otherwise it only hears it (`OnHit`).
    pub in_sight: bool,
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

    /// The explosions waiting from last frame go off; lights fade.
    pub(crate) fn update_explosions(&mut self, dt: f32) {
        self.explosions.lights.retain_mut(|f| {
            f.age += dt;
            f.age < f.life
        });
        let mut over = Vec::new();
        self.explosions.refs.retain_mut(|(r, left)| {
            *left -= dt;
            let keep = *left > 0.0;
            if !keep {
                over.push(*r);
            }
            keep
        });
        for r in over {
            self.delete_created(r);
        }
        self.update_runes(dt);
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
            self.explosion_strikes_actor(&x, d, owner, hostile_enchantment, r);
        }
        for r in &objects {
            if r.in_sight && x.damage > 0.0 && self.destructible_data(r.what).is_some() {
                self.damage_object_by_blow(r.what, x.damage * falloff(r.distance, x.radius));
            }
            if d.spell.is_none() {
                self.send_explosion_hit(d, owner, r.what, false);
            }
        }
        let in_sight: Vec<Reached> = actors.iter().copied().filter(|r| r.in_sight).collect();
        self.explosion_force(&x, d, &in_sight);
        let life = self.show_explosion(&x, d, owner);
        let reference = d.reference.or_else(|| self.make_explosion_ref(d));
        if let Some(r) = reference {
            self.explosions.refs.push((r, life));
            self.place_explosion_object(&x, d, owner, r);
        }
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
                let left = self.actor_value_fraction(r.what, av::HEALTH);
                if knocks_down(x.flags, left, 1.0, threshold) {
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
            // The force as a Havok impulse (kg m/s), falling off to nothing at
            // the radius, no faster than `MAX_PUSH_SPEED` for light things.
            let falloff = if source_only {
                1.0
            } else {
                (1.0 - at.distance(d.at) / x.radius).clamp(0.0, 1.0)
            };
            let impulse =
                (force * falloff * crate::physics::METRE).min(mass.max(0.1) * MAX_PUSH_SPEED);
            self.disturb(owner);
            self.physics.apply_impulse(owner, dir * impulse);
        }
    }

    /// What a blast shows and sounds like: its model played once where it
    /// goes off (facing back the way what set it off came, or the world's
    /// way with "Always Uses World Orientation"), its light for as long,
    /// its two sounds, its image space modifier for the player within its IS
    /// radius, and its impact on the surface it went off on.
    /// Returns how long it lasts.
    fn show_explosion(&mut self, x: &ExplosionData, d: &Detonation, owner: Option<FormId>) -> f32 {
        let rot = if x.has(flag::WORLD_ORIENTATION) {
            glam::Quat::IDENTITY
        } else {
            glam::Quat::from_rotation_arc(Vec3::Z, (-d.dir).normalize_or(Vec3::Z))
        };
        let transform = glam::Mat4::from_rotation_translation(rot, d.at);
        let mut life = MIN_FLASH;
        if let Some(model) = &x.model
            && let Some(mut fx) = self.load_spell_model(model, transform)
        {
            // Explosion models name their one sequence for its use
            // (`SpecialIdle_AreaEffect`...): played when there's no `Idle`.
            if fx.graph.is_none() && !fx.is_playing() {
                let first = fx
                    .anim
                    .as_ref()
                    .and_then(|a| a.sequences.first())
                    .map(|s| s.name.clone());
                if let Some(name) = first {
                    fx.play(&name);
                }
            }
            fx.duration = fx.one_shot_duration();
            fx.fade_duration = 0.0;
            log::debug!("explosion model {model} for {:.2}s", fx.duration);
            life = life.max(fx.duration);
            self.spell_fx.tails.push(fx);
        }
        if let Some(light) = self.spell_light(x.light, d.at + Vec3::Z * SIGHT_LIFT) {
            self.explosions.lights.push(Flash {
                light,
                age: 0.0,
                life,
            });
        }
        for s in x.sounds.into_iter().filter(|s| !s.is_null()) {
            self.play_sound(s, d.at);
        }
        if !x.image_space.is_null()
            && !self.player_dead()
            && self.player.position.distance(d.at) <= x.image_space_radius
        {
            self.apply_imod(x.image_space, 1.0);
        }
        if !x.impacts.is_null() {
            self.impact_at(x.impacts, owner, None, d.at, d.dir.normalize_or(-Vec3::Z));
        }
        life
    }

    /// A reference for an explosion with none, where it goes off: in the
    /// place of its source (or owner, or the player).
    fn make_explosion_ref(&mut self, d: &Detonation) -> Option<FormId> {
        let anchor = d.source.or(d.owner).unwrap_or(PLAYER_REF);
        let place = self.place_near(anchor, d.at)?;
        Some(self.create_object_at(d.explosion, place, d.at, Vec3::ZERO, 1))
    }

    /// The place a point is in: `anchor`'s interior, or the exterior cell
    /// of `anchor`'s worldspace the point lies in.
    fn place_near(&self, anchor: FormId, at: Vec3) -> Option<crate::ai::schedule::Place> {
        let (place, _) = self.current_place_of(anchor)?;
        Some(match place {
            crate::ai::schedule::Place::Exterior(world, _) => {
                crate::ai::schedule::Place::Exterior(world, crate::engine::grid_of(at.truncate()))
            }
            interior => interior,
        })
    }

    /// A spell's projectile landed: if it has an alternate trigger (a rune),
    /// it lies there, the caster's last rune going away, and true.
    pub(crate) fn lay_rune(
        &mut self,
        caster: FormId,
        spell: FormId,
        at: Vec3,
        dir: Vec3,
        dual: bool,
    ) -> bool {
        let Some(proj) = self
            .magic_item(spell)
            .and_then(|item| self.item_projectile(&item))
        else {
            return false;
        };
        let Some((proximity, timer)) = alt_trigger(&self.lo, proj) else {
            return false;
        };
        let old: Vec<Rune> = self
            .explosions
            .runes
            .extract_if(.., |r| r.caster == caster)
            .collect();
        for r in old {
            self.rune_ends(r);
        }
        log::info!("{caster}'s rune {spell} lies at {at:.0} (proximity {proximity})");
        let place = self.place_near(caster, at);
        self.explosions.runes.push(Rune {
            caster,
            spell,
            dual,
            at,
            dir,
            proximity,
            timer: (timer > 0.0).then_some(timer),
            place,
            fx: None,
        });
        true
    }

    /// A rune gone without going off: its effect stops.
    fn rune_ends(&mut self, r: Rune) {
        if let Some(mut fx) = r.fx {
            fx.stop();
            self.spell_fx.tails.push(fx);
        }
    }

    /// Whether `who` sets off `caster`'s rune: an actor they fight or that
    /// would attack them on sight (or they it); never the caster.
    fn sets_off_rune(&mut self, caster: FormId, who: FormId) -> bool {
        if who == caster {
            return false;
        }
        let fighting = |e: &Engine, a: FormId, b: FormId| {
            e.actor_ref(a)
                .and_then(|x| x.exec.combat.as_ref())
                .is_some_and(|c| c.target == b)
        };
        fighting(self, who, caster)
            || fighting(self, caster, who)
            || who != PLAYER_REF && self.would_attack(who, caster)
            || caster != PLAYER_REF && self.would_attack(caster, who)
    }

    /// Runes wait: those whose timer runs out, or an enemy of whose caster
    /// stands within their proximity, go off; those whose cell is gone go.
    fn update_runes(&mut self, dt: f32) {
        if self.explosions.runes.is_empty() {
            return;
        }
        let mut bodies: Vec<(FormId, Vec3)> = self
            .cells
            .values()
            .flat_map(|rt| &rt.actors)
            .filter(|a| !a.dead)
            .map(|a| (a.ref_id, a.exec.pos))
            .collect();
        if !self.player_dead()
            && let Some(p) = self.ref_position(PLAYER_REF)
        {
            bodies.push((PLAYER_REF, p));
        }
        // What becomes of each: kept, gone off, or gone.
        let mut fates = Vec::with_capacity(self.explosions.runes.len());
        for i in 0..self.explosions.runes.len() {
            let (caster, at, proximity, place) = {
                let r = &mut self.explosions.runes[i];
                if let Some(t) = r.timer.as_mut() {
                    *t -= dt;
                }
                (r.caster, r.at, r.proximity, r.place)
            };
            if place.is_some_and(|p| self.key_of_place(p).is_none()) {
                fates.push(Some(false));
                continue;
            }
            let due = self.explosions.runes[i].timer.is_some_and(|t| t <= 0.0);
            let near: Vec<FormId> = bodies
                .iter()
                .filter(|(_, p)| p.distance(at) <= proximity)
                .map(|&(who, _)| who)
                .collect();
            let fires = due || near.into_iter().any(|who| self.sets_off_rune(caster, who));
            fates.push(fires.then_some(true));
        }
        let runes = std::mem::take(&mut self.explosions.runes);
        let mut taken = Vec::new();
        for (r, fate) in runes.into_iter().zip(fates) {
            match fate {
                None => self.explosions.runes.push(r),
                Some(goes_off) => taken.push((r, goes_off)),
            }
        }
        for (r, goes_off) in taken {
            if goes_off {
                self.rune_goes_off(r);
            } else {
                self.rune_ends(r);
            }
        }
    }

    fn rune_goes_off(&mut self, r: Rune) {
        log::info!("{}'s rune {} goes off at {:.0}", r.caster, r.spell, r.at);
        let (caster, spell, at, dir, dual) = (r.caster, r.spell, r.at, r.dir, r.dual);
        self.rune_ends(r);
        self.spell_lands(caster, spell, None, at, dir, dual);
    }

    /// A spell landing at `at` sets off the runes it lands within reach of.
    pub(crate) fn spell_sets_off_runes(&mut self, at: Vec3) {
        let hit: Vec<Rune> = self
            .explosions
            .runes
            .extract_if(.., |r| r.at.distance(at) <= r.proximity.max(1.0))
            .collect();
        for r in hit {
            self.rune_goes_off(r);
        }
    }

    /// Draw the runes' effects.
    pub(crate) fn draw_runes(&mut self, dt: f32) {
        for r in &mut self.explosions.runes {
            if let Some(fx) = r.fx.as_mut() {
                fx.advance(dt);
                fx.draw(&mut self.scene);
            }
        }
    }

    /// Run the runes' effects' particles.
    pub(crate) fn step_runes(
        &mut self,
        dt: f32,
        right: Vec3,
        up: Vec3,
        batches: &mut Vec<crate::render::particles::ParticleBatch>,
    ) {
        for r in &mut self.explosions.runes {
            if let Some(fx) = r.fx.as_mut() {
                fx.step(dt, right, up, batches);
            }
        }
    }

    /// What an explosion leaves where it went off (`DATA` placed object):
    /// a hazard, an explosion going off in turn, or a reference of anything
    /// else made at its reference.
    fn place_explosion_object(
        &mut self,
        x: &ExplosionData,
        d: &Detonation,
        owner: Option<FormId>,
        reference: FormId,
    ) {
        let Some(tag) = self.lo.tag_of(x.placed).map(|t| t.0) else {
            return;
        };
        match &tag {
            b"HAZD" => self.spawn_hazard(x.placed, d.at),
            b"EXPL" => {
                let mut next = Detonation::new(x.placed, d.at);
                next.dir = d.dir;
                next.owner = owner;
                self.explode(next);
            }
            _ => {
                let made = self.create_ref(x.placed, reference, false);
                log::debug!("explosion {} leaves {made:?} ({})", d.explosion, x.placed);
            }
        }
    }

    /// The lights of the explosions going off, fading over their life.
    pub(crate) fn explosion_lights(&self) -> impl Iterator<Item = crate::render::GpuLight> + '_ {
        self.explosions.lights.iter().map(|f| {
            let fade = (1.0 - f.age / f.life.max(1e-3)).clamp(0.0, 1.0);
            let mut light = f.light;
            for c in &mut light.color[..3] {
                *c *= fade;
            }
            light
        })
    }

    /// An actor in the blast. In its sight: its damage, falling off with
    /// the square of the distance and through armor as a blow's (Skyrim SE,
    /// `test-mod explosions`), and its enchantment. Out of sight, only its
    /// `OnHit`. Then (if not a spell's) `OnHit`: the `Unarmed` weapon's
    /// first when it did damage, then the explosion's.
    fn explosion_strikes_actor(
        &mut self,
        x: &ExplosionData,
        d: &Detonation,
        owner: Option<FormId>,
        hostile_enchantment: bool,
        r: &Reached,
    ) {
        let target = r.what;
        if self.is_ethereal(target) {
            return;
        }
        if !r.in_sight {
            if d.spell.is_none() {
                self.send_explosion_hit(d, owner, target, false);
            }
            return;
        }
        let damage = x.damage * falloff(r.distance, x.radius);
        if damage > 0.0 {
            let amount =
                self.after_armor(target, damage, 1.0) * self.difficulty_multiplier(target, false);
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
            self.send_explosion_hit(d, owner, target, damage > 0.0);
        }
    }

    /// `OnHit` from an explosion: its owner the aggressor, the weapon behind
    /// it (or the explosion) the source, the projectile behind it.
    fn send_explosion_hit(
        &mut self,
        d: &Detonation,
        owner: Option<FormId>,
        target: FormId,
        hurt: bool,
    ) {
        let value = |e: &Engine, f: Option<FormId>| f.map_or(Value::None, |f| e.object_value(f));
        if hurt {
            let unarmed = vec![
                value(self, owner),
                value(self, Some(crate::ai::combat::UNARMED)),
                Value::None,
                Value::Bool(false),
                Value::Bool(false),
                Value::Bool(false),
                Value::Bool(false),
            ];
            self.send_script_event(target, "OnHit", unarmed);
        }
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
            .map(|a| {
                let feet = a.exec.pos;
                (a.ref_id, feet, feet + Vec3::Z * BODY_CENTRE * a.scale)
            });
        let player = (!self.player_dead())
            .then(|| {
                self.ref_position(PLAYER_REF)
                    .map(|f| (PLAYER_REF, f, self.player.position))
            })
            .flatten();
        // Reached by the distance to its feet (SE's falloff fits that, not
        // the body's middle); seen at the middle of its body.
        let mut out: Vec<Reached> = near
            .chain(player)
            .filter(|&(id, _, _)| Some(id) != d.source)
            .filter_map(|(what, feet, at)| {
                let distance = feet.distance(d.at);
                (distance <= x.radius).then(|| Reached {
                    what,
                    at,
                    distance,
                    in_sight: self.blast_reaches(x, d, from, at, what),
                })
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
                    in_sight: false,
                })
            })
            .filter(|r| {
                self.destructible_data(r.what).is_some()
                    || self.vm.handles(papyrus::ObjectId::Form(r.what.0), "OnHit")
            })
            .map(|mut r| {
                let at = self.physics.owner_center(r.what).unwrap_or(r.at);
                r.in_sight = self.blast_reaches(x, d, from, at, r.what);
                r
            })
            .collect();
        out.sort_by(|a, b| a.distance.total_cmp(&b.distance));
        out
    }
}

/// A projectile's alternate trigger (`PROJ` `DATA` flag 0x4 "Alt.
/// Trigger"): its proximity (0x1C) and timer (0x20).
pub(crate) fn alt_trigger(lo: &LoadOrder, proj: FormId) -> Option<(f32, f32)> {
    let rec = lo.get(proj).filter(|r| r.tag().0 == *b"PROJ")?;
    let d = rec.get(b"DATA").filter(|d| d.len() >= 0x24)?;
    let f = |o: usize| f32::from_le_bytes(d[o..o + 4].try_into().unwrap());
    (u16::from_le_bytes([d[0], d[1]]) & 0x4 != 0).then(|| (f(0x1C).max(0.0), f(0x20).max(0.0)))
}

/// The explosion a projectile (`PROJ`) sets off where it lands (`DATA` 0x24).
pub(crate) fn projectile_explosion(lo: &LoadOrder, proj: FormId) -> Option<FormId> {
    let rec = lo.get(proj).filter(|r| r.tag().0 == *b"PROJ")?;
    let d = rec.get(b"DATA")?;
    let v = u32::from_le_bytes(d.get(0x24..0x28)?.try_into().ok()?);
    (v != 0).then(|| rec.fid(FormId(v)))
}

impl Engine {
    /// What goes off where a spell lands: a dual cast's `DUAL` explosion,
    /// else its projectile's, else the first of its effects' that names one.
    pub(crate) fn spell_explosion(
        &self,
        item: &crate::magic::MagicItem,
        dual: bool,
    ) -> Option<FormId> {
        let effects: Vec<_> = item
            .effects
            .iter()
            .filter_map(|e| self.magic_effect(e.effect))
            .collect();
        let dual_cast = dual
            .then(|| {
                effects.iter().find_map(|m| {
                    crate::spell_fx::records::DualCastData::load(&self.lo, m.dual_cast.0)
                        .map(|d| d.explosion)
                        .filter(|x| !x.is_null())
                })
            })
            .flatten();
        dual_cast
            .or_else(|| {
                self.item_projectile(item)
                    .and_then(|p| projectile_explosion(&self.lo, p))
            })
            .or_else(|| effects.iter().map(|m| m.explosion).find(|x| !x.is_null()))
    }
}

/// What share of its damage an explosion deals `distance` from its centre:
/// 1 − (d / radius)², Skyrim SE's readings to the hundredth (`test-mod
/// explosions`: 80 damage, radius 320 gave 78.75, 68.75, 48.75 and 18.75
/// at 40, 120, 200 and 280).
fn falloff(distance: f32, radius: f32) -> f32 {
    if radius <= 0.0 {
        return 0.0;
    }
    (1.0 - (distance / radius).powi(2)).clamp(0.0, 1.0)
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
