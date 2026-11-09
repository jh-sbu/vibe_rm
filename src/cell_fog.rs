//! `Cell.SetFogColor`, `SetFogPlanes` and `SetFogPower`: a script's fog for
//! an interior that isn't sky-lit, over its own (not over what a lighting
//! template gives it), kept in memory only
//! (`research/pex-natives/cell-batch17-fog-constraints.md`).

use std::collections::HashMap;

use esp::FormId;
use glam::Vec3;

use crate::engine::{Engine, Location};
use crate::render::Environment;

/// What scripts set of one cell's fog.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct FogOverride {
    /// Near and far colours (0 to 1).
    pub colors: Option<(Vec3, Vec3)>,
    pub planes: Option<(f32, f32)>,
    pub power: Option<f32>,
}

pub type CellFog = HashMap<FormId, FogOverride>;

/// `XCLL` inherit flags: the template gives the fog colours, near, far and
/// power.
const INHERIT_COLORS: u32 = 0x4;
const INHERIT_NEAR: u32 = 0x8;
const INHERIT_FAR: u32 = 0x10;
const INHERIT_POWER: u32 = 0x100;
/// Cell `DATA` flag "Show Sky" (sky-lit interiors).
const SHOW_SKY: u8 = 0x80;

impl Engine {
    /// Change `cell`'s fog override and show it if the player is there.
    pub fn set_cell_fog(&mut self, cell: FormId, change: impl FnOnce(&mut FogOverride)) {
        change(self.cell_fog.entry(cell).or_default());
        if self.location == Location::Interior(cell) {
            let mut env = self.scene.env.clone();
            self.apply_cell_fog(cell, &mut env);
            self.scene.env = env;
        }
    }

    /// Put `cell`'s fog override in the environment, where the cell's own
    /// lighting (not its template's) and the sky allow.
    pub(crate) fn apply_cell_fog(&self, cell: FormId, env: &mut Environment) {
        let Some(o) = self.cell_fog.get(&cell) else {
            return;
        };
        let Some(rec) = self.lo.get(cell) else { return };
        let flags = rec
            .get(b"DATA")
            .and_then(|d| d.first().copied())
            .unwrap_or(0);
        if flags & 1 == 0 || flags & SHOW_SKY != 0 {
            return;
        }
        let info = crate::world::records::cell_info(&self.lo, cell);
        let inherited = |bit: u32| {
            info.as_ref().is_some_and(|i| {
                !i.lighting_template.is_null()
                    && i.lighting.as_ref().is_none_or(|l| l.inherit & bit != 0)
            })
        };
        if let Some((near, far)) = o.colors.filter(|_| !inherited(INHERIT_COLORS)) {
            env.fog_near_color = near;
            env.fog_far_color = far;
            env.clear_color = far;
        }
        if let Some((near, far)) = o.planes {
            if !inherited(INHERIT_NEAR) {
                env.fog_near = near;
            }
            if !inherited(INHERIT_FAR) {
                env.fog_far = far.max(env.fog_near + 1.0);
            }
        }
        if let Some(p) = o.power.filter(|_| !inherited(INHERIT_POWER)) {
            env.fog_power = p;
        }
    }
}

/// A colour component, 0 to 255 clipped, as 0 to 1.
pub fn channel(v: i32) -> f32 {
    v.clamp(0, 255) as f32 / 255.0
}

#[cfg(test)]
mod tests {
    use esp::FormId;
    use esp::writer::{Node, RecordBuilder, write_plugin};
    use papyrus::Value;

    const LIT: FormId = FormId(0x800);
    const SKY: FormId = FormId(0x801);

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn script_fog_shows_in_the_players_interior_but_not_a_sky_lit_one() {
        let dir = std::env::temp_dir().join(format!(
            "vrm-cell-fog-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&dir).unwrap();
        let records = [Node::top(
            b"CELL",
            vec![
                RecordBuilder::new(b"CELL", LIT).field(b"DATA", &[1, 0]),
                RecordBuilder::new(b"CELL", SKY).field(b"DATA", &[0x81, 0]),
            ],
        )];
        std::fs::write(
            dir.join("Fog.esp"),
            write_plugin(0, &[], "test", "synthetic fog", 0x900, &records),
        )
        .unwrap();
        let mut e = crate::stats::action_tests::headless_engine(&dir, "Fog.esp");
        let _ = std::fs::remove_dir_all(&dir);
        e.location = crate::engine::Location::Interior(LIT);
        let lit = e.object_value(LIT);
        let call = |e: &mut crate::engine::Engine, c: &Value, f: &str, args: &[Value]| {
            crate::script::natives::call(e, "cell", f, Some(c), args);
        };
        // Swapped planes come out near first.
        call(
            &mut e,
            &lit,
            "setfogplanes",
            &[Value::Float(900.0), Value::Float(100.0)],
        );
        assert_eq!((e.scene.env.fog_near, e.scene.env.fog_far), (100.0, 900.0));
        let ints = [255, 0, 0, 0, 0, 300].map(Value::Int);
        call(&mut e, &lit, "setfogcolor", &ints);
        assert_eq!(e.scene.env.fog_near_color, glam::Vec3::X);
        assert_eq!(e.scene.env.fog_far_color, glam::Vec3::Z);
        call(&mut e, &lit, "setfogpower", &[Value::Float(2.0)]);
        assert_eq!(e.scene.env.fog_power, 2.0);
        // A sky-lit interior keeps its weather's fog.
        e.location = crate::engine::Location::Interior(SKY);
        let before = e.scene.env.fog_near;
        let sky = e.object_value(SKY);
        call(
            &mut e,
            &sky,
            "setfogplanes",
            &[Value::Float(5.0), Value::Float(6.0)],
        );
        assert_eq!(e.scene.env.fog_near, before);
    }

    #[test]
    fn channels_clip_to_the_byte_range() {
        assert_eq!(super::channel(-5), 0.0);
        assert_eq!(super::channel(255), 1.0);
        assert_eq!(super::channel(400), 1.0);
    }
}
