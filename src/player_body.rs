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
    sprinting: bool,
    /// Off the ground this long; whether the graph was told they fall (or jumped).
    airborne: Option<f32>,
    fell: bool,
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
            sprinting: false,
            airborne: None,
            fell: false,
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
        let flat = moved.truncate();
        let walking = self.player.moving && !self.player.noclip;
        let motion = Motion {
            walking,
            speed: if dt > 0.0 { flat.length() / dt } else { 0.0 },
            heading: (flat.length_squared() > 1e-4).then(|| flat.x.atan2(flat.y)),
            sneaking: self.player.sneaking,
            sprinting: walking && self.player.sprinting,
            jumped: self.player.jumped && !self.player.noclip,
            grounded: self.player.grounded || self.player.noclip,
        };
        let Some(body) = self.player_body.as_mut() else {
            return;
        };
        // In first person they face the view; in third person where they go.
        if !third {
            body.heading = yaw;
        } else if let Some(want) = motion.heading.filter(|_| walking) {
            let d = wrap(want - body.heading);
            let step = TURN_RATE * dt;
            body.heading = wrap(body.heading + d.clamp(-step, step));
        }
        let pose = body.animate(dt, &motion, &self.vfs, &mut self.anims);
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

/// How the player moved this update, for the graph.
struct Motion {
    walking: bool,
    /// Over the ground, units per second.
    speed: f32,
    /// Which way they went (clockwise from +Y), if they moved.
    heading: Option<f32>,
    sneaking: bool,
    sprinting: bool,
    /// Left the ground jumping this update.
    jumped: bool,
    grounded: bool,
}

/// How long off the ground, not having jumped, before they fall.
const FALL_AFTER: f32 = 0.3;

impl PlayerBody {
    fn transform(&self, feet: Vec3) -> Mat4 {
        Mat4::from_scale_rotation_translation(
            Vec3::splat(self.scale),
            Quat::from_rotation_z(-self.heading),
            feet,
        )
    }

    fn event(g: &mut GraphAnim, ev: &str) -> bool {
        let took = g.send_event(ev);
        log::debug!(
            "player body: {ev}{}",
            if took {
                ""
            } else {
                " (the graph won't take it)"
            }
        );
        took
    }

    /// Drive the graph as the AI does an NPC's (`Speed`, `TurnDelta`,
    /// `moveStart` / `moveStop`, sneaking), with the player's own: `Direction`
    /// (where they go against where they face: strafing and walking backwards),
    /// sprinting and jumping, falling and landing; and run it.
    fn animate(
        &mut self,
        dt: f32,
        m: &Motion,
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
        if m.sneaking != self.sneaking {
            g.send_event(if m.sneaking {
                "SneakStart"
            } else {
                "SneakStop"
            });
            self.sneaking = m.sneaking;
        }
        let state = if m.sneaking {
            self.sneak_state.or(self.state)
        } else {
            self.state
        };
        if let Some(s) = state {
            g.set_variable("iState", s);
        }
        g.set_variable("Speed", if m.walking { m.speed } else { 0.0 });
        // A fraction of a turn clockwise from ahead, as the locomotion blends
        // have it (0 forward, 0.25 right, 0.5 back, 0.75 left).
        if let Some(h) = m.heading.filter(|_| m.walking) {
            let d = wrap(h - self.heading) / std::f32::consts::TAU;
            g.set_variable("Direction", d.rem_euclid(1.0));
        }
        // Degrees per second, counter-clockwise positive as Havok has it.
        g.set_variable(
            "TurnDelta",
            if dt > 1e-4 {
                -turned.to_degrees() / dt
            } else {
                0.0
            },
        );
        // Off the ground: a jump (standing or on the move) or, after a moment
        // without one, a fall; back on it, the landing, after which the graph
        // stands again until told they move.
        if m.jumped && self.airborne.is_none() {
            Self::event(
                g,
                if m.walking {
                    "JumpDirectionalStart"
                } else {
                    "JumpStandingStart"
                },
            );
            self.airborne = Some(0.0);
            self.fell = true;
        } else if let Some(t) = self.airborne.as_mut() {
            *t += dt;
            if m.grounded {
                // Only a jump or a fall the graph knows of ends in a landing
                // (not a step down a slope).
                if self.fell {
                    Self::event(
                        g,
                        if m.walking {
                            "JumpLandDirectional"
                        } else {
                            "JumpLand"
                        },
                    );
                    self.moving = false;
                    self.sprinting = false;
                }
                self.airborne = None;
            } else if !self.fell && *t >= FALL_AFTER {
                Self::event(g, "JumpFall");
                self.fell = true;
            }
        } else if !m.grounded {
            self.airborne = Some(0.0);
            self.fell = false;
        }
        if self.airborne.is_none() || !self.fell {
            if m.walking != self.moving {
                Self::event(g, if m.walking { "moveStart" } else { "moveStop" });
                self.moving = m.walking;
            }
            let sprint = m.sprinting && m.walking;
            if sprint != self.sprinting {
                Self::event(g, if sprint { "SprintStart" } else { "SprintStop" });
                self.sprinting = sprint;
            }
        }
        Some(g.update(dt, vfs, anims, &self.skeleton).pose)
    }
}

/// The third-person camera's distance behind the eye: nearest, farthest and
/// at first (chosen by eye; the game's come from its INI settings).
pub const THIRD_PERSON_MIN: f32 = 60.0;
pub const THIRD_PERSON_MAX: f32 = 600.0;
pub const THIRD_PERSON_DEFAULT: f32 = 200.0;
/// How far short of a wall the camera stops.
const CAMERA_CLEARANCE: f32 = 12.0;

impl Engine {
    /// The camera the frame is drawn from: the eye in first person, else pulled
    /// back behind it along the view, short of whatever is in the way (the
    /// world, not actors).
    pub fn view_camera(&self) -> crate::render::Camera {
        let mut cam = self.camera_copy();
        if self.third_person {
            let back = -cam.forward();
            let want = self.third_person_distance;
            let dist = self
                .physics
                .ground_ray(cam.position, back, want + CAMERA_CLEARANCE)
                .map_or(want, |(t, _)| (t - CAMERA_CLEARANCE).clamp(0.0, want));
            cam.position += back * dist;
        }
        cam
    }

    /// Switch between first and third person.
    pub fn set_third_person(&mut self, on: bool) {
        if self.third_person != on {
            log::info!("{} person", if on { "third" } else { "first" });
        }
        self.third_person = on;
    }

    /// Move the third-person camera nearer (negative) or farther.
    pub fn zoom_third_person(&mut self, by: f32) {
        self.third_person_distance =
            (self.third_person_distance + by).clamp(THIRD_PERSON_MIN, THIRD_PERSON_MAX);
    }
}
