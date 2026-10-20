//! Navmesh obstacles: references whose base is flagged `Obstacle` (record flag
//! 0x02000000; CommonLibSSE's `kObstacle`, UESP's STAT / CONT / MSTT pages) —
//! portcullises, movable shelves, barricades — block the navmesh triangles
//! their collision stands on, so paths don't run through a closed gate. Most
//! are also flagged `Filter` (navmesh generation ignores them), so the navmesh
//! runs on under them for when they are out of the way: a raised portcullis's
//! bars no longer reach down into the triangles, and they are free again.

use esp::FormId;
use esp::record_flags::OBSTACLE;
use glam::Vec3;
use rapier3d::prelude::{ColliderHandle, Pose, SharedShape};

use crate::engine::Engine;

/// The slab above each triangle the obstacle's collision must reach into to
/// block it: above a step (a gate's sill), up to about knee height (bars
/// raised above that let actors under). No source; Skyrim's own test is
/// unknown.
const SLAB: (f32, f32) = (16.0, 96.0);

/// An obstacle in a loaded cell and where its collision was when its triangles
/// were last found.
pub(crate) struct Obstacle {
    ref_id: FormId,
    /// The colliders that move (a gate's bars), or all of them if none do.
    colliders: Vec<ColliderHandle>,
    seen: Option<(u64, Vec<Option<Pose>>)>,
}

impl Obstacle {
    /// An obstacle for a reference of `base` with these colliders (and the
    /// animated node each moves with), if the base is flagged one.
    pub(crate) fn new(
        lo: &esp::LoadOrder,
        ref_id: FormId,
        base: FormId,
        tagged: &[(ColliderHandle, Option<String>)],
    ) -> Option<Obstacle> {
        if tagged.is_empty() || lo.get(base)?.header.flags & OBSTACLE == 0 {
            return None;
        }
        let moving: Vec<ColliderHandle> = tagged
            .iter()
            .filter(|(_, n)| n.is_some())
            .map(|(h, _)| *h)
            .collect();
        Some(Obstacle {
            ref_id,
            colliders: if moving.is_empty() {
                tagged.iter().map(|(h, _)| *h).collect()
            } else {
                moving
            },
            seen: None,
        })
    }
}

/// Whether two sets of collider poses are the same, near enough.
fn same_poses(a: &[Option<Pose>], b: &[Option<Pose>]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(a, b)| match (a, b) {
            (None, None) => true,
            (Some(a), Some(b)) => {
                a.translation.distance(b.translation) < 0.5
                    && a.rotation.angle_between(b.rotation) < 1e-3
            }
            _ => false,
        })
}

impl Engine {
    /// Find again the triangles of obstacles that moved, were enabled or
    /// disabled, or whose navmeshes loaded or unloaded since.
    pub(crate) fn update_nav_obstacles(&mut self) {
        let generation = self.nav.generation();
        for rt in self.cells.values_mut() {
            for o in &mut rt.obstacles {
                let poses = self.physics.collider_poses(&o.colliders);
                if o.seen
                    .as_ref()
                    .is_some_and(|(g, p)| *g == generation && same_poses(p, &poses))
                {
                    continue;
                }
                let blocked = self
                    .physics
                    .colliders_bounds(&o.colliders)
                    .map(|(min, max)| {
                        // Triangles below the collision by up to the slab's top.
                        let lo = min - Vec3::new(0.0, 0.0, SLAB.1);
                        self.nav
                            .triangles_in(lo, max)
                            .into_iter()
                            .filter(|(_, corners)| {
                                let points: Vec<Vec3> = [SLAB.0, SLAB.1]
                                    .iter()
                                    .flat_map(|dz| corners.map(|c| c + Vec3::Z * dz))
                                    .collect();
                                SharedShape::convex_hull(&points).is_some_and(|slab| {
                                    self.physics.colliders_intersect(
                                        &o.colliders,
                                        &*slab.0,
                                        &Pose::IDENTITY,
                                    )
                                })
                            })
                            .map(|(n, _)| n)
                            .collect()
                    })
                    .unwrap_or_default();
                self.nav.set_obstacle(o.ref_id, blocked);
                o.seen = Some((generation, poses));
            }
        }
    }

    /// Forget the triangles of a cell's obstacles (it unloads).
    pub(crate) fn remove_nav_obstacles(&mut self, obstacles: &[Obstacle]) {
        for o in obstacles {
            self.nav.set_obstacle(o.ref_id, Vec::new());
        }
    }
}
