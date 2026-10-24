//! Per-reference state scripts set: scale (`SetScale`), and the favor refusal
//! (`SetNoFavorAllowed`). Saved: `RSCL`, `NOFV`. Choices:
//! `known_ambiguities/remaining-natives.md`.

use std::collections::{HashMap, HashSet};

use esp::FormId;
use glam::{Mat4, Vec3};

use crate::engine::Engine;

#[derive(Default)]
pub struct RefState {
    /// The scale scripts gave a reference, in place of its placed one (`XSCL`).
    pub(crate) scales: HashMap<FormId, f32>,
    /// References followers refuse to do favors on.
    pub(crate) no_favor: HashSet<FormId>,
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
    ) {
        self.ref_state.scales = scales;
        self.ref_state.no_favor = no_favor;
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
}
