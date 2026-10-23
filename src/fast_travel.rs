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
            Self::Dead => ("sCantSaveNow", "You cannot do that right now."),
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

impl Engine {
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
            marker(0x933),
            marker(0x934).field(b"XLKR", &link(0, 0xDEAD)),
            reference(0x935, f32::NAN, 0.0, 0.0, 0.0)
                .field(b"XMRK", &[])
                .field(b"FNAM", &[3]),
        ];
        let nodes = vec![
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
