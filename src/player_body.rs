//! The player's body: their race's skeleton, skin, head parts and what they
//! wear, running the race's behaviour graph as the player moves. Drawn in the
//! third-person view; seen from inside it in first person, it only casts a
//! shadow.

use std::sync::Arc;

use esp::FormId;
use glam::{Mat4, Quat, Vec2, Vec3};

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
    worn: (Vec<FormId>, Option<FormId>),
    /// The graph's movement type states (`iState`): default and sneaking.
    state: Option<f32>,
    sneak_state: Option<f32>,
    moving: bool,
    sneaking: bool,
    sprinting: bool,
    /// Off the ground this long; whether the graph was told they fall (or jumped).
    airborne: Option<f32>,
    fell: bool,
    /// The graph took the draw (`WeapEquip`), and the weapon is in hand (its
    /// `weaponDraw` / `weaponSheathe`).
    drawn: bool,
    weapon_out: bool,
    /// The draw is over (`WeapEquip_Out`): the graph swings from here on.
    ready: bool,
    /// The guard is up in the graph (`blockStart`).
    guard: bool,
    /// The graph's `HitFrame`s this update (a swing lands: `HIT_RIGHT`,
    /// `HIT_LEFT`), and whether it raised `attackStop` (a swing is over).
    hit_frames: u8,
    attack_stop: bool,
    /// `Direction` while moving (none standing), and sprinting.
    direction: Option<f32>,
}

fn wrap(a: f32) -> f32 {
    (a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

impl Engine {
    /// The player's equipped items, and the weapon in their left hand.
    fn player_worn(&self) -> (Vec<FormId>, Option<FormId>) {
        self.inventories
            .get(&PLAYER_REF)
            .map_or_else(Default::default, |i| {
                (
                    i.equipped().to_vec(),
                    i.weapon_in(&self.lo, crate::world::inventory::LEFT),
                )
            })
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
            worn: (
                inventory.equipped().to_vec(),
                inventory.weapon_in(&self.lo, crate::world::inventory::LEFT),
            ),
            state: None,
            sneak_state: None,
            moving: false,
            sneaking: false,
            sprinting: false,
            airborne: None,
            fell: false,
            drawn: false,
            weapon_out: false,
            ready: false,
            guard: false,
            hit_frames: 0,
            attack_stop: false,
            direction: None,
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
            let speeds = self.weapon_speed_mults(PLAYER_REF);
            for (var, value) in crate::ai::combat::hand_graph_vars(&self.lo, &inventory, speeds) {
                g.set_variable(var, value);
            }
            pose = g.update(0.0, &self.vfs, &mut self.anims, &skel).pose;
            body.graph = Some(g);
        }
        let (meshes, _, rigid) = self.actor_meshes(&d, &skel);
        self.rigid_models.insert(PLAYER_REF, rigid);
        let (equipment, _) = self.rigid_equipment(PLAYER_REF, &skel, false);
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
            blocking: self.player_blocking,
        };
        let Some(body) = self.player_body.as_mut() else {
            return;
        };
        // In first person, and with their weapon out, they face the view;
        // otherwise in third person where they go.
        if !third || body.drawn {
            body.heading = yaw;
        } else if let Some(want) = motion.heading.filter(|_| walking) {
            let d = wrap(want - body.heading);
            let step = TURN_RATE * dt;
            body.heading = wrap(body.heading + d.clamp(-step, step));
        }
        let out = body.weapon_out;
        let pose = body.animate(dt, &motion, &self.vfs, &mut self.anims);
        let transform = body.transform(feet);
        let hit = std::mem::take(&mut body.hit_frames);
        let stop = std::mem::take(&mut body.attack_stop);
        let (skel, weapon_out) = (body.skeleton.clone(), body.weapon_out);
        if weapon_out != out {
            log::debug!(
                "player's weapon {}",
                if weapon_out { "in hand" } else { "put away" }
            );
            // The weapon goes from its sheath to the hand, or back.
            let (equipment, _) = self.rigid_equipment(PLAYER_REF, &skel, weapon_out);
            if let Some(inst) = self.scene.player.as_mut() {
                inst.equipment = equipment;
            }
        }
        self.update_player_swing(hit, stop, dt);
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
    /// Holding the guard up.
    blocking: bool,
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
        self.direction = m
            .heading
            .filter(|_| m.walking)
            .map(|h| (wrap(h - self.heading) / std::f32::consts::TAU).rem_euclid(1.0));
        if let Some(d) = self.direction {
            g.set_variable("Direction", d);
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
        // The guard goes up with the weapon out (and down when it is put away).
        let guard = m.blocking && self.drawn;
        if guard != self.guard {
            let took = Self::event(g, if guard { "blockStart" } else { "blockStop" });
            if took || !guard {
                self.guard = guard;
                g.set_variable("IsBlocking", if guard { 1.0 } else { 0.0 });
            }
        }
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
        let frame = g.update(dt, vfs, anims, &self.skeleton);
        for r in &frame.raised {
            log::trace!(target: "player_events", "player raised {}{}", r.event, r.payload.as_deref().map(|p| format!(" ({p})")).unwrap_or_default());
            match r.event.to_ascii_lowercase().as_str() {
                "weapondraw" => self.weapon_out = true,
                "weaponsheathe" => self.weapon_out = false,
                "hitframe" => {
                    self.hit_frames |= crate::ai::combat::hit_frame_bit(r.payload.as_deref())
                }
                "attackstop" => self.attack_stop = true,
                "weapequip_out" => self.ready = self.drawn,
                _ => {}
            }
        }
        Some(frame.pose)
    }
}

/// How long a swing the graph took may wait for its `HitFrame`, or for its
/// next one after it lands.
const SWING_TIMEOUT: f32 = 1.5;

/// How long the character graphs blend out of an attack (`AttackState` to
/// the ready state on `attackStop`).
const ATTACK_BLEND_OUT: f32 = 0.4;

/// A swing of the player's waiting for the graph's `HitFrame`s to land.
pub(crate) struct PlayerSwing {
    /// The hands it strikes with, and each one's blow.
    pub side: crate::ai::combat::Side,
    pub blows: [Option<crate::ai::combat::Blow>; 2],
    pub power: bool,
    /// Its attack type keyword (`IsAttackType`).
    pub attack_type: FormId,
    pub stagger: f32,
    /// The stamina each further blow of a power attack costs (UESP's Combat
    /// page: a two-handed power attack's blows "count as three separate power
    /// attacks").
    pub again: f32,
    /// Seconds it has waited (since its last blow).
    pub waited: f32,
    /// Blows landed, and seconds since each hand's last.
    pub strikes: u8,
    pub strike_age: [f32; 2],
}

impl Engine {
    /// Send an event to the player's body's graph; whether it took it.
    pub(crate) fn player_graph_event(&mut self, ev: &str) -> bool {
        self.player_body
            .as_mut()
            .and_then(|b| b.graph.as_mut())
            .is_some_and(|g| PlayerBody::event(g, ev))
    }

    /// Whether the player's body has a graph and their weapon (or fists) is
    /// put away in it.
    pub(crate) fn player_sheathed(&self) -> bool {
        self.player_body
            .as_ref()
            .is_some_and(|b| b.graph.is_some() && !b.drawn)
    }

    /// The race's power attack event for how the player moves: standing
    /// (`attackPowerStartInPlace`), sprinting (`attackPowerStart_Sprint`) or by
    /// the way they go (`...Forward`, `...Right`, `...Backward`, `...Left`).
    pub(crate) fn player_power_attack_event(&self) -> &'static str {
        let Some(body) = self.player_body.as_ref() else {
            return "attackPowerStartInPlace";
        };
        match body.direction {
            None => "attackPowerStartInPlace",
            Some(_) if body.sprinting => "attackPowerStart_Sprint",
            Some(d) => match ((d * 4.0).round() as i32).rem_euclid(4) {
                0 => "attackPowerStartForward",
                1 => "attackPowerStartRight",
                2 => "attackPowerStartBackward",
                _ => "attackPowerStartLeft",
            },
        }
    }

    /// Whether the player has their weapon (or fists) out.
    pub fn player_weapon_drawn(&self) -> bool {
        self.player_body.as_ref().is_some_and(|b| b.drawn)
    }

    /// Whether the draw is over (the graph's `WeapEquip_Out`): the graph
    /// won't swing before (it takes `attackStart` mid-draw and does nothing).
    pub(crate) fn player_weapon_ready(&self) -> bool {
        self.player_body
            .as_ref()
            .is_some_and(|b| b.drawn && b.ready)
    }

    /// Draw (or sheathe) the player's weapon: the graph plays it and the
    /// weapon goes to the hand as it raises `weaponDraw`. False if it won't.
    pub fn player_draw_weapon(&mut self, draw: bool) -> bool {
        if draw && self.disabled_controls.fighting {
            return false;
        }
        let Some(body) = self.player_body.as_mut() else {
            return false;
        };
        if body.drawn == draw {
            return true;
        }
        let Some(g) = body.graph.as_mut() else {
            return false;
        };
        let took = PlayerBody::event(g, if draw { "WeapEquip" } else { "Unequip" });
        if took {
            body.drawn = draw;
            body.ready = false;
            log::info!(
                "player {} their weapon",
                if draw { "draws" } else { "sheathes" }
            );
        }
        took
    }

    /// A swing waiting on the graph lands at its `HitFrame`s, a hand at a
    /// time (`Side::struck`): a basic swing once a hand, a power attack each
    /// time (its further blows costing stamina again), never twice within
    /// `STRIKE_GAP`. It is over at the graph's `attackStop` once it has
    /// landed, or when it waits too long (cut short).
    fn update_player_swing(&mut self, frames: u8, stop: bool, dt: f32) {
        use crate::ai::combat::STRIKE_GAP;
        self.player_attack_recovery = (self.player_attack_recovery - dt).max(0.0);
        let Some(s) = self.player_swing.as_mut() else {
            return;
        };
        s.waited += dt;
        for t in &mut s.strike_age {
            *t += dt;
        }
        let power = s.power;
        let hands: Vec<usize> = s
            .side
            .struck(frames)
            .into_iter()
            .filter(|&h| s.strike_age[h] >= STRIKE_GAP && (power || s.strike_age[h].is_infinite()))
            .collect();
        if frames != 0 && !hands.is_empty() {
            let again = (s.strikes > 0 && power).then_some(s.again);
            s.strikes += 1;
            s.waited = 0.0;
            for &h in &hands {
                s.strike_age[h] = 0.0;
            }
            let (blows, stagger) = (s.blows, s.stagger);
            if let Some(cost) = again {
                self.spend_stamina(PLAYER_REF, cost);
            }
            // The swing stays while it lands (`IsAttackType` asks of it).
            for h in hands {
                if let Some(b) = blows[h] {
                    self.player_strike(h, power, b.damage, b.reach, stagger);
                }
            }
        }
        let Some(s) = self.player_swing.as_ref() else {
            return;
        };
        // The swing before may end (`attackStop`) after this one began: that
        // doesn't end one yet to land.
        if (stop && s.strikes > 0) || s.waited > SWING_TIMEOUT {
            if s.strikes == 0 {
                log::debug!("player swing lost: no HitFrame");
            }
            if stop {
                self.player_attack_recovery = ATTACK_BLEND_OUT;
            }
            self.player_swing = None;
        }
    }
}

/// The third-person camera's distance behind the eye: nearest and farthest
/// (the game's `[Camera]` defaults `fVanityModeMinDist` / `MaxDist`), and at
/// first (chosen by eye: no setting gives it).
pub const THIRD_PERSON_MIN: f32 = 155.0;
pub const THIRD_PERSON_MAX: f32 = 600.0;
pub const THIRD_PERSON_DEFAULT: f32 = 200.0;
/// Where the camera sits over the shoulder, to the right and up: sheathed
/// (`fOverShoulderPosX` / `PosZ`) and drawn (`fOverShoulderCombatPosX` /
/// `PosZ`), and how much farther forward it sits drawn
/// (`fOverShoulderCombatAddY`: negative, so farther back).
const OVER_SHOULDER: Vec2 = Vec2::new(30.0, -10.0);
const OVER_SHOULDER_COMBAT: Vec2 = Vec2::new(0.0, 20.0);
const OVER_SHOULDER_COMBAT_ADD_Y: f32 = -100.0;
/// How long the camera takes to move between its sheathed and drawn places
/// (chosen).
const OVER_SHOULDER_BLEND: f32 = 0.4;
/// How far short of a wall the camera stops.
const CAMERA_CLEARANCE: f32 = 12.0;
/// How far the crosshair's point is looked for, for the player's aim.
const AIM_FAR: f32 = 10_000.0;

impl Engine {
    /// The camera the frame is drawn from: the eye in first person, else pulled
    /// back behind it along the view, short of whatever is in the way (the
    /// world, not actors).
    pub fn view_camera(&self) -> crate::render::Camera {
        let mut cam = self.camera_copy();
        if self.third_person {
            // Over the shoulder first, then back, each short of a wall.
            let k = self.third_person_combat;
            let side = OVER_SHOULDER.lerp(OVER_SHOULDER_COMBAT, k);
            let right = cam.forward().cross(Vec3::Z).normalize_or(Vec3::X);
            let offset = right * side.x + Vec3::Z * side.y;
            let len = offset.length();
            if len > 0.0 {
                let dir = offset / len;
                let reach = self
                    .physics
                    .ground_ray(cam.position, dir, len + CAMERA_CLEARANCE)
                    .map_or(len, |(t, _)| (t - CAMERA_CLEARANCE).clamp(0.0, len));
                cam.position += dir * reach;
            }
            let back = -cam.forward();
            let want = self.third_person_distance - OVER_SHOULDER_COMBAT_ADD_Y * k;
            let dist = self
                .physics
                .ground_ray(cam.position, back, want + CAMERA_CLEARANCE)
                .map_or(want, |(t, _)| (t - CAMERA_CLEARANCE).clamp(0.0, want));
            cam.position += back * dist;
        }
        cam
    }

    /// Move the third-person camera towards its drawn or sheathed place.
    pub(crate) fn update_third_person_camera(&mut self, dt: f32) {
        let goal = if self.player_weapon_drawn() { 1.0 } else { 0.0 };
        let step = dt / OVER_SHOULDER_BLEND;
        let k = self.third_person_combat;
        self.third_person_combat = if goal > k {
            (k + step).min(goal)
        } else {
            (k - step).max(goal)
        };
    }

    /// The player's aim: from the eye along the view in first person. In third
    /// person the view is beside and behind the eye, so the aim goes from the
    /// eye to where the crosshair is, the first thing the view's centre meets
    /// (from level with the eye, past the player's own body).
    pub fn player_aim(&self) -> (Vec3, Vec3) {
        let (eye, forward) = (self.camera.position, self.camera.forward());
        if !self.third_person {
            return (eye, forward);
        }
        let view = self.view_camera().position;
        let start = view + forward * (eye - view).dot(forward).max(0.0);
        let t = self
            .physics
            .raycast(start, forward, AIM_FAR)
            .map_or(AIM_FAR, |(t, _)| t);
        (eye, (start + forward * t - eye).normalize_or(forward))
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
