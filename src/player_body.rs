//! The player's body: their race's skeleton, skin, head parts and what they
//! wear, running the race's behaviour graph as the player moves. Drawn in the
//! third-person view; seen from inside it in first person, it only casts a
//! shadow.

use std::sync::Arc;

use esp::FormId;
use glam::{Mat4, Quat, Vec3};

use crate::engine::{Engine, PLAYER_REF};
use crate::world::behavior::GraphAnim;
use crate::world::skeleton::Skeleton;

/// The player's base record.
const PLAYER_NPC: FormId = FormId(0x7);

/// How quickly the body turns to face where it goes in third person (radians
/// per second).
const TURN_RATE: f32 = 10.0;

pub struct PlayerBody {
    skeleton: Arc<Skeleton>,
    graph: Option<GraphAnim>,
    scale: f32,
    /// Facing, clockwise from +Y as the camera's yaw.
    pub heading: f32,
    graph_heading: f32,
    /// What they had equipped when it was built: rebuilt when that changes.
    worn: Vec<FormId>,
    /// The graph's movement type states (`iState`): default and sneaking.
    state: Option<f32>,
    sneak_state: Option<f32>,
    moving: bool,
    sneaking: bool,
}

fn wrap(a: f32) -> f32 {
    (a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

impl Engine {
    /// The player's equipped items.
    fn player_worn(&self) -> Vec<FormId> {
        self.inventories
            .get(&PLAYER_REF)
            .map(|i| i.equipped.clone())
            .unwrap_or_default()
    }

    /// Assemble the player's body from their record and what they wear,
    /// keeping its facing.
    pub(crate) fn build_player_body(&mut self) {
        let heading = self
            .player_body
            .as_ref()
            .map_or(self.camera.yaw, |b| b.heading);
        self.player_body = None;
        self.scene.player = None;
        let inventory = self.inventory_mut(PLAYER_REF).clone();
        let Some(d) =
            crate::world::actor::describe_player(&self.lo, PLAYER_REF, PLAYER_NPC, &inventory)
        else {
            log::warn!("the player's body can't be assembled");
            return;
        };
        let paths: Vec<String> = d.models.iter().map(|(m, _)| m.clone()).collect();
        self.models.load_all(&mut self.renderer, &self.vfs, &paths);
        let Some(skel) = self.skeleton(&d.skeleton) else {
            return;
        };
        let mut body = PlayerBody {
            skeleton: skel.clone(),
            graph: None,
            scale: d.transform.x_axis.length(),
            heading,
            graph_heading: f32::NAN,
            worn: inventory.equipped.clone(),
            state: None,
            sneak_state: None,
            moving: false,
            sneaking: false,
        };
        let mut pose = skel.model_space(&skel.bind_locals());
        if let Some(project) = d
            .behavior
            .as_deref()
            .and_then(|p| self.graphs.project(&self.vfs, p))
        {
            let character = project
                .shared
                .project
                .character
                .as_ref()
                .map_or("", |c| c.name.as_str());
            let (default, sneak) =
                crate::world::movement::graph_movement_types(project.shared.variables(), character);
            body.state = default.map(|(_, v)| v);
            body.sneak_state = sneak.map(|(_, v)| v);
            let mut g = GraphAnim::new(project, &d.skeleton, d.female, &skel, self.rng);
            g.label = "player".into();
            for (var, value) in [("IsNPC", 0.0), ("i1stPerson", 0.0), ("IsFirstPerson", 0.0)] {
                g.set_variable(var, value);
            }
            let weight = self.lo.get(PLAYER_NPC).and_then(|r| {
                r.get(b"NAM7")
                    .filter(|b| b.len() >= 4)
                    .map(|b| f32::from_le_bytes(b[0..4].try_into().unwrap()))
            });
            g.set_variable("weapAdj", weight.unwrap_or(50.0) / 100.0);
            g.set_variable(
                "iLeftHandType",
                inventory.hand(&self.lo, true) as i32 as f32,
            );
            g.set_variable(
                "iRightHandType",
                inventory.hand(&self.lo, false) as i32 as f32,
            );
            pose = g.update(0.0, &self.vfs, &mut self.anims, &skel).pose;
            body.graph = Some(g);
        }
        let (meshes, equipment, _) = self.actor_meshes(&d, &skel);
        log::info!(
            "player body: {} meshes, {} rigid, {}",
            meshes.len(),
            equipment.len(),
            if body.graph.is_some() {
                "behaviour graph"
            } else {
                "no behaviour graph"
            }
        );
        self.scene.player = Some(crate::render::ActorInstance {
            meshes,
            attachments: Vec::new(),
            equipment,
            held_light: None,
            transform: body.transform(self.player_feet()),
            pose,
            lights: [0xFFFF; 8],
            radius: 120.0,
            shadow_only: !self.third_person,
        });
        self.player_body = Some(body);
    }

    /// Place and animate the body after the player moved `moved` this update.
    pub(crate) fn update_player_body(&mut self, moved: Vec3, dt: f32) {
        if self.player_body.is_none()
            || self.player_body.as_ref().unwrap().worn != self.player_worn()
        {
            self.build_player_body();
        }
        let feet = self.player_feet();
        let third = self.third_person;
        let yaw = self.camera.yaw;
        let sneaking = self.player.sneaking;
        let speed = if dt > 0.0 {
            moved.truncate().length() / dt
        } else {
            0.0
        };
        let walking = self.player.moving && !self.player.noclip;
        let Some(body) = self.player_body.as_mut() else {
            return;
        };
        // In first person they face the view; in third person where they go.
        if !third {
            body.heading = yaw;
        } else if walking && moved.truncate().length_squared() > 1e-4 {
            let want = moved.x.atan2(moved.y);
            let d = wrap(want - body.heading);
            let step = TURN_RATE * dt;
            body.heading = wrap(body.heading + d.clamp(-step, step));
        }
        let pose = body.animate(dt, walking, speed, sneaking, &self.vfs, &mut self.anims);
        let transform = body.transform(feet);
        if let Some(inst) = self.scene.player.as_mut() {
            inst.transform = transform;
            inst.shadow_only = !third;
            if let Some(pose) = pose {
                inst.pose = pose;
            }
        }
    }
}

impl PlayerBody {
    fn transform(&self, feet: Vec3) -> Mat4 {
        Mat4::from_scale_rotation_translation(
            Vec3::splat(self.scale),
            Quat::from_rotation_z(-self.heading),
            feet,
        )
    }

    /// Drive the graph as the AI does an NPC's (`Speed`, `TurnDelta`,
    /// `moveStart` / `moveStop`, sneaking) and run it.
    fn animate(
        &mut self,
        dt: f32,
        walking: bool,
        speed: f32,
        sneaking: bool,
        vfs: &vfs::Vfs,
        anims: &mut crate::world::animation::AnimationLibrary,
    ) -> Option<Vec<Mat4>> {
        let turned = if self.graph_heading.is_finite() {
            wrap(self.heading - self.graph_heading)
        } else {
            0.0
        };
        self.graph_heading = self.heading;
        let g = self.graph.as_mut()?;
        if sneaking != self.sneaking {
            g.send_event(if sneaking { "SneakStart" } else { "SneakStop" });
            self.sneaking = sneaking;
        }
        let state = if sneaking {
            self.sneak_state.or(self.state)
        } else {
            self.state
        };
        if let Some(s) = state {
            g.set_variable("iState", s);
        }
        g.set_variable("Speed", if walking { speed } else { 0.0 });
        // Degrees per second, counter-clockwise positive as Havok has it.
        g.set_variable(
            "TurnDelta",
            if dt > 1e-4 {
                -turned.to_degrees() / dt
            } else {
                0.0
            },
        );
        if walking != self.moving {
            let ev = if walking { "moveStart" } else { "moveStop" };
            let took = g.send_event(ev);
            log::debug!(
                "player body: {ev}{}",
                if took {
                    ""
                } else {
                    " (the graph won't take it)"
                }
            );
            if walking {
                g.send_event("SprintStop");
            }
            self.moving = walking;
        }
        Some(g.update(dt, vfs, anims, &self.skeleton).pose)
    }
}
