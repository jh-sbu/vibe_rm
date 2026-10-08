//! Bows: archers keeping their distance, drawing and loosing arrows through
//! their graphs; arrows flying as their projectiles do (speed, gravity) and
//! striking actors, the player or the world.

use std::sync::Arc;

use esp::{FormId, LoadOrder};
use glam::{Mat4, Quat, Vec3};

use super::combat::{arrived_away, place_away};
use super::{ActorRuntime, State, World, uniform};
use crate::engine::{Engine, PLAYER_REF};
use crate::render::{GpuModel, Instance};

/// AMMO `DATA` flag: an arrow (not a bolt).
const AMMO_NON_BOLT: u32 = 0x4;
/// Iron arrows: what archers without arrows of their own shoot.
const IRON_ARROW: FormId = FormId(0x1397D);
/// Archers shoot from up to this far, with a clear line to the target.
pub(crate) const BOW_RANGE: f32 = 2000.0;
/// Archers back off from a target closer than this (at scale 1), as many times a
/// second as their combat style's fallback multiplier times the rate (tuned by
/// eye), to somewhere up to `FALLBACK_STEP` away.
const FALLBACK_RANGE: f32 = 450.0;
const FALLBACK_RATE: f32 = 2.0;
const FALLBACK_STEP: f32 = 600.0;
/// Seconds to draw a bow at speed 1 (`Bow_DrawNock` until it is held drawn).
pub(crate) const DRAW_TIME: f32 = 1.07;
/// Seconds from `attackRelease` to the graph's `arrowRelease`, if it never comes.
pub(crate) const RELEASE_FALLBACK: f32 = 0.3;
/// Seconds an arrow may fly, and stays stuck where it struck; the most kept stuck.
const FLIGHT_TIME: f32 = 8.0;
const STUCK_TIME: f32 = 60.0;
/// An arrow's mass, for the push it gives what it strikes (kg; no source).
const ARROW_MASS: f32 = 0.1;
const MAX_STUCK: usize = 64;
/// Height of the middle of an actor's body (its collision capsule) at scale 1.
const CAPSULE_CENTRE: f32 = 60.0;
/// How far behind its point an arrow is drawn (flight models are 58 units long,
/// head to nock): the head sinks into what it strikes.
const ARROW_EMBED: f32 = 40.0;
/// Bow animation type (`WEAP` `DNAM`).
const ANIM_BOW: u8 = 7;

/// Where an archer is in its shot.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum Draw {
    #[default]
    Idle,
    /// Drawing (seconds so far), to be loosed once held this long.
    Drawing(f32, f32),
    /// Loosed (seconds since), and whether the arrow has flown.
    Loosed(f32, bool),
}

/// An arrow's make: what it adds to the bow's damage, how fast it flies, how
/// much gravity pulls it (`PROJ` `DATA`), and its model in flight.
#[derive(Debug, Clone)]
pub struct Arrow {
    pub ammo: FormId,
    /// The projectile it flies as (`PROJ`).
    pub projectile: FormId,
    pub damage: f32,
    pub speed: f32,
    pub gravity: f32,
    pub model: Option<String>,
}

fn f32_at(d: &[u8], o: usize) -> f32 {
    d.get(o..o + 4)
        .map_or(0.0, |b| f32::from_le_bytes(b.try_into().unwrap()))
}

/// What an `AMMO` record shoots (`DATA`: projectile, flags, damage).
pub fn arrow(lo: &LoadOrder, ammo: FormId) -> Option<Arrow> {
    let rec = lo.get(ammo).filter(|r| r.tag().0 == *b"AMMO")?;
    let d = rec.get(b"DATA").filter(|d| d.len() >= 12)?;
    let proj = rec.fid(FormId(u32::from_le_bytes(d[0..4].try_into().unwrap())));
    let mut a = Arrow {
        ammo,
        projectile: proj,
        damage: f32_at(d, 8),
        speed: 3600.0,
        gravity: 0.35,
        model: None,
    };
    if let Some(p) = lo.get(proj) {
        if let Some(pd) = p.get(b"DATA").filter(|d| d.len() >= 12) {
            a.gravity = f32_at(pd, 4);
            a.speed = f32_at(pd, 8).max(100.0);
        }
        a.model = crate::world::records::model_path(&p);
    }
    if a.model.is_none() {
        a.model = crate::world::records::model_path(&rec);
    }
    Some(a)
}

/// Whether a weapon is a bow.
pub fn is_bow(lo: &LoadOrder, weapon: FormId) -> bool {
    lo.get(weapon)
        .and_then(|r| r.get(b"DNAM").and_then(|d| d.first().copied()))
        .is_some_and(|t| t == ANIM_BOW)
}

/// A bow's draw speed (`DNAM` speed).
pub(crate) fn bow_speed(lo: &LoadOrder, bow: FormId) -> f32 {
    lo.get(bow)
        .and_then(|r| {
            r.get(b"DNAM")
                .filter(|d| d.len() >= 8)
                .map(|d| f32_at(d, 4))
        })
        .filter(|s| *s > 0.05)
        .unwrap_or(1.0)
}

/// An arrow in flight, or stuck where it struck.
pub struct Projectile {
    pub shooter: FormId,
    pub arrow: Arrow,
    pub pos: Vec3,
    pub vel: Vec3,
    pub damage: f32,
    pub model: Option<Arc<GpuModel>>,
    pub age: f32,
    /// Stuck in the world (pointing the way it flew), for this many seconds more.
    pub stuck: Option<f32>,
    pub dir: Vec3,
}

/// Elevation (radians above the straight line) to strike a point `d` away
/// horizontally and `h` above at speed `v` under gravity `g`: the low arc, or
/// as high as the arrow can reach.
fn elevation(d: f32, h: f32, v: f32, g: f32) -> f32 {
    if g <= 0.0 || d <= 1.0 {
        return h.atan2(d.max(1.0));
    }
    let v2 = v * v;
    let disc = v2 * v2 - g * (g * d * d + 2.0 * h * v2);
    if disc < 0.0 {
        return std::f32::consts::FRAC_PI_4;
    }
    ((v2 - disc.sqrt()) / (g * d)).atan()
}

/// The point on segment `a`-`b` closest to segment `c`-`d`, and the distance
/// between the two.
fn segment_distance(a: Vec3, b: Vec3, c: Vec3, d: Vec3) -> (f32, f32) {
    let (u, v, w) = (b - a, d - c, a - c);
    let (aa, bb, cc, dd, ee) = (u.dot(u), u.dot(v), v.dot(v), u.dot(w), v.dot(w));
    let den = aa * cc - bb * bb;
    let mut s = if den > 1e-6 {
        ((bb * ee - cc * dd) / den).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let t = if cc > 1e-6 {
        ((bb * s + ee) / cc).clamp(0.0, 1.0)
    } else {
        0.0
    };
    if aa > 1e-6 {
        s = ((bb * t - dd) / aa).clamp(0.0, 1.0);
    }
    let p = a + u * s;
    let q = c + v * t;
    (s, p.distance(q))
}

impl ActorRuntime {
    /// An archer's turn: close in until in range with a clear shot, then stand
    /// facing the target, draw, hold a moment to aim, and loose. Returns the
    /// graph event to send.
    pub(crate) fn archer_step(
        &mut self,
        dt: f32,
        w: &mut World,
        target: Vec3,
        dist: f32,
    ) -> Option<String> {
        let run = self.run_speed();
        let pos = self.pos;
        let close = dist < FALLBACK_RANGE * self.scale;
        let fallback = self.stats.fallback;
        let c = self.combat.as_mut()?;
        let draw = c.draw;
        // Backing off from a target close in: between shots, now and then.
        if c.away_to.is_some_and(|t| arrived_away(pos, t, target)) {
            c.away_to = None;
        }
        if c.away_to.is_none()
            && close
            && draw == Draw::Idle
            && uniform(w.rand, 0.0, 1.0) < fallback * FALLBACK_RATE * dt
        {
            c.away_to = place_away(w, pos, target, FALLBACK_STEP);
            c.repath = 0.0;
            if c.away_to.is_some() {
                log::debug!("{} backs off from its target", self.ref_id);
            }
        }
        if let Some(to) = c.away_to {
            self.chase(dt, w, to, run);
            return None;
        }
        let in_range = dist < BOW_RANGE * 0.9 && c.clear_shot;
        if !in_range {
            // Lower the bow to move.
            if draw != Draw::Idle {
                c.draw = Draw::Idle;
                return Some("attackStop".into());
            }
            self.chase(dt, w, target, run);
            return None;
        }
        self.speed = 0.0;
        self.state = State::Idle(1.0);
        let to = (target - self.pos).normalize_or_zero();
        let off = self.turn_towards(to, dt).abs();
        let c = self.combat.as_mut()?;
        match draw {
            Draw::Idle if c.cooldown <= 0.0 && off < 0.5 => {
                // The last release's clip raises arrowRelease more than once.
                self.arrow_release = false;
                c.draw = Draw::Drawing(0.0, DRAW_TIME / self.bow_speed + uniform(w.rand, 0.2, 0.9));
                Some("bowAttackStart".into())
            }
            Draw::Idle => None,
            Draw::Drawing(t, hold) => {
                let t = t + dt;
                c.draw = Draw::Drawing(t, hold);
                (t >= hold && off < 0.12).then(|| {
                    c.draw = Draw::Loosed(0.0, false);
                    c.cooldown = uniform(w.rand, 0.4, 1.4);
                    "attackRelease".into()
                })
            }
            Draw::Loosed(t, flown) => {
                let t = t + dt;
                // The graph's arrowRelease looses it; if that never comes, loose anyway.
                let fire =
                    !flown && (std::mem::take(&mut self.arrow_release) || t >= RELEASE_FALLBACK);
                c.draw = if t > 0.8 {
                    Draw::Idle
                } else {
                    Draw::Loosed(t, flown || fire)
                };
                if fire {
                    self.loose = true;
                }
                None
            }
        }
    }
}

impl Engine {
    /// The arrows an actor (or the player) shoots: its best by damage, or iron
    /// ones for an NPC that has none (NPCs never run out).
    pub(crate) fn arrows_of(&self, actor: FormId) -> Option<Arrow> {
        let inv = self.inventories.get(&actor);
        let carried = inv.map(|i| &i.items[..]).unwrap_or_default();
        let equipped = inv.and_then(|i| {
            i.equipped
                .iter()
                .copied()
                .find(|&f| self.lo.tag_of(f).map(|t| t.0) == Some(*b"AMMO"))
        });
        let best = equipped
            .filter(|&f| carried.iter().any(|&(i, n)| i == f && n > 0))
            .or_else(|| {
                carried
                    .iter()
                    .filter(|&&(_, n)| n > 0)
                    .filter_map(|&(f, _)| {
                        let rec = self.lo.get(f).filter(|r| r.tag().0 == *b"AMMO")?;
                        let d = rec.get(b"DATA").filter(|d| d.len() >= 12)?;
                        let flags = u32::from_le_bytes(d[4..8].try_into().unwrap());
                        (flags & AMMO_NON_BOLT != 0).then_some((f, f32_at(d, 8)))
                    })
                    .max_by(|a, b| a.1.total_cmp(&b.1))
                    .map(|a| a.0)
            });
        match best {
            Some(a) => arrow(&self.lo, a),
            None if actor != PLAYER_REF => arrow(&self.lo, IRON_ARROW),
            None => None,
        }
    }

    /// Whether each archer has a clear shot at its target's body: nothing but the
    /// target along the line to it, nor along the edges of the spread cone (a
    /// shot within the spread could clip what the line just passes).
    pub(crate) fn update_clear_shots(&mut self) {
        let spread = crate::ai::combat::gmst_f32(&self.lo, "fBowNPCSpreadAngle", 4.0).to_radians();
        let edge = (spread * 0.5).tan();
        let archers: Vec<(FormId, FormId, Vec3)> = self
            .cells
            .values()
            .flat_map(|rt| &rt.actors)
            .filter(|a| a.bow && !a.dead)
            .filter_map(|a| {
                Some((
                    a.ref_id,
                    a.combat.as_ref()?.target,
                    a.pos + Vec3::Z * 110.0 * a.scale,
                ))
            })
            .collect();
        for (archer, target, eye) in archers {
            let aim = if target == PLAYER_REF {
                Some(self.player.position + Vec3::Z * 20.0)
            } else {
                self.actor_ref(target)
                    .map(|t| t.pos + Vec3::Z * CAPSULE_CENTRE * t.scale)
            };
            let clear = aim.is_some_and(|aim| {
                let to = aim - eye;
                let (dir, dist) = (to.normalize_or_zero(), to.length());
                let right = dir.cross(Vec3::Z).normalize_or(Vec3::X);
                let up = right.cross(dir);
                // The edge rays stop a little short: they pass beside the target,
                // into the ground around it.
                [
                    (0.0, 0.0, 1.0),
                    (edge, 0.0, 0.95),
                    (-edge, 0.0, 0.95),
                    (0.0, edge, 0.95),
                    (0.0, -edge, 0.95),
                ]
                .iter()
                .all(|&(r, u, reach)| {
                    let d = (dir + right * r + up * u).normalize();
                    match self.physics.raycast_excluding(eye, d, dist * reach, archer) {
                        Some((_, owner)) => owner == Some(target),
                        None => true,
                    }
                })
            });
            if let Some(c) = self.actor_mut(archer).and_then(|a| a.combat.as_mut()) {
                c.clear_shot = clear;
            }
        }
    }

    /// Loose an arrow from `from` towards `dir`.
    fn spawn_arrow(
        &mut self,
        shooter: FormId,
        arrow: Arrow,
        from: Vec3,
        dir: Vec3,
        speed_mult: f32,
        damage: f32,
    ) {
        let model = arrow.model.clone().and_then(|m| {
            self.models
                .load_all(&mut self.renderer, &self.vfs, std::slice::from_ref(&m));
            self.models.get(&m)
        });
        let vel = dir.normalize_or_zero() * arrow.speed * speed_mult;
        log::info!(
            "{shooter} looses {} ({damage:.0} damage, {:.0} units/s)",
            arrow.ammo,
            vel.length()
        );
        self.projectiles.push(Projectile {
            shooter,
            arrow,
            pos: from,
            vel,
            damage,
            model,
            age: 0.0,
            stuck: None,
            dir: vel.normalize_or_zero(),
        });
    }

    /// Archers whose graph loosed an arrow this frame shoot at their target: from
    /// the bow, aimed (over the drop) at the middle of the target's body, spread
    /// by `fBowNPCSpreadAngle`.
    pub(crate) fn loose_arrows(&mut self) {
        // (shooter, target, eye, practice aim point)
        let mut shots: Vec<(FormId, FormId, Vec3, Option<Vec3>)> = Vec::new();
        for rt in self.cells.values_mut() {
            for a in rt.actors.iter_mut() {
                if !std::mem::take(&mut a.loose) {
                    continue;
                }
                let eye = a.pos + Vec3::Z * 110.0 * a.scale;
                match (a.combat.as_ref(), a.practice_aim.take()) {
                    (Some(c), _) => shots.push((a.ref_id, c.target, eye, None)),
                    (None, Some(aim)) => shots.push((a.ref_id, FormId::NULL, eye, Some(aim))),
                    _ => {}
                }
            }
        }
        if shots.is_empty() {
            return;
        }
        let spread = crate::ai::combat::gmst_f32(&self.lo, "fBowNPCSpreadAngle", 4.0).to_radians();
        for (shooter, target, eye, practice) in shots {
            let aim = if let Some(p) = practice {
                Some(p)
            } else if target == PLAYER_REF {
                Some(self.player.position + Vec3::Z * 20.0)
            } else {
                self.actor_ref(target)
                    .map(|t| t.pos + Vec3::Z * CAPSULE_CENTRE * t.scale)
            };
            let Some(aim) = aim else { continue };
            let Some(arrow) = self.arrows_of(shooter) else {
                continue;
            };
            let bow = self
                .inventories
                .get(&shooter)
                .and_then(|i| i.weapon(&self.lo));
            // Practice shots harm nobody.
            let damage = if practice.is_some() {
                0.0
            } else {
                bow.map_or(0.0, |b| crate::ai::combat::weapon_damage(&self.lo, b)) + arrow.damage
            };
            let to = aim - eye;
            let flat = to.truncate();
            let from = eye + flat.normalize_or_zero().extend(0.0) * 30.0;
            let pitch = elevation(
                flat.length(),
                to.z,
                arrow.speed,
                arrow.gravity * crate::physics::GRAVITY,
            );
            // Spread: within a cone the spread angle wide, most shots near its middle.
            let mut off =
                || ((self.rand() % 1001) as f32 + (self.rand() % 1001) as f32) / 2000.0 - 0.5;
            let yaw = flat.y.atan2(flat.x) + off() * spread;
            let pitch = pitch + off() * spread;
            let dir = Vec3::new(
                yaw.cos() * pitch.cos(),
                yaw.sin() * pitch.cos(),
                pitch.sin(),
            );
            log::debug!(
                "{shooter} shoots from {from:?} at {aim:?} ({target}), pitch {:.1} deg",
                pitch.to_degrees()
            );
            self.spawn_arrow(shooter, arrow, from, dir, 1.0, damage);
        }
    }

    /// The player's bow: drawn while the attack button is held, loosed when it
    /// comes up (arrows from the inventory). An arrow loosed before the bow is
    /// fully drawn flies slower and strikes softer, in proportion; loosed
    /// straight away, it isn't nocked and nothing flies.
    pub fn player_loose(&mut self, held: f32) {
        if self.disabled_controls.fighting {
            return;
        }
        let Some(bow) = self.player_weapon().filter(|&w| is_bow(&self.lo, w)) else {
            return;
        };
        let full = DRAW_TIME / bow_speed(&self.lo, bow);
        if held < full * 0.3 {
            return;
        }
        let Some(arrow) = self.arrows_of(PLAYER_REF) else {
            self.scripts.notify("You have no arrows.");
            return;
        };
        let power = (held / full).min(1.0);
        self.remove_item(PLAYER_REF, arrow.ammo, 1, None);
        let damage = (crate::ai::combat::weapon_damage(&self.lo, bow) + arrow.damage) * power;
        let dir = self.camera.forward();
        let from = self.camera.position + dir * 20.0 - Vec3::Z * 6.0;
        self.spawn_arrow(PLAYER_REF, arrow, from, dir, power.max(0.5), damage);
    }

    /// Arrows fly on under gravity: into an actor (a hit, as a blow; it may end
    /// up in their inventory, `iArrowInventoryChance`), the player, or the world
    /// (stuck there a while). The scene's moving instances follow them.
    pub(crate) fn update_projectiles(&mut self, dt: f32) {
        let mut hits: Vec<(FormId, FormId, f32, Arrow)> = Vec::new();
        let mut stuck: Vec<(FormId, Vec3, f32, Option<FormId>, Vec3)> = Vec::new();
        // References struck (shooter, what, its projectile, the arrow's velocity).
        let mut struck: Vec<(FormId, FormId, FormId, Vec3)> = Vec::new();
        let player = self.player.position;
        let (pr, ph) = (self.physics.player_radius, self.physics.player_half_height);
        for p in self.projectiles.iter_mut() {
            p.age += dt;
            if let Some(t) = p.stuck.as_mut() {
                *t -= dt;
                continue;
            }
            let start = p.pos;
            p.vel.z -= p.arrow.gravity * crate::physics::GRAVITY * dt;
            let step = p.vel * dt;
            let len = step.length();
            if len <= 0.0 {
                continue;
            }
            let dir = step / len;
            p.dir = dir;
            let world = self.physics.raycast_excluding(start, dir, len, p.shooter);
            // The player isn't in the physics world: test their capsule.
            let at_player = (p.shooter != PLAYER_REF)
                .then(|| {
                    segment_distance(
                        start,
                        start + step,
                        player - Vec3::Z * ph,
                        player + Vec3::Z * ph,
                    )
                })
                .filter(|&(_, d)| d < pr)
                .map(|(s, _)| s * len);
            match (world, at_player) {
                (_, Some(t)) if world.is_none_or(|(w, _)| t < w) => {
                    hits.push((PLAYER_REF, p.shooter, p.damage, p.arrow.clone()));
                    p.pos = start + dir * t;
                    p.stuck = Some(0.0);
                }
                (Some((t, owner)), _) => {
                    p.pos = start + dir * t;
                    match owner.filter(|o| self.lo.tag_of(*o).map(|t| t.0) == Some(*b"ACHR")) {
                        Some(actor) => {
                            hits.push((actor, p.shooter, p.damage, p.arrow.clone()));
                            p.stuck = Some(0.0);
                        }
                        None => {
                            if let Some(o) = owner {
                                struck.push((p.shooter, o, p.arrow.projectile, p.vel));
                            }
                            // A loose object is knocked away; the arrow falls.
                            if owner.is_some_and(|o| self.physics.is_dynamic_owner(o)) {
                                p.stuck = Some(0.0);
                                continue;
                            }
                            stuck.push((p.shooter, p.pos, p.age, owner, dir));
                            p.stuck = Some(STUCK_TIME);
                        }
                    }
                }
                _ => p.pos += step,
            }
            if p.age > FLIGHT_TIME && p.stuck.is_none() {
                p.stuck = Some(0.0);
            }
        }
        for (shooter, what, projectile, vel) in struck {
            self.physics.apply_impulse(what, vel * ARROW_MASS);
            self.send_hit_event(what, shooter, Some(projectile), false, false, false);
        }
        for (shooter, at, age, owner, dir) in stuck {
            let what = owner
                .and_then(|o| self.base_of(o).or(Some(o)))
                .and_then(|b| self.lo.get(b))
                .map(|r| format!("{} {}", r.tag(), r.editor_id().unwrap_or_default()));
            log::debug!(
                "{shooter}'s arrow sticks at {at:?} after {age:.2}s in {owner:?} ({what:?}), flying {dir:?}"
            );
        }
        // Keep the newest stuck arrows, a while.
        self.projectiles.retain(|p| p.stuck.is_none_or(|t| t > 0.0));
        let stuck = self
            .projectiles
            .iter()
            .filter(|p| p.stuck.is_some())
            .count();
        if stuck > MAX_STUCK {
            let mut drop = stuck - MAX_STUCK;
            self.projectiles.retain(|p| {
                let gone = drop > 0 && p.stuck.is_some();
                drop -= gone as usize;
                !gone
            });
        }
        for (target, shooter, damage, arrow) in hits {
            if target != PLAYER_REF && self.is_dead(target) {
                continue;
            }
            log::info!("{shooter}'s arrow strikes {target}");
            self.hit(target, shooter, damage, false, 0.0, Some(arrow.projectile));
            let chance = crate::ai::combat::gmst_i32(&self.lo, "iArrowInventoryChance", 33)
                .clamp(0, 100) as u64;
            if target != PLAYER_REF && self.rand() % 100 < chance {
                self.add_item(target, arrow.ammo, 1);
            }
        }
        self.scene.dynamic = self
            .projectiles
            .iter()
            .filter_map(|p| {
                let m = p.model.clone()?;
                // Flight models point along +Y, the head at the far end: draw the
                // arrow behind its point, the head in what it struck.
                let dir = p.dir.normalize_or(Vec3::Y);
                let rot = Quat::from_rotation_arc(Vec3::Y, dir);
                let mut inst = Instance::new(
                    m,
                    Mat4::from_rotation_translation(rot, p.pos - dir * ARROW_EMBED),
                );
                inst.lights = crate::render::pick_lights(
                    &self.scene.lights,
                    inst.world_center,
                    inst.world_radius,
                );
                Some(inst)
            })
            .collect();
    }
}
