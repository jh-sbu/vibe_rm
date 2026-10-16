//! UseWeapon packages: at the package's location the actor draws its weapon and
//! attacks one of the package's targets (training dummies, archery butts) in
//! barrages of "Min / Max Attacks per Barrage", pausing "Min / Max Pause"
//! seconds between them when enabled. Trigger distances gate each target;
//! "Aim Only" keeps a bow drawn without releasing. Practice arrows stick in
//! the target; melee swings strike nothing. Always Power Attack selects an
//! authored power attack suited to the actor's hands. Outside combat only.

use esp::FormId;
use glam::Vec3;

use super::archery::{DRAW_TIME, RELEASE_FALLBACK};
use super::{ActorRuntime, World, uniform};

/// A UseWeapon goal: the package's targets as references, and its barrage timing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PracticeGoal {
    pub weapon: super::package::WeaponKind,
    pub targets: [Option<FormId>; 3],
    /// Bounds-centre offsets, kept separate from references' distance origins.
    pub aim_offsets: [Vec3; 3],
    pub triggers: [Option<PracticeTrigger>; 3],
    pub aim_only: bool,
    pub always_power_attack: bool,
    pub pause: (f32, f32),
    pub attacks: (u32, u32),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PracticeTrigger {
    /// An authored reference that failed resolution must keep the gate closed.
    pub reference: Option<FormId>,
    pub radius: f32,
}

impl PracticeGoal {
    fn eligible(
        &self,
        index: usize,
        actor: FormId,
        pos: Vec3,
        positions: &std::collections::HashMap<FormId, Vec3>,
    ) -> bool {
        let position = |reference| {
            if reference == actor {
                Some(pos)
            } else {
                positions.get(&reference).copied()
            }
        };
        let Some(target) = self.targets[index].and_then(position) else {
            return false;
        };
        match self.triggers[index] {
            None => true,
            Some(trigger) => trigger
                .reference
                .and_then(position)
                .is_some_and(|origin| origin.distance(target) <= trigger.radius),
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
enum Phase {
    #[default]
    Ready,
    /// Bow drawn this long, to be loosed after `hold`.
    Drawing(f32, f32),
    /// Keep retrying a rejected stop event before starting another draw.
    Cancelling,
    /// Arrow loosed this long ago (and whether it flew).
    Loosed(f32, bool),
}

/// Where an actor is in its practice.
#[derive(Debug, Clone, Copy, Default)]
pub struct Practice {
    target: Option<usize>,
    /// Attacks left in this barrage.
    left: u32,
    /// Seconds before the next step.
    wait: f32,
    phase: Phase,
}

impl Practice {
    /// Advance only attacks. Graph acceptance commits each attempted action;
    /// the return value requests one projectile, after release notification.
    fn attack_step(
        &mut self,
        dt: f32,
        goal: &PracticeGoal,
        bow: bool,
        bow_speed: f32,
        melee_event: Option<&str>,
        off: f32,
        arrow_release: bool,
        rand: &mut dyn FnMut() -> u64,
        mut event: impl FnMut(&str) -> bool,
    ) -> bool {
        self.wait -= dt;
        let mut fire = false;
        match self.phase {
            Phase::Ready if self.wait > 0.0 || off > 0.3 || self.left == 0 => {}
            Phase::Ready if bow => {
                if event("bowAttackStart") {
                    self.phase =
                        Phase::Drawing(0.0, DRAW_TIME / bow_speed + uniform(rand, 0.4, 1.2));
                } else {
                    self.wait = 0.5;
                }
            }
            Phase::Ready if goal.aim_only => {}
            Phase::Ready => {
                let Some(melee_event) = melee_event else {
                    return false;
                };
                if event(melee_event) {
                    self.left -= 1;
                    self.wait = uniform(rand, 1.2, 2.2);
                    if self.left == 0 {
                        self.wait += uniform(rand, goal.pause.0, goal.pause.1);
                    }
                } else {
                    self.wait = 0.5;
                }
            }
            Phase::Drawing(t, hold) => {
                let t = (t + dt).min(hold);
                self.phase = Phase::Drawing(t, hold);
                if !goal.aim_only && t >= hold && off < 0.12 && event("attackRelease") {
                    self.phase = Phase::Loosed(0.0, false);
                }
            }
            Phase::Loosed(t, flown) => {
                let t = t + dt;
                fire = !flown && (arrow_release || t >= RELEASE_FALLBACK);
                self.phase = Phase::Loosed(t, flown || fire);
                if t > 0.8 {
                    self.phase = Phase::Ready;
                    self.left = self.left.saturating_sub(1);
                    self.wait = uniform(rand, 0.6, 1.6);
                    if self.left == 0 {
                        self.wait += uniform(rand, goal.pause.0, goal.pause.1);
                    }
                }
            }
            Phase::Cancelling => {}
        }
        fire
    }

    /// An interrupted draw has no release; a released shot cannot be recalled.
    fn cancel(&mut self, mut event: impl FnMut(&str) -> bool) -> bool {
        if matches!(self.phase, Phase::Drawing(..) | Phase::Cancelling) && !event("attackStop") {
            self.phase = Phase::Cancelling;
            return false;
        }
        *self = Self::default();
        true
    }
}

/// The forced melee attack must exist in the actor's attack data and suit its
/// hands. Prefer the stationary weapon attack; otherwise use its first authored
/// right-hand power attack (including bare-handed and creature attacks). A missing
/// power attack keeps the barrage pending instead of silently making a basic one.
fn practice_attack_event(
    always_power_attack: bool,
    attacks: &[super::combat::Attack],
    hands: [super::combat::Fights; 2],
    bare_handed: bool,
) -> Option<&str> {
    if !always_power_attack {
        return Some("attackStart");
    }
    let eligible = |attack: &&super::combat::Attack| {
        attack.power()
            && attack.flags & 0x2 == 0 // ATKD: bash, not a practice swing.
            && attack.side() == super::combat::Side::Right
            && attack.fits(hands, bare_handed)
    };
    let mut available = attacks.iter().filter(eligible);
    available
        .clone()
        .find(|a| a.event.eq_ignore_ascii_case("attackPowerStartInPlace"))
        .or_else(|| available.next())
        .map(|a| a.event.as_str())
}

impl ActorRuntime {
    /// One step of practice; `w.targets` has raw reference positions.
    pub(crate) fn practice_step(&mut self, dt: f32, goal: &PracticeGoal, w: &mut World) {
        self.speed = 0.0;
        let mut p = self.practice.unwrap_or_default();
        if matches!(p.phase, Phase::Cancelling)
            || (p
                .target
                .is_some_and(|t| !goal.eligible(t, self.ref_id, self.pos, w.targets))
                && !matches!(p.phase, Phase::Loosed(..)))
        {
            self.arrow_release = false;
            if !p.cancel(|event| self.graph_event(event, &mut w.clips)) {
                self.practice = Some(p);
                return;
            }
        }
        // A new barrage: choose only among targets whose own trigger passes.
        if p.left == 0 && matches!(p.phase, Phase::Ready) && p.wait <= 0.0 {
            let targets: Vec<usize> = (0..goal.targets.len())
                .filter(|&t| goal.eligible(t, self.ref_id, self.pos, w.targets))
                .collect();
            if targets.is_empty() {
                self.practice = Some(p);
                return;
            }
            p.target = Some(targets[((w.rand)() % targets.len() as u64) as usize]);
            p.left = goal.attacks.0
                + ((w.rand)() % u64::from(goal.attacks.1 - goal.attacks.0 + 1)) as u32;
            log::debug!(
                "{} practises on {:?}: {} attacks",
                self.ref_id,
                p.target,
                p.left
            );
        }
        let Some(aim) = p.target.and_then(|index| {
            let target = goal.targets[index]?;
            let pos = if target == self.ref_id {
                self.pos
            } else {
                *w.targets.get(&target)?
            };
            Some(pos + goal.aim_offsets[index])
        }) else {
            self.practice = Some(p);
            return;
        };
        let off = self
            .turn_towards((aim - self.pos).with_z(0.0).normalize_or_zero(), dt)
            .abs();
        if !self.drawn {
            if self.graph_event("WeapEquip", &mut w.clips) {
                self.drawn = true;
            }
            p.wait = 1.5;
            self.practice = Some(p);
            return;
        }
        let release = std::mem::take(&mut self.arrow_release);
        let stats = self.stats.clone();
        let melee_event = practice_attack_event(
            goal.always_power_attack,
            &stats.attacks,
            self.hands,
            self.stats.bare_handed,
        );
        let fire = p.attack_step(
            dt,
            goal,
            self.bow,
            self.bow_speed,
            melee_event,
            off,
            release,
            w.rand,
            |event| self.graph_event(event, &mut w.clips),
        );
        if fire {
            self.loose = true;
            self.practice_aim = Some(aim);
        }
        self.practice = Some(p);
    }

    /// Done practising: put the weapon away.
    pub(crate) fn end_practice(&mut self, w: &mut World) {
        let Some(mut practice) = self.practice.take() else {
            return;
        };
        self.arrow_release = false;
        if !practice.cancel(|event| self.graph_event(event, &mut w.clips)) {
            self.practice = Some(practice);
            return;
        }
        if self.drawn && self.combat.is_none() && self.graph_event("Unequip", &mut w.clips) {
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
                        .max_by(|x, y| {
                            crate::ai::combat::weapon_damage(&self.lo, x.0)
                                .total_cmp(&crate::ai::combat::weapon_damage(&self.lo, y.0))
                        });
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
            log::debug!(
                "{actor} takes {weapon} in hand{}",
                if was.is_some() {
                    " to practise"
                } else {
                    " again"
                }
            );
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
    let Some(anim) = lo
        .get(w)
        .filter(|r| r.tag().0 == *b"WEAP")
        .and_then(|r| r.get(b"DNAM").and_then(|d| d.first().copied()))
    else {
        return false;
    };
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

#[cfg(test)]
mod tests {
    use super::*;

    fn goal() -> PracticeGoal {
        PracticeGoal {
            weapon: super::super::package::WeaponKind::Any,
            targets: [Some(FormId(10)), Some(FormId(20)), None],
            aim_offsets: [Vec3::Z * 500.0; 3],
            triggers: [None; 3],
            aim_only: false,
            always_power_attack: false,
            pause: (4.0, 4.0),
            attacks: (1, 1),
        }
    }

    fn attack(event: &str, flags: u32) -> super::super::combat::Attack {
        super::super::combat::Attack {
            event: event.to_owned(),
            damage_mult: 1.0,
            chance: 1.0,
            flags,
            strike_angle: 45.0,
            stagger: 0.0,
            stamina_mult: 1.0,
            attack_type: FormId::NULL,
        }
    }

    #[test]
    fn forced_power_attacks_follow_authored_hands_and_prefer_standing() {
        use super::super::combat::Fights::{Fist, Not, Weapon};
        let attacks = [
            attack("attackStart", 0),
            attack("attackPowerStartForward", 4),
            attack("attackPowerStartInPlaceLeftHand", 4),
            attack("attackPowerStartDualWield", 4),
            attack("bashPowerStart", 6),
            attack("attackPowerStartInPlace", 4),
            attack("attackPowerStartH2HRight", 4),
        ];
        assert_eq!(
            practice_attack_event(true, &attacks, [Weapon, Not], true),
            Some("attackPowerStartInPlace")
        );
        assert_eq!(
            practice_attack_event(true, &attacks, [Fist, Fist], true),
            Some("attackPowerStartH2HRight")
        );
        // Creatures without humanoid H2H data can use their authored attacks.
        let creature = [attack("attackCreaturePower", 4)];
        assert_eq!(
            practice_attack_event(true, &creature, [Fist, Fist], false),
            Some("attackCreaturePower")
        );
        assert_eq!(
            practice_attack_event(false, &[], [Fist, Fist], true),
            Some("attackStart")
        );
        assert_eq!(practice_attack_event(true, &[], [Weapon, Not], true), None);
        assert_eq!(
            practice_attack_event(true, &attacks[..5], [Fist, Fist], true),
            None,
            "weapon, left-hand, dual and bash attacks cannot replace a fist's power swing"
        );
    }

    #[test]
    fn forced_power_swing_commits_one_attack_only_when_the_graph_accepts() {
        let mut goal = goal();
        goal.always_power_attack = true;
        let mut practice = Practice {
            left: 1,
            ..Practice::default()
        };
        let event = Some("attackPowerStartInPlace");
        assert!(
            !practice.attack_step(0.0, &goal, false, 1.0, event, 0.0, false, &mut || 0, |e| {
                assert_eq!(e, event.unwrap());
                false
            })
        );
        assert_eq!(practice.left, 1);
        assert!(
            !practice.attack_step(1.0, &goal, false, 1.0, None, 0.0, false, &mut || 0, |_| {
                panic!("missing authored power attack must not fall back to a normal swing")
            })
        );
        assert_eq!(practice.left, 1);
        assert!(
            !practice.attack_step(0.0, &goal, false, 1.0, event, 0.0, false, &mut || 0, |e| {
                assert_eq!(e, event.unwrap());
                true
            })
        );
        assert_eq!(practice.left, 0);
        assert_eq!(
            practice.wait, 5.2,
            "one swing completes the barrage and adds its pause"
        );
    }

    #[test]
    fn forced_power_does_not_override_aim_only_or_bow_release() {
        let mut goal = goal();
        goal.always_power_attack = true;
        goal.aim_only = true;
        let mut practice = Practice {
            left: 1,
            ..Practice::default()
        };
        let power = Some("attackPowerStartInPlace");
        assert!(!practice.attack_step(
            0.0,
            &goal,
            false,
            1.0,
            power,
            0.0,
            false,
            &mut || 0,
            |_| { panic!("aim-only must suppress forced power swings") }
        ));
        assert_eq!(practice.left, 1);
        goal.aim_only = false;
        let mut events = Vec::new();
        // Bows follow their own draw/release events even without a melee attack.
        for dt in [0.0, 3.0, 0.1, 1.0] {
            practice.attack_step(dt, &goal, true, 1.0, None, 0.0, true, &mut || 0, |e| {
                events.push(e.to_owned());
                true
            });
        }
        assert_eq!(events, ["bowAttackStart", "attackRelease"]);
        assert_eq!(practice.left, 0);
        assert!(practice.wait > goal.pause.0);
    }

    #[test]
    fn aim_only_holds_a_draw_and_never_releases_a_projectile() {
        let mut goal = goal();
        goal.aim_only = true;
        let mut practice = Practice {
            left: 1,
            ..Practice::default()
        };
        let mut events = Vec::new();
        for _ in 0..20 {
            assert!(!practice.attack_step(
                1.0,
                &goal,
                true,
                1.0,
                Some("attackStart"),
                0.0,
                true,
                &mut || 0,
                |event| {
                    events.push(event.to_owned());
                    true
                }
            ));
        }
        assert_eq!(events, ["bowAttackStart"]);
        assert!(matches!(practice.phase, Phase::Drawing(..)));
        assert_eq!(practice.left, 1);

        // Removing Aim Only releases the existing draw rather than redrawing.
        goal.aim_only = false;
        assert!(!practice.attack_step(
            0.1,
            &goal,
            true,
            1.0,
            Some("attackStart"),
            0.0,
            false,
            &mut || 0,
            |event| {
                events.push(event.to_owned());
                true
            }
        ));
        assert_eq!(events, ["bowAttackStart", "attackRelease"]);
        assert!(practice.attack_step(
            0.1,
            &goal,
            true,
            1.0,
            Some("attackStart"),
            0.0,
            true,
            &mut || 0,
            |_| panic!("no further attack event before recovery")
        ));
        assert!(!practice.attack_step(
            0.1,
            &goal,
            true,
            1.0,
            Some("attackStart"),
            0.0,
            true,
            &mut || 0,
            |_| panic!("one release must not spawn a second arrow")
        ));
    }

    #[test]
    fn aim_only_melee_does_not_swing_and_pause_is_only_between_barrages() {
        let mut goal = goal();
        goal.aim_only = true;
        let mut practice = Practice {
            left: 2,
            ..Practice::default()
        };
        assert!(!practice.attack_step(
            10.0,
            &goal,
            false,
            1.0,
            Some("attackStart"),
            0.0,
            false,
            &mut || 0,
            |_| panic!("aim-only melee must not attack")
        ));
        assert_eq!(practice.left, 2);
        goal.aim_only = false;
        practice.attack_step(
            0.0,
            &goal,
            false,
            1.0,
            Some("attackStart"),
            0.0,
            false,
            &mut || 0,
            |event| {
                assert_eq!(event, "attackStart");
                true
            },
        );
        assert_eq!(practice.left, 1);
        assert_eq!(practice.wait, 1.2);
        practice.attack_step(
            2.0,
            &goal,
            false,
            1.0,
            Some("attackStart"),
            0.0,
            false,
            &mut || 0,
            |_| true,
        );
        assert_eq!(practice.left, 0);
        assert_eq!(practice.wait, 5.2);

        // Disabling pauses retains the recovery between individual swings.
        goal.pause = (0.0, 0.0);
        practice.left = 1;
        practice.wait = 0.0;
        practice.attack_step(
            0.0,
            &goal,
            false,
            1.0,
            Some("attackStart"),
            0.0,
            false,
            &mut || 0,
            |_| true,
        );
        assert_eq!(practice.wait, 1.2);
    }

    #[test]
    fn trigger_gates_use_each_targets_reference_origin_and_live_self_position() {
        let mut goal = goal();
        let actor = FormId(1);
        let positions = std::collections::HashMap::from([
            (actor, Vec3::splat(1000.0)), // Stale snapshot must not replace Self.
            (FormId(10), Vec3::X * 100.0),
            (FormId(20), Vec3::X * 500.0),
        ]);
        goal.triggers = [
            Some(PracticeTrigger {
                reference: Some(actor),
                radius: 150.0,
            }),
            Some(PracticeTrigger {
                reference: Some(actor),
                radius: 150.0,
            }),
            None,
        ];
        assert!(goal.eligible(0, actor, Vec3::ZERO, &positions));
        assert!(!goal.eligible(1, actor, Vec3::ZERO, &positions));
        assert!(!goal.eligible(0, actor, Vec3::X * 400.0, &positions));
        assert!(goal.eligible(1, actor, Vec3::X * 400.0, &positions));
        // Vertical distance counts, but the 500-unit aim offset never does.
        assert!(!goal.eligible(0, actor, Vec3::Z * 200.0, &positions));
        goal.triggers[0].as_mut().unwrap().reference = None;
        assert!(!goal.eligible(0, actor, Vec3::ZERO, &positions));
        goal.triggers[0].as_mut().unwrap().reference = Some(FormId(99));
        assert!(!goal.eligible(0, actor, Vec3::ZERO, &positions));
    }

    #[test]
    fn cancelling_a_draw_retries_graph_rejection_without_releasing() {
        let mut practice = Practice {
            target: Some(0),
            left: 3,
            phase: Phase::Drawing(20.0, 1.0),
            ..Practice::default()
        };
        assert!(!practice.cancel(|event| {
            assert_eq!(event, "attackStop");
            false
        }));
        assert!(matches!(practice.phase, Phase::Cancelling));
        assert!(!practice.attack_step(
            10.0,
            &goal(),
            true,
            1.0,
            Some("attackStart"),
            0.0,
            true,
            &mut || 0,
            |_| panic!("pending cancellation cannot start or release an attack")
        ));
        assert!(practice.cancel(|event| {
            assert_eq!(event, "attackStop");
            true
        }));
        assert!(matches!(practice.phase, Phase::Ready));
        assert_eq!(practice.left, 0);
        assert_eq!(practice.target, None);
    }
}
