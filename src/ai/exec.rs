//! Actors' execution: what they are doing, kept apart from the body that
//! does it while loaded (skeleton, graph, collision), so it can outlast
//! unloading (`plans/ai-actor-execution.md`).

use esp::FormId;
use glam::Vec3;

use super::package::{Behaviour, Package};
use super::schedule::Place;
use super::{ActorRuntime, Goal, StopProgress, combat};
use crate::engine::Engine;
use crate::world_state::Wounds;

/// How close to a travel package's location counts as arrived, beyond its radius.
const ARRIVED_SLACK: f32 = 150.0;

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
    /// Where it is away (unset while loaded: `Engine::actor_cells` says where
    /// it is; and for actors a save gave only wounds, placed by their
    /// schedules). Its position there is NaN when its package put it
    /// anywhere in a cell.
    pub place: Option<Place>,
    /// What it was doing there as its cell unloaded, to take up again as it
    /// attaches; none when it isn't where it left (gone through a door or out
    /// of sight, moved by a script, or its package changed or moved it on
    /// meanwhile).
    pub stage: Option<Stage>,
    /// Away: where its package has it is worked out (`Engine::step_away`);
    /// unset while it has a journey to make, or hasn't been looked at since
    /// its package started or it left.
    pub(crate) settled: bool,
    /// Away: the real time (`ScriptState::real_time`) its run ends, having
    /// been put at its target (`OnPackageEnd` 2.5 s after the start:
    /// autotest14).
    pub(crate) ends_at: Option<f64>,
    /// Away: asked to evaluate its packages (`EvaluatePackage`, a scene
    /// giving it a package) since it was last stepped: a traveller in range
    /// is put at its target at once rather than late (autotest15).
    pub(crate) prompted: bool,
    /// Away: the real time a traveller not put at once is put at its target
    /// (autotest15: 34 and 39 s after its package started).
    pub(crate) jump_at: Option<f64>,
    /// Away: how to turn it as it loads where its package put it.
    pub(crate) on_arrival: Option<super::schedule::PlacementHeading>,
    /// Away: the real time (`ScriptState::real_time`) and the game time
    /// (hours, `Engine::game_hours_total`) it was last stepped, and the half
    /// hour of game time its packages were last read in. Which clock each
    /// piece follows is on `Processing`.
    pub(crate) stepped_at: f64,
    pub(crate) stepped_hours: f64,
    pub(crate) eval_slot: Option<i64>,
    /// The NPC record its packages come from (taken again away when its
    /// aliases change).
    pub(crate) npc: FormId,
    /// The goal was set by hand (console `travel`): packages leave it alone.
    pub(super) pinned: bool,
    /// Which of a multi-location sandbox package's stops it is at.
    pub(super) stop: StopProgress,
    /// Patrol progress: (route start, index of the point heading for).
    pub(super) patrol: Option<(FormId, usize)>,
    /// The run of its current package, and one a scene's package took over
    /// from, to resume once the scene lets it go.
    pub run: Option<PackageRun>,
    pub(crate) suspended: Option<PackageRun>,
    /// A package whose run completed and gives way (an Activate, UseWeapon or
    /// UseMagic package that ran its course): it is passed over until it stops
    /// applying.
    pub(crate) done_once: Option<FormId>,
    /// Package events its scripts haven't heard yet (`Engine::report_package_runs`).
    pub(crate) run_events: Vec<RunEvent>,
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

/// How an actor is run: its processing tier (`Engine::processing`).
/// CommonLibSSE names four process levels (High, MiddleHigh, MiddleLow, Low)
/// but no rules for them; these tiers follow what `test-mod autotest13` and
/// `autotest14` saw from outside, where the cell, not the distance, made the
/// difference (actors 2,700 away in a cell the player hadn't been in jumped;
/// ones 3,000 away in the cell just left walked on).
///
/// Clocks: game time drives schedules (when packages apply and the half hour
/// they are read again in), runs' start times and catching up after time
/// passed in a menu (`Engine::pass_time`). Real time
/// (`ScriptState::real_time`, which stands still in menus) drives the reading
/// cadence, a jumped traveller's `OnPackageEnd` delay, walking in a buffered
/// cell, bleeding out and wounds healing (`Wounds::health_at`): time passed
/// in a menu counts for these as the real seconds it stands for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Processing {
    /// Loaded: stepped every frame, with its body.
    Full,
    /// Away in a cell the player has just left (`Stepper::buffered`):
    /// stepped in turn, walking on at about loaded speed.
    Buffered { cadence: f32 },
    /// Away anywhere else: stepped in turn, put where its package has it.
    Coarse { cadence: f32 },
}

impl Processing {
    /// Real seconds between readings of its packages (none: every frame's own).
    pub fn cadence(self) -> Option<f32> {
        match self {
            Processing::Full => None,
            Processing::Buffered { cadence } | Processing::Coarse { cadence } => Some(cadence),
        }
    }
}

/// Real seconds between readings of an away actor's packages (autotest13:
/// within about 3 s of a condition changing, buffered or not).
pub(crate) const AWAY_CADENCE: f32 = 3.0;

/// How far a loaded actor had got with its package as it unloaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// Standing (or getting out of furniture).
    Standing,
    /// On its way somewhere: it sets off again from where it stood.
    Walking,
    /// In (or getting into) marker `marker` of `furniture`.
    Seated { furniture: FormId, marker: u8 },
}

/// One package from when an actor takes it up until another takes its place.
/// Unloading neither ends nor restarts it; combat and conversations leave it
/// as it is (their procedures run instead of its); a scene's package
/// suspends it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PackageRun {
    pub id: FormId,
    /// Game time it started at, in hours (`Engine::game_hours_total`).
    pub started_at: f64,
    pub outcome: Outcome,
    /// A scene's package: done, it gives way without `OnPackageChange`
    /// (autotest15).
    pub(crate) scene: bool,
    /// Times it has activated an Activate package's target.
    pub(crate) activations: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Running,
    /// Done (`OnPackageEnd`): it stays the actor's package until another
    /// applies, or gives way at once (`ActorExec::done_once`).
    Completed,
}

/// What a run's change tells the package's scripts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunEvent {
    /// `OnPackageStart` and the begin fragment.
    Start(FormId),
    /// `OnPackageEnd` and the end fragment.
    End(FormId),
    /// `OnPackageChange` and the change fragment: another package took over
    /// from this one, ended or not (autotest13).
    Change(FormId),
}

impl ActorExec {
    /// The package it runs.
    pub fn package(&self) -> Option<FormId> {
        self.run.map(|r| r.id)
    }

    /// Whether the package it runs has completed.
    pub(crate) fn run_completed(&self) -> bool {
        self.run.is_some_and(|r| r.outcome == Outcome::Completed)
    }

    /// Take up package `pick` (an index into `packages`) at game time `now`:
    /// the run it replaces is over, or, `for_scene`, kept to resume once the
    /// scene's package gives way. The same package carries on.
    pub(crate) fn begin_run(&mut self, pick: Option<usize>, now: f64, for_scene: bool) {
        let id = pick.and_then(|i| self.packages.get(i)).map(|p| p.id);
        if self.package() == id {
            return;
        }
        if let Some(old) = self.run.take() {
            // A scene's package that ran its course just gives way.
            if !(old.scene && old.outcome == Outcome::Completed) {
                self.run_events.push(RunEvent::Change(old.id));
            }
            if for_scene && old.outcome == Outcome::Running && self.suspended.is_none() {
                self.suspended = Some(old);
            }
        }
        let resumed = self.suspended.filter(|s| Some(s.id) == id);
        if !for_scene {
            self.suspended = None;
        }
        self.run = id.map(|id| {
            resumed.unwrap_or(PackageRun {
                id,
                started_at: now,
                outcome: Outcome::Running,
                activations: 0,
                scene: for_scene,
            })
        });
        if let Some(id) = id {
            self.run_events.push(RunEvent::Start(id));
        }
    }

    /// The package it runs is done; `gives_way`, it is passed over until it
    /// stops applying, and its packages are evaluated again at once.
    pub(crate) fn complete_run(&mut self, gives_way: bool) {
        if gives_way {
            self.next_eval = 0.0;
        }
        let Some(run) = self.run.as_mut().filter(|r| r.outcome == Outcome::Running) else {
            return;
        };
        run.outcome = Outcome::Completed;
        self.run_events.push(RunEvent::End(run.id));
        if gives_way {
            self.done_once = Some(run.id);
        }
    }

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

    /// Its actor unloaded from `place` at real time `now` and game time
    /// `hours`, `wounds` as hurt as it left, at `stage` of its package when it
    /// unloaded with its cell. One standing or seated is where its package
    /// has it; one walking, or with nothing to take up, has its journey still
    /// to make.
    pub(crate) fn away(
        mut self,
        place: Option<Place>,
        wounds: Option<Wounds>,
        stage: Option<Stage>,
        (now, hours): (f64, f64),
    ) -> Self {
        self.place = place;
        self.wounds = wounds;
        self.stage = stage;
        self.settled = matches!(stage, Some(Stage::Standing | Stage::Seated { .. }));
        self.ends_at = None;
        self.jump_at = None;
        self.prompted = false;
        self.on_arrival = None;
        self.stamp(now, hours);
        self
    }

    /// Stamped as stepped at real time `now` and game time `hours`.
    pub(crate) fn stamp(&mut self, now: f64, hours: f64) {
        self.stepped_at = now;
        self.stepped_hours = hours;
    }

    /// Its actor loads again: it carries on with the package runs it left with
    /// (unloading neither ends nor restarts them). `resuming` where it left, it
    /// takes up its package where it left off too: its goal, a sandbox's stop
    /// and a patrol's point, and a goal set by hand. Fights and searches
    /// aren't taken back: it attaches out of combat, not searching.
    pub(crate) fn take_back(&mut self, away: &ActorExec, resuming: bool) {
        self.run = away.run;
        self.suspended = away.suspended;
        self.done_once = away.done_once;
        if !resuming {
            return;
        }
        if away.pinned {
            self.pinned = true;
            self.goal = away.goal;
            return;
        }
        let same = away
            .current
            .and_then(|i| self.packages.get(i))
            .is_some_and(|p| Some(p.id) == away.package());
        if same {
            self.current = away.current;
            self.goal = away.goal;
            self.stop = away.stop;
            self.patrol = away.patrol;
            // Back from a save, which keeps no goal: worked out again at once.
            if away.goal.is_none() {
                self.next_eval = 0.0;
            }
        }
    }

    /// Where it is to attach in `here`, as it left: it unloaded there with its
    /// cell, and nothing has moved it on since.
    pub(crate) fn resumes_in(&self, here: Option<Place>) -> Option<Stage> {
        self.stage
            .filter(|_| self.place.is_some() && self.place == here)
    }

    /// Only wounds: all that saves without executions (`AEXC`) keep of an
    /// actor away.
    pub(crate) fn wounded(wounds: Wounds) -> Self {
        ActorExec {
            health: wounds.health,
            wounds: Some(wounds),
            ..Default::default()
        }
    }

    /// What saves keep of it (`AEXC`).
    pub(crate) fn saved(&self) -> SavedExec {
        SavedExec {
            npc: Some(self.npc).filter(|n| !n.is_null()),
            place: self.place,
            pos: self.pos,
            heading: self.heading,
            stage: self.stage,
            settled: self.settled,
            prompted: self.prompted,
            ends_at: self.ends_at,
            jump_at: self.jump_at,
            on_arrival: self.on_arrival,
            stepped_at: self.stepped_at,
            stepped_hours: self.stepped_hours,
            eval_slot: self.eval_slot,
            next_eval: self.next_eval,
            current: self.current.map(|i| i as u32),
            stop: (self.stop.index as u32, self.stop.arrived),
            patrol: self.patrol.map(|(start, i)| (start, i as u32)),
            run: self.run,
            suspended: self.suspended,
            done_once: self.done_once,
            health: self.health,
            stamina: self.stamina,
            bleeding: self.bleeding,
        }
    }

    /// An execution back from a save, `wounds` as hurt as the save has it
    /// (`WRLD`). Its packages are taken again as it is next stepped or loads
    /// (its alias generation is none the scripts have); its goal is worked out
    /// again from its package.
    pub(crate) fn from_saved(s: SavedExec, wounds: Option<Wounds>) -> Self {
        ActorExec {
            npc: s.npc.unwrap_or(FormId::NULL),
            place: s.place,
            pos: s.pos,
            heading: s.heading,
            stage: s.stage,
            settled: s.settled,
            prompted: s.prompted,
            ends_at: s.ends_at,
            jump_at: s.jump_at,
            on_arrival: s.on_arrival,
            stepped_at: s.stepped_at,
            stepped_hours: s.stepped_hours,
            eval_slot: s.eval_slot,
            next_eval: s.next_eval,
            current: s.current.map(|i| i as usize),
            stop: StopProgress {
                index: s.stop.0 as usize,
                arrived: s.stop.1,
            },
            patrol: s.patrol.map(|(start, i)| (start, i as usize)),
            run: s.run,
            suspended: s.suspended,
            done_once: s.done_once,
            health: s.health,
            stamina: s.stamina,
            wounds,
            bleeding: s.bleeding,
            alias_gen: 0,
            ..Default::default()
        }
    }
}

/// What saves keep of an actor's execution (`AEXC`): its runs, how far it
/// got with them, where it is and its clocks. Not kept: its package list and
/// goal (taken again from its records), a goal set by hand (console
/// `travel`), fights, searches and greetings (it attaches without them), and
/// package events its scripts hadn't heard yet. Wounds are `WRLD`'s.
#[derive(Debug, Clone, PartialEq)]
pub struct SavedExec {
    pub npc: Option<FormId>,
    pub place: Option<Place>,
    pub pos: Vec3,
    pub heading: f32,
    pub stage: Option<Stage>,
    pub settled: bool,
    pub prompted: bool,
    pub ends_at: Option<f64>,
    pub jump_at: Option<f64>,
    pub on_arrival: Option<super::schedule::PlacementHeading>,
    pub stepped_at: f64,
    pub stepped_hours: f64,
    pub eval_slot: Option<i64>,
    pub next_eval: f32,
    pub current: Option<u32>,
    /// A multi-location sandbox's stop, and when it arrived there.
    pub stop: (u32, Option<f64>),
    pub patrol: Option<(FormId, u32)>,
    pub run: Option<PackageRun>,
    pub suspended: Option<PackageRun>,
    pub done_once: Option<FormId>,
    pub health: f32,
    pub stamina: f32,
    pub bleeding: Option<f32>,
}

impl Engine {
    /// The tier actor `r` runs at: loaded, in a cell the player just left, or
    /// elsewhere (none for actors not running: dead, disabled, or never
    /// touched by the world and placed by their schedules).
    pub fn processing(&self, r: FormId) -> Option<Processing> {
        if let Some(a) = self.actor_ref(r) {
            return (!a.dead).then_some(Processing::Full);
        }
        let place = self.away.get(&r)?.place?;
        if self.is_disabled(r) {
            return None;
        }
        Some(if self.away_steps.buffered.contains(&place) {
            Processing::Buffered {
                cadence: AWAY_CADENCE,
            }
        } else {
            Processing::Coarse {
                cadence: AWAY_CADENCE,
            }
        })
    }

    /// Whether loaded actor `a`, running Travel or Escort package `p` toward
    /// `goal`, has arrived: it is in the place the package names (not on the
    /// way out of another), standing near its location.
    pub(crate) fn arrived(&self, a: &ActorRuntime, p: &Package, goal: Goal) -> bool {
        let here = self.current_place_of(a.ref_id).map(|x| x.0);
        let there = self.package_place(a.ref_id, p);
        let in_place = match there {
            Some((place, _)) => here.is_some_and(|h| super::schedule::same_space(h, place)),
            None => true,
        };
        let centre = there
            .map(|t| t.1)
            .filter(|c| !c.is_nan())
            .unwrap_or(goal.centre);
        a.exiting.is_none()
            && in_place
            && !a.is_walking()
            && a.exec.pos.truncate().distance(centre.truncate()) <= goal.radius + ARRIVED_SLACK
    }

    /// Loaded actors' Travel and Escort runs complete once they arrive (out of
    /// combat). They stay their actors' packages.
    pub(crate) fn complete_arrivals(&mut self) {
        let mut done = Vec::new();
        for a in self.cells.values().flat_map(|rt| &rt.actors) {
            if a.dead || a.exec.pinned || a.exec.combat.is_some() {
                continue;
            }
            let (Some(run), Some(goal)) = (a.exec.run, a.exec.goal) else {
                continue;
            };
            let Some(p) = a
                .exec
                .current
                .and_then(|i| a.exec.packages.get(i))
                .filter(|p| p.id == run.id)
            else {
                continue;
            };
            if run.outcome == Outcome::Running
                && matches!(p.behaviour, Behaviour::Travel | Behaviour::Escort)
                && self.arrived(a, p, goal)
            {
                done.push(a.ref_id);
            }
        }
        for r in done {
            if let Some(a) = self.actor_mut(r) {
                a.exec.complete_run(false);
            }
        }
    }

    /// Tell actors' scripts what their package runs did: `OnPackageStart`,
    /// `OnPackageEnd` and `OnPackageChange`, each after the package's own
    /// fragment for it.
    pub(crate) fn report_package_runs(&mut self) {
        let mut events = Vec::new();
        for a in self.cells.values_mut().flat_map(|rt| rt.actors.iter_mut()) {
            events.extend(a.exec.run_events.drain(..).map(|e| (a.ref_id, e)));
        }
        for (&r, x) in &mut self.away {
            events.extend(x.run_events.drain(..).map(|e| (r, e)));
        }
        for (actor, e) in events {
            let (package, event) = match e {
                RunEvent::Start(p) => (p, "OnPackageStart"),
                RunEvent::End(p) => (p, "OnPackageEnd"),
                RunEvent::Change(p) => (p, "OnPackageChange"),
            };
            log::debug!("{actor} {event} {package}");
            self.run_package_fragment(package, actor, e);
            let arg = self.object_value(package);
            self.send_script_event(actor, event, vec![arg]);
        }
    }

    /// Run package `package`'s fragment for `e`, if it has one, for `actor`.
    fn run_package_fragment(&mut self, package: FormId, actor: FormId, e: RunEvent) {
        let Some(vmad) = self
            .lo
            .get(package)
            .and_then(|r| crate::script::vmad::parse(&r))
        else {
            return;
        };
        let Some(frag) = &vmad.package else { return };
        let func = match e {
            RunEvent::Start(_) => &frag.begin,
            RunEvent::End(_) => &frag.end,
            RunEvent::Change(_) => &frag.change,
        };
        let Some(func) = func.clone() else { return };
        let obj = papyrus::ObjectId::Form(package.0);
        let actor = self.object_value(actor);
        let mut vm = std::mem::take(&mut self.vm);
        {
            let mut host = crate::script::EngineHost { engine: self };
            for s in &vmad.scripts {
                let props: Vec<(String, papyrus::Value)> = s
                    .properties
                    .iter()
                    .map(|(n, pv)| {
                        (
                            n.clone(),
                            crate::script::vmad::to_value(pv, &|f| host.engine.native_class(f)),
                        )
                    })
                    .collect();
                vm.attach(&mut host, obj, &s.name, &props);
            }
            log::debug!("package {package} fragment {func}");
            vm.call_method(&mut host, obj, &frag.script, &func, vec![actor]);
        }
        self.vm = vm;
    }

    /// The package an actor runs (`GetCurrentPackage`): its run's, loaded or
    /// away; for one the world hasn't touched, the one its schedule gives it.
    pub(crate) fn running_package(&self, r: FormId) -> Option<FormId> {
        if let Some(a) = self.actor_ref(r) {
            return a.exec.package();
        }
        if let Some(x) = self.away.get(&r).filter(|x| x.place.is_some()) {
            return x.package();
        }
        match self.whereabouts.package.get(&r) {
            Some(&p) => p,
            None => self.away.get(&r).and_then(|x| x.package()),
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    const HOME: FormId = FormId(0x810);
    const FAR: FormId = FormId(0x811);
    const SCENE: FormId = FormId(0x812);

    fn exec() -> ActorExec {
        let packages = [SCENE, FAR, HOME]
            .map(|id| Package::bare(id, Behaviour::Travel))
            .to_vec();
        ActorExec::new(packages, Vec3::ZERO, 0.0, 0.0)
    }

    #[test]
    fn a_change_of_package_tells_the_old_one_then_starts_the_new() {
        let mut x = exec();
        x.begin_run(Some(2), 1.0, false);
        assert_eq!(x.run_events, [RunEvent::Start(HOME)]);
        x.run_events.clear();
        // The same package carries on.
        x.begin_run(Some(2), 2.0, false);
        assert!(x.run_events.is_empty());
        assert_eq!(x.run.unwrap().started_at, 1.0);
        x.begin_run(Some(1), 3.0, false);
        assert_eq!(x.run_events, [RunEvent::Change(HOME), RunEvent::Start(FAR)]);
        assert_eq!(x.package(), Some(FAR));
        assert_eq!(x.run.unwrap().started_at, 3.0);
        x.run_events.clear();
        x.begin_run(None, 4.0, false);
        assert_eq!(x.run_events, [RunEvent::Change(FAR)]);
        assert_eq!(x.run, None);
    }

    #[test]
    fn a_completed_run_ends_once_and_another_package_still_tells_it() {
        let mut x = exec();
        x.begin_run(Some(1), 0.0, false);
        x.run_events.clear();
        x.complete_run(false);
        x.complete_run(false);
        assert_eq!(x.run_events, [RunEvent::End(FAR)]);
        assert!(x.run_completed());
        // It stays the actor's package, and isn't passed over.
        assert_eq!(x.package(), Some(FAR));
        assert_eq!(x.done_once, None);
        x.run_events.clear();
        x.begin_run(Some(2), 1.0, false);
        assert_eq!(x.run_events, [RunEvent::Change(FAR), RunEvent::Start(HOME)]);
    }

    #[test]
    fn a_run_that_gives_way_is_passed_over_and_evaluated_again_at_once() {
        let mut x = exec();
        x.begin_run(Some(1), 0.0, false);
        x.next_eval = 5.0;
        x.complete_run(true);
        assert_eq!(x.done_once, Some(FAR));
        assert_eq!(x.next_eval, 0.0);
    }

    #[test]
    fn a_scene_suspends_the_run_and_it_resumes_after() {
        let mut x = exec();
        x.begin_run(Some(2), 1.0, false);
        x.run.as_mut().unwrap().activations = 2;
        x.begin_run(Some(0), 3.0, true);
        assert_eq!(x.package(), Some(SCENE));
        assert_eq!(x.suspended.map(|r| r.id), Some(HOME));
        x.run_events.clear();
        x.begin_run(Some(2), 5.0, false);
        // Its scripts hear of it as of any change; its progress is kept.
        assert_eq!(
            x.run_events,
            [RunEvent::Change(SCENE), RunEvent::Start(HOME)]
        );
        let run = x.run.unwrap();
        assert_eq!((run.started_at, run.activations), (1.0, 2));
        assert_eq!(x.suspended, None);
    }

    #[test]
    fn a_scene_package_that_ran_its_course_gives_way_without_a_change() {
        let mut x = exec();
        x.begin_run(Some(0), 3.0, true);
        x.complete_run(false);
        x.run_events.clear();
        // autotest15: OnPackageEnd, then the next one's OnPackageStart.
        x.begin_run(Some(2), 5.0, false);
        assert_eq!(x.run_events, [RunEvent::Start(HOME)]);
    }

    #[test]
    fn a_loading_actor_carries_on_with_its_run() {
        let mut away = exec();
        away.begin_run(Some(1), 1.0, false);
        away.complete_run(true);
        let mut x = exec();
        x.take_back(&away, false);
        assert_eq!(x.run, away.run);
        assert_eq!(x.done_once, Some(FAR));
        // Taking up the same package again tells its scripts nothing.
        x.begin_run(Some(1), 2.0, false);
        assert!(x.run_events.is_empty());
    }

    #[test]
    fn a_scene_giving_way_to_another_package_drops_the_suspended_run() {
        let mut x = exec();
        x.begin_run(Some(2), 1.0, false);
        x.begin_run(Some(0), 3.0, true);
        x.begin_run(Some(1), 5.0, false);
        assert_eq!(x.suspended, None);
        x.begin_run(Some(2), 6.0, false);
        assert_eq!(x.run.unwrap().started_at, 6.0);
    }

    /// Away on `HOME`'s run, part way through it, at `stage` in `place`.
    fn left_at(stage: Option<Stage>, place: Place) -> ActorExec {
        let mut away = exec();
        away.begin_run(Some(2), 1.0, false);
        away.current = Some(2);
        away.goal = Some(Goal::travel(Vec3::X * 500.0));
        away.stop.index = 1;
        away.patrol = Some((FAR, 3));
        away.combat = Some(combat::Combat::new(FAR));
        away.away(Some(place), None, stage, (0.0, 0.0))
    }

    #[test]
    fn an_actor_resuming_where_it_left_takes_up_its_package_where_it_left_off() {
        let here = Place::Interior(FormId(0x830));
        let away = left_at(Some(Stage::Walking), here);
        assert_eq!(away.resumes_in(Some(here)), Some(Stage::Walking));
        assert_eq!(away.resumes_in(Some(Place::Interior(FAR))), None);
        assert_eq!(away.resumes_in(None), None);
        let mut x = exec();
        x.take_back(&away, true);
        assert_eq!(x.current, Some(2));
        assert_eq!(x.goal.map(|g| g.centre), Some(Vec3::X * 500.0));
        assert_eq!(x.stop.index, 1);
        assert_eq!(x.patrol, Some((FAR, 3)));
        // Out of combat.
        assert!(x.combat.is_none());
        // Not where it left: only its runs.
        let mut x = exec();
        x.take_back(&away, false);
        assert_eq!(x.package(), Some(HOME));
        assert_eq!((x.current, x.stop.index, x.patrol), (None, 0, None));
        assert!(x.goal.is_none());
        // Nothing to take up where it was gone (its schedule moved it on).
        let gone = left_at(None, here);
        assert_eq!(gone.resumes_in(Some(here)), None);
    }

    #[test]
    fn a_resuming_actor_whose_packages_changed_starts_its_package_afresh() {
        let here = Place::Interior(FormId(0x830));
        let away = left_at(Some(Stage::Standing), here);
        // Its package list isn't what it was: index 2 is another package.
        let mut x = ActorExec::new(
            vec![Package::bare(FAR, Behaviour::Travel); 3],
            Vec3::ZERO,
            0.0,
            0.0,
        );
        x.take_back(&away, true);
        assert_eq!(x.package(), Some(HOME));
        assert_eq!((x.current, x.stop.index, x.patrol), (None, 0, None));
        // A goal set by hand is kept, whatever its packages.
        let mut pinned = away.clone();
        pinned.pinned = true;
        x.take_back(&pinned, true);
        assert!(x.pinned);
        assert_eq!(x.goal.map(|g| g.centre), Some(Vec3::X * 500.0));
    }

    #[test]
    fn unloading_captures_the_stage_the_actor_reached() {
        use crate::ai::State;
        let mut a = ActorRuntime::new(
            ACTOR,
            BASE,
            std::sync::Arc::new(crate::world::skeleton::Skeleton::default()),
            Mat4::IDENTITY,
            Vec::new(),
            0.0,
        );
        // Not yet started on anything.
        assert_eq!(a.stage(), None);
        a.fresh = false;
        assert_eq!(a.stage(), Some(Stage::Standing));
        a.state = State::Walk {
            path: Vec::new(),
            next: 0,
            budget: 0.0,
            to_seat: true,
        };
        assert_eq!(a.stage(), Some(Stage::Walking));
        a.state = State::Approach(1.0);
        assert_eq!(a.stage(), Some(Stage::Walking));
        let chair = FormId(0x840);
        let seated = Stage::Seated {
            furniture: chair,
            marker: 2,
        };
        for s in [State::Enter(1.0), State::Use(5.0)] {
            assert_eq!(crate::ai::stage_of(&s, Some((chair, 2))), seated);
        }
        // Getting out, or playing an idle where it stands.
        let out = crate::ai::stage_of(&State::Exit(1.0), Some((chair, 2)));
        assert_eq!(out, Stage::Standing);
        let idle = crate::ai::stage_of(&State::Use(1.0), Some((FormId::NULL, 0)));
        assert_eq!(idle, Stage::Standing);
    }

    #[test]
    fn the_seat_it_left_is_tried_first() {
        let mut options = vec![(FAR, 0, ()), (HOME, 1, ()), (HOME, 0, ())];
        crate::ai::put_first(&mut options, Some((HOME, 0)));
        assert_eq!(options, [(HOME, 0, ()), (FAR, 0, ()), (HOME, 1, ())]);
        // Taken or gone: as they were.
        crate::ai::put_first(&mut options, Some((SCENE, 0)));
        crate::ai::put_first(&mut options, None);
        assert_eq!(options, [(HOME, 0, ()), (FAR, 0, ()), (HOME, 1, ())]);
    }
    use esp::writer::{Node, RecordBuilder, write_plugin};
    use glam::Mat4;

    pub(crate) const BASE: FormId = FormId(0x800);
    pub(crate) const ACTOR: FormId = FormId(0x801);
    pub(crate) const PACKAGE: FormId = FormId(0x802);
    pub(crate) const PACKAGE2: FormId = FormId(0x823);
    pub(crate) const CELL: FormId = FormId(0x820);
    /// A marker in `CELL`, 3,000 east of its origin.
    pub(crate) const MARKER: FormId = FormId(0x821);
    const MARKER_BASE: FormId = FormId(0x822);

    pub(crate) struct Fixture(std::path::PathBuf);

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    pub(crate) fn fixture() -> (Fixture, Engine) {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let f =
            Fixture(std::env::temp_dir().join(format!("vrm-exec-{}-{stamp}", std::process::id())));
        std::fs::create_dir(&f.0).unwrap();
        let mut marker = [0; 24];
        marker[0..4].copy_from_slice(&3000.0f32.to_le_bytes());
        let records = [
            Node::top(b"STAT", vec![RecordBuilder::new(b"STAT", MARKER_BASE)]),
            Node::top(b"NPC_", vec![RecordBuilder::new(b"NPC_", BASE)]),
            Node::top(
                b"PACK",
                vec![
                    RecordBuilder::new(b"PACK", PACKAGE),
                    RecordBuilder::new(b"PACK", PACKAGE2),
                ],
            ),
            Node::Group {
                label: *b"CELL",
                group_type: 0,
                children: vec![
                    Node::Record(RecordBuilder::new(b"CELL", CELL).field(b"DATA", &[1, 0])),
                    Node::Group {
                        label: CELL.0.to_le_bytes(),
                        group_type: esp::group_type::CELL_CHILDREN,
                        children: vec![Node::Group {
                            label: CELL.0.to_le_bytes(),
                            group_type: esp::group_type::CELL_TEMPORARY,
                            children: vec![
                                Node::Record(
                                    RecordBuilder::new(b"ACHR", ACTOR).form(b"NAME", BASE),
                                ),
                                Node::Record(
                                    RecordBuilder::new(b"REFR", MARKER)
                                        .form(b"NAME", MARKER_BASE)
                                        .field(b"DATA", &marker),
                                ),
                            ],
                        }],
                    },
                ],
            },
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
    pub(crate) fn loaded_actor(e: &mut Engine) {
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

    /// Save `e` and load the save back into it, `without` its sections.
    fn reload(e: &mut Engine, without: &[&[u8; 4]]) {
        use crate::save::state::Snapshot;
        let mut file = Snapshot::capture(e).write();
        file.sections.retain(|(tag, _)| !without.contains(&tag));
        // What happens meanwhile is undone by the load.
        e.unload_all();
        e.away.get_mut(&ACTOR).unwrap().pos = Vec3::ZERO;
        Snapshot::read(&file).unwrap().apply(e).unwrap();
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn a_save_keeps_a_loaded_actors_execution_to_attach_as_it_was() {
        let (_f, mut e) = fixture();
        travelling(&mut e);
        e.actor_mut(ACTOR).unwrap().fresh = false;
        e.actor_mut(ACTOR).unwrap().exec.npc = BASE;
        reload(&mut e, &[]);
        let x = &e.away[&ACTOR];
        let here = Place::Interior(CELL);
        assert_eq!(x.place, Some(here));
        assert_eq!((x.pos, x.heading), (Vec3::new(10.0, 20.0, 30.0), 1.25));
        assert_eq!(x.resumes_in(Some(here)), Some(Stage::Standing));
        let run = x.run.expect("its run");
        assert_eq!((run.id, run.started_at), (PACKAGE, 2.0));
        assert_eq!(x.current, Some(0));
        assert_eq!(x.npc, BASE);
        // Its packages are taken again; its goal is worked out again.
        assert!(x.packages.is_empty() && x.goal.is_none());
        assert_ne!(x.alias_gen, e.scripts.alias_gen);
        // As hurt as it was (`WRLD`).
        assert_eq!(x.wounds.map(|w| (w.health, w.max)), Some((30.0, 80.0)));
        assert_eq!(
            e.whereabouts.of[&ACTOR],
            (here, Vec3::new(10.0, 20.0, 30.0))
        );
        assert_eq!(e.running_package(ACTOR), Some(PACKAGE));
        // Taking it back as it loads, it carries on with the same run.
        let mut a = exec();
        a.packages = vec![Package::bare(PACKAGE, Behaviour::Travel)];
        a.take_back(x, true);
        assert_eq!((a.current, a.run), (Some(0), x.run));
        assert_eq!(a.next_eval, 0.0);
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn a_save_keeps_an_away_actors_journey() {
        let (_f, mut e) = fixture();
        travelling(&mut e);
        e.unload_all();
        let x = e.away.get_mut(&ACTOR).unwrap();
        x.stage = None;
        x.settled = false;
        x.jump_at = Some(40.0);
        x.bleeding = Some(3.0);
        x.eval_slot = Some(5);
        let before = x.saved();
        reload(&mut e, &[]);
        assert_eq!(e.away[&ACTOR].saved(), before);
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn a_save_without_executions_falls_back_to_schedules() {
        let (_f, mut e) = fixture();
        travelling(&mut e);
        e.actor_mut(ACTOR).unwrap().fresh = false;
        reload(&mut e, &[b"AEXC"]);
        let x = &e.away[&ACTOR];
        assert_eq!((x.place, x.run, x.stage), (None, None, None));
        assert_eq!(x.wounds.map(|w| w.health), Some(30.0));
        // Well actors loaded as it was saved leave nothing.
        let (_f, mut e) = fixture();
        travelling(&mut e);
        e.actor_mut(ACTOR).unwrap().exec.health = 80.0;
        let mut file = crate::save::state::Snapshot::capture(&e).write();
        file.sections.retain(|(tag, _)| tag != b"AEXC");
        e.unload_all();
        crate::save::state::Snapshot::read(&file)
            .unwrap()
            .apply(&mut e)
            .unwrap();
        assert!(!e.away.contains_key(&ACTOR));
    }

    /// The loaded actor running a Travel package to where it stands.
    fn travelling(e: &mut Engine) {
        loaded_actor(e);
        let a = e.actor_mut(ACTOR).unwrap();
        a.exec.packages = vec![Package::bare(PACKAGE, Behaviour::Travel)];
        a.exec.current = Some(0);
        a.exec.goal = Some(Goal::travel(a.exec.pos));
        a.exec.begin_run(Some(0), 2.0, false);
    }

    fn heard(e: &Engine) -> Vec<String> {
        e.scripts
            .pending_events
            .iter()
            .filter(|(o, _, args)| {
                *o == papyrus::ObjectId::Form(ACTOR.0)
                    && matches!(&args[..], [papyrus::Value::Object(papyrus::ObjectId::Form(p), _)]
                        if *p == PACKAGE.0)
            })
            .map(|(_, ev, _)| ev.clone())
            .collect()
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn an_arrival_ends_the_travel_run_and_its_scripts_hear_of_it() {
        let (_f, mut e) = fixture();
        travelling(&mut e);
        e.complete_arrivals();
        e.report_package_runs();
        assert_eq!(heard(&e), ["OnPackageStart", "OnPackageEnd"]);
        // Done once: it stays the actor's package.
        assert!(e.actor_ref(ACTOR).unwrap().exec.run_completed());
        assert_eq!(e.running_package(ACTOR), Some(PACKAGE));
        e.scripts.pending_events.clear();
        e.complete_arrivals();
        e.report_package_runs();
        assert!(heard(&e).is_empty());
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn a_traveller_short_of_its_goal_runs_on() {
        let (_f, mut e) = fixture();
        travelling(&mut e);
        let a = e.actor_mut(ACTOR).unwrap();
        a.exec.goal = Some(Goal::travel(a.exec.pos + Vec3::X * 1000.0));
        e.complete_arrivals();
        assert_eq!(
            e.actor_ref(ACTOR).unwrap().exec.run.unwrap().outcome,
            Outcome::Running
        );
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn unloading_neither_ends_nor_restarts_the_run() {
        let (_f, mut e) = fixture();
        travelling(&mut e);
        e.report_package_runs();
        e.unload_all();
        let run = e.away[&ACTOR].run.expect("still running");
        assert_eq!((run.id, run.started_at), (PACKAGE, 2.0));
        assert!(e.away[&ACTOR].run_events.is_empty());
        // Away, it runs the package it left with until its own packages say
        // otherwise, whatever a schedule would give it.
        assert_eq!(e.running_package(ACTOR), Some(PACKAGE));
        e.whereabouts.package.insert(ACTOR, None);
        assert_eq!(e.running_package(ACTOR), Some(PACKAGE));
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn an_actor_unloading_with_its_cell_stays_where_it_left_while_nothing_moves_it_on() {
        let (_f, mut e) = fixture();
        loaded_actor(&mut e);
        e.actor_mut(ACTOR).unwrap().fresh = false;
        e.unload_all();
        let here = Place::Interior(CELL);
        let left = (here, Vec3::new(10.0, 20.0, 30.0));
        assert_eq!(e.away[&ACTOR].stage, Some(Stage::Standing));
        assert_eq!(e.whereabouts.of.get(&ACTOR), Some(&left));
        assert_eq!(e.whereabouts.by_place[&here], [ACTOR]);
        // It has no package to take it anywhere: it stays.
        e.step_away(ACTOR);
        e.refresh_whereabouts();
        assert_eq!(e.whereabouts.of.get(&ACTOR), Some(&left));
        assert_eq!(e.away[&ACTOR].resumes_in(Some(here)), Some(Stage::Standing));
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn a_script_moving_an_actor_away_puts_it_where_it_was_moved() {
        let (_f, mut e) = fixture();
        loaded_actor(&mut e);
        e.actor_mut(ACTOR).unwrap().fresh = false;
        e.unload_all();
        // Back to where the editor placed it (the cell's origin).
        assert!(e.move_to_editor_location(ACTOR));
        assert_eq!(e.away[&ACTOR].stage, None);
        let there = (Place::Interior(CELL), Vec3::ZERO);
        assert_eq!(e.whereabouts.of.get(&ACTOR), Some(&there));
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn an_actor_on_its_way_out_unloads_with_nothing_to_resume() {
        let (_f, mut e) = fixture();
        loaded_actor(&mut e);
        let a = e.actor_mut(ACTOR).unwrap();
        a.fresh = false;
        a.exiting = Some(FormId(0x850));
        e.unload_all();
        assert_eq!(e.away[&ACTOR].stage, None);
    }
}
