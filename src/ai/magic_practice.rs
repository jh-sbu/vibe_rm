//! UseMagic packages: at the package's location the actor casts the package's
//! spell at its target (the College's practice targets, Neloth's candlelight)
//! or on itself, a number of casts apart by a cooldown, holding a
//! concentration spell for the cast time. Outside combat only; the engine
//! chooses the spell for the hands (`Engine::update_combat_spells`) and pays
//! for it (`Engine::npc_casts`), as for the casters of a fight.

use esp::FormId;
use glam::Vec3;

use super::casters::{CHANNEL_TICK, CombatCast, HandCast, HandPhase};
use super::{ActorRuntime, State, World, uniform};
use crate::casting::hand_prefix;
use crate::world::inventory::RIGHT;

/// A UseMagic goal: what to cast at what, and its timing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MagicGoal {
    pub spell: FormId,
    /// What it is cast at; the actor itself casts on itself.
    pub target: Option<FormId>,
    /// Bounds-centre offset of the target, to aim at.
    pub aim_offset: Vec3,
    pub hold_when_blocked: bool,
    pub cast_time: (f32, f32),
    pub cooldown: (f32, f32),
    pub casts: (u32, u32),
    pub dual: bool,
}

/// Where an actor is in its casting practice.
#[derive(Debug, Clone, Copy, Default)]
pub struct MagicPractice {
    /// Casts before the package is done; picked at the first.
    left: Option<u32>,
    /// Seconds before the next cast.
    wait: f32,
    /// The cast in a humanoid's hand.
    cast: Option<HandCast>,
    /// Seconds left of a concentration spell being held.
    channel: f32,
    /// Something is in the way of the target (set by the engine).
    pub(crate) blocked: bool,
    /// Where the cast in progress aims, for the engine.
    pub(crate) aim: Option<Vec3>,
    /// The package this is the practice of: each of several packages
    /// (Onmund's four spells) starts afresh.
    package: Option<FormId>,
}

/// Seconds a caster waits before trying again when it can't start a cast
/// (hands still coming out, nothing to aim at; no source).
const RETRY: f32 = 0.5;
/// How far off its target a caster may face and still cast (radians; as the
/// casters of a fight).
const FACING: f32 = 0.2;

impl MagicPractice {
    /// Holding a concentration spell.
    pub(crate) fn channelling(&self) -> bool {
        self.channel > 0.0
    }
}

impl MagicGoal {
    /// Whether the spell is cast on the caster itself.
    pub fn at_self(&self, actor: FormId) -> bool {
        self.target.is_none_or(|t| t == actor)
    }
}

impl ActorRuntime {
    /// At a UseMagic goal's location, out of combat.
    pub(crate) fn magic_practising(&self) -> bool {
        self.exec.combat.is_none()
            && self.exec.goal.is_some_and(|g| {
                g.magic.is_some()
                    && self.exec.pos.truncate().distance(g.centre.truncate()) <= g.radius.max(96.0)
            })
    }

    /// A caster's turn at its practice: face the target, then cast every
    /// cooldown until the package's casts are done.
    pub(crate) fn magic_practice_step(&mut self, dt: f32, goal: &MagicGoal, w: &mut World) {
        self.speed = 0.0;
        let package = self
            .exec
            .current
            .and_then(|i| self.exec.packages.get(i))
            .map(|p| p.id);
        if package.is_some() && package == self.exec.finished_package {
            return;
        }
        // The engine puts the spell in the hands once it is here and known.
        let Some(spell) = self.combat_spell.filter(|s| s.spell == goal.spell) else {
            return;
        };
        let mut m = self.magic_practice.unwrap_or_default();
        if m.package != package {
            if let Some(h) = m.cast {
                self.release_hand(h.hand);
            }
            m = MagicPractice {
                blocked: m.blocked,
                package,
                ..Default::default()
            };
        }
        let left = *m.left.get_or_insert_with(|| {
            goal.casts.0 + ((w.rand)() % u64::from(goal.casts.1 - goal.casts.0 + 1)) as u32
        });
        let at_self = spell.self_target || goal.at_self(self.ref_id);
        let aim = if at_self {
            None
        } else {
            goal.target
                .and_then(|t| w.targets.get(&t))
                .map(|&p| p + goal.aim_offset)
        };
        m.aim = aim;
        let mut off = 0.0;
        if let Some(aim) = aim {
            self.state = State::Idle(1.0);
            off = self
                .turn_towards((aim - self.exec.pos).with_z(0.0).normalize_or_zero(), dt)
                .abs();
        }
        let humanoid = self.graph.as_ref().is_some_and(|g| g.project().humanoid());
        let recipient = if at_self {
            self.ref_id
        } else {
            goal.target.unwrap_or(self.ref_id)
        };
        // A cast under way: a held spell flows, a thrown one plays out.
        if m.channel > 0.0 {
            self.channel_step(dt, &mut m, spell, recipient);
            if m.channel <= 0.0 {
                self.cast_over(&mut m, goal, left, package, w);
            }
            self.magic_practice = Some(m);
            return;
        }
        if let Some(h) = m.cast {
            match self.advance_throw(dt, w, h) {
                Some(h) => m.cast = Some(h),
                None => {
                    self.release_hand(h.hand);
                    m.cast = None;
                    self.cast_over(&mut m, goal, left, package, w);
                }
            }
            self.magic_practice = Some(m);
            return;
        }
        m.wait -= dt;
        if m.wait > 0.0 || off > FACING || (goal.hold_when_blocked && m.blocked) {
            self.magic_practice = Some(m);
            return;
        }
        if humanoid && !self.drawn {
            if self.graph_event("WeapEquip", &mut w.clips) {
                self.drawn = true;
            }
            m.wait = RETRY * 2.0;
            self.magic_practice = Some(m);
            return;
        }
        if aim.is_none() && !at_self {
            // Its target isn't loaded.
            m.wait = RETRY;
            self.magic_practice = Some(m);
            return;
        }
        let cast = CombatCast {
            spell,
            target: recipient,
            landings: 1,
            dual: spell.dual && humanoid,
        };
        let mut started = None;
        if humanoid {
            let hand = self.cast_hand.unwrap_or(RIGHT);
            match self.begin_hand_cast(hand, cast, w) {
                Some(h) => started = Some(h),
                None => {
                    // Not drawn yet, or busy: again shortly.
                    m.wait = RETRY;
                    self.magic_practice = Some(m);
                    return;
                }
            }
        }
        log::debug!(
            "{} casts {} at {recipient} ({left} to go)",
            self.ref_id,
            spell.spell
        );
        m.cast = started;
        if spell.concentration {
            m.channel = uniform(w.rand, goal.cast_time.0, goal.cast_time.1).max(CHANNEL_TICK);
        } else if !humanoid {
            self.casts = Some(cast);
            self.cast_over(&mut m, goal, left, package, w);
        }
        self.magic_practice = Some(m);
    }

    /// A held spell flowing: a landing each tick for as long as it is held,
    /// and the hand's cast over after.
    fn channel_step(
        &mut self,
        dt: f32,
        m: &mut MagicPractice,
        spell: super::casters::CombatSpell,
        recipient: FormId,
    ) {
        // Flowing in the hand once its state machine runs.
        if let Some(h) = m.cast.as_mut()
            && h.phase == HandPhase::Charging
        {
            h.phase = HandPhase::Flowing;
            let ev = format!("{}_SpellRelease_Event", hand_prefix(h.hand));
            if let Some(g) = self.graph.as_mut() {
                g.send_event(&ev);
            }
        }
        let before = (m.channel / CHANNEL_TICK).ceil();
        m.channel -= dt;
        let after = (m.channel.max(0.0) / CHANNEL_TICK).ceil();
        self.casts = Some(CombatCast {
            spell,
            target: recipient,
            landings: (before - after) as u32,
            dual: m.cast.is_some_and(|h| h.cast.dual),
        });
        if m.channel <= 0.0 {
            m.channel = 0.0;
            if let Some(h) = m.cast.take() {
                self.release_hand(h.hand);
            }
        }
    }

    /// A cast is over: the next one comes after the cooldown, and the package
    /// is done after its last.
    fn cast_over(
        &mut self,
        m: &mut MagicPractice,
        goal: &MagicGoal,
        left: u32,
        package: Option<FormId>,
        w: &mut World,
    ) {
        m.left = Some(left.saturating_sub(1));
        m.wait = uniform(w.rand, goal.cooldown.0, goal.cooldown.1);
        if left <= 1 {
            log::debug!("{} finished its casting practice", self.ref_id);
            self.exec.finished_package = package;
            self.exec.next_eval = 0.0;
            *m = MagicPractice {
                package: m.package,
                ..Default::default()
            };
        }
    }

    /// The practice is over (the package ended, the actor left its place):
    /// the hand lets its spell go and the actor puts its hands away.
    pub(crate) fn end_magic_practice(&mut self, w: &mut World) {
        let Some(m) = self.magic_practice.take() else {
            return;
        };
        if let Some(h) = m.cast {
            self.release_hand(h.hand);
        }
        if self.drawn && self.exec.combat.is_none() && self.graph_event("Unequip", &mut w.clips) {
            self.drawn = false;
        }
    }
}

/// How far short of the target a hit counts as in the way (the ray runs to
/// the target's bounds centre, inside the thing: no source).
const BLOCK_SLACK: f32 = 100.0;

impl crate::engine::Engine {
    /// Whether something stands between each practising caster and its target
    /// (`HoldWhenBlocked`): a ray from its eye to the target, past its own body.
    pub(crate) fn update_magic_blocks(&mut self) {
        let casters: Vec<(FormId, FormId, Vec3, Vec3)> = self
            .cells
            .values()
            .flat_map(|rt| &rt.actors)
            .filter(|a| a.magic_practising() && !a.dead)
            .filter_map(|a| {
                let goal = a.exec.goal.and_then(|g| g.magic)?;
                let target = goal.target.filter(|&t| t != a.ref_id)?;
                goal.hold_when_blocked.then_some((
                    a.ref_id,
                    target,
                    a.exec.pos + Vec3::Z * 110.0 * a.scale,
                    goal.aim_offset,
                ))
            })
            .collect();
        for (actor, target, eye, offset) in casters {
            let aim = self.ref_position(target).map(|p| p + offset);
            let blocked = aim.is_some_and(|aim| {
                let to = aim - eye;
                let (dir, dist) = (to.normalize_or_zero(), to.length());
                self.physics
                    .raycast_excluding(eye, dir, dist, actor)
                    .is_some_and(|(at, owner)| at < dist - BLOCK_SLACK && owner != Some(target))
            });
            if let Some(a) = self.actor_mut(actor) {
                let mut m = a.magic_practice.unwrap_or_default();
                m.blocked = blocked;
                a.magic_practice = Some(m);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::casters::CombatSpell;
    use crate::ai::{Clips, Goal, furniture::FurnitureWorld, nav::NavWorld};
    use crate::world::animation::{AnimationLibrary, BehaviorLibrary};
    use crate::world::skeleton::Skeleton;
    use glam::Mat4;
    use std::collections::HashMap;
    use std::sync::Arc;

    const SPELL: FormId = FormId(0x500);
    const TARGET: FormId = FormId(0x600);

    fn goal() -> MagicGoal {
        MagicGoal {
            spell: SPELL,
            target: Some(TARGET),
            aim_offset: Vec3::Z * 50.0,
            hold_when_blocked: false,
            cast_time: (2.0, 2.0),
            cooldown: (3.0, 3.0),
            casts: (2, 2),
            dual: false,
        }
    }

    fn spell(concentration: bool, self_target: bool) -> CombatSpell {
        CombatSpell {
            spell: SPELL,
            range: 1000.0,
            concentration,
            self_target,
            start: "SpellAimedStart",
            charge: 0.0,
            dual: false,
            staff: false,
            ward: false,
        }
    }

    /// A creature (no graph, so no hands) that holds `spell`, facing +Y with the
    /// target 500 units ahead.
    fn caster(spell: CombatSpell, goal: MagicGoal) -> ActorRuntime {
        let mut a = ActorRuntime::new(
            FormId(1),
            FormId(2),
            Arc::new(Skeleton::default()),
            Mat4::IDENTITY,
            Vec::new(),
            0.0,
        );
        a.combat_spell = Some(spell);
        let mut g = Goal::travel(Vec3::ZERO);
        g.magic = Some(goal);
        a.exec.goal = Some(g);
        a
    }

    fn with_world(target: Vec3, test: impl FnOnce(&mut World<'_>)) {
        let nav = NavWorld::default();
        let mut furniture = FurnitureWorld::default();
        let vfs = vfs::Vfs::new("/nonexistent-vrm-magic-practice-fixture", &[]);
        let mut anims = AnimationLibrary::default();
        let mut behaviors = BehaviorLibrary::default();
        let targets = HashMap::from([(TARGET, target)]);
        let mut world = World {
            nav: &nav,
            furniture: &mut furniture,
            clips: Clips {
                vfs: &vfs,
                anims: &mut anims,
                behaviors: &mut behaviors,
            },
            may_use: &|_, _, _| false,
            claimed: &HashMap::new(),
            patrols: &HashMap::new(),
            targets: &targets,
            holds: &|_, _| false,
            rand: &mut || 1,
        };
        test(&mut world);
    }

    #[test]
    fn a_thrown_spell_goes_at_the_target_then_waits_its_cooldown() {
        with_world(Vec3::Y * 500.0, |w| {
            let g = goal();
            let mut a = caster(spell(false, false), g);
            a.magic_practice_step(0.1, &g, w);
            let cast = a.casts.take().expect("a cast");
            assert_eq!((cast.target, cast.landings), (TARGET, 1));
            // Aimed at the target's body, not its feet.
            assert_eq!(
                a.magic_practice.unwrap().aim,
                Some(Vec3::new(0.0, 500.0, 50.0))
            );
            // Cooling down.
            a.magic_practice_step(2.0, &g, w);
            assert!(a.casts.is_none());
            a.magic_practice_step(1.5, &g, w);
            assert!(a.casts.is_some());
        });
    }

    #[test]
    fn the_package_is_done_after_its_casts() {
        with_world(Vec3::Y * 500.0, |w| {
            let g = goal();
            let mut a = caster(spell(false, false), g);
            a.exec.next_eval = 9.0;
            a.magic_practice_step(0.1, &g, w);
            assert!(a.casts.take().is_some());
            assert_eq!(a.exec.next_eval, 9.0, "one to go");
            a.magic_practice_step(3.1, &g, w);
            assert!(a.casts.take().is_some());
            // Done: the actor looks for its next package at once.
            assert_eq!(a.exec.next_eval, 0.0);
        });
    }

    #[test]
    fn a_held_spell_lands_every_tick_for_the_cast_time() {
        with_world(Vec3::Y * 500.0, |w| {
            let g = goal();
            let mut a = caster(spell(true, false), g);
            a.magic_practice_step(0.1, &g, w);
            assert!(a.casts.is_none(), "it is begun, not landed");
            let mut landings = 0;
            for _ in 0..30 {
                a.magic_practice_step(0.1, &g, w);
                landings += a.casts.take().map_or(0, |c| c.landings);
            }
            // Two seconds in quarter-second ticks, the cast over after.
            assert_eq!(landings, 8);
            assert_eq!(a.magic_practice.unwrap().channel, 0.0);
        });
    }

    #[test]
    fn a_spell_on_itself_needs_no_target_or_facing() {
        with_world(Vec3::ZERO, |w| {
            let mut g = goal();
            g.target = None;
            let mut a = caster(spell(false, true), g);
            a.exec.heading = 3.0;
            a.magic_practice_step(0.1, &g, w);
            let cast = a.casts.take().expect("a cast");
            assert_eq!(cast.target, a.ref_id);
            assert_eq!(a.exec.heading, 3.0);
        });
    }

    #[test]
    fn a_caster_faces_its_target_before_casting() {
        with_world(Vec3::Y * 500.0, |w| {
            let g = goal();
            let mut a = caster(spell(false, false), g);
            a.exec.heading = std::f32::consts::PI;
            a.magic_practice_step(0.1, &g, w);
            assert!(a.casts.is_none());
            for _ in 0..40 {
                a.magic_practice_step(0.1, &g, w);
            }
            assert!(
                a.exec.heading.abs() < 0.3 || (a.exec.heading - std::f32::consts::TAU).abs() < 0.3
            );
            assert!(a.casts.is_some() || a.magic_practice.unwrap().left == Some(1));
        });
    }

    #[test]
    fn a_blocked_target_holds_the_cast_only_when_asked() {
        with_world(Vec3::Y * 500.0, |w| {
            let mut g = goal();
            g.hold_when_blocked = true;
            let mut a = caster(spell(false, false), g);
            a.magic_practice = Some(MagicPractice {
                blocked: true,
                ..Default::default()
            });
            a.magic_practice_step(0.1, &g, w);
            assert!(a.casts.is_none());
            g.hold_when_blocked = false;
            a.magic_practice_step(0.1, &g, w);
            assert!(a.casts.is_some());
        });
    }

    #[test]
    fn another_package_starts_its_practice_afresh() {
        with_world(Vec3::Y * 500.0, |w| {
            let mut g = goal();
            g.casts = (5, 5);
            let mut a = caster(spell(false, false), g);
            a.magic_practice_step(0.1, &g, w);
            assert_eq!(a.magic_practice.unwrap().left, Some(4));
            // Its package is another's now: its own count, no cooldown owed.
            a.magic_practice.as_mut().unwrap().package = Some(FormId(0x77));
            a.magic_practice_step(0.1, &g, w);
            assert_eq!(a.magic_practice.unwrap().left, Some(4));
            assert!(a.casts.is_some());
        });
    }

    #[test]
    fn it_waits_for_the_spell_in_its_hands_and_its_target_to_be_here() {
        with_world(Vec3::Y * 500.0, |w| {
            let g = goal();
            let mut a = caster(spell(false, false), g);
            // Some other spell is in hand: not this practice's.
            a.combat_spell = Some(CombatSpell {
                spell: FormId(0x501),
                ..spell(false, false)
            });
            a.magic_practice_step(0.1, &g, w);
            assert!(a.casts.is_none());
            a.combat_spell = Some(spell(false, false));
            let mut elsewhere = goal();
            elsewhere.target = Some(FormId(0x601));
            a.magic_practice_step(0.1, &elsewhere, w);
            assert!(a.casts.is_none(), "its target isn't loaded");
        });
    }

    mod engine {
        use super::*;
        use crate::engine::Engine;
        use esp::actor_value as av;
        use esp::writer::{Node, RecordBuilder, write_plugin};

        const SELF_SPELL: FormId = FormId(0x810);
        const AIMED_SPELL: FormId = FormId(0x811);
        const TOUCH_SPELL: FormId = FormId(0x812);
        const ACTOR: FormId = FormId(0x901);

        /// Synthetic records: a spell on the caster, an aimed one and a touch one.
        fn engine() -> (std::path::PathBuf, Engine) {
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let dir = std::env::temp_dir()
                .join(format!("vrm-magic-practice-{}-{stamp}", std::process::id()));
            std::fs::create_dir(&dir).unwrap();
            let mut data = [0; 0x6c];
            data[0x0c..0x10].copy_from_slice(&u32::MAX.to_le_bytes());
            data[0x10..0x14].copy_from_slice(&u32::MAX.to_le_bytes());
            data[0x44..0x48].copy_from_slice(&av::HEALTH.to_le_bytes());
            let spell = |id: u32, delivery: u32| {
                let mut spit = [0; 36];
                spit[0..4].copy_from_slice(&50u32.to_le_bytes());
                spit[16..20].copy_from_slice(&1u32.to_le_bytes());
                spit[20..24].copy_from_slice(&delivery.to_le_bytes());
                RecordBuilder::new(b"SPEL", FormId(id))
                    .zstring(b"FULL", "Test")
                    .field(b"SPIT", &spit)
                    .u32(b"EFID", 0x800)
                    .field(b"EFIT", &[0; 12])
            };
            let records = [
                Node::top(
                    b"MGEF",
                    vec![RecordBuilder::new(b"MGEF", FormId(0x800)).field(b"DATA", &data)],
                ),
                Node::top(
                    b"SPEL",
                    vec![
                        spell(SELF_SPELL.0, 0),
                        spell(AIMED_SPELL.0, 2),
                        spell(TOUCH_SPELL.0, 1),
                    ],
                ),
            ];
            std::fs::write(
                dir.join("Practice.esp"),
                write_plugin(0, &[], "test", "casting practice", 0x900, &records),
            )
            .unwrap();
            let mut e = crate::stats::action_tests::headless_engine(&dir, "Practice.esp");
            let key = crate::render::CellKey::Interior(FormId(0x820));
            e.actor_cells.insert(ACTOR, key);
            e.cells
                .entry(key)
                .or_default()
                .actors
                .push(ActorRuntime::new(
                    ACTOR,
                    FormId(2),
                    Arc::new(Skeleton::default()),
                    Mat4::IDENTITY,
                    Vec::new(),
                    0.0,
                ));
            (dir, e)
        }

        fn practise(e: &mut Engine, spell: FormId) {
            let mut g = Goal::travel(Vec3::ZERO);
            g.magic = Some(MagicGoal {
                spell,
                target: Some(TARGET),
                ..goal()
            });
            e.actor_mut(ACTOR).unwrap().exec.goal = Some(g);
        }

        #[test]
        #[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
        fn the_hands_take_the_spell_of_a_practice_at_its_place_and_it_is_paid_for() {
            let (dir, mut e) = engine();
            practise(&mut e, SELF_SPELL);
            e.set_actor_value(ACTOR, av::MAGICKA, 100.0);
            e.update_combat_spells(0.1);
            let held = e.actor_ref(ACTOR).unwrap().combat_spell.expect("in hand");
            assert!(held.self_target && !held.concentration);
            assert_eq!(e.actor_ref(ACTOR).unwrap().cast_hand, Some(RIGHT));
            // A cast made now is paid from its magicka, though nothing fights.
            e.actor_mut(ACTOR).unwrap().casts = Some(CombatCast {
                spell: held,
                target: ACTOR,
                landings: 1,
                dual: false,
            });
            e.npc_casts();
            assert_eq!(e.actor_value(ACTOR, av::MAGICKA), 50.0);
            // Away from its place the spell goes back.
            e.actor_mut(ACTOR).unwrap().exec.pos = Vec3::X * 5000.0;
            e.update_combat_spells(0.1);
            assert!(e.actor_ref(ACTOR).unwrap().combat_spell.is_none());
            let _ = std::fs::remove_dir_all(dir);
        }

        #[test]
        #[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
        fn aimed_spells_are_cast_at_their_target_and_touch_ones_are_not_cast() {
            let (dir, e) = engine();
            let aimed = e.practice_spell(AIMED_SPELL).expect("aimed");
            assert!(!aimed.self_target && aimed.range > 0.0);
            assert!(e.practice_spell(TOUCH_SPELL).is_none());
            assert!(e.practice_spell(FormId(0x999)).is_none());
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}
