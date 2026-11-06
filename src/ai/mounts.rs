//! Vanilla horse associations and package-controlled riding.
use super::*;

pub(crate) const APPROACH: u8 = 0;
pub(crate) const ENTER: u8 = 1;
pub(crate) const MOUNTED: u8 = 2;
pub(crate) const EXIT: u8 = 3;
pub(crate) const DISMOUNTED: u8 = 4;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Ride {
    pub horse: FormId,
    pub package: FormId,
    pub phase: u8,
    pub elapsed: f32,
    pub walk: f32,
    pub run: f32,
}
impl Ride {
    pub(crate) fn mounted(self) -> bool {
        matches!(self.phase, ENTER | MOUNTED | EXIT)
    }
    pub(crate) fn valid(self, actor: FormId) -> bool {
        !actor.is_null()
            && actor != PLAYER_REF
            && !self.horse.is_null()
            && actor != self.horse
            && !self.package.is_null()
            && self.phase <= DISMOUNTED
            && self.elapsed.is_finite()
            && (0. ..=3600.).contains(&self.elapsed)
            && [self.walk, self.run]
                .into_iter()
                .all(|v| v.is_finite() && (1. ..=10000.).contains(&v))
    }
}

fn horse_input(n: &package::Node) -> bool {
    let i = match n.procedure.as_str() {
        "Travel" => 1,
        "Patrol" | "Follow" => 4,
        "Escort" => 6,
        "Wander" => 2,
        _ => return false,
    };
    matches!(n.values.get(i), Some(package::Input::Bool(true)))
}

impl Engine {
    pub(crate) fn is_on_mount(&self, actor: FormId) -> bool {
        self.rides.get(&actor).is_some_and(|r| r.mounted())
    }

    fn riding_package(&self, actor: FormId) -> Option<FormId> {
        let x = self.execution(actor)?;
        if x.pinned
            || x.combat.is_some()
            || self.is_dead(actor)
            || self.is_disabled(actor)
            || !self.ai_enabled
            || self.ai_off.contains(&actor)
        {
            return None;
        }
        let p = x.current.and_then(|i| x.packages.get(i))?;
        let c = x.run.as_ref()?.cursor.as_ref()?;
        let (_, n) = c.moving(p.tree.as_ref()?)?;
        horse_input(n).then_some(p.id)
    }

    pub(super) fn horse_for(&self, actor: FormId) -> Option<FormId> {
        let (place, pos) = self.current_place_of(actor)?;
        let available = |horse: FormId| {
            horse != actor
                && !self.is_disabled(horse)
                && !self.is_dead(horse)
                && self.has_keyword(horse, FormId(0x26110))
                && self
                    .current_place_of(horse)
                    .is_some_and(|(p, _)| schedule::same_space(p, place))
                && !self
                    .rides
                    .iter()
                    .any(|(&r, m)| r != actor && m.phase != DISMOUNTED && m.horse == horse)
        };
        if let Some(horse) = self
            .lo
            .get(actor)
            .and_then(|r| {
                r.get(b"XHOR")
                    .filter(|d| d.len() >= 4)
                    .map(|d| r.fid(FormId(u32::from_le_bytes(d[..4].try_into().unwrap()))))
            })
            .filter(|&h| available(h))
        {
            return Some(horse);
        }
        if let Some(horse) = self.linked_ref(actor, None).filter(|&h| available(h)) {
            return Some(horse);
        }
        let base = self.base_of(actor)?;
        self.lo
            .ids_of_type(b"ACHR")
            .iter()
            .copied()
            .chain(
                self.created_refs
                    .refs
                    .iter()
                    .filter_map(|(&id, r)| r.actor.then_some(id)),
            )
            .filter(|&h| available(h) && self.ref_owner(h) == Some(base))
            .filter_map(|h| Some((h, self.ref_position(h)?.distance_squared(pos))))
            .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)))
            .map(|v| v.0)
    }

    pub(crate) fn dismount(&mut self, actor: FormId) -> bool {
        if let Some(ride) = self.rides.get_mut(&actor)
            && ride.phase < EXIT
        {
            ride.phase = if ride.phase == APPROACH {
                DISMOUNTED
            } else {
                EXIT
            };
            ride.elapsed = 0.;
            self.play_animation_event(actor, "HorseExit");
            return true;
        }
        false
    }

    fn new_ride(&self, horse: FormId, package: FormId, phase: u8) -> Ride {
        let types = crate::world::movement::movement_types(&self.lo);
        let speed = types.get("horsedefault");
        Ride {
            horse,
            package,
            phase,
            elapsed: 0.,
            walk: self
                .actor_ref(horse)
                .map(|a| a.walk_speed())
                .or(speed.map(|m| m.walk))
                .unwrap_or(125.),
            run: self
                .actor_ref(horse)
                .map(|a| a.run_speed())
                .or(speed.map(|m| m.run))
                .unwrap_or(450.),
        }
    }

    pub(crate) fn clear_mount_bodies(&mut self) {
        for a in self.cells.values_mut().flat_map(|rt| &mut rt.actors) {
            if a.mount_speeds.take().is_some()
                && let Some(c) = a.capsule
            {
                self.physics.set_enabled(&[c], true);
            }
            a.mount_driver = None;
            a.mount_capsule = None;
            a.attachment = None;
        }
    }

    pub(super) fn prepare_mounts(&mut self, dt: f32) {
        let previous: std::collections::HashSet<_> = self
            .cells
            .values()
            .flat_map(|rt| &rt.actors)
            .filter(|a| a.mount_speeds.is_some())
            .map(|a| a.ref_id)
            .collect();
        self.clear_mount_bodies();
        let mut actors: Vec<_> = self
            .actor_cells
            .keys()
            .copied()
            .filter(|&r| r != PLAYER_REF)
            .collect();
        actors.sort();
        for actor in actors {
            let package = self.riding_package(actor);
            if self.is_dead(actor) || self.is_disabled(actor) {
                self.rides.remove(&actor);
                continue;
            }
            if let Some(ride) = self.rides.get(&actor).copied() {
                if self.is_dead(ride.horse) || self.is_disabled(ride.horse) {
                    self.rides.remove(&actor);
                    self.play_animation_event(actor, "HorseExitOut");
                    continue;
                }
                if ride.phase == DISMOUNTED {
                    if self.execution(actor).and_then(|x| x.package()) == Some(ride.package) {
                        continue;
                    }
                    self.rides.remove(&actor);
                } else if package != Some(ride.package) {
                    self.dismount(actor);
                }
            }
            if !self.rides.contains_key(&actor)
                && let Some(package) = package
                && let Some(horse) = self.horse_for(actor)
            {
                if self.actor_ref(horse).is_none()
                    && self.run_away(horse)
                    && let Some((place, pos)) = self.current_place_of(horse)
                {
                    self.put_away(horse, place, pos);
                }
                if self.actor_ref(horse).is_some() {
                    let ride = self.new_ride(horse, package, APPROACH);
                    self.rides.insert(actor, ride);
                }
            }
            let Some(mut ride) = self
                .rides
                .get(&actor)
                .copied()
                .filter(|r| r.phase != DISMOUNTED)
            else {
                continue;
            };
            let Some((horse_pos, capsule)) =
                self.actor_ref(ride.horse).map(|h| (h.exec.pos, h.capsule))
            else {
                continue;
            };
            if !self
                .current_place_of(actor)
                .zip(self.current_place_of(ride.horse))
                .is_some_and(|((a, _), (b, _))| schedule::same_space(a, b))
            {
                self.dismount(actor);
                continue;
            }
            ride.elapsed = (ride.elapsed + dt.max(0.)).min(3600.);
            if ride.phase == APPROACH {
                if self
                    .actor_ref(actor)
                    .is_some_and(|a| a.exec.pos.distance(horse_pos) <= 128.)
                {
                    ride.phase = ENTER;
                    ride.elapsed = 0.;
                    self.play_animation_event(actor, "HorseEnter");
                    self.play_animation_event(ride.horse, "IdleStart");
                } else {
                    let key = self.actor_cells[&actor];
                    if let Some(a) = self
                        .cells
                        .get_mut(&key)
                        .and_then(|rt| rt.actors.iter_mut().find(|a| a.ref_id == actor))
                    {
                        if a.exec.goal.is_none_or(|g| g.target != Some(ride.horse)) {
                            a.interrupt(&mut self.furniture);
                        }
                        let mut goal = Goal::travel(horse_pos);
                        goal.behaviour = Behaviour::Follow;
                        goal.follow_radius = (100., 128.);
                        goal.target = Some(ride.horse);
                        goal.radius = 100.;
                        a.exec.goal = Some(goal);
                    }
                }
            }
            if ride.phase == ENTER && ride.elapsed >= 2.5 {
                ride.phase = MOUNTED;
                ride.elapsed = 0.;
                self.play_animation_event(actor, "HorseEnterInstant");
                if let Some(a) = self.actor_mut(actor) {
                    a.state = State::Idle(0.);
                    a.exec.next_eval = 0.;
                }
            }
            if ride.phase == EXIT && ride.elapsed >= 2.5 {
                ride.phase = DISMOUNTED;
                ride.elapsed = 0.;
                self.play_animation_event(actor, "HorseExitOut");
                let heading = self.actor_ref(ride.horse).map_or(0., |h| h.exec.heading);
                let pos = self
                    .nav
                    .floor_below(horse_pos + Vec3::new(heading.cos(), -heading.sin(), 0.) * 100.);
                if let Some(a) = self.actor_mut(actor) {
                    a.exec.pos = pos;
                    a.exec.next_eval = 0.;
                    a.state = State::Idle(0.);
                }
            }
            self.rides.insert(actor, ride);
            if ride.mounted() {
                if !previous.contains(&actor) && ride.phase == MOUNTED {
                    self.play_animation_event(actor, "HorseEnterInstant");
                }
                let (velocity, speed) = self
                    .actor_ref(actor)
                    .map_or((Vec3::ZERO, 0.), |a| (a.velocity, a.speed));
                if let Some(h) = self.actor_mut(ride.horse) {
                    h.mount_driver = Some(actor);
                    h.velocity = velocity;
                    h.speed = speed;
                }
                if let Some(a) = self.actor_mut(actor) {
                    a.mount_speeds = Some((ride.walk, ride.run));
                    a.mount_capsule = capsule;
                    if ride.phase != MOUNTED {
                        a.halt(3600.);
                        a.exec.pos = horse_pos;
                    }
                }
                if let Some(c) = self.actor_ref(actor).and_then(|a| a.capsule) {
                    self.physics.set_enabled(&[c], false);
                }
            }
        }
    }

    pub(super) fn mount_driver(&self, horse: FormId) -> Option<FormId> {
        self.rides.iter().find_map(|(&actor, r)| {
            (r.horse == horse && r.mounted() && !self.is_dead(actor) && !self.is_disabled(actor))
                .then_some(actor)
        })
    }

    pub(super) fn prepare_away_mount(&mut self, actor: FormId) {
        let package = self.riding_package(actor);
        if let Some(ride) = self.rides.get(&actor).copied() {
            if self.is_dead(actor)
                || self.is_disabled(actor)
                || self.is_dead(ride.horse)
                || self.is_disabled(ride.horse)
            {
                self.rides.remove(&actor);
                return;
            }
            if package != Some(ride.package) {
                self.rides.remove(&actor);
            } else {
                if ride.phase != DISMOUNTED {
                    self.rides.get_mut(&actor).unwrap().phase = if ride.phase == EXIT {
                        DISMOUNTED
                    } else {
                        MOUNTED
                    };
                }
                return;
            }
        }
        if let Some(package) = package
            && let Some(horse) = self.horse_for(actor)
            && self.actor_ref(horse).is_none()
            && self.run_away(horse)
        {
            let ride = self.new_ride(horse, package, MOUNTED);
            self.rides.insert(actor, ride);
        }
    }

    pub(super) fn away_mount_speed(&self, actor: FormId) -> Option<f32> {
        let ride = self.rides.get(&actor).filter(|r| r.mounted())?;
        let x = self.execution(actor)?;
        let p = x.current.and_then(|i| x.packages.get(i))?;
        Some(match p.gait {
            package::Gait::Run | package::Gait::Jog => ride.run,
            _ => ride.walk,
        })
    }

    pub(super) fn sync_away_mount(&mut self, actor: FormId) {
        let Some(ride) = self.rides.get(&actor).copied().filter(|r| r.mounted()) else {
            return;
        };
        if let Some((place, pos)) = self.execution(actor).and_then(|x| Some((x.place?, x.pos))) {
            self.put_away(ride.horse, place, pos);
        }
    }

    pub(super) fn sync_riding_bodies(&mut self) {
        let rides: Vec<_> = self
            .rides
            .iter()
            .filter(|(_, r)| r.mounted())
            .map(|(&a, &r)| (a, r))
            .collect();
        for (actor, ride) in rides {
            let Some((pos, heading, velocity)) = self
                .actor_ref(actor)
                .map(|a| (a.exec.pos, a.exec.heading, a.velocity))
            else {
                continue;
            };
            let Some(&key) = self.actor_cells.get(&ride.horse) else {
                continue;
            };
            let Some(index) = self
                .cells
                .get(&key)
                .and_then(|rt| rt.actors.iter().position(|a| a.ref_id == ride.horse))
            else {
                continue;
            };
            let h = &mut self.cells.get_mut(&key).unwrap().actors[index];
            h.exec.pos = pos;
            h.exec.heading = heading;
            h.velocity = velocity;
            let transform = h.transform();
            if let Some(c) = h.capsule {
                self.physics.move_actor_capsule(c, h.feet());
            }
            let bind = h.skeleton.model_space(&[]);
            let saddle = h.skeleton.find("SaddleBone");
            let attachment = self
                .scene
                .cells
                .get_mut(&key)
                .and_then(|rc| rc.actors.get_mut(index))
                .map(|inst| {
                    inst.transform = transform;
                    saddle
                        .and_then(|i| {
                            Some(
                                inst.pose.get(i)?.to_owned()
                                    * bind.get(i)?.inverse()
                                    * Mat4::from_translation(bind[i].w_axis.truncate()),
                            )
                        })
                        .map_or(transform, |bone| transform * bone)
                });
            if let Some(m) = attachment {
                if let Some(a) = self.actor_mut(actor) {
                    a.attachment = Some(m);
                }
                if let Some(&key) = self.actor_cells.get(&actor)
                    && let Some(i) = self
                        .cells
                        .get(&key)
                        .and_then(|rt| rt.actors.iter().position(|a| a.ref_id == actor))
                    && let Some(inst) = self
                        .scene
                        .cells
                        .get_mut(&key)
                        .and_then(|rc| rc.actors.get_mut(i))
                {
                    inst.transform = m;
                }
            }
            self.moved_refs.insert(ride.horse, pos);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn horse_input_uses_procedure_positions_including_follow_accompany_slot() {
        for (name, slot) in [
            ("Travel", 1),
            ("Patrol", 4),
            ("Follow", 4),
            ("Escort", 6),
            ("Wander", 2),
        ] {
            let mut n = package::Node {
                kind: "Procedure".into(),
                procedure: name.into(),
                flags: 0,
                conditions: vec![],
                args: vec![],
                values: vec![package::Input::Bool(false); 8],
                children: vec![],
            };
            n.values[slot] = package::Input::Bool(true);
            assert!(horse_input(&n));
            n.values[slot] = package::Input::Bool(false);
            assert!(!horse_input(&n));
        }
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::ai::exec::tests::{ACTOR, BASE, CELL, PACKAGE, fixture, loaded_actor};
    use esp::writer::RecordBuilder;
    const HORSE: FormId = FormId(0xFF000810);
    const HORSE_BASE: FormId = FormId(0x811);

    fn install(e: &mut Engine) {
        use esp::writer::{Node, write_plugin};
        let dir = std::env::temp_dir().join(format!(
            "vrm-mount-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&dir).unwrap();
        std::fs::write(
            dir.join("Mount.esp"),
            write_plugin(
                0,
                &[],
                "test",
                "mounts",
                0x900,
                &[
                    Node::top(
                        b"NPC_",
                        vec![
                            RecordBuilder::new(b"NPC_", BASE),
                            RecordBuilder::new(b"NPC_", HORSE_BASE)
                                .field(b"KWDA", &0x26110u32.to_le_bytes()),
                        ],
                    ),
                    Node::top(
                        b"ACHR",
                        vec![RecordBuilder::new(b"ACHR", ACTOR).form(b"NAME", BASE)],
                    ),
                    Node::top(b"PACK", vec![RecordBuilder::new(b"PACK", PACKAGE)]),
                    Node::top(
                        b"CELL",
                        vec![RecordBuilder::new(b"CELL", CELL).field(b"DATA", &[1, 0])],
                    ),
                ],
            ),
        )
        .unwrap();
        e.lo = esp::LoadOrder::load(&dir, &["Mount.esp".into()]).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        e.created_refs.refs.insert(
            HORSE,
            crate::created::Created {
                base: HORSE_BASE,
                position: Vec3::ZERO,
                rotation: Vec3::ZERO,
                origin: None,
                place: Some(Place::Interior(CELL)),
                container: None,
                location: None,
                actor: true,
                count: 1,
                owner: Some(BASE),
            },
        );
        loaded_actor(e);
        let mut p = Package::bare(PACKAGE, Behaviour::Travel);
        p.tree = Some(Arc::new(package::Node {
            kind: "Procedure".into(),
            procedure: "Travel".into(),
            flags: 0,
            conditions: vec![],
            args: vec![],
            children: vec![],
            values: vec![
                package::Input::Location(package::Location {
                    kind: package::LocationKind::NearSelf,
                    radius: 0.,
                }),
                package::Input::Bool(true),
            ],
        }));
        let a = e.actor_mut(ACTOR).unwrap();
        a.exec.packages = vec![p];
        a.exec.current = Some(0);
        a.exec.begin_run(Some(0), 0., false);
        let tree = a.exec.packages[0].tree.as_ref().unwrap();
        let mut c = procedure::Cursor::default();
        c.step(tree, 0., false, |_| true, || 0);
        a.exec.run.as_mut().unwrap().cursor = Some(c);
        a.exec.pos = Vec3::ZERO;
        let horse = ActorRuntime::new(
            HORSE,
            HORSE_BASE,
            Arc::new(crate::world::skeleton::Skeleton::default()),
            Mat4::IDENTITY,
            vec![],
            0.,
        );
        let key = crate::render::CellKey::Interior(CELL);
        e.actor_cells.insert(HORSE, key);
        e.cells.get_mut(&key).unwrap().actors.push(horse);
        e.location = crate::engine::Location::Interior(CELL);
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn mount_lifecycle_claims_owned_horse_preserves_run_and_dismount_returns_bool() {
        let (_f, mut e) = fixture();
        install(&mut e);
        assert_eq!(e.horse_for(ACTOR), Some(HORSE));
        let run = e.execution(ACTOR).unwrap().run.clone();
        e.prepare_mounts(0.);
        assert_eq!(e.rides[&ACTOR].phase, ENTER);
        assert!(e.is_on_mount(ACTOR));
        e.prepare_mounts(3.);
        assert_eq!(e.rides[&ACTOR].phase, MOUNTED);
        assert_eq!(e.execution(ACTOR).unwrap().run, run);
        assert_eq!(e.actor_ref(HORSE).unwrap().mount_driver, Some(ACTOR));
        e.actor_mut(ACTOR).unwrap().exec.pos.x = 500.;
        e.sync_riding_bodies();
        assert_eq!(e.actor_ref(HORSE).unwrap().exec.pos.x, 500.);
        let saved = crate::save::state::Snapshot::capture(&e).write();
        assert_eq!(
            crate::save::state::Snapshot::read(&saved)
                .unwrap()
                .continuity
                .rides,
            e.rides
        );
        assert!(e.dismount(ACTOR));
        assert!(!e.dismount(ACTOR));
        e.prepare_mounts(3.);
        assert!(!e.is_on_mount(ACTOR));
        assert!(e.actor_ref(ACTOR).unwrap().mount_speeds.is_none());
        e.prepare_mounts(3.);
        assert_eq!(
            e.rides[&ACTOR].phase, DISMOUNTED,
            "do not remount the same package after Dismount"
        );
        e.scripts.disabled.insert(HORSE, true);
        assert!(e.horse_for(ACTOR).is_none());
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn unloaded_rider_keeps_horse_speed_and_moves_the_horse_with_it() {
        let (_f, mut e) = fixture();
        install(&mut e);
        e.prepare_mounts(0.);
        e.prepare_mounts(3.);
        let walk = e.rides[&ACTOR].walk;
        e.unload_all();
        e.location = crate::engine::Location::Nowhere;
        assert_eq!(e.mount_driver(HORSE), Some(ACTOR));
        e.prepare_away_mount(ACTOR);
        assert_eq!(e.away_mount_speed(ACTOR), Some(walk));
        e.away.get_mut(&ACTOR).unwrap().pos.x = 1200.;
        e.sync_away_mount(ACTOR);
        assert_eq!(e.away[&HORSE].pos, e.away[&ACTOR].pos);
        assert!(e.is_on_mount(ACTOR));
        e.dismount(ACTOR);
        e.prepare_away_mount(ACTOR);
        assert_eq!(e.away_mount_speed(ACTOR), None);
    }

    #[test]
    #[ignore = "requires installed Skyrim SE masters and a headless GPU"]
    fn vanilla_horse_associations_are_reference_and_ownership_data() {
        let data = vfs::locate_data_dir().unwrap();
        let names = esp::BASE_MASTERS
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>();
        let e = crate::stats::action_tests::headless_engine_plugins(&data, &names);
        let mut associations = 0;
        for &id in e.lo.ids_of_type(b"ACHR") {
            let rec = e.lo.get(id).unwrap();
            if let Some(d) = rec.get(b"XHOR").filter(|d| d.len() >= 4) {
                let horse = rec.fid(FormId(u32::from_le_bytes(d[..4].try_into().unwrap())));
                println!("rider {id} horse {horse}");
                if e.has_keyword(horse, FormId(0x26110)) {
                    associations += 1;
                }
            }
        }
        assert!(associations > 0);
    }
}
