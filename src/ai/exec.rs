//! Actors' execution: what they are doing, kept apart from the body that
//! does it while loaded (skeleton, graph, collision), so it can outlast
//! unloading (`plans/ai-actor-execution.md`).

use esp::FormId;
use glam::Vec3;

use super::package::Package;
use super::schedule::Place;
use super::{Goal, StopProgress, combat};
use crate::world_state::Wounds;

/// An actor's execution: its packages and how far it has got with them, its
/// position and its condition. A loaded actor's is in its `ActorRuntime`;
/// one that unloaded keeps it in `Engine::away` until it loads again.
#[derive(Debug, Clone, Default)]
pub struct ActorExec {
    pub packages: Vec<Package>,
    pub current: Option<usize>,
    pub goal: Option<Goal>,
    /// Feet position and heading (radians clockwise from +Y). The height is the
    /// navmesh's, which pathing and movement keep to.
    pub pos: Vec3,
    pub heading: f32,
    /// Where it was as it unloaded (unset while loaded: `Engine::actor_cells`
    /// says where it is).
    pub place: Option<Place>,
    /// The goal was set by hand (console `travel`): packages leave it alone.
    pub(super) pinned: bool,
    /// Which of a multi-location sandbox package's stops it is at.
    pub(super) stop: StopProgress,
    /// Patrol progress: (route start, index of the point heading for).
    pub(super) patrol: Option<(FormId, usize)>,
    /// Times it has activated the target of the Activate package it runs.
    pub(super) activations: (Option<FormId>, u32),
    /// A UseWeapon package that completed (its barrages or time ran out): it is
    /// not picked again until it stops applying.
    pub(crate) finished_package: Option<FormId>,
    /// `ScriptState::alias_gen` its packages were taken at.
    pub(crate) alias_gen: u64,
    /// Real seconds until its packages are evaluated again.
    pub(super) next_eval: f32,
    /// Health and stamina left; how hurt it was as it unloaded (its health
    /// then follows the clock: `Wounds::health_at`), none when well or loaded.
    pub(crate) health: f32,
    pub(crate) stamina: f32,
    pub(crate) wounds: Option<Wounds>,
    /// Whom it is fighting, and seconds left bleeding out (essential actors
    /// brought down).
    pub(crate) combat: Option<combat::Combat>,
    pub(crate) bleeding: Option<f32>,
    /// Searching for someone it noticed or lost (detection's Alert and Lost).
    pub(crate) search: Option<crate::detection::Search>,
    /// Force greet: going up to the player; seconds until it checks again whether
    /// to, and until it may greet again after a conversation.
    pub(crate) greeting: bool,
    pub(crate) greet_check: f32,
    pub(crate) greet_wait: f32,
}

impl ActorExec {
    /// A fresh start: `stagger` spreads actors' first package evaluations and
    /// greeting checks.
    pub fn new(packages: Vec<Package>, pos: Vec3, heading: f32, stagger: f32) -> Self {
        ActorExec {
            packages,
            pos,
            heading,
            health: 50.0,
            stamina: 50.0,
            greet_check: stagger,
            next_eval: stagger,
            ..Default::default()
        }
    }

    /// Its actor unloaded from `place`, `wounds` as hurt as it left.
    pub(crate) fn away(mut self, place: Option<Place>, wounds: Option<Wounds>) -> Self {
        self.place = place;
        self.wounds = wounds;
        self
    }

    /// Only wounds: all that saves keep yet of an actor away.
    pub(crate) fn wounded(wounds: Wounds) -> Self {
        ActorExec {
            health: wounds.health,
            wounds: Some(wounds),
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Engine;
    use esp::writer::{Node, RecordBuilder, write_plugin};
    use glam::Mat4;

    const BASE: FormId = FormId(0x800);
    const ACTOR: FormId = FormId(0x801);
    const CELL: FormId = FormId(0x820);

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
        let f =
            Fixture(std::env::temp_dir().join(format!("vrm-exec-{}-{stamp}", std::process::id())));
        std::fs::create_dir(&f.0).unwrap();
        let records = [
            Node::top(b"NPC_", vec![RecordBuilder::new(b"NPC_", BASE)]),
            Node::top(
                b"ACHR",
                vec![RecordBuilder::new(b"ACHR", ACTOR).form(b"NAME", BASE)],
            ),
        ];
        std::fs::write(
            f.0.join("Exec.esp"),
            write_plugin(0, &[], "test", "synthetic actor execution", 0x900, &records),
        )
        .unwrap();
        let e = crate::stats::action_tests::headless_engine(&f.0, "Exec.esp");
        (f, e)
    }

    /// The actor loaded in `CELL`, hurt, at a goal, with its packages
    /// taken at alias generation 7.
    fn loaded_actor(e: &mut Engine) {
        let key = crate::render::CellKey::Interior(CELL);
        e.actor_cells.insert(ACTOR, key);
        let mut a = crate::ai::ActorRuntime::new(
            ACTOR,
            BASE,
            std::sync::Arc::new(crate::world::skeleton::Skeleton::default()),
            Mat4::from_translation(Vec3::new(10.0, 20.0, 30.0)),
            Vec::new(),
            0.0,
        );
        a.stats = std::sync::Arc::new(crate::ai::combat::CombatStats {
            max_health: 80.0,
            health_regen: 0.0,
            ..(*a.stats).clone()
        });
        a.exec.health = 30.0;
        a.exec.alias_gen = 7;
        a.exec.current = Some(0);
        a.exec.heading = 1.25;
        e.cells.entry(key).or_default().actors.push(a);
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn unloading_keeps_the_execution_and_loading_takes_it_back() {
        let (_f, mut e) = fixture();
        loaded_actor(&mut e);
        e.unload_all();
        let x = &e.away[&ACTOR];
        assert_eq!(x.pos, Vec3::new(10.0, 20.0, 30.0));
        assert_eq!(x.heading, 1.25);
        assert_eq!(x.current, Some(0));
        assert_eq!(x.alias_gen, 7);
        assert_eq!(x.health, 30.0);
        assert_eq!(x.place, Some(Place::Interior(CELL)));
        let w = x.wounds.expect("it left hurt");
        assert_eq!((w.health, w.max), (30.0, 80.0));
        assert_eq!(e.actor_health(ACTOR), Some((30.0, 80.0)));
        // What saves keep of it is its wounds.
        assert_eq!(e.away_wounds()[&ACTOR].health, 30.0);
        // Loading again takes it back, as hurt as it left.
        assert_eq!(e.returning_health(ACTOR, 80.0), 30.0);
        assert!(!e.away.contains_key(&ACTOR));
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn a_well_actor_away_comes_back_at_its_most_and_dead_ones_have_none() {
        let (_f, mut e) = fixture();
        loaded_actor(&mut e);
        e.actor_mut(ACTOR).unwrap().exec.health = 80.0;
        e.unload_all();
        assert!(e.away[&ACTOR].wounds.is_none());
        assert!(e.away_wounds().is_empty());
        assert_eq!(e.returning_health(ACTOR, 80.0), 80.0);

        loaded_actor(&mut e);
        e.actor_mut(ACTOR).unwrap().dead = true;
        e.unload_all();
        assert!(!e.away.contains_key(&ACTOR));
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn a_created_actor_deleted_while_away_leaves_nothing_away() {
        let (_f, mut e) = fixture();
        loaded_actor(&mut e);
        e.created_refs.refs.insert(
            ACTOR,
            crate::created::Created {
                base: BASE,
                position: Vec3::ZERO,
                rotation: Vec3::ZERO,
                origin: None,
                place: Some(Place::Interior(CELL)),
                container: None,
                location: None,
                actor: true,
                count: 1,
                owner: None,
            },
        );
        e.unload_all();
        assert!(e.away.contains_key(&ACTOR));
        e.delete_created(ACTOR);
        assert!(!e.away.contains_key(&ACTOR));
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn resurrecting_an_actor_away_heals_its_wounds() {
        let (_f, mut e) = fixture();
        loaded_actor(&mut e);
        e.unload_all();
        e.resurrect(ACTOR);
        assert!(e.away[&ACTOR].wounds.is_none());
        assert_eq!(e.returning_health(ACTOR, 80.0), 80.0);
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn a_save_without_executions_puts_back_only_the_hurt() {
        let (_f, mut e) = fixture();
        loaded_actor(&mut e);
        e.unload_all();
        let wounds = e.away_wounds();
        e.restore_away_wounds(wounds);
        let x = &e.away[&ACTOR];
        assert_eq!(x.health, 30.0);
        assert_eq!(x.place, None);
        assert!(x.packages.is_empty());
        assert_eq!(e.returning_health(ACTOR, 80.0), 30.0);
    }
}
