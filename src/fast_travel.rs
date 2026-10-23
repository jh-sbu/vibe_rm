//! World-map travel. Public evidence and native policies: `known_ambiguities/fast-travel.md`.

use crate::ai::schedule::Place;
use crate::engine::{Engine, Location, PLAYER_REF};
use esp::FormId;
use glam::Vec3;

/// A refusal has a GMST key and an English fallback for incomplete load orders.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    Disabled,
    Indoors,
    RestrictedWorld,
    Dead,
    Overencumbered,
    InAir,
    Enemies,
    Pursued,
    TakingDamage,
    Undiscovered,
    InvalidDestination,
}

impl Refusal {
    pub fn message(self, e: &Engine) -> String {
        let (key, fallback) = match self {
            Self::Disabled => (
                "sFastTravelDisabled",
                "Fast travel is currently unavailable from this location.",
            ),
            Self::Indoors | Self::RestrictedWorld => (
                "sNoFastTravelInCell",
                "You cannot fast travel from this location.",
            ),
            Self::Dead => (
                "sNoFastTravelWhileDead",
                "You cannot fast travel right now.",
            ),
            Self::Overencumbered => (
                "sNoFastTravelOverencumbered",
                "You cannot fast travel while overencumbered.",
            ),
            Self::InAir => (
                "sNoFastTravelInAir",
                "You cannot fast travel while jumping or falling.",
            ),
            Self::Enemies => (
                "sNoFastTravelHostileActorsNear",
                "You cannot fast travel when enemies are nearby.",
            ),
            Self::Pursued => (
                "sNoFastTravelWhileAlarm",
                "You cannot fast travel while guards are pursuing you.",
            ),
            Self::TakingDamage => (
                "sNoFastTravelTakingHealthDamage",
                "You cannot fast travel while taking health damage.",
            ),
            Self::Undiscovered => (
                "sNoFastTravelUndiscovered",
                "You have not discovered this location yet.",
            ),
            Self::InvalidDestination => (
                "sNoFastTravelInCell",
                "You cannot fast travel to this location.",
            ),
        };
        e.gmst_string(key).unwrap_or_else(|| fallback.into())
    }
}

/// A resolved landing transform; positions are feet and rotations are radians.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Destination {
    pub anchor: FormId,
    pub place: Place,
    pub pos: Vec3,
    pub yaw: f32,
}

/// Native approximation: walking distance, the configured speed multiplier and
/// TimeScale. Exact Skyrim route/speed accounting is not publicly established.
fn travel_time(distance: f32, speed_mult: f32, time_scale: f32) -> Option<(f32, f32)> {
    if !distance.is_finite()
        || distance < 0.0
        || !speed_mult.is_finite()
        || speed_mult <= 0.0
        || !time_scale.is_finite()
        || time_scale < 0.0
    {
        return None;
    }
    let seconds = distance / (crate::ai::WALK_SPEED * speed_mult);
    let hours = seconds * time_scale / 3600.0;
    // Bound catch-up work and reject corrupt settings/positions before mutation.
    (seconds.is_finite() && hours.is_finite() && seconds <= 129600.0 && hours <= 720.0)
        .then_some((hours, seconds))
}

impl Engine {
    /// The map's marker click: preflight, then the shared native message box.
    pub fn request_map_travel(&mut self, marker: FormId) {
        if self.messages.boxes.iter().any(|b| {
            matches!(
                b.action,
                Some(crate::messages::BoxAction::FastTravel { .. })
            )
        }) {
            return;
        }
        let refusal = self
            .fast_travel_refusal()
            .or_else(|| self.map_travel_destination(marker).err());
        if let Some(reason) = refusal {
            let text = reason.message(self);
            self.scripts.notify(text);
            return;
        }
        let name = self
            .map_marker_list()
            .iter()
            .find(|m| m.r == marker)
            .map(|m| m.name.clone())
            .unwrap_or_default();
        let prompt = self
            .gmst_string("sConfirmFastTravel")
            .unwrap_or_else(|| "Fast travel to %s?".into());
        let yes = self.gmst_string("sYes").unwrap_or_else(|| "Yes".into());
        let no = self.gmst_string("sNo").unwrap_or_else(|| "No".into());
        self.ask(
            prompt.replace("%s", &name),
            vec![yes, no],
            crate::messages::BoxAction::FastTravel { marker },
        );
    }

    /// Revalidate a map request and load its destination using the normal
    /// placement, weather, streaming and party hooks.
    pub fn travel_to_map_marker(&mut self, marker: FormId) -> Result<(), Refusal> {
        if let Some(reason) = self.fast_travel_refusal() {
            return Err(reason);
        }
        let destination = self.map_travel_destination(marker)?;
        let (hours, seconds) = self.map_travel_time(destination)?;
        self.advance_travel_time(hours, seconds);
        let result = match destination.place {
            Place::Exterior(world, _) => {
                self.enter_exterior(world, destination.pos, destination.yaw)
            }
            Place::Interior(cell) => {
                self.enter_interior(cell, Some((destination.pos, destination.yaw)))
            }
        };
        if let Err(err) = result {
            log::warn!("fast travel to {marker} failed: {err:#}");
            return Err(Refusal::InvalidDestination);
        }
        self.menu = None;
        self.close_map_view();
        Ok(())
    }

    fn map_travel_time(&self, destination: Destination) -> Result<(f32, f32), Refusal> {
        // Coordinates in unrelated worldspaces cannot be subtracted meaningfully.
        let comparable = matches!((self.location, destination.place),
            (Location::Exterior { world: a, .. }, Place::Exterior(b, _)) if a == b);
        if !comparable {
            return Ok((0.0, 0.0));
        }
        let distance = self
            .player_feet()
            .truncate()
            .distance(destination.pos.truncate());
        let multiplier = crate::ai::combat::gmst_f32(&self.lo, "fFastTravelSpeedMult", 1.0);
        let scale = self
            .lo
            .find_editor_id("TimeScale")
            .map_or(20.0, |g| self.global_value(g));
        let time = travel_time(distance, multiplier, scale).ok_or(Refusal::InvalidDestination)?;
        let total = self.hour as f64 + time.0 as f64;
        if !total.is_finite()
            || total < 0.0
            || self.day as f64 + (total / 24.0).floor() > u32::MAX as f64
        {
            return Err(Refusal::InvalidDestination);
        }
        Ok(time)
    }

    fn advance_travel_time(&mut self, hours: f32, seconds: f32) {
        if seconds <= 0.0 {
            return;
        }
        let steps = (seconds / 180.0).ceil().max(1.0) as u32;
        let dt = seconds / steps as f32;
        let dh = hours / steps as f32;
        for _ in 0..steps {
            let total = self.hour as f64 + dh as f64;
            self.day += (total / 24.0).floor() as u32;
            self.hour = (total % 24.0) as f32;
            self.update_weather(dh);
            self.update_magic(dt);
            self.update_stamina(dt);
            self.regen_magicka(dt);
            self.regen_npc_magicka(dt);
            if !self.player_dead() {
                let stats = self.player_stats();
                self.damage_actor_value(
                    PLAYER_REF,
                    esp::actor_value::HEALTH,
                    -stats.max_health * stats.health_regen.max(0.0) / 100.0 * dt,
                );
            }
        }
        self.heal_away(seconds);
        if self.ai_enabled {
            self.jump_to_schedules();
        }
    }

    /// A map marker lands at its unkeyed linked reference, or at itself.
    /// Broken links fail rather than silently using an unintended spawn point.
    pub fn map_travel_destination(&self, marker: FormId) -> Result<Destination, Refusal> {
        let rec = self
            .lo
            .get(marker)
            .filter(|r| r.tag().0 == *b"REFR" && r.get(b"XMRK").is_some())
            .ok_or(Refusal::InvalidDestination)?;
        if self.is_disabled(marker) || rec.flags() & 0x20 != 0 {
            return Err(Refusal::InvalidDestination);
        }
        if !self.map_marker_visible(marker) || !self.can_fast_travel_to(marker) {
            return Err(Refusal::Undiscovered);
        }
        let anchor = self
            .linked_ref(marker, Some(FormId::NULL))
            .unwrap_or(marker);
        let target = self
            .lo
            .get(anchor)
            .filter(|r| matches!(&r.tag().0, b"REFR" | b"ACHR") && r.flags() & 0x20 == 0)
            .ok_or(Refusal::InvalidDestination)?;
        if self.is_disabled(anchor) || target.get(b"DATA").is_none_or(|d| d.len() < 24) {
            return Err(Refusal::InvalidDestination);
        }
        let (place, pos) = self
            .current_place_of(anchor)
            .ok_or(Refusal::InvalidDestination)?;
        let rot = self.ref_rotation(anchor);
        if !pos.is_finite() || !rot.is_finite() {
            return Err(Refusal::InvalidDestination);
        }
        let valid = match place {
            Place::Exterior(world, _) => self.lo.world(world).is_some(),
            Place::Interior(cell) => self.lo.cell(cell).is_some(),
        };
        if !valid {
            return Err(Refusal::InvalidDestination);
        }
        Ok(Destination {
            anchor,
            place,
            pos,
            yaw: rot.z,
        })
    }

    /// Current player restrictions, independent of any selected destination.
    /// Checked again on confirmation, rather than trusting the map's snapshot.
    pub fn fast_travel_refusal(&mut self) -> Option<Refusal> {
        use crate::ai::combat::{CombatState, ENGAGE_DISTANCE};
        if self.fast_travel_disabled {
            return Some(Refusal::Disabled);
        }
        if self.player_dead() {
            return Some(Refusal::Dead);
        }
        let Location::Exterior { world, .. } = self.location else {
            return Some(Refusal::Indoors);
        };
        if self
            .lo
            .get(world)
            .and_then(|r| r.get(b"DATA").and_then(|d| d.first().copied()))
            .is_some_and(|flags| flags & 2 != 0)
        {
            return Some(Refusal::RestrictedWorld);
        }
        if self.player_overencumbered() {
            return Some(Refusal::Overencumbered);
        }
        if !self.player.grounded && !self.player.noclip {
            return Some(Refusal::InAir);
        }
        if !self.crime.arrests.alarmed.is_empty() {
            return Some(Refusal::Pursued);
        }
        if self.taking_health_damage() {
            return Some(Refusal::TakingDamage);
        }
        if self.combat_state(PLAYER_REF) != CombatState::None {
            return Some(Refusal::Enemies);
        }
        let near: Vec<FormId> = self
            .cells
            .values()
            .flat_map(|rt| &rt.actors)
            .filter(|a| {
                !a.dead
                    && a.bleeding.is_none()
                    && a.pos.distance(self.player.position) < ENGAGE_DISTANCE
            })
            .map(|a| a.ref_id)
            .collect();
        if near.into_iter().any(|r| self.would_attack_player(r)) {
            return Some(Refusal::Enemies);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use crate::engine::PLAYER_REF;
    use papyrus::{NativeResult, Value};

    #[test]
    #[ignore = "requires a headless GPU; synthetic records only"]
    fn fast_travel_script_gate_and_scripted_player_moves() {
        let dir = tempfile_dir("gate");
        let mut e = crate::stats::action_tests::headless_engine(&dir, "Travel.esp");
        let enabled = |e: &mut crate::engine::Engine| match crate::script::natives::call(
            e,
            "game",
            "isfasttravelenabled",
            None,
            &[],
        ) {
            NativeResult::Value(Value::Bool(b)) => b,
            _ => panic!("boolean expected"),
        };
        assert!(enabled(&mut e));
        crate::script::natives::call(
            &mut e,
            "game",
            "enablefasttravel",
            None,
            &[Value::Bool(false)],
        );
        assert!(!enabled(&mut e));
        crate::script::natives::call(&mut e, "game", "enablefasttravel", None, &[]);
        assert!(enabled(&mut e));
        e.fast_travel_disabled = true;
        let player = e.object_value(PLAYER_REF);
        crate::script::natives::call(
            &mut e,
            "objectreference",
            "setposition",
            Some(&player),
            &[Value::Float(1.0), Value::Float(2.0), Value::Float(3.0)],
        );
        assert!(enabled(&mut e));
        e.fast_travel_disabled = true;
        e.map_markers.shown.insert(esp::FormId(0x900));
        e.map_markers.denied.insert(esp::FormId(0x900));
        let file = crate::save::state::Snapshot::capture(&e).write();
        e.fast_travel_disabled = false;
        e.map_markers.travel.insert(esp::FormId(0x901));
        crate::save::state::Snapshot::read(&file)
            .unwrap()
            .apply(&mut e)
            .unwrap();
        assert!(!enabled(&mut e));
        assert!(e.map_markers.denied.contains(&esp::FormId(0x900)));
        assert!(e.map_markers.travel.is_empty());
        let mut legacy = file;
        legacy.sections.retain(|(tag, _)| tag != b"MAPS");
        crate::save::state::Snapshot::read(&legacy)
            .unwrap()
            .apply(&mut e)
            .unwrap();
        assert!(enabled(&mut e));
        assert!(e.map_markers.denied.is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    #[ignore = "requires a headless GPU; synthetic records only"]
    fn fast_travel_refusals_follow_live_player_and_world_state() {
        use super::Refusal;
        use crate::engine::Location;
        let dir = tempfile_dir("refusals");
        let mut e = crate::stats::action_tests::headless_engine(&dir, "Travel.esp");
        e.player.grounded = true;
        e.player_health = 100.0;
        assert_eq!(e.fast_travel_refusal(), Some(Refusal::Indoors));
        e.location = Location::Exterior {
            world: esp::FormId(0x901),
            center: (0, 0),
        };
        assert_eq!(e.fast_travel_refusal(), Some(Refusal::RestrictedWorld));
        e.location = Location::Exterior {
            world: esp::FormId(0x900),
            center: (0, 0),
        };
        assert_eq!(e.fast_travel_refusal(), None);
        e.fast_travel_disabled = true;
        assert_eq!(e.fast_travel_refusal(), Some(Refusal::Disabled));
        e.fast_travel_disabled = false;
        e.player.grounded = false;
        assert_eq!(e.fast_travel_refusal(), Some(Refusal::InAir));
        e.player.grounded = true;
        e.set_actor_value(PLAYER_REF, esp::actor_value::CARRY_WEIGHT, -1.0);
        assert_eq!(e.fast_travel_refusal(), Some(Refusal::Overencumbered));
        e.set_actor_value(PLAYER_REF, esp::actor_value::CARRY_WEIGHT, 300.0);
        e.player_died_at = Some(0.0);
        assert_eq!(e.fast_travel_refusal(), Some(Refusal::Dead));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    #[ignore = "requires a headless GPU; synthetic records only"]
    fn fast_travel_destinations_use_unkeyed_links_and_validate_live_state() {
        use super::Refusal;
        use esp::FormId;
        use glam::Vec3;
        let dir = destination_fixture("destinations");
        let mut e = crate::stats::action_tests::headless_engine(&dir, "Travel.esp");
        let linked = e.map_travel_destination(FormId(0x930)).unwrap();
        assert_eq!(linked.anchor, FormId(0x931));
        assert_eq!(linked.pos, Vec3::new(6000.0, 7000.0, 80.0));
        assert_eq!(linked.yaw, 1.25);
        assert_eq!(
            e.map_travel_destination(FormId(0x932)).unwrap().anchor,
            FormId(0x932)
        );
        assert_eq!(
            e.map_travel_destination(FormId(0x933)).unwrap().anchor,
            FormId(0x933)
        );
        assert_eq!(
            e.map_travel_destination(FormId(0x934)),
            Err(Refusal::InvalidDestination)
        );
        e.scripts.disabled.insert(FormId(0x931), true);
        assert_eq!(
            e.map_travel_destination(FormId(0x930)),
            Err(Refusal::InvalidDestination)
        );
        e.scripts.disabled.clear();
        e.enable_fast_travel(FormId(0x930), false);
        assert_eq!(
            e.map_travel_destination(FormId(0x930)),
            Err(Refusal::Undiscovered)
        );
        assert_eq!(
            e.map_travel_destination(FormId(0x935)),
            Err(Refusal::InvalidDestination)
        );
        assert_eq!(
            e.map_travel_destination(FormId(0x920)),
            Err(Refusal::InvalidDestination)
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    #[ignore = "requires a headless GPU; synthetic records only"]
    fn fast_travel_relocates_and_rejects_before_unloading() {
        use crate::engine::Location;
        use esp::FormId;
        use glam::Vec3;
        let dir = destination_fixture("relocation");
        let mut e = crate::stats::action_tests::headless_engine(&dir, "Travel.esp");
        e.location = Location::Exterior {
            world: FormId(0x900),
            center: (0, 0),
        };
        e.player.grounded = true;
        let before = e.player.position;
        assert_eq!(
            e.travel_to_map_marker(FormId(0x934)),
            Err(super::Refusal::InvalidDestination)
        );
        assert_eq!(e.player.position, before);
        assert_eq!(
            e.location,
            Location::Exterior {
                world: FormId(0x900),
                center: (0, 0)
            }
        );
        e.travel_to_map_marker(FormId(0x930)).unwrap();
        assert_eq!(
            e.location,
            Location::Exterior {
                world: FormId(0x900),
                center: (1, 1)
            }
        );
        assert!((e.player_feet() - Vec3::new(6000.0, 7000.0, 82.0)).length() < 0.001);
        assert_eq!(e.camera.yaw, 1.25);
        e.player.grounded = true;
        e.travel_to_map_marker(FormId(0x936)).unwrap();
        assert_eq!(e.location, Location::Interior(FormId(0x940)));
        assert!((e.player_feet() - Vec3::new(100.0, 200.0, 32.0)).length() < 0.001);
        assert_eq!(e.camera.yaw, 2.0);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn fast_travel_time_is_distance_dependent_and_bounded() {
        assert_eq!(super::travel_time(28800.0, 1.0, 20.0), Some((2.0, 360.0)));
        assert_eq!(super::travel_time(28800.0, 2.0, 20.0), Some((1.0, 180.0)));
        assert_eq!(super::travel_time(0.0, 1.0, 20.0), Some((0.0, 0.0)));
        assert!(super::travel_time(f32::NAN, 1.0, 20.0).is_none());
        assert!(super::travel_time(f32::MAX, 1.0, 20.0).is_none());
        for invalid in [0.0, -1.0, f32::INFINITY, f32::NAN] {
            assert!(super::travel_time(1000.0, invalid, 20.0).is_none());
        }
    }

    #[test]
    #[ignore = "requires a headless GPU; synthetic records only"]
    fn fast_travel_advances_clock_and_wounds_without_wait_statistics() {
        use esp::FormId;
        use glam::Vec3;
        let dir = destination_fixture("clock");
        let mut e = crate::stats::action_tests::headless_engine(&dir, "Travel.esp");
        e.location = crate::engine::Location::Exterior {
            world: FormId(0x900),
            center: (0, 0),
        };
        e.place_player(Vec3::ZERO, 0.0);
        e.player.grounded = true;
        e.day = 4;
        e.hour = 23.9;
        let wall = e.scripts.wall_time;
        let real = e.scripts.real_time;
        e.world_state.wounds.insert(
            FormId(0x999),
            crate::world_state::Wounds {
                health: 10.0,
                max: 100.0,
                rate: 0.01,
                at: real,
            },
        );
        let expected = super::travel_time(6000.0f32.hypot(7000.0), 1.0, 20.0).unwrap();
        e.travel_to_map_marker(FormId(0x930)).unwrap();
        assert_eq!(e.day, 5);
        assert!((e.hour - (23.9 + expected.0 - 24.0)).abs() < 0.001);
        assert_eq!(e.scripts.wall_time, wall);
        assert_eq!(e.scripts.real_time, real);
        assert!(e.world_state.wounds[&FormId(0x999)].health_at(real) > 99.0);
        assert_eq!(e.scripts.stats.get("Hours Waiting"), None);
        assert_eq!(e.scripts.stats.get("Hours Slept"), None);
        e.player.grounded = true;
        e.day = u32::MAX;
        e.hour = 23.999;
        let before = e.player.position;
        assert!(e.travel_to_map_marker(FormId(0x933)).is_err());
        assert_eq!(e.player.position, before);
        assert_eq!(e.hour, 23.999);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    #[ignore = "requires a headless GPU; synthetic records only"]
    fn fast_travel_confirmation_cancels_revalidates_and_survives_save() {
        use crate::engine::Location;
        use crate::messages::BoxAction;
        use esp::FormId;
        let dir = destination_fixture("confirmation");
        let mut e = crate::stats::action_tests::headless_engine(&dir, "Travel.esp");
        e.location = Location::Exterior {
            world: FormId(0x900),
            center: (0, 0),
        };
        e.player.grounded = true;
        e.menu = Some(crate::items::Menu::Map);
        let before = e.player.position;
        e.request_map_travel(FormId(0x930));
        e.request_map_travel(FormId(0x930));
        assert_eq!(e.messages.boxes.len(), 1);
        assert!(e.messages.boxes[0].text.contains("Synthetic destination"));
        e.choose_message_button(1);
        assert_eq!(e.player.position, before);
        assert_eq!(e.menu, Some(crate::items::Menu::Map));
        e.request_map_travel(FormId(0x930));
        let file = crate::save::state::Snapshot::capture(&e).write();
        let s = crate::save::state::Snapshot::read(&file).unwrap();
        assert!(matches!(
            s.vm.message_boxes[0].action,
            Some(BoxAction::FastTravel {
                marker: FormId(0x930)
            })
        ));
        e.fast_travel_disabled = true;
        e.choose_message_button(0);
        assert_eq!(e.player.position, before);
        assert!(e.messages.boxes.is_empty());
        e.fast_travel_disabled = false;
        e.request_map_travel(FormId(0x930));
        e.choose_message_button(0);
        assert_eq!(
            e.location,
            Location::Exterior {
                world: FormId(0x900),
                center: (1, 1)
            }
        );
        assert_eq!(e.menu, None);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    #[ignore = "requires a headless GPU; synthetic records only"]
    fn fast_travel_party_only_brings_living_following_teammates_and_summons() {
        use crate::ai::package::Behaviour;
        use crate::ai::{ActorRuntime, Goal};
        use crate::engine::Location;
        use crate::render::CellKey;
        use esp::FormId;
        use glam::{Mat4, Vec3};
        let dir = destination_fixture("party");
        let mut e = crate::stats::action_tests::headless_engine(&dir, "Travel.esp");
        e.location = Location::Exterior {
            world: FormId(0x900),
            center: (0, 0),
        };
        e.player.grounded = true;
        let key = CellKey::Exterior(0, 0);
        for id in 0x950..=0x954 {
            let r = FormId(id);
            let mut a = ActorRuntime::new(
                r,
                FormId(0x960),
                std::sync::Arc::new(crate::world::skeleton::Skeleton::default()),
                Mat4::IDENTITY,
                Vec::new(),
                0.0,
            );
            let mut goal = Goal::travel(Vec3::ZERO);
            goal.behaviour = if id == 0x951 {
                Behaviour::Hold
            } else {
                Behaviour::Follow
            };
            goal.target = Some(PLAYER_REF);
            // Pin through the production pursuit hook; no PACK assets exist.
            a.pursue(PLAYER_REF, &mut e.furniture);
            a.goal = Some(goal);
            a.dead = id == 0x952;
            e.cells.entry(key).or_default().actors.push(a);
            e.actor_cells.insert(r, key);
            if id != 0x953 {
                e.set_player_teammate(r, true, id != 0x954);
            }
        }
        e.magic.commanded.insert(FormId(0x954), PLAYER_REF);
        assert_eq!(e.player_party(), vec![FormId(0x950), FormId(0x954)]);
        e.travel_to_map_marker(FormId(0x930)).unwrap();
        for id in [0x950, 0x954] {
            let moved = e.world_state.moved[&FormId(id)];
            assert!(moved.pos.distance(e.player_feet()) < 400.0);
            assert_eq!(
                moved.place,
                crate::ai::schedule::Place::Exterior(FormId(0x900), (1, 1))
            );
        }
        for id in [0x951, 0x952, 0x953] {
            assert!(!e.world_state.moved.contains_key(&FormId(id)));
        }
        let file = crate::save::state::Snapshot::capture(&e).write();
        let s = crate::save::state::Snapshot::read(&file).unwrap();
        assert_eq!(s.teammates, e.scripts.teammates);
        assert!(s.moved.contains_key(&FormId(0x950)));
        e.scripts.teammates.clear();
        s.apply(&mut e).unwrap();
        assert!(e.is_player_teammate(FormId(0x950)));
        let mut legacy = file;
        legacy.sections.retain(|(tag, _)| tag != b"TEAM");
        crate::save::state::Snapshot::read(&legacy)
            .unwrap()
            .apply(&mut e)
            .unwrap();
        assert!(!e.is_player_teammate(FormId(0x950)));
        std::fs::remove_dir_all(dir).unwrap();
    }

    fn destination_fixture(label: &str) -> std::path::PathBuf {
        use esp::writer::{Node, RecordBuilder as R, write_plugin};
        use esp::{FormId, group_type as g};
        let dir = tempfile_dir(label);
        let data = |x: f32, y: f32, z: f32, yaw: f32| {
            [x, y, z, 0.0, 0.0, yaw]
                .into_iter()
                .flat_map(f32::to_le_bytes)
                .collect::<Vec<_>>()
        };
        let reference = |id, x, y, z, yaw| {
            R::new(b"REFR", FormId(id))
                .form(b"NAME", FormId(0x920))
                .field(b"DATA", &data(x, y, z, yaw))
        };
        let marker = |id| {
            reference(id, 1000.0, 2000.0, 80.0, 0.5)
                .field(b"XMRK", &[])
                .field(b"FNAM", &[3])
                .field(b"TNAM", &[1, 0])
                .zstring(b"FULL", "Synthetic destination")
        };
        let link = |keyword: u32, dest: u32| {
            [keyword, dest]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>()
        };
        let refs = vec![
            marker(0x930).field(b"XLKR", &link(0, 0x931)),
            reference(0x931, 6000.0, 7000.0, 80.0, 1.25),
            marker(0x932).field(b"XLKR", &link(0x920, 0x931)),
            marker(0x936).field(b"XLKR", &link(0, 0x937)),
            marker(0x933),
            marker(0x934).field(b"XLKR", &link(0, 0xDEAD)),
            reference(0x935, f32::NAN, 0.0, 0.0, 0.0)
                .field(b"XMRK", &[])
                .field(b"FNAM", &[3]),
        ];
        let nodes = vec![
            Node::top(b"NPC_", vec![R::new(b"NPC_", FormId(0x960))]),
            Node::top(
                b"ACHR",
                (0x950..=0x954)
                    .map(|id| {
                        R::new(b"ACHR", FormId(id))
                            .form(b"NAME", FormId(0x960))
                            .field(b"DATA", &data(0.0, 0.0, 0.0, 0.0))
                    })
                    .collect(),
            ),
            Node::Group {
                label: *b"CELL",
                group_type: g::TOP,
                children: vec![
                    Node::Record(R::new(b"CELL", FormId(0x940)).field(b"DATA", &[1, 0])),
                    Node::Group {
                        label: 0x940u32.to_le_bytes(),
                        group_type: g::CELL_CHILDREN,
                        children: vec![Node::Group {
                            label: 0x940u32.to_le_bytes(),
                            group_type: g::CELL_TEMPORARY,
                            children: vec![Node::Record(reference(0x937, 100.0, 200.0, 30.0, 2.0))],
                        }],
                    },
                ],
            },
            Node::top(b"STAT", vec![R::new(b"STAT", FormId(0x920))]),
            Node::Group {
                label: *b"WRLD",
                group_type: g::TOP,
                children: vec![
                    Node::Record(
                        R::new(b"WRLD", FormId(0x900))
                            .zstring(b"EDID", "TestWorld")
                            .field(b"DATA", &[0]),
                    ),
                    Node::Group {
                        label: 0x900u32.to_le_bytes(),
                        group_type: g::WORLD_CHILDREN,
                        children: vec![
                            Node::Record(
                                R::new(b"CELL", FormId(0x910))
                                    .flags(0x400)
                                    .field(b"DATA", &[0, 0]),
                            ),
                            Node::Group {
                                label: 0x910u32.to_le_bytes(),
                                group_type: g::CELL_CHILDREN,
                                children: vec![Node::Group {
                                    label: 0x910u32.to_le_bytes(),
                                    group_type: g::CELL_PERSISTENT,
                                    children: refs.into_iter().map(Node::Record).collect(),
                                }],
                            },
                        ],
                    },
                ],
            },
        ];
        std::fs::write(
            dir.join("Travel.esp"),
            write_plugin(0, &[], "test", "synthetic travel", 0xA00, &nodes),
        )
        .unwrap();
        dir
    }

    fn tempfile_dir(label: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("vrm-travel-{label}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("Travel.esp"),
            esp::writer::write_plugin(
                0,
                &[],
                "test",
                "synthetic travel",
                0xA00,
                &[esp::writer::Node::top(
                    b"WRLD",
                    vec![
                        esp::writer::RecordBuilder::new(b"WRLD", esp::FormId(0x900))
                            .field(b"DATA", &[0]),
                        esp::writer::RecordBuilder::new(b"WRLD", esp::FormId(0x901))
                            .field(b"DATA", &[2]),
                    ],
                )],
            ),
        )
        .unwrap();
        dir
    }
}
