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
fn surface_at<'a>(
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

impl Engine {
    /// The water surface height at a point in the currently loaded space.
    pub(crate) fn water_level(&self, at: Vec3) -> Option<f32> {
        surface_at(self.cells.values().flat_map(|c| &c.water), at).map(|w| w.height)
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
    fn installed_water_flags_distinguish_apocrypha_from_default_water() {
        // Optional local asset check; no game data is copied into the repo.
        let Some(data) = vfs::locate_data_dir() else {
            return;
        };
        let names = esp::LoadOrder::default_plugin_list(&data, None);
        let lo = esp::LoadOrder::load(&data, &names).unwrap();
        let Some(apocrypha) = lo.find_editor_id("DLC2ApocryphaWater") else {
            return;
        };
        let ordinary = lo.find_editor_id("DefaultWater").unwrap();
        let mut surfaces = [surface(0.0, 100.0, apocrypha.0)];
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
        surfaces[0].water_type = ordinary;
        assert!(!dangerous_water_at(
            &lo,
            &surfaces,
            Vec3::new(10.0, 10.0, 99.0)
        ));
        surfaces[0].water_type = FormId::NULL;
        assert!(!dangerous_water_at(
            &lo,
            &surfaces,
            Vec3::new(10.0, 10.0, 99.0)
        ));
    }
}
