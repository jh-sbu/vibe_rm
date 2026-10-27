//! Gameplay queries on cell water. The loader resolves CELL XCWT, then WRLD
//! NAM2 (including parent-world inheritance), then DefaultWater, for both
//! rendering and these surfaces. WATR FNAM bit 0 marks dangerous water;
//! see `known_ambiguities/water.md` for contact and processing limitations.

use esp::FormId;
use glam::{Vec2, Vec3};

use crate::engine::Engine;

#[derive(Debug, Clone, Copy)]
pub(crate) struct WaterSurface {
    pub corner: Vec2,
    pub size: f32,
    pub height: f32,
    pub water_type: FormId,
}

/// Half-open bounds give the cell to the east/north its own boundary, as
/// grid_of does. Height is deliberately ignored: callers may query above water.
pub(crate) fn surface_at<'a>(
    surfaces: impl IntoIterator<Item = &'a WaterSurface>,
    at: Vec3,
) -> Option<&'a WaterSurface> {
    surfaces
        .into_iter()
        .filter(|w| {
            let d = at.truncate() - w.corner;
            (0.0..w.size).contains(&d.x) && (0.0..w.size).contains(&d.y)
        })
        .max_by(|a, b| a.height.total_cmp(&b.height))
}

fn touching_surface_at(surfaces: &[WaterSurface], feet: Vec3) -> Option<&WaterSurface> {
    surface_at(surfaces, feet).filter(|w| feet.z < w.height)
}

fn dangerous_water_at(lo: &esp::LoadOrder, surfaces: &[WaterSurface], feet: Vec3) -> bool {
    touching_surface_at(surfaces, feet)
        .and_then(|w| lo.get(w.water_type))
        .filter(|r| r.tag().0 == *b"WATR")
        .and_then(|r| r.get(b"FNAM").and_then(|d| d.first()).copied())
        .is_some_and(|flags| flags & 1 != 0)
}

/// Provisional feet depth until autotest20; the float height is observed in SE.
pub(crate) const SWIM_DEPTH: f32 = 100.0;
pub(crate) const FLOAT_DEPTH: f32 = 92.0;
const SWIM_HYSTERESIS: f32 = 4.0;

fn swim_state(surface: Option<f32>, feet: f32, bed: Option<f32>, was: bool) -> bool {
    let Some(surface) = surface else { return false };
    if was {
        // Floating feet are shallower than the entry threshold. The bed, not
        // the floated pose, determines leaving the swim state at the shore.
        surface - feet >= FLOAT_DEPTH - SWIM_HYSTERESIS
            && bed.is_none_or(|z| surface - z >= SWIM_DEPTH - SWIM_HYSTERESIS)
    } else {
        surface - feet > SWIM_DEPTH
    }
}

impl Engine {
    /// The water surface height at a point in the currently loaded space.
    pub(crate) fn water_level(&self, at: Vec3) -> Option<f32> {
        surface_at(self.cells.values().flat_map(|c| &c.water), at).map(|w| w.height)
    }

    pub(crate) fn is_swimming(&self, actor: FormId) -> bool {
        if actor == crate::engine::PLAYER_REF {
            self.player.swimming
        } else {
            self.actor_ref(actor).is_some_and(|a| a.swimming)
        }
    }

    pub(crate) fn update_swimming_states(&mut self) {
        let states: Vec<_> = self
            .cells
            .values()
            .flat_map(|rt| &rt.actors)
            .filter(|a| !a.dead)
            .map(|a| {
                let feet = a.feet();
                let surface = self.water_level(feet);
                let bed = surface.and_then(|h| {
                    self.physics
                        .ground_ray(feet.with_z(h + 1.0), -Vec3::Z, 8192.0)
                        .map(|(t, _)| h + 1.0 - t)
                });
                (a.ref_id, swim_state(surface, feet.z, bed, a.swimming))
            })
            .collect();
        for (actor, swimming) in states {
            if let Some(a) = self.actor_mut(actor) {
                a.swimming = swimming;
            }
        }
        let feet = self.player_feet();
        let surface = self.water_level(feet);
        let bed = surface.and_then(|h| {
            self.physics
                .ground_ray(feet.with_z(h + 1.0), -Vec3::Z, 8192.0)
                .map(|(t, _)| h + 1.0 - t)
        });
        self.player.swimming =
            !self.player.noclip && swim_state(surface, feet.z, bed, self.player.swimming);
        self.player.swim_surface = surface;
    }

    /// An actor's feet touch water whose WATR has the Causes Damage flag.
    /// This is also true when wading; swimming is a separate condition.
    pub(crate) fn is_in_dangerous_water(&self, actor: FormId) -> bool {
        if !self
            .base_of(actor)
            .and_then(|b| self.lo.tag_of(b))
            .is_some_and(|t| t.0 == *b"NPC_")
        {
            return false;
        }
        let Some((place, feet)) = self.current_place_of(actor) else {
            return false;
        };
        // A loaded actor's owning cell can lag its position while crossing a
        // cell boundary. Use its current coordinates in the same space.
        let Some(key) = self.key_of_place(Self::place_at(place, feet)) else {
            return false;
        };
        self.cells
            .get(&key)
            .is_some_and(|c| dangerous_water_at(&self.lo, &c.water, feet))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn surface(x: f32, height: f32, water_type: u32) -> WaterSurface {
        WaterSurface {
            corner: Vec2::new(x, 0.0),
            size: 4096.0,
            height,
            water_type: FormId(water_type),
        }
    }

    #[test]
    fn adjacent_cells_keep_their_water_type_and_height_at_the_boundary() {
        let surfaces = [surface(-4096.0, 200.0, 1), surface(0.0, 100.0, 2)];
        let west = surface_at(&surfaces, Vec3::new(-0.01, 10.0, 1000.0)).unwrap();
        assert_eq!((west.height, west.water_type), (200.0, FormId(1)));
        let east = surface_at(&surfaces, Vec3::new(0.0, 10.0, 1000.0)).unwrap();
        assert_eq!((east.height, east.water_type), (100.0, FormId(2)));
        assert!(surface_at(&surfaces, Vec3::new(4096.0, 10.0, 0.0)).is_none());
        assert!(surface_at(&surfaces, Vec3::new(0.0, -0.01, 0.0)).is_none());
        assert!(surface_at(&surfaces, Vec3::new(0.0, 4096.0, 0.0)).is_none());
    }

    #[test]
    fn overlapping_surfaces_select_the_top_surface_and_its_type() {
        let surfaces = [surface(0.0, 100.0, 1), surface(0.0, 200.0, 2)];
        let top = surface_at(&surfaces, Vec3::new(10.0, 10.0, 50.0)).unwrap();
        assert_eq!((top.height, top.water_type), (200.0, FormId(2)));
        assert!(surface_at([], Vec3::ZERO).is_none());
    }

    #[test]
    fn wading_touches_water_but_dry_feet_do_not() {
        let surfaces = [surface(0.0, 100.0, 1)];
        for z in [99.99, 0.0] {
            assert!(touching_surface_at(&surfaces, Vec3::new(10.0, 10.0, z)).is_some());
        }
        for z in [100.0, 100.01, 1000.0] {
            assert!(touching_surface_at(&surfaces, Vec3::new(10.0, 10.0, z)).is_none());
        }
        assert!(touching_surface_at(&surfaces, Vec3::new(-1.0, 10.0, 0.0)).is_none());
    }

    #[test]
    fn synthetic_water_flags_distinguish_dangerous_from_ordinary_water() {
        use esp::writer::{Node, RecordBuilder, write_plugin};

        struct Fixture(std::path::PathBuf);
        impl Drop for Fixture {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = Fixture(
            std::env::temp_dir().join(format!("vrm-water-flags-{}-{stamp}", std::process::id())),
        );
        std::fs::create_dir(&dir.0).unwrap();
        let water = |id| RecordBuilder::new(b"WATR", FormId(id));
        let groups = [
            Node::top(
                b"WATR",
                vec![
                    water(0x800).field(b"FNAM", &[1]),
                    water(0x801).field(b"FNAM", &[0]),
                    water(0x802).field(b"FNAM", &[2]),
                    water(0x803),
                    water(0x804).field(b"FNAM", &[]),
                    water(0x806).field(b"FNAM", &[3]),
                ],
            ),
            Node::top(
                b"STAT",
                vec![RecordBuilder::new(b"STAT", FormId(0x805)).field(b"FNAM", &[1])],
            ),
        ];
        std::fs::write(
            dir.0.join("Water.esp"),
            write_plugin(0, &[], "test", "synthetic water flags", 0x808, &groups),
        )
        .unwrap();
        let lo = esp::LoadOrder::load(&dir.0, &["Water.esp".into()]).unwrap();
        let mut surfaces = [surface(0.0, 100.0, 0x800)];
        assert!(dangerous_water_at(
            &lo,
            &surfaces,
            Vec3::new(10.0, 10.0, 99.0)
        ));
        assert!(!dangerous_water_at(
            &lo,
            &surfaces,
            Vec3::new(10.0, 10.0, 101.0)
        ));
        surfaces[0].water_type = FormId(0x806);
        assert!(dangerous_water_at(
            &lo,
            &surfaces,
            Vec3::new(10.0, 10.0, 99.0)
        ));
        // Ordinary/other flags, absent/empty fields, a non-water record,
        // a missing record and a null reference must all be safe.
        for id in [0x801, 0x802, 0x803, 0x804, 0x805, 0x807, 0] {
            surfaces[0].water_type = FormId(id);
            assert!(
                !dangerous_water_at(&lo, &surfaces, Vec3::new(10.0, 10.0, 99.0)),
                "water type {id:08X}"
            );
        }
    }
    #[test]
    fn swim_depth_has_hysteresis_and_uses_the_bed_while_floating() {
        assert!(!swim_state(Some(200.0), 120.0, Some(120.0), false));
        assert!(!swim_state(Some(200.0), 100.0, Some(100.0), false));
        assert!(swim_state(Some(200.0), 99.0, Some(99.0), false));
        assert!(swim_state(Some(200.0), 108.0, Some(20.0), true));
        assert!(swim_state(Some(200.0), 108.0, Some(103.0), true));
        assert!(!swim_state(Some(200.0), 108.0, Some(105.0), true));
        assert!(!swim_state(None, 0.0, None, true));
        assert!(!swim_state(Some(200.0), 300.0, Some(0.0), false));
    }
}
