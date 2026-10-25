//! Actor state scripts set or ask for: critical stage (`SetCriticalStage`),
//! the stealth meter exemption (`SetNotShowOnStealthMeter`), the voice
//! recovery timer of actors other than the player, `Resurrect`, health and
//! equipment resets, and the sit and sleep states. Saved: `ACST`. Choices:
//! `known_ambiguities/remaining-natives.md`.

use std::collections::{HashMap, HashSet};

use esp::FormId;

use crate::ai::furniture::Use;
use crate::engine::{Engine, PLAYER_REF};

#[derive(Default)]
pub struct ActorFlags {
    /// The critical stage (1..=4) scripts gave an actor; none is 0.
    pub(crate) critical: HashMap<FormId, i32>,
    /// Actors that don't count on the stealth meter while not hostile.
    pub(crate) not_on_meter: HashSet<FormId>,
    /// When (unpaused real-time seconds) each actor's voice has rested.
    pub(crate) voice_until: HashMap<FormId, f64>,
    /// Actors flagged as doing the player a favor (`SetDoingFavor`).
    pub(crate) doing_favor: HashSet<FormId>,
}

impl Engine {
    /// `SetCriticalStage`. The "end" stages (goo end, disintegrate end) take
    /// the actor out of the world; the start stages are only remembered until
    /// effect shaders can show them.
    pub fn set_critical_stage(&mut self, actor: FormId, stage: i32) {
        if !(0..=4).contains(&stage) || actor == PLAYER_REF {
            return;
        }
        if stage == 0 {
            self.actor_flags.critical.remove(&actor);
        } else {
            self.actor_flags.critical.insert(actor, stage);
        }
        if matches!(stage, 2 | 4) {
            self.set_disabled(actor, true);
        }
    }

    pub fn critical_stage(&self, actor: FormId) -> i32 {
        self.actor_flags.critical.get(&actor).copied().unwrap_or(0)
    }

    /// `SetNotShowOnStealthMeter`.
    pub fn set_not_on_stealth_meter(&mut self, actor: FormId, not_shown: bool) {
        if not_shown {
            self.actor_flags.not_on_meter.insert(actor);
        } else {
            self.actor_flags.not_on_meter.remove(&actor);
        }
    }

    /// `SetDoingFavor`: a flag only; nothing asks followers for favors yet, so
    /// it changes nothing else.
    pub fn set_doing_favor(&mut self, actor: FormId, doing: bool) {
        if doing {
            self.actor_flags.doing_favor.insert(actor);
        } else {
            self.actor_flags.doing_favor.remove(&actor);
        }
    }

    /// `SetVoiceRecoveryTime`: the player's voice is the shout meter's.
    pub fn set_voice_recovery_time(&mut self, actor: FormId, secs: f32) {
        let secs = if secs.is_finite() { secs.max(0.0) } else { 0.0 };
        if actor == PLAYER_REF {
            self.shouts.recovery = secs;
            self.shouts.recovery_total = secs;
        } else if secs > 0.0 {
            let until = self.scripts.real_time + secs as f64;
            self.actor_flags.voice_until.insert(actor, until);
        } else {
            self.actor_flags.voice_until.remove(&actor);
        }
    }

    /// `GetVoiceRecoveryTime`: seconds until the voice has rested.
    pub fn voice_recovery_time(&self, actor: FormId) -> f32 {
        if actor == PLAYER_REF {
            return self.shouts.recovery;
        }
        self.actor_flags
            .voice_until
            .get(&actor)
            .map_or(0.0, |&t| (t - self.scripts.real_time).max(0.0) as f32)
    }

    /// `Resurrect`: back from the dead with its health, its body's ragdoll
    /// gone and its AI running. Its inventory is as it died with. Nothing but
    /// `dead` marks a body, so an actor not loaded just comes back alive when
    /// its cell next loads.
    pub fn resurrect(&mut self, actor: FormId) {
        if actor == PLAYER_REF {
            if self.player_dead() {
                self.player_died_at = None;
                self.player_health = self.player_max_health();
            }
            return;
        }
        self.world_state.dead.remove(&actor);
        self.world_state.deaths.remove(&actor);
        self.world_state.body_poses.remove(&actor);
        self.heal_wounds_away(actor);
        let Some(a) = self.actor_mut(actor).filter(|a| a.dead) else {
            return;
        };
        a.dead = false;
        a.exec.health = a.stats.max_health;
        a.exec.bleeding = None;
        a.exec.combat = None;
        a.halt(1.0);
        let ragdoll = a.ragdoll.take();
        let capsule = a.capsule;
        if let Some((rd, _)) = ragdoll {
            self.physics.remove_ragdoll(&rd);
        }
        if let Some(c) = capsule {
            self.physics.set_enabled(&[c], true);
        }
        log::info!("{actor} is resurrected");
    }

    /// `ResetHealthAndLimbs`: health back to its most, no longer bleeding
    /// out. Limbs: nothing here cripples one.
    pub fn reset_health_and_limbs(&mut self, actor: FormId) {
        if actor == PLAYER_REF {
            if !self.player_dead() {
                self.player_health = self.player_max_health();
            }
            return;
        }
        if let Some(a) = self.actor_mut(actor).filter(|a| !a.dead) {
            a.exec.health = a.stats.max_health;
            a.exec.bleeding = None;
        }
    }

    /// `UnequipAll`: everything worn and held except the spells, powers and
    /// shouts readied.
    pub fn unequip_all(&mut self, actor: FormId) {
        self.inventory_mut(actor);
        let lo = &self.lo;
        if let Some(inv) = self.inventories.get_mut(&actor) {
            inv.take_off_where(|f| !crate::world::inventory::is_magic(lo, f));
        }
    }

    /// `UnequipItemSlot`: the armor worn over a biped slot (30..=61).
    pub fn unequip_item_slot(&mut self, actor: FormId, slot: i32) {
        let Some(bit) = slot.checked_sub(30).filter(|b| (0..32).contains(b)) else {
            return;
        };
        self.inventory_mut(actor);
        let lo = &self.lo;
        if let Some(inv) = self.inventories.get_mut(&actor) {
            inv.take_off_where(|f| {
                lo.get(f).is_some_and(|rec| {
                    rec.tag().0 == *b"ARMO"
                        && crate::world::inventory::armor_slots(&rec) & (1 << bit) != 0
                })
            });
        }
    }

    /// `GetSitState` / `GetSleepState`: 0 neither, 2 on its way to the
    /// furniture, 3 in it, 4 getting out. The player's furniture use isn't
    /// tracked, so reads 0.
    pub fn furniture_state(&self, actor: FormId, kind: Use) -> i32 {
        self.actor_ref(actor)
            .and_then(|a| a.furniture_phase())
            .filter(|&(k, _)| k == kind)
            .map_or(0, |(_, phase)| phase)
    }

    pub(crate) fn restore_actor_flags(
        &mut self,
        critical: HashMap<FormId, i32>,
        not_on_meter: HashSet<FormId>,
        voice_remaining: HashMap<FormId, f32>,
        doing_favor: HashSet<FormId>,
    ) {
        let now = self.scripts.real_time;
        self.actor_flags.critical = critical;
        self.actor_flags.not_on_meter = not_on_meter;
        self.actor_flags.doing_favor = doing_favor;
        self.actor_flags.voice_until = voice_remaining
            .into_iter()
            .map(|(a, secs)| (a, now + secs as f64))
            .collect();
    }

    /// Each actor's voice recovery left, for saving.
    pub(crate) fn voice_remaining(&self) -> HashMap<FormId, f32> {
        let now = self.scripts.real_time;
        self.actor_flags
            .voice_until
            .iter()
            .filter(|&(_, &t)| t > now)
            .map(|(&a, &t)| (a, (t - now) as f32))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use esp::writer::{Node, RecordBuilder, write_plugin};
    use glam::Mat4;
    use papyrus::{NativeResult, Value};

    const BASE: FormId = FormId(0x800);
    const ACTOR: FormId = FormId(0x801);
    const HELM: FormId = FormId(0x810);
    const CUIRASS: FormId = FormId(0x811);
    const BOOTS: FormId = FormId(0x812);
    const SWORD: FormId = FormId(0x813);

    struct Fixture(std::path::PathBuf);

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn fixture() -> (Fixture, Engine) {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let f = Fixture(
            std::env::temp_dir().join(format!("vrm-actorflags-{}-{stamp}", std::process::id())),
        );
        std::fs::create_dir(&f.0).unwrap();
        let armor = |id, slots: u32| {
            let mut bod2 = slots.to_le_bytes().to_vec();
            bod2.extend([0; 4]);
            RecordBuilder::new(b"ARMO", id).field(b"BOD2", &bod2)
        };
        let records = [
            Node::top(b"NPC_", vec![RecordBuilder::new(b"NPC_", BASE)]),
            Node::top(
                b"ACHR",
                vec![RecordBuilder::new(b"ACHR", ACTOR).form(b"NAME", BASE)],
            ),
            Node::top(
                b"ARMO",
                vec![
                    armor(HELM, 1 << 0),
                    armor(CUIRASS, 1 << 2),
                    armor(BOOTS, 1 << 7),
                ],
            ),
            Node::top(b"WEAP", vec![RecordBuilder::new(b"WEAP", SWORD)]),
        ];
        std::fs::write(
            f.0.join("ActorFlags.esp"),
            write_plugin(0, &[], "test", "synthetic actor flags", 0x900, &records),
        )
        .unwrap();
        let e = crate::stats::action_tests::headless_engine(&f.0, "ActorFlags.esp");
        (f, e)
    }

    fn call(e: &mut Engine, func: &str, args: &[Value]) -> Value {
        let this = e.object_value(ACTOR);
        match crate::script::natives::call(e, "actor", func, Some(&this), args) {
            NativeResult::Value(v) => v,
            _ => panic!("Actor.{func} didn't return"),
        }
    }

    fn loaded_actor(e: &mut Engine) {
        let key = crate::render::CellKey::Interior(FormId(0x820));
        e.actor_cells.insert(ACTOR, key);
        let a = crate::ai::ActorRuntime::new(
            ACTOR,
            BASE,
            std::sync::Arc::new(crate::world::skeleton::Skeleton::default()),
            Mat4::IDENTITY,
            Vec::new(),
            0.0,
        );
        e.cells.entry(key).or_default().actors.push(a);
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn critical_stages_are_kept_and_the_end_stages_remove_the_actor() {
        let (_f, mut e) = fixture();
        call(&mut e, "setcriticalstage", &[Value::Int(3)]);
        assert_eq!(e.critical_stage(ACTOR), 3);
        assert_ne!(e.scripts.disabled.get(&ACTOR), Some(&true));
        call(&mut e, "setcriticalstage", &[Value::Int(9)]);
        assert_eq!(e.critical_stage(ACTOR), 3);
        call(&mut e, "setcriticalstage", &[Value::Int(4)]);
        assert_eq!(e.critical_stage(ACTOR), 4);
        assert_eq!(e.scripts.disabled.get(&ACTOR), Some(&true));
        call(&mut e, "setcriticalstage", &[Value::Int(0)]);
        assert_eq!(e.critical_stage(ACTOR), 0);
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn resurrect_brings_back_loaded_and_unloaded_bodies() {
        let (_f, mut e) = fixture();
        loaded_actor(&mut e);
        {
            let a = e.actor_mut(ACTOR).unwrap();
            a.dead = true;
            a.exec.health = 0.0;
            a.stats = std::sync::Arc::new(crate::ai::combat::CombatStats {
                max_health: 80.0,
                ..(*a.stats).clone()
            });
        }
        let place = crate::ai::schedule::Place::Interior(FormId(0x820));
        e.world_state.dead.insert(ACTOR, (place, glam::Vec3::ZERO));
        e.world_state.deaths.insert(
            ACTOR,
            crate::world_state::Death {
                killer: None,
                at: 1.0,
            },
        );
        assert!(e.is_dead(ACTOR));
        call(&mut e, "resurrect", &[]);
        assert!(!e.is_dead(ACTOR));
        assert_eq!(e.actor_ref(ACTOR).unwrap().exec.health, 80.0);
        assert!(e.death(ACTOR).is_none());
        assert!(e.world_state.dead.is_empty());
        // Unloaded: only the record of the body goes.
        e.world_state.dead.insert(ACTOR, (place, glam::Vec3::ZERO));
        e.actor_cells.remove(&ACTOR);
        e.resurrect(ACTOR);
        assert!(e.world_state.dead.is_empty());
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn health_resets_to_its_most() {
        let (_f, mut e) = fixture();
        loaded_actor(&mut e);
        let max = e.actor_ref(ACTOR).unwrap().stats.max_health;
        e.actor_mut(ACTOR).unwrap().exec.health = 1.0;
        call(&mut e, "resethealthandlimbs", &[]);
        assert_eq!(e.actor_ref(ACTOR).unwrap().exec.health, max);
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn unequipping_by_slot_and_all() {
        let (_f, mut e) = fixture();
        for item in [HELM, CUIRASS, BOOTS, SWORD] {
            e.inventory_mut(ACTOR).add(item, 1);
            e.inventory_mut(ACTOR).wear(item);
        }
        // Slot 32 is the body; 33 (hands) covers nothing worn.
        call(&mut e, "unequipitemslot", &[Value::Int(33)]);
        assert_eq!(e.inventories[&ACTOR].equipped().len(), 4);
        call(&mut e, "unequipitemslot", &[Value::Int(32)]);
        assert_eq!(e.inventories[&ACTOR].equipped(), [HELM, BOOTS, SWORD]);
        call(&mut e, "unequipitemslot", &[Value::Int(99)]);
        assert_eq!(e.inventories[&ACTOR].equipped().len(), 3);
        call(&mut e, "unequipall", &[]);
        assert!(e.inventories[&ACTOR].equipped().is_empty());
        // Still carried.
        assert_eq!(e.inventories[&ACTOR].count(SWORD), 1);
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn voice_recovery_counts_down_for_actors_and_is_the_meter_for_the_player() {
        let (_f, mut e) = fixture();
        assert_eq!(call(&mut e, "getvoicerecoverytime", &[]), Value::Float(0.0));
        call(&mut e, "setvoicerecoverytime", &[Value::Float(20.0)]);
        assert_eq!(
            call(&mut e, "getvoicerecoverytime", &[]),
            Value::Float(20.0)
        );
        e.scripts.real_time += 5.0;
        assert_eq!(
            call(&mut e, "getvoicerecoverytime", &[]),
            Value::Float(15.0)
        );
        e.scripts.real_time += 60.0;
        assert_eq!(call(&mut e, "getvoicerecoverytime", &[]), Value::Float(0.0));
        e.set_voice_recovery_time(PLAYER_REF, 12.0);
        assert_eq!(e.shouts.recovery, 12.0);
        assert_eq!(e.voice_recovery_time(PLAYER_REF), 12.0);
        e.set_voice_recovery_time(PLAYER_REF, f32::NAN);
        assert_eq!(e.shouts.recovery, 0.0);
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn without_furniture_sit_and_sleep_read_zero_and_arrows_clear_quietly() {
        let (_f, mut e) = fixture();
        loaded_actor(&mut e);
        assert_eq!(call(&mut e, "getsitstate", &[]), Value::Int(0));
        assert_eq!(call(&mut e, "getsleepstate", &[]), Value::Int(0));
        assert_eq!(call(&mut e, "clearextraarrows", &[]), Value::None);
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn the_stealth_meter_reads_the_actors_it_counts() {
        let (_f, mut e) = fixture();
        let eye = |e: &Engine| e.sneak_eye();
        let before = eye(&e);
        e.detection.of_player.insert(ACTOR, 1.0e6);
        assert_eq!(eye(&e), before);
        e.detection.on_meter.insert(ACTOR, 1.0e6);
        assert_eq!(eye(&e), 1.0);
        call(&mut e, "setnotshowonstealthmeter", &[Value::Bool(true)]);
        assert!(e.actor_flags.not_on_meter.contains(&ACTOR));
        call(&mut e, "setnotshowonstealthmeter", &[Value::Bool(false)]);
        assert!(e.actor_flags.not_on_meter.is_empty());
    }
}
