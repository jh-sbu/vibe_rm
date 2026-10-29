//! Per-reference state scripts set: scale (`SetScale`), and the favor refusal
//! (`SetNoFavorAllowed`). Saved: `RSCL`, `NOFV`. Choices:
//! `known_ambiguities/remaining-natives.md`.

use std::collections::{HashMap, HashSet};

use esp::FormId;
use glam::{Mat4, Vec3};

use crate::engine::Engine;

#[derive(Default)]
pub struct RefState {
    pub(crate) item_copies: HashMap<FormId, Vec<crate::item_copies::ItemCopy>>,
    /// The scale scripts gave a reference, in place of its placed one (`XSCL`).
    pub(crate) scales: HashMap<FormId, f32>,
    /// References followers refuse to do favors on.
    pub(crate) no_favor: HashSet<FormId>,
    /// References that don't turn on an ally who hits them
    /// (`IgnoreFriendlyHits`).
    pub(crate) ignore_friendly_hits: HashSet<FormId>,
    /// Lower-cased node names of the models `HasNode` has looked into.
    nodes: HashMap<String, std::sync::Arc<HashSet<String>>>,
}

/// `GetHeadingAngle`: the angle, -180..180 degrees, to add to a heading
/// (`z` turn, radians, clockwise from +Y) to face from `from` toward `to`.
pub fn heading_angle(from: Vec3, heading: f32, to: Vec3) -> f32 {
    let d = to - from;
    let toward = d.x.atan2(d.y);
    let mut a = (toward - heading).to_degrees();
    while a > 180.0 {
        a -= 360.0;
    }
    while a < -180.0 {
        a += 360.0;
    }
    a
}

/// The player's level held to an encounter zone's (`ECZN` `DATA`) minimum,
/// unless the zone matches the player below it, and its maximum.
pub fn zone_level(pc: u16, zone: Option<&[u8]>) -> f32 {
    let mut level = pc.max(1) as f32;
    if let Some(d) = zone.filter(|d| d.len() >= 12) {
        let (min, flags, max) = (d[9] as f32, d[10], d[11] as f32);
        if min > 0.0 && level < min && flags & 0x01 == 0 {
            level = min;
        }
        if max > 0.0 && level > max {
            level = max;
        }
    }
    level
}

impl Engine {
    /// `SetScale`: the reference's own scale from now on. A loaded actor keeps
    /// its NPC and race heights on top; a loaded object is resized about its
    /// origin. Its collision stays as it was (the CK wiki notes this).
    pub fn set_ref_scale(&mut self, r: FormId, scale: f32) {
        if r == crate::engine::PLAYER_REF || !scale.is_finite() || scale <= 0.0 {
            return;
        }
        let old = self.reference_of(r).map_or(1.0, |rf| rf.scale);
        self.ref_state.scales.insert(r, scale);
        if let Some(a) = self.actor_mut(r) {
            if old > 0.0 {
                a.scale *= scale / old;
            }
            return;
        }
        if old <= 0.0 || old == scale {
            return;
        }
        let Some(pos) = self.ref_position(r) else {
            return;
        };
        let k = scale / old;
        let delta = Mat4::from_translation(pos)
            * Mat4::from_scale(Vec3::splat(k))
            * Mat4::from_translation(-pos);
        for rc in self.scene.cells.values_mut() {
            for inst in rc.instances.iter_mut().filter(|i| i.ref_id == r.0) {
                inst.transform = delta * inst.transform;
                inst.world_center = inst.transform.transform_point3(inst.model.bound_center);
            }
        }
        for rt in self.cells.values_mut() {
            for o in rt.animated.iter_mut().filter(|o| o.ref_id == r) {
                o.transform = delta * o.transform;
            }
        }
    }

    /// While building a cell's objects: the scale scripts gave a reference.
    pub(crate) fn apply_scales(&self, objects: &mut [crate::world::cell::PlacedObject]) {
        if self.ref_state.scales.is_empty() {
            return;
        }
        for o in objects {
            let Some(&scale) = self.ref_state.scales.get(&o.ref_id) else {
                continue;
            };
            let (_, rot, pos) = o.transform.to_scale_rotation_translation();
            o.transform = Mat4::from_scale_rotation_translation(Vec3::splat(scale), rot, pos);
        }
    }

    /// `SetNoFavorAllowed`.
    pub fn set_no_favor_allowed(&mut self, r: FormId, no_favor: bool) {
        if no_favor {
            self.ref_state.no_favor.insert(r);
        } else {
            self.ref_state.no_favor.remove(&r);
        }
    }

    pub(crate) fn restore_ref_state(
        &mut self,
        scales: HashMap<FormId, f32>,
        no_favor: HashSet<FormId>,
        ignore_friendly_hits: HashSet<FormId>,
    ) {
        self.ref_state.scales = scales;
        self.ref_state.no_favor = no_favor;
        self.ref_state.ignore_friendly_hits = ignore_friendly_hits;
    }

    /// `IgnoreFriendlyHits`.
    pub fn set_ignore_friendly_hits(&mut self, r: FormId, ignore: bool) {
        if ignore {
            self.ref_state.ignore_friendly_hits.insert(r);
        } else {
            self.ref_state.ignore_friendly_hits.remove(&r);
        }
    }

    /// Whether a blow from `attacker` is one `target` ignores: it was told to
    /// ignore friendly hits, and the attacker is an ally of its factions or
    /// the player's teammate (or the player, if it is the player's teammate).
    pub(crate) fn ignores_friendly_hit(&self, target: FormId, attacker: FormId) -> bool {
        if !self.ref_state.ignore_friendly_hits.contains(&target) {
            return false;
        }
        let side = |a: FormId| {
            if a == crate::engine::PLAYER_REF {
                true
            } else {
                self.is_player_teammate(a)
            }
        };
        (side(attacker) && side(target) && attacker != target)
            || self.faction_relation(target, attacker) >= 2
    }

    /// Whether a reference has 3D loaded: the player's body, an actor in a
    /// shown cell, or an object with a render instance.
    pub fn ref_3d_loaded(&self, r: FormId) -> bool {
        if r == crate::engine::PLAYER_REF {
            return self.player_body.is_some() || self.scene.player.is_some();
        }
        let actor_loaded = self.actor_cells.get(&r).is_some_and(|key| {
            self.cells
                .get(key)
                .and_then(|rt| rt.actors.iter().position(|a| a.ref_id == r))
                .is_some_and(|i| {
                    self.scene
                        .cells
                        .get(key)
                        .is_some_and(|cell| i < cell.actors.len())
                })
        });
        actor_loaded
            || self
                .scene
                .cells
                .values()
                .flat_map(|cell| &cell.instances)
                .chain(self.scene.far.iter())
                .any(|instance| instance.ref_id == r.0)
    }

    /// `HasNode`: whether a loaded reference's 3D has a node of that name (an
    /// actor's skeleton, else its base's model); false when not loaded.
    pub fn ref_has_node(&mut self, r: FormId, name: &str) -> bool {
        if !self.ref_3d_loaded(r) {
            return false;
        }
        let name = name.to_ascii_lowercase();
        if let Some(a) = self.actor_ref(r) {
            return a.skeleton.find(&name).is_some();
        }
        if r == crate::engine::PLAYER_REF {
            return false;
        }
        let Some(model) = self
            .base_of(r)
            .and_then(|b| self.lo.get(b))
            .and_then(|rec| rec.get(b"MODL").map(esp::decode_zstring))
            .filter(|m| !m.is_empty())
            .map(|m| crate::world::records::mesh_path(&m))
        else {
            return false;
        };
        if !self.ref_state.nodes.contains_key(&model) {
            let names: HashSet<String> = self
                .vfs
                .read(&model)
                .and_then(|b| nif::Nif::parse(&b).ok())
                .map(|n| {
                    crate::render::model::node_transforms(&n)
                        .into_keys()
                        .map(|k| k.to_ascii_lowercase())
                        .collect()
                })
                .unwrap_or_default();
            self.ref_state
                .nodes
                .insert(model.clone(), std::sync::Arc::new(names));
        }
        self.ref_state.nodes[&model].contains(&name)
    }

    /// `GetMass`: the Havok mass of a loose object; 0 for anything Havok
    /// doesn't hold (the CK wiki's note).
    pub fn ref_mass(&self, r: FormId) -> f32 {
        self.cells
            .values()
            .flat_map(|rt| &rt.loose)
            .filter(|l| l.ref_id == r)
            .flat_map(|l| &l.bodies)
            .map(|b| self.physics.body_mass_speed(b.handle).0)
            .sum()
    }

    /// `IsDeleted`: a record flagged deleted, or a created reference that was
    /// deleted.
    pub fn ref_is_deleted(&self, r: FormId) -> bool {
        const DELETED: u32 = 0x20;
        match self.lo.get(r) {
            Some(rec) => rec.flags() & DELETED != 0,
            None => r.0 >= 0xFF00_0000 && self.created(r).is_none(),
        }
    }

    /// `IsFurnitureMarkerInUse`: someone has reserved the marker; with
    /// `ignore_reserved`, only someone already in the furniture counts.
    pub fn furniture_marker_in_use(&self, f: FormId, marker: usize, ignore_reserved: bool) -> bool {
        let Some(user) = u8::try_from(marker)
            .ok()
            .filter(|&m| {
                self.furniture
                    .get(f)
                    .is_some_and(|i| marker < i.markers.len())
                    && m < 255
            })
            .and_then(|m| self.furniture.user_of(f, m))
        else {
            return false;
        };
        !ignore_reserved
            || self.actor_ref(user).is_some_and(|a| {
                a.in_furniture() && a.seat.as_ref().is_some_and(|s| s.furniture == f)
            })
    }

    /// `IsFurnitureInUse`: any marker is.
    pub fn furniture_in_use(&self, f: FormId, ignore_reserved: bool) -> bool {
        let count = self.furniture.get(f).map_or(0, |i| i.markers.len());
        (0..count).any(|m| self.furniture_marker_in_use(f, m, ignore_reserved))
    }

    /// `CalculateEncounterLevel`: the player's level, held to the encounter
    /// zone's minimum (unless it matches the player below it) and maximum, then
    /// scaled by the difficulty's leveled actor multiplier (0 easy .. 3 very
    /// hard; anything else none).
    pub fn encounter_level(&self, r: FormId, difficulty: i32) -> i32 {
        let zone = self
            .encounter_zone_of(r)
            .and_then(|z| self.lo.get(z))
            .and_then(|rec| {
                rec.get(b"DATA")
                    .filter(|d| d.len() >= 12)
                    .map(|d| d.to_vec())
            });
        let level = zone_level(crate::skills::player_level(), zone.as_deref());
        let (name, default) = match difficulty {
            0 => ("fLeveledActorMultEasy", 0.5),
            1 => ("fLeveledActorMultMedium", 0.75),
            2 => ("fLeveledActorMultHard", 1.0),
            3 => ("fLeveledActorMultVeryHard", 1.5),
            _ => return level.round() as i32,
        };
        let mult = crate::ai::combat::gmst_f32(&self.lo, name, default);
        ((level * mult).round() as i32).max(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use esp::writer::{Node, RecordBuilder, write_plugin};
    use papyrus::{NativeResult, Value};

    const BASE: FormId = FormId(0x800);
    const REF: FormId = FormId(0x810);
    const SCALED: FormId = FormId(0x811);
    const GONE: FormId = FormId(0x812);
    const CHILD: FormId = FormId(0x813);
    const ZONED: FormId = FormId(0x814);
    const ZONE: FormId = FormId(0x820);
    const CELL: FormId = FormId(0x830);

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
            std::env::temp_dir().join(format!("vrm-refstate-{}-{stamp}", std::process::id())),
        );
        std::fs::create_dir(&f.0).unwrap();
        let records = [
            Node::top(b"STAT", vec![RecordBuilder::new(b"STAT", BASE)]),
            Node::top(
                b"REFR",
                vec![
                    RecordBuilder::new(b"REFR", REF).form(b"NAME", BASE),
                    RecordBuilder::new(b"REFR", SCALED)
                        .form(b"NAME", BASE)
                        .f32(b"XSCL", 2.0),
                    RecordBuilder::new(b"REFR", GONE)
                        .flags(0x20)
                        .form(b"NAME", BASE),
                    RecordBuilder::new(b"REFR", CHILD)
                        .form(b"NAME", BASE)
                        .field(b"XAPR", &[REF.0.to_le_bytes(), 0f32.to_le_bytes()].concat()),
                    RecordBuilder::new(b"REFR", ZONED)
                        .form(b"NAME", BASE)
                        .form(b"XEZN", ZONE),
                ],
            ),
            Node::top(
                b"ECZN",
                vec![
                    RecordBuilder::new(b"ECZN", ZONE)
                        .field(b"DATA", &[vec![0u8; 9], vec![10, 0, 20]].concat()),
                ],
            ),
        ];
        std::fs::write(
            f.0.join("RefState.esp"),
            write_plugin(0, &[], "test", "synthetic reference state", 0x900, &records),
        )
        .unwrap();
        let e = crate::stats::action_tests::headless_engine(&f.0, "RefState.esp");
        (f, e)
    }

    fn call(e: &mut Engine, r: FormId, func: &str, args: &[Value]) -> Value {
        let this = e.object_value(r);
        match crate::script::natives::call(e, "objectreference", func, Some(&this), args) {
            NativeResult::Value(v) => v,
            _ => panic!("ObjectReference.{func} didn't return"),
        }
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn set_scale_replaces_the_placed_scale_and_get_scale_sees_it() {
        let (_f, mut e) = fixture();
        assert_eq!(call(&mut e, SCALED, "getscale", &[]), Value::Float(2.0));
        call(&mut e, SCALED, "setscale", &[Value::Float(0.5)]);
        assert_eq!(call(&mut e, SCALED, "getscale", &[]), Value::Float(0.5));
        assert_eq!(call(&mut e, REF, "getscale", &[]), Value::Float(1.0));
        // Not a scale at all.
        call(&mut e, REF, "setscale", &[Value::Float(-1.0)]);
        call(&mut e, REF, "setscale", &[Value::Float(f32::NAN)]);
        assert_eq!(call(&mut e, REF, "getscale", &[]), Value::Float(1.0));
        assert_eq!(e.ref_state.scales.len(), 1);
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn loading_a_cell_applies_scripts_scale() {
        let (_f, mut e) = fixture();
        call(&mut e, REF, "setscale", &[Value::Float(3.0)]);
        let mut objects = vec![crate::world::cell::PlacedObject {
            ref_id: REF,
            transform: Mat4::from_translation(Vec3::new(1.0, 2.0, 3.0)),
            base: BASE,
            model: String::new(),
        }];
        e.apply_scales(&mut objects);
        let (scale, _, pos) = objects[0].transform.to_scale_rotation_translation();
        assert_eq!(scale, Vec3::splat(3.0));
        assert_eq!(pos, Vec3::new(1.0, 2.0, 3.0));
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn favor_refusal_is_set_and_cleared() {
        let (_f, mut e) = fixture();
        call(&mut e, REF, "setnofavorallowed", &[Value::Bool(true)]);
        assert!(e.ref_state.no_favor.contains(&REF));
        call(&mut e, REF, "setnofavorallowed", &[Value::Bool(false)]);
        assert!(e.ref_state.no_favor.is_empty());
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn get_heading_angle_native_uses_the_two_positions() {
        let (_f, mut e) = fixture();
        let other = e.object_value(SCALED);
        let angle = call(&mut e, REF, "getheadingangle", &[other]);
        assert!(matches!(angle, Value::Float(a) if a.abs() <= 180.0));
    }

    #[test]
    fn zone_levels_clamp_the_players() {
        let zone = |min: u8, flags: u8, max: u8| {
            let mut d = vec![0u8; 9];
            d.extend([min, flags, max]);
            d
        };
        assert_eq!(zone_level(5, None), 5.0);
        assert_eq!(zone_level(0, None), 1.0);
        assert_eq!(zone_level(5, Some(&zone(10, 0, 0))), 10.0);
        assert_eq!(zone_level(5, Some(&zone(10, 1, 0))), 5.0);
        assert_eq!(zone_level(30, Some(&zone(10, 0, 20))), 20.0);
        assert_eq!(zone_level(15, Some(&zone(10, 0, 20))), 15.0);
        assert_eq!(zone_level(5, Some(&[0; 8])), 5.0);
    }

    #[test]
    fn heading_angle_is_signed_degrees_from_the_heading() {
        let o = Vec3::ZERO;
        // Heading 0 faces +Y; turning is clockwise from above.
        let at = |to: Vec3, heading: f32| heading_angle(o, heading, to);
        assert!((at(Vec3::Y, 0.0) - 0.0).abs() < 1e-3);
        assert!((at(Vec3::X, 0.0) - 90.0).abs() < 1e-3);
        assert!((at(-Vec3::Y, 0.0).abs() - 180.0).abs() < 1e-3);
        assert!((at(-Vec3::X, 0.0) + 90.0).abs() < 1e-3);
        // Facing +X already.
        assert!(at(Vec3::X, std::f32::consts::FRAC_PI_2).abs() < 1e-3);
        // Wraps into range.
        let a = at(Vec3::Y, 3.0 * std::f32::consts::PI);
        assert!((-180.0..=180.0).contains(&a));
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn flags_and_queries_read_the_records_and_state() {
        let (_f, mut e) = fixture();
        let other = e.object_value(CHILD);
        // IsDeleted: the record flag; a created id nothing holds; not a live ref.
        assert_eq!(call(&mut e, GONE, "isdeleted", &[]), Value::Bool(true));
        assert_eq!(call(&mut e, REF, "isdeleted", &[]), Value::Bool(false));
        assert!(e.ref_is_deleted(FormId(0xFF00_0900)));
        // IsActivateChild: the child's XAPR names the parent, not the reverse.
        let reference = e.object_value(REF);
        assert_eq!(
            call(&mut e, REF, "isactivatechild", &[other.clone()]),
            Value::Bool(true)
        );
        assert_eq!(
            call(&mut e, CHILD, "isactivatechild", &[reference]),
            Value::Bool(false)
        );
        // Nothing 3D is loaded, so there is no node and no mass.
        let node = Value::String("Root".into());
        assert_eq!(call(&mut e, REF, "hasnode", &[node]), Value::Bool(false));
        assert_eq!(call(&mut e, REF, "getmass", &[]), Value::Float(0.0));
        // IgnoreFriendlyHits.
        call(&mut e, REF, "ignorefriendlyhits", &[Value::Bool(true)]);
        assert_eq!(
            call(&mut e, REF, "isignoringfriendlyhits", &[]),
            Value::Bool(true)
        );
        call(&mut e, REF, "ignorefriendlyhits", &[Value::Bool(false)]);
        assert_eq!(
            call(&mut e, REF, "isignoringfriendlyhits", &[]),
            Value::Bool(false)
        );
        // Nobody is talking to the player.
        assert_eq!(
            call(&mut e, REF, "isindialoguewithplayer", &[]),
            Value::Bool(false)
        );
        // Cell.SetPublic reaches the map crime and locks read.
        let this = e.object_value(BASE);
        crate::script::natives::call(
            &mut e,
            "cell",
            "setpublic",
            Some(&this),
            &[Value::Bool(false)],
        );
        assert_eq!(e.scripts.cell_public.get(&BASE), Some(&false));
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn a_friend_the_player_travels_with_is_ignored_only_when_told_to() {
        let (_f, mut e) = fixture();
        e.set_player_teammate(REF, true, true);
        let player = crate::engine::PLAYER_REF;
        assert!(!e.ignores_friendly_hit(REF, player));
        e.set_ignore_friendly_hits(REF, true);
        assert!(e.ignores_friendly_hit(REF, player));
        // Not a teammate, not an ally: a stranger's blow counts.
        assert!(!e.ignores_friendly_hit(REF, SCALED));
        e.set_ignore_friendly_hits(SCALED, true);
        assert!(!e.ignores_friendly_hit(SCALED, player));
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn furniture_markers_are_in_use_when_reserved() {
        use crate::ai::furniture::{Furniture, Marker, Use};
        let (_f, mut e) = fixture();
        let marker = Marker {
            pos: Vec3::ZERO,
            heading: 0.0,
            kind: Use::Sit,
            entries: 0,
            anim_type: 1,
        };
        e.furniture.items.push(Furniture {
            ref_id: REF,
            cell: crate::render::CellKey::Interior(CELL),
            owner: None,
            markers: vec![marker, marker],
            events: Vec::new(),
            idle_time: 0.0,
            ways: Vec::new(),
        });
        let in_use = |e: &mut Engine, func: &str, args: &[Value]| {
            call(e, REF, func, args) == Value::Bool(true)
        };
        assert!(!in_use(&mut e, "isfurnitureinuse", &[]));
        e.furniture.reserve(REF, 1, SCALED);
        assert!(in_use(&mut e, "isfurnitureinuse", &[]));
        assert!(in_use(&mut e, "isfurnituremarkerinuse", &[Value::Int(1)]));
        assert!(!in_use(&mut e, "isfurnituremarkerinuse", &[Value::Int(0)]));
        assert!(!in_use(&mut e, "isfurnituremarkerinuse", &[Value::Int(2)]));
        // Reserved but nobody is in it.
        let ignore = Value::Bool(true);
        assert!(!in_use(&mut e, "isfurnitureinuse", &[ignore]));
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn encounter_levels_use_the_zone_and_difficulty() {
        let (_f, mut e) = fixture();
        let pc = crate::skills::player_level();
        let held = zone_level(pc, Some(&[vec![0u8; 9], vec![10, 0, 20]].concat()));
        let level = |e: &mut Engine, r, args: &[Value]| call(e, r, "calculateencounterlevel", args);
        // Without a zone: the player's level; with one: held to it.
        assert_eq!(level(&mut e, REF, &[]), Value::Int(pc.max(1) as i32));
        assert_eq!(level(&mut e, ZONED, &[]), Value::Int(held as i32));
        // Hard is the zone's level; easy and very hard scale it.
        assert_eq!(
            level(&mut e, ZONED, &[Value::Int(2)]),
            Value::Int(held as i32)
        );
        assert_eq!(
            level(&mut e, ZONED, &[Value::Int(0)]),
            Value::Int(((held * 0.5).round() as i32).max(1))
        );
        assert_eq!(
            level(&mut e, ZONED, &[Value::Int(3)]),
            Value::Int((held * 1.5).round() as i32)
        );
    }
}
