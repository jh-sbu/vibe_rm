//! Footstep sounds: actors' graphs raise their footstep events (`FootLeft`,
//! `FootRight`, `FootFront`...) as feet come down; the player's steps are paced by
//! the distance walked. Each sounds as the walker's footstep set has it on the
//! material of the ground below.

use std::collections::HashMap;
use std::sync::Arc;

use esp::FormId;
use glam::Vec3;

use crate::engine::{Engine, PLAYER_REF};
use crate::physics::Surface;
use crate::world::footsteps::{self, FootstepSet, Gait, Impacts};
use crate::world::terrain::CELL_SIZE;

/// Footsteps further than this from the listener aren't played.
pub const HEARING: f32 = 2500.0;

/// The player's stride (game units per footstep) walking, running and sprinting.
const STRIDES: [f32; 3] = [70.0, 110.0, 145.0];

/// Strides sneaking are this much shorter.
const SNEAK_STRIDE: f32 = 0.8;

/// Seconds off the ground that count as a jump or a fall rather than a stumble.
const MAX_AIRBORNE: f32 = 0.3;

/// Feet further under water than this are swimming, not wading: no footsteps.
const WADE_DEPTH: f32 = 100.0;

/// Water's material type.
const WATER: FormId = FormId(0x12F40);

#[derive(Default)]
pub struct Footsteps {
    sets: HashMap<FormId, Option<Arc<FootstepSet>>>,
    impacts: Impacts,
    /// Material types by Havok material id (built on first use).
    materials: std::cell::OnceCell<HashMap<u32, FormId>>,
    /// The player's footstep set, for what they were wearing.
    player_set: Option<(Vec<FormId>, Option<Arc<FootstepSet>>)>,
    /// Distance the player walked since their last step, which foot is next, and
    /// how long they have been off the ground (-inf until first on it).
    stride: f32,
    right: bool,
    pub(crate) airborne: f32,
}

/// A footstep to sound: its event (lowercase), the gait, where the foot came down,
/// and the walker's footstep set.
pub struct Step {
    pub tag: String,
    pub gait: Gait,
    pub at: Vec3,
    pub set: Arc<FootstepSet>,
}

impl Engine {
    pub(crate) fn footstep_set(&mut self, fsts: FormId) -> Option<Arc<FootstepSet>> {
        let lo = &self.lo;
        self.footsteps
            .sets
            .entry(fsts)
            .or_insert_with(|| FootstepSet::load(lo, fsts).map(Arc::new))
            .clone()
    }

    /// The material type (MATT) of the ground just below a point: water for feet
    /// in shallow water, nothing for ones in deep water.
    pub(crate) fn ground_material(&self, at: Vec3) -> Option<FormId> {
        if let Some(h) = self.water_level(at)
            && h > at.z
        {
            return (h - at.z < WADE_DEPTH).then_some(WATER);
        }
        let (_, surface) = self.physics.surface_below(at + Vec3::Z * 32.0, 96.0)?;
        Some(self.surface_material(surface, at))
    }

    /// The height of the water surface over a point, if any loaded cell has water there.
    pub(crate) fn water_level(&self, at: Vec3) -> Option<f32> {
        self.cells
            .values()
            .flat_map(|c| &c.water)
            .filter(|(corner, size, _)| {
                let d = at.truncate() - *corner;
                (0.0..=*size).contains(&d.x) && (0.0..=*size).contains(&d.y)
            })
            .map(|w| w.2)
            .reduce(f32::max)
    }

    /// The material type (MATT) of a surface at a point: the collision's Havok
    /// material, or the landscape texture showing most there.
    pub(crate) fn surface_material(&self, surface: Surface, at: Vec3) -> FormId {
        match surface {
            Surface::Havok(id) => {
                let materials = self
                    .footsteps
                    .materials
                    .get_or_init(|| footsteps::havok_materials(&self.lo));
                materials.get(&id).copied().unwrap_or(footsteps::STONE)
            }
            Surface::Terrain => self
                .cells
                .values()
                .filter_map(|c| c.land.as_ref())
                .find(|l| {
                    let d = at.truncate() - l.origin();
                    (0.0..=CELL_SIZE).contains(&d.x) && (0.0..=CELL_SIZE).contains(&d.y)
                })
                .and_then(|l| l.texture_at(at.truncate()))
                .and_then(|t| footsteps::land_material(&self.lo, t))
                .unwrap_or(footsteps::DIRT),
        }
    }

    pub(crate) fn play_footstep(&mut self, step: &Step) {
        let Some(ipds) = step.set.impacts(&step.tag, step.gait) else {
            return;
        };
        let Some(material) = self.ground_material(step.at) else {
            return;
        };
        let sound = self.footsteps.impacts.sound(&self.lo, ipds, material);
        if log::log_enabled!(target: "footsteps", log::Level::Trace) {
            let surface = self
                .physics
                .surface_below(step.at + Vec3::Z * 32.0, 96.0)
                .map(|s| s.1);
            let name = self
                .lo
                .get(material)
                .and_then(|r| r.editor_id())
                .unwrap_or_default();
            log::trace!(target: "footsteps", "{} {:?} at {:.0} on {name} ({surface:?}): {sound:?}", step.tag, step.gait, step.at);
        }
        if let Some(s) = sound {
            self.play_sound(s, step.at);
        }
    }

    /// The player's footsteps: one each stride walked on the ground (`moved`:
    /// horizontal distance this frame), and their jumps and landings. The character
    /// controller loses the ground for a frame now and then going over uneven
    /// meshes; only a while in the air (a jump, a fall) starts the stride over.
    pub(crate) fn update_player_footsteps(&mut self, dt: f32, moved: f32, run: bool, sprint: bool) {
        if self.player.noclip {
            return;
        }
        let gait = if self.player.sneaking {
            Gait::Sneak
        } else if sprint {
            Gait::Sprint
        } else if run {
            Gait::Run
        } else {
            Gait::Walk
        };
        if self.player.jumped {
            self.player_step("jumpup", gait);
        }
        if self.player.grounded {
            if self.footsteps.airborne > MAX_AIRBORNE {
                self.player_step("jumpdown", gait);
            }
            self.footsteps.airborne = 0.0;
        } else {
            self.footsteps.airborne += dt;
            if self.footsteps.airborne > MAX_AIRBORNE {
                self.footsteps.stride = 0.0;
                return;
            }
        }
        if moved <= 0.0 {
            return;
        }
        self.footsteps.stride += moved;
        let stride = match gait {
            Gait::Sprint => STRIDES[2],
            Gait::Run => STRIDES[1],
            Gait::Sneak if run => STRIDES[1] * SNEAK_STRIDE,
            _ if self.player.sneaking => STRIDES[0] * SNEAK_STRIDE,
            _ => STRIDES[0],
        };
        if self.footsteps.stride < stride {
            return;
        }
        self.footsteps.stride -= stride;
        self.footsteps.right = !self.footsteps.right;
        let foot = if self.footsteps.right {
            "right"
        } else {
            "left"
        };
        let sprinting = format!("footsprint{foot}");
        if gait == Gait::Sprint
            && self
                .player_footstep_set()
                .is_some_and(|s| s.has_tag(&sprinting))
        {
            self.player_step(&sprinting, gait);
        } else {
            self.player_step(&format!("foot{foot}"), gait);
        }
    }

    /// Sound one of the player's footstep events at their feet.
    fn player_step(&mut self, tag: &str, gait: Gait) {
        let Some(set) = self.player_footstep_set() else {
            return;
        };
        let at = self.player.position
            - Vec3::Z * (self.physics.player_half_height + self.physics.player_radius);
        self.play_footstep(&Step {
            tag: tag.to_owned(),
            gait,
            at,
            set,
        });
    }

    /// The footstep set of what the player wears (worked out again when that changes).
    fn player_footstep_set(&mut self) -> Option<Arc<FootstepSet>> {
        let worn = self
            .inventories
            .get(&PLAYER_REF)
            .map(|i| i.equipped.clone())
            .unwrap_or_default();
        if let Some((w, set)) = &self.footsteps.player_set
            && *w == worn
        {
            return set.clone();
        }
        let race = self.npc_race(FormId(0x7)).unwrap_or(FormId(0x13746));
        let skin = self.lo.get(race).and_then(|r| {
            r.get(b"WNAM")
                .map(|d| r.fid(FormId(u32::from_le_bytes(d[0..4].try_into().unwrap()))))
        });
        let fsts =
            crate::world::actor::footstep_set(&self.lo, &worn, skin.unwrap_or_default(), race);
        let set = fsts.and_then(|f| self.footstep_set(f));
        self.footsteps.player_set = Some((worn, set.clone()));
        set
    }
}
