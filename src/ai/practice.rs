//! UseWeapon packages: at the package's location the actor draws its weapon and
//! attacks one of the package's targets (training dummies, archery butts) in
//! barrages of "Min / Max Attacks per Barrage", pausing "Min / Max Pause"
//! seconds between them. Archers draw, aim and loose real arrows at the target
//! (they stick in it); melee swings strike nothing. Outside combat only.

use esp::FormId;
use glam::Vec3;

use super::archery::{DRAW_TIME, RELEASE_FALLBACK};
use super::{ActorRuntime, World, uniform};

/// A UseWeapon goal: the package's targets as references, and its barrage timing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PracticeGoal {
    pub weapon: super::package::WeaponKind,
    pub targets: [Option<FormId>; 3],
    pub pause: (f32, f32),
    pub attacks: (u32, u32),
}

#[derive(Debug, Clone, Copy, Default)]
enum Phase {
    #[default]
    Ready,
    /// Bow drawn this long, to be loosed after `hold`.
    Drawing(f32, f32),
    /// Arrow loosed this long ago (and whether it flew).
    Loosed(f32, bool),
}

/// Where an actor is in its practice.
#[derive(Debug, Clone, Copy, Default)]
pub struct Practice {
    target: Option<FormId>,
    /// Attacks left in this barrage.
    left: u32,
    /// Seconds before the next step.
    wait: f32,
    phase: Phase,
}

impl ActorRuntime {
    /// One step of practice with weapons; `w.targets` has the targets' aim points.
    pub(crate) fn practice_step(&mut self, dt: f32, goal: &PracticeGoal, w: &mut World) {
        self.speed = 0.0;
        let mut p = self.practice.unwrap_or_default();
        // A new barrage: at one of the targets.
        if p.left == 0 && matches!(p.phase, Phase::Ready) && p.wait <= 0.0 {
            let targets: Vec<FormId> = goal.targets.iter().flatten().copied().filter(|t| w.targets.contains_key(t)).collect();
            if targets.is_empty() {
                return;
            }
            p.target = Some(targets[((w.rand)() % targets.len() as u64) as usize]);
            p.left = goal.attacks.0 + ((w.rand)() % u64::from(goal.attacks.1 - goal.attacks.0 + 1)) as u32;
            log::debug!("{} practises on {:?}: {} attacks", self.ref_id, p.target, p.left);
        }
        let Some(aim) = p.target.and_then(|t| w.targets.get(&t).copied()) else {
            self.practice = Some(p);
            return;
        };
        let off = self.turn_towards((aim - self.pos).with_z(0.0).normalize_or_zero(), dt).abs();
        if !self.drawn {
            if self.graph_event("WeapEquip", &mut w.clips) {
                self.drawn = true;
            }
            p.wait = 1.5;
            self.practice = Some(p);
            return;
        }
        p.wait -= dt;
        match p.phase {
            Phase::Ready if p.wait > 0.0 || off > 0.3 || p.left == 0 => {}
            Phase::Ready if self.bow => {
                self.arrow_release = false;
                if self.graph_event("bowAttackStart", &mut w.clips) {
                    p.phase = Phase::Drawing(0.0, DRAW_TIME / self.bow_speed + uniform(w.rand, 0.4, 1.2));
                } else {
                    p.wait = 0.5;
                }
            }
            Phase::Ready => {
                if self.graph_event("attackStart", &mut w.clips) {
                    p.left -= 1;
                    p.wait = uniform(w.rand, 1.2, 2.2);
                    if p.left == 0 {
                        p.wait += uniform(w.rand, goal.pause.0, goal.pause.1);
                    }
                } else {
                    p.wait = 0.5;
                }
            }
            Phase::Drawing(t, hold) => {
                let t = t + dt;
                p.phase = Phase::Drawing(t, hold);
                if t >= hold && off < 0.12 && self.graph_event("attackRelease", &mut w.clips) {
                    p.phase = Phase::Loosed(0.0, false);
                }
            }
            Phase::Loosed(t, flown) => {
                let t = t + dt;
                let fire = !flown && (std::mem::take(&mut self.arrow_release) || t >= RELEASE_FALLBACK);
                if fire {
                    self.loose = true;
                    self.practice_aim = Some(aim);
                }
                p.phase = Phase::Loosed(t, flown || fire);
                if t > 0.8 {
                    p.phase = Phase::Ready;
                    p.left = p.left.saturating_sub(1);
                    p.wait = uniform(w.rand, 0.6, 1.6);
                    if p.left == 0 {
                        p.wait += uniform(w.rand, goal.pause.0, goal.pause.1);
                    }
                }
            }
        }
        self.practice = Some(p);
    }

    /// Done practising: put the weapon away.
    pub(crate) fn end_practice(&mut self, w: &mut World) {
        if self.practice.take().is_some() && self.drawn && self.combat.is_none() && self.graph_event("Unequip", &mut w.clips) {
            self.drawn = false;
        }
    }
}

impl crate::engine::Engine {
    /// Actors practising with a kind of weapon take one they carry in hand (best
    /// by damage), and the one they had back afterwards.
    pub(crate) fn update_practice_weapons(&mut self) {
        let mut swaps = Vec::new();
        for a in self.cells.values().flat_map(|rt| &rt.actors) {
            if a.combat.is_some() || a.dead {
                continue;
            }
            let kind = a.goal.and_then(|g| g.practice).map(|p| p.weapon);
            let inv = self.inventories.get(&a.ref_id);
            let current = inv.and_then(|i| i.weapon(&self.lo));
            match kind {
                Some(kind) if !current.is_some_and(|w| weapon_is(&self.lo, w, kind)) => {
                    let carried = inv.map(|i| i.items.clone()).unwrap_or_default();
                    let best = carried
                        .iter()
                        .filter(|(f, n)| *n > 0 && weapon_is(&self.lo, *f, kind))
                        .max_by(|x, y| crate::ai::combat::weapon_damage(&self.lo, x.0).total_cmp(&crate::ai::combat::weapon_damage(&self.lo, y.0)));
                    if let Some(&(w, _)) = best {
                        swaps.push((a.ref_id, w, Some(current)));
                    }
                }
                None if a.practice_weapon.is_some() => {
                    if let Some(was) = a.practice_weapon.flatten() {
                        swaps.push((a.ref_id, was, None));
                    }
                }
                _ => {}
            }
        }
        for (actor, weapon, was) in swaps {
            log::debug!("{actor} takes {weapon} in hand{}", if was.is_some() { " to practise" } else { " again" });
            self.wield(actor, weapon);
            if let Some(a) = self.actor_mut(actor) {
                a.practice_weapon = match was {
                    Some(w) => Some(a.practice_weapon.unwrap_or(w)),
                    None => None,
                };
            }
        }
    }
}

/// Whether `w` is a weapon of `kind`.
fn weapon_is(lo: &esp::LoadOrder, w: FormId, kind: super::package::WeaponKind) -> bool {
    use super::package::WeaponKind;
    let Some(anim) = lo.get(w).filter(|r| r.tag().0 == *b"WEAP").and_then(|r| r.get(b"DNAM").and_then(|d| d.first().copied())) else { return false };
    match kind {
        WeaponKind::Any => true,
        WeaponKind::Melee => (1..=6).contains(&anim),
        WeaponKind::Ranged => anim == 7 || anim == 9,
        WeaponKind::Specific(f) => f == w,
    }
}

/// Where to aim at a target object: the middle of its bounds (`OBND`).
pub fn aim_point(lo: &esp::LoadOrder, r: FormId, pos: Vec3) -> Vec3 {
    let Some(rec) = lo.get(r) else { return pos };
    let rf = crate::world::records::reference(&rec);
    let centre_z = lo.get(rf.base).and_then(|b| {
        let d = b.get(b"OBND").filter(|d| d.len() >= 12)?;
        let z = |o: usize| i16::from_le_bytes([d[o], d[o + 1]]) as f32;
        Some((z(4) + z(10)) * 0.5)
    });
    pos + Vec3::Z * centre_z.unwrap_or(0.0) * rf.scale
}
