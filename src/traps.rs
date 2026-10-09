//! Trap hits: a loose object whose scripts handle trap events (rocks, logs,
//! swinging blades with `TrapHitBase`) touching a live target, while it is
//! awake, sends its scripts `OnTrapHitStart` when the touch begins,
//! `OnTrapHit` then and every so often while it lasts, and `OnTrapHitStop`
//! when it ends. The scripts decide what hurts (`PhysicsTrapHit` only past a
//! speed) and call `ProcessTrapHit` on the target.

use std::collections::{HashMap, HashSet};

use esp::FormId;
use glam::Vec3;
use papyrus::Value;

use crate::engine::{Engine, PLAYER_REF};
use crate::physics::METRE;

/// How near a trap's collision must come to a body to touch it (game units;
/// no source).
const TOUCH_MARGIN: f32 = 2.0;

/// Seconds between `OnTrapHit`s while a trap stays in touch (the CK wiki: "every
/// so often"; no figure).
const HIT_INTERVAL: f64 = 0.25;

#[derive(Default)]
pub(crate) struct Traps {
    /// Whether each loose reference's scripts handle trap hits (asked once).
    is_trap: HashMap<FormId, bool>,
    /// Trap and target in touch, and when the last `OnTrapHit` went.
    touching: HashMap<(FormId, FormId), f64>,
    /// Traps and targets that have hit before (`abInitialHit` is false after).
    hit_before: HashSet<(FormId, FormId)>,
}

/// One touch: where, the trap's velocity there (game units / s), its Havok
/// material and how it moves (Papyrus `Motion_*`).
struct Touch {
    at: Vec3,
    vel: Vec3,
    material: u32,
    motion: i32,
}

impl Engine {
    /// Whether `r`'s scripts handle trap hits.
    fn is_trap(&mut self, r: FormId) -> bool {
        if let Some(&t) = self.traps.is_trap.get(&r) {
            return t;
        }
        let obj = papyrus::ObjectId::Form(r.0);
        let t = ["OnTrapHitStart", "OnTrapHit", "OnTrapHitStop"]
            .iter()
            .any(|e| self.vm.handles(obj, e));
        self.traps.is_trap.insert(r, t);
        t
    }

    /// After each physics step: what the awake traps touch, and the events.
    pub(crate) fn update_traps(&mut self) {
        let traps: Vec<(FormId, Vec<rapier3d::prelude::RigidBodyHandle>)> = self
            .cells
            .values()
            .flat_map(|rt| &rt.loose)
            .map(|l| (l.ref_id, l.bodies.iter().map(|b| b.handle).collect()))
            .collect();
        let player = (!self.player_dead()).then(|| self.player.position);
        let mut now: HashMap<(FormId, FormId), Touch> = HashMap::new();
        for (r, bodies) in traps {
            if !self.is_trap(r) || self.is_disabled(r) {
                continue;
            }
            for h in bodies {
                let Some(b) = self.physics.world.bodies.get(h) else {
                    continue;
                };
                if b.is_sleeping() || b.is_fixed() {
                    continue;
                }
                let motion = if b.is_dynamic() { 1 } else { 4 };
                // What the player holds doesn't hit them.
                let player = player.filter(|_| self.physics.held != Some(h));
                for (who, at, vel, material) in self.physics.body_touches(h, player, TOUCH_MARGIN) {
                    let target = who.unwrap_or(PLAYER_REF);
                    if target != PLAYER_REF && self.is_dead(target) {
                        continue;
                    }
                    now.entry((r, target)).or_insert(Touch {
                        at,
                        vel,
                        material,
                        motion,
                    });
                }
            }
        }
        // Parts moved by objects' behaviour graphs (swinging blades, battering
        // rams): keyframed.
        for part in self.moving_graph_parts() {
            if !self.is_trap(part.ref_id) || self.is_disabled(part.ref_id) {
                continue;
            }
            let mut touches = Vec::new();
            self.physics.collider_touches(
                part.collider,
                player,
                TOUCH_MARGIN,
                &|p| part.velocity_at(p),
                &mut touches,
            );
            for (who, at, vel, material) in touches {
                let target = who.unwrap_or(PLAYER_REF);
                if target != PLAYER_REF && self.is_dead(target) {
                    continue;
                }
                now.entry((part.ref_id, target)).or_insert(Touch {
                    at,
                    vel,
                    material,
                    motion: 4,
                });
            }
        }
        let time = self.scripts.real_time;
        let ended: Vec<(FormId, FormId)> = self
            .traps
            .touching
            .keys()
            .filter(|k| !now.contains_key(k))
            .copied()
            .collect();
        for (trap, target) in ended {
            self.traps.touching.remove(&(trap, target));
            log::debug!("trap {trap} stops hitting {target}");
            let t = self.object_value(target);
            self.send_script_event(trap, "OnTrapHitStop", vec![t]);
        }
        for ((trap, target), touch) in now {
            let first = !self.traps.touching.contains_key(&(trap, target));
            if !first && time - self.traps.touching[&(trap, target)] < HIT_INTERVAL {
                continue;
            }
            self.traps.touching.insert((trap, target), time);
            let initial = self.traps.hit_before.insert((trap, target));
            let v = touch.vel / METRE;
            let args = vec![
                self.object_value(target),
                Value::Float(v.x),
                Value::Float(v.y),
                Value::Float(v.z),
                Value::Float(touch.at.x),
                Value::Float(touch.at.y),
                Value::Float(touch.at.z),
                Value::Int(touch.material as i32),
                Value::Bool(initial),
                Value::Int(touch.motion),
            ];
            if first {
                log::debug!(
                    "trap {trap} hits {target} at {:.1} m/s{}",
                    v.length(),
                    if initial { " (first time)" } else { "" }
                );
                self.send_script_event(trap, "OnTrapHitStart", args.clone());
            }
            self.send_script_event(trap, "OnTrapHit", args);
        }
    }

    /// Papyrus `ProcessTrapHit` on `target`: the trap's damage (and stagger)
    /// to a living actor or the player. Pushback isn't applied.
    pub(crate) fn process_trap_hit(
        &mut self,
        target: FormId,
        trap: Option<FormId>,
        damage: f32,
        stagger: f32,
    ) {
        if damage <= 0.0 && stagger <= 0.0 {
            return;
        }
        log::info!("{target} is hit by trap {trap:?} for {damage:.0}");
        // No attacker: a trap starts no fight and is no crime.
        self.damage(target, damage.max(0.0), None, stagger.clamp(0.0, 1.0));
    }
}
