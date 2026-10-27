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
    pub(crate) skeleton: Arc<Skeleton>,
    graph: Option<GraphAnim>,
    scale: f32,
    /// Facing, clockwise from +Y as the camera's yaw.
    pub heading: f32,
    graph_heading: f32,
    /// What they had equipped when it was built: rebuilt when that changes.
    worn: (Vec<FormId>, Option<FormId>),
    /// What the hands and voice held when the graph was last told their
    /// types: spells change them without a rebuild.
    held: [Option<FormId>; 3],
    /// The graph's movement type states (`iState`): default and sneaking.
    state: Option<f32>,
    sneak_state: Option<f32>,
    sprint_state: Option<f32>,
    swim_told: bool,
    step_events: Vec<String>,
    /// How the graph was last told they move, and its state changes by then
    /// ([`GraphAnim::changes`]): told again when either changes.
    told: Option<Stance>,
    told_at: u64,
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
    combat_events: PlayerCombatEvents,
    /// A bow shot waiting for this body's graph to release it.
    bow: PlayerBow,
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
    /// keeping its facing and, for an unchanged rig/project, its animation.
    pub(crate) fn build_player_body(&mut self) {
        let mut previous = self.player_body.take();
        // Equipment changes cannot complete a shot with the old equipment.
        if previous.as_ref().is_some_and(|b| b.bow.drawing) {
            self.player_attack_held = None;
        }
        if let Some(body) = &mut previous {
            body.cancel_equipment_attack(self.player_swing.is_some(), &self.vfs, &mut self.anims);
        }
        self.player_swing = None;
        let heading = previous.as_ref().map_or(self.camera.yaw, |b| b.heading);
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
        let project = d
            .behavior
            .as_deref()
            .and_then(|p| self.graphs.project(&self.vfs, p));
        let mut body = previous
            .filter(|b| b.can_reuse(&skel, project.as_ref(), &d.skeleton, d.female))
            .unwrap_or_else(|| PlayerBody::new(skel.clone(), heading));
        body.scale = d.transform.x_axis.length();
        body.worn = (
            inventory.equipped().to_vec(),
            inventory.weapon_in(&self.lo, crate::world::inventory::LEFT),
        );
        body.held = inventory.held_all();
        let mut pose = skel.model_space(&skel.bind_locals());
        if let Some(project) = project {
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
            body.sprint_state =
                crate::world::movement::special_movement_type(project.shared.variables(), "sprint")
                    .map(|(_, v)| v);
            let mut g = body
                .graph
                .take()
                .unwrap_or_else(|| GraphAnim::new(project, &d.skeleton, d.female, &skel, self.rng));
            g.label = "player".into();
            g.replace_aux(self.wearer_graph(&d, &skel));
            for (var, value) in [("IsNPC", 0.0), ("i1stPerson", 0.0), ("IsFirstPerson", 0.0)] {
                g.set_variable(var, value);
            }
            g.set_variable("weapAdj", d.weight);
            let speeds = self.weapon_speed_mults(PLAYER_REF);
            for (var, value) in crate::ai::combat::hand_graph_vars(&self.lo, &inventory, speeds) {
                g.set_variable(var, value);
            }
            body.graph = Some(g);
            // Consume notifications even at zero time (e.g. a draw committed
            // just before the menu opened), keeping attachment/ready state in sync.
            if let Some(frame) = body.sample(0.0, &self.vfs, &mut self.anims) {
                pose = frame;
            }
        }
        let (meshes, _, rigid) = self.actor_meshes(&d, &skel);
        self.rigid_models.insert(PLAYER_REF, rigid);
        let (equipment, _) = self.rigid_equipment(PLAYER_REF, &skel, body.weapon_out);
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
            skin_tone: d.skin_tone,
            weight: d.weight,
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
        if let Some(inv) = self.inventories.get(&PLAYER_REF)
            && let held = inv.held_all()
            && self.player_body.as_ref().is_some_and(|b| b.held != held)
        {
            let speeds = self.weapon_speed_mults(PLAYER_REF);
            let vars = crate::ai::combat::hand_graph_vars(&self.lo, inv, speeds);
            let body = self.player_body.as_mut().unwrap();
            body.held = held;
            if let Some(g) = body.graph.as_mut() {
                for (var, value) in vars {
                    g.set_variable(var, value);
                }
            }
        }
        let feet = self.player_feet();
        let third = self.third_person;
        let yaw = self.camera.yaw;
        let can_shoot =
            !self.player_dead() && self.conversation.is_none() && !self.disabled_controls.fighting;
        let flat = moved.truncate();
        let walking = self.player.moving && !self.player.noclip;
        let motion = Motion {
            walking,
            speed: if dt > 0.0 { flat.length() / dt } else { 0.0 },
            heading: (flat.length_squared() > 1e-4).then(|| flat.x.atan2(flat.y)),
            sneaking: self.player.sneaking && !self.player.swimming,
            sprinting: walking && self.player.sprinting,
            jumped: self.player.jumped && !self.player.noclip,
            grounded: self.player.grounded || self.player.noclip || self.player.swimming,
            blocking: self.player_blocking,
            swimming: self.player.swimming,
        };
        let actions = self.locomotion_actions(PLAYER_REF);
        let Some(body) = self.player_body.as_mut() else {
            return;
        };
        let cancel_draw = !can_shoot && body.bow.drawing;
        if !can_shoot && (body.bow.drawing || body.bow.shot.is_some()) {
            body.bow.cancel();
            if let Some(g) = body.graph.as_mut() {
                PlayerBody::event(g, "attackStop", &self.vfs, &mut self.anims, &body.skeleton);
            }
        }
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
        let pose = body.animate(dt, &motion, &actions, &self.vfs, &mut self.anims);
        let transform = body.transform(feet);
        let events = std::mem::take(&mut body.combat_events);
        let step_events = std::mem::take(&mut body.step_events);
        let shot = body
            .bow
            .update(events.arrow_release, can_shoot && body.drawn, dt);
        let (skel, weapon_out) = (body.skeleton.clone(), body.weapon_out);
        if cancel_draw {
            self.player_attack_held = None;
        }
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
        self.player_swim_steps(&step_events);
        self.update_player_swing(events.hit_frames, events.attack_stop, dt);
        self.player_spell_events(events.spell_fire, events.hand_equipped, dt);
        if let Some(held) = shot {
            self.player_loose(held);
        }
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
    swimming: bool,
}

/// Moving, sprinting and sneaking, as the graph is told them.
#[derive(Clone, Copy, PartialEq, Debug)]
struct Stance {
    moving: bool,
    sprinting: bool,
    sneaking: bool,
}

impl Stance {
    /// The events putting the graph in this stance, from wherever it is: the
    /// graph ignores those for where it already is. A sprint ends while
    /// moving (standing, the graph ignores `SprintStop`, and its locomotion
    /// sprints again at the next `moveStart`).
    fn events(self) -> [&'static str; 3] {
        match (self.moving, self.sprinting) {
            (false, _) => ["ActionSneak", "ActionSprintStop", "ActionMoveStop"],
            (true, false) => ["ActionSneak", "ActionMoveStart", "ActionSprintStop"],
            (true, true) => ["ActionSneak", "ActionMoveStart", "ActionSprintStart"],
        }
    }
}

/// How long off the ground, not having jumped, before they fall.
const FALL_AFTER: f32 = 0.3;

impl PlayerBody {
    pub(crate) fn graph_sneaking(&self) -> bool {
        self.told.is_some_and(|s| s.sneaking)
    }
    pub(crate) fn project(&self) -> Option<&Arc<crate::world::behavior::ProjectRuntime>> {
        Some(self.graph.as_ref()?.project())
    }

    fn new(skeleton: Arc<Skeleton>, heading: f32) -> Self {
        Self {
            skeleton,
            graph: None,
            scale: 1.0,
            heading,
            graph_heading: f32::NAN,
            worn: Default::default(),
            held: [None; 3],
            state: None,
            sneak_state: None,
            sprint_state: None,
            swim_told: false,
            step_events: Vec::new(),
            told: None,
            told_at: 0,
            airborne: None,
            fell: false,
            drawn: false,
            weapon_out: false,
            ready: false,
            guard: false,
            combat_events: PlayerCombatEvents::default(),
            bow: PlayerBow::default(),
            direction: None,
        }
    }

    fn can_reuse(
        &self,
        skeleton: &Arc<Skeleton>,
        project: Option<&Arc<crate::world::behavior::ProjectRuntime>>,
        rig: &str,
        female: bool,
    ) -> bool {
        Arc::ptr_eq(&self.skeleton, skeleton)
            && match (&self.graph, project) {
                (Some(g), Some(p)) => g.matches(p, rig, female),
                (None, None) => true,
                _ => false,
            }
    }

    fn cancel_equipment_attack(
        &mut self,
        swinging: bool,
        vfs: &vfs::Vfs,
        anims: &mut crate::world::animation::AnimationLibrary,
    ) {
        if (swinging || self.bow.drawing || self.bow.shot.is_some())
            && let Some(g) = self.graph.as_mut()
        {
            Self::event(g, "attackStop", vfs, anims, &self.skeleton);
        }
        self.bow.cancel();
        self.combat_events = PlayerCombatEvents::default();
    }

    fn transform(&self, feet: Vec3) -> Mat4 {
        Mat4::from_scale_rotation_translation(
            Vec3::splat(self.scale),
            Quat::from_rotation_z(-self.heading),
            feet,
        )
    }

    /// Handle an event in the graph now; whether it took it (changed state).
    fn event(
        g: &mut GraphAnim,
        ev: &str,
        vfs: &vfs::Vfs,
        anims: &mut crate::world::animation::AnimationLibrary,
        skeleton: &Skeleton,
    ) -> bool {
        let took = g.handle_event(ev, vfs, anims, skeleton);
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
        actions: &crate::ai::idles::LocomotionActions,
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
        if self.swim_told != m.swimming {
            actions.send(g, "ActionSwimStateChange");
            self.swim_told = m.swimming;
            self.airborne = None;
            self.fell = false;
            self.told = None;
            if m.swimming {
                self.drawn = false;
                self.guard = false;
            }
        }
        g.foot_ik_enabled = !m.swimming;
        let state = if m.sprinting {
            self.sprint_state.or(self.state)
        } else if m.sneaking {
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
            if m.swimming {
                g.set_variable("SwimDirection", d);
            }
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
        let guard = m.blocking && self.drawn && !m.swimming;
        if guard != self.guard {
            let ev = if guard { "blockStart" } else { "blockStop" };
            let took = Self::event(g, ev, vfs, anims, &self.skeleton);
            if took || !guard {
                self.guard = guard;
                g.set_variable("IsBlocking", if guard { 1.0 } else { 0.0 });
            }
        }
        // Off the ground: a jump (standing or on the move) or, after a moment
        // without one, a fall; back on it, the landing, after which the graph
        // stands again until told they move.
        if m.jumped && self.airborne.is_none() {
            let ev = if m.walking {
                "JumpDirectionalStart"
            } else {
                "JumpStandingStart"
            };
            // A jump the graph won't play (mid-swing, say) is a fall later.
            self.fell = Self::event(g, ev, vfs, anims, &self.skeleton);
            self.airborne = Some(0.0);
        } else if let Some(t) = self.airborne.as_mut() {
            *t += dt;
            if m.grounded {
                // Only a jump or a fall the graph knows of ends in a landing
                // (not a step down a slope).
                if self.fell {
                    let ev = if m.walking {
                        "JumpLandDirectional"
                    } else {
                        "JumpLand"
                    };
                    Self::event(g, ev, vfs, anims, &self.skeleton);
                }
                self.airborne = None;
            } else if !self.fell && *t >= FALL_AFTER {
                // Until the graph takes it.
                self.fell = Self::event(g, "JumpFall", vfs, anims, &self.skeleton);
            }
        } else if !m.grounded {
            self.airborne = Some(0.0);
            self.fell = false;
        }
        // On the ground, the graph is told how they move when that changes,
        // and again whenever it changed state since: much it does on its own
        // (a landing, a draw, the end of a swing) puts it back to standing,
        // and an event can come while it is between states and won't take it
        // (the landing's own `JumpLandEnd` queues behind what is sent with it).
        let stance = Stance {
            moving: m.walking,
            sprinting: m.sprinting && m.walking,
            sneaking: m.sneaking,
        };
        if (self.airborne.is_none() || !self.fell)
            && (self.told != Some(stance) || g.changes() != self.told_at)
        {
            for action in stance.events() {
                if action == "ActionSneak"
                    && self
                        .told
                        .map_or(!stance.sneaking, |s| s.sneaking == stance.sneaking)
                {
                    continue;
                }
                actions.send(g, action);
            }
            if self.told != Some(stance) {
                log::debug!("player body: {stance:?}");
            }
            self.told = Some(stance);
            // Taken, these change it again: told once more, then ignored.
            self.told_at = g.changes();
        }
        self.sample(dt, vfs, anims)
    }

    fn sample(
        &mut self,
        dt: f32,
        vfs: &vfs::Vfs,
        anims: &mut crate::world::animation::AnimationLibrary,
    ) -> Option<Vec<Mat4>> {
        let frame = self.graph.as_mut()?.update(dt, vfs, anims, &self.skeleton);
        for r in &frame.raised {
            let tag = r.event.to_ascii_lowercase();
            if self.swim_told && !self.step_events.contains(&tag) {
                self.step_events.push(tag);
            }
            log::trace!(target: "player_events", "player raised {}{}", r.event, r.payload.as_deref().map(|p| format!(" ({p})")).unwrap_or_default());
            self.combat_events.record(&r.event, r.payload.as_deref());
            match r.event.to_ascii_lowercase().as_str() {
                "weapondraw" => self.weapon_out = true,
                "weaponsheathe" => self.weapon_out = false,
                "weapequip_out" => self.ready = self.drawn,
                _ => {}
            }
        }
        Some(frame.pose)
    }
}

/// Collapse duplicate body-part layer notifications into one update's events.
#[derive(Default)]
struct PlayerCombatEvents {
    hit_frames: u8,
    attack_stop: bool,
    arrow_release: bool,
    /// By hand (`RIGHT`, `LEFT`): a spell's release clip reached its
    /// `SpellFire`, or ended (`Equipped`).
    spell_fire: [bool; 2],
    hand_equipped: [bool; 2],
}

impl PlayerCombatEvents {
    fn record(&mut self, event: &str, payload: Option<&str>) {
        match event.to_ascii_lowercase().as_str() {
            "hitframe" => self.hit_frames |= crate::ai::combat::hit_frame_bit(payload),
            "attackstop" => self.attack_stop = true,
            "arrowrelease" => self.arrow_release = true,
            "mrh_spellfire_event" => self.spell_fire[crate::spells::RIGHT] = true,
            "mlh_spellfire_event" => self.spell_fire[crate::spells::LEFT] = true,
            "mrh_equipped_event" => self.hand_equipped[crate::spells::RIGHT] = true,
            "mlh_equipped_event" => self.hand_equipped[crate::spells::LEFT] = true,
            _ => {}
        }
    }
}

/// How long a swing the graph took may wait for its `HitFrame`, or for its
/// next one after it lands.
const SWING_TIMEOUT: f32 = 1.5;

/// How long the character graphs blend out of an attack (`AttackState` to
/// the ready state on `attackStop`).
const ATTACK_BLEND_OUT: f32 = 0.4;

/// A missing release notification cancels the shot rather than firing it at an
/// invented time. These limits are runtime choices, not published game rules.
const BOW_RELEASE_TIMEOUT: f32 = 1.5;
const BOW_RELEASE_QUIET: f32 = 0.4;

/// One draw/release at a time. The outgoing clip can raise `arrowRelease`
/// repeatedly while blending out; drain it before allowing another draw.
#[derive(Default)]
struct PlayerBow {
    drawing: bool,
    shot: Option<(f32, f32)>, // held draw duration, seconds waiting for release
    quiet: f32,
}

impl PlayerBow {
    fn start(&mut self, accept: impl FnOnce() -> bool) -> bool {
        if self.drawing || self.shot.is_some() || self.quiet > 0.0 || !accept() {
            return false;
        }
        self.drawing = true;
        true
    }

    fn release(&mut self, held: f32, accept: impl FnOnce() -> bool) -> bool {
        if !self.drawing {
            return false;
        }
        if self.quiet > 0.0 || !accept() {
            self.cancel();
            return false;
        }
        self.drawing = false;
        self.shot = Some((held, 0.0));
        true
    }

    fn cancel(&mut self) {
        if self.drawing || self.shot.is_some() {
            self.quiet = BOW_RELEASE_QUIET;
        }
        self.drawing = false;
        self.shot = None;
    }

    /// Return a draw duration once, only on its release notification. Events
    /// outside a pending release are drained, including duplicate layer events.
    fn update(&mut self, arrow_release: bool, can_shoot: bool, dt: f32) -> Option<f32> {
        self.quiet = (self.quiet - dt).max(0.0);
        if !can_shoot {
            self.cancel();
        }
        if arrow_release {
            self.quiet = BOW_RELEASE_QUIET;
            return self.shot.take().map(|(held, _)| held);
        }
        if let Some((_, waited)) = self.shot.as_mut() {
            *waited += dt;
            if *waited >= BOW_RELEASE_TIMEOUT {
                self.cancel();
            }
        }
        None
    }
}

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
    /// Begin a bow draw only when the graph can take it. Without a body graph,
    /// retain the immediate release used by the console and graphless player.
    pub(crate) fn player_begin_bow_draw(&mut self) -> bool {
        let Some(body) = self.player_body.as_mut() else {
            return true;
        };
        let Some(g) = body.graph.as_mut() else {
            return true;
        };
        if !body.drawn || !body.ready {
            return false;
        }
        body.bow.start(|| {
            PlayerBody::event(
                g,
                "bowAttackStart",
                &self.vfs,
                &mut self.anims,
                &body.skeleton,
            )
        })
    }

    /// Releasing the button requests the release animation. Its `arrowRelease`
    /// notification later consumes ammo and spawns the projectile.
    pub(crate) fn player_release_bow(&mut self, held: f32) {
        let Some(body) = self.player_body.as_mut().filter(|b| b.graph.is_some()) else {
            self.player_loose(held);
            return;
        };
        let g = body.graph.as_mut().unwrap();
        let drawing = body.bow.drawing;
        let released = body.bow.release(held, || {
            PlayerBody::event(
                g,
                "attackRelease",
                &self.vfs,
                &mut self.anims,
                &body.skeleton,
            )
        });
        if drawing && !released {
            PlayerBody::event(g, "attackStop", &self.vfs, &mut self.anims, &body.skeleton);
        }
    }

    /// Send an event to the player's body's graph; whether it took it.
    pub(crate) fn player_graph_event(&mut self, ev: &str) -> bool {
        self.spell_fx.events.push((PLAYER_REF, ev.to_owned()));
        self.player_body.as_mut().is_some_and(|b| {
            let Some(g) = b.graph.as_mut() else {
                return false;
            };
            PlayerBody::event(g, ev, &self.vfs, &mut self.anims, &b.skeleton)
        })
    }

    /// Whether the player's body runs a behaviour graph.
    /// The states the player's body's graph is in (console `gstate`).
    pub(crate) fn player_graph_states(&self) -> Option<Vec<String>> {
        Some(self.player_body.as_ref()?.graph.as_ref()?.active_states())
    }

    pub(crate) fn player_has_graph(&self) -> bool {
        self.player_body.as_ref().is_some_and(|b| b.graph.is_some())
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
            Some(_) if body.told.is_some_and(|t| t.sprinting) => "attackPowerStart_Sprint",
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
        let ev = if draw { "WeapEquip" } else { "Unequip" };
        let took = PlayerBody::event(g, ev, &self.vfs, &mut self.anims, &body.skeleton);
        if took {
            body.drawn = draw;
            body.ready = false;
            if !draw {
                body.bow.cancel();
            }
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
            cam.yaw += self.test_orbit;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn equipment_body() -> PlayerBody {
        use havok::behavior::runtime::Shared;
        use havok::behavior::{
            BehaviorGraph, EventProperty, Generator, Project, State, Transition,
        };
        let state = |id, transitions, events: &[i32]| State {
            id,
            name: format!("s{id}"),
            generator: None,
            transitions,
            enter_events: events
                .iter()
                .map(|&event| EventProperty {
                    event,
                    payload: None,
                })
                .collect(),
            exit_events: vec![],
        };
        let graph = BehaviorGraph::new(
            "Equipment",
            Some(0),
            vec![Generator::StateMachine {
                name: "Root".into(),
                start: 0,
                start_variable: None,
                start_mode: Default::default(),
                sync_variable: None,
                change_event: None,
                states: vec![
                    state(0, vec![Transition::new(0, 1, None)], &[6]),
                    state(
                        1,
                        vec![Transition::new(1, 2, None), Transition::new(2, 3, None)],
                        &[4],
                    ),
                    state(2, vec![Transition::new(2, 3, None)], &[5]),
                    state(3, vec![Transition::new(3, 0, None)], &[]),
                ],
                wildcards: vec![],
            }],
            [
                "WeapEquip",
                "DrawDone",
                "Unequip",
                "SheathDone",
                "weaponDraw",
                "WeapEquip_Out",
                "weaponSheathe",
            ]
            .map(String::from)
            .to_vec(),
        );
        let project = Arc::new(crate::world::behavior::ProjectRuntime {
            shared: Shared::new(Arc::new(Project {
                graphs: vec![("synthetic".into(), graph)],
                character: None,
            })),
            dir: "synthetic".into(),
            name: "equipment".into(),
            files: vec![],
        });
        let skel = Arc::new(Skeleton::default());
        let mut body = PlayerBody::new(skel.clone(), 0.5);
        body.graph = Some(GraphAnim::new(project, "rig", false, &skel, 1));
        body
    }

    #[test]
    fn equipment_refresh_preserves_draw_progress_and_consumes_zero_time_notifications() {
        let vfs = vfs::Vfs::new("/tmp/vrm-synthetic-player-no-assets", &[]);
        let mut anims = crate::world::animation::AnimationLibrary::default();
        let mut body = equipment_body();
        body.sample(0.0, &vfs, &mut anims);
        let event =
            |body: &mut PlayerBody, name, anims: &mut crate::world::animation::AnimationLibrary| {
                PlayerBody::event(
                    body.graph.as_mut().unwrap(),
                    name,
                    &vfs,
                    anims,
                    &body.skeleton,
                )
            };
        assert!(event(&mut body, "WeapEquip", &mut anims));
        body.drawn = true;
        body.sample(0.0, &vfs, &mut anims);
        assert!(body.weapon_out);
        assert!(!body.ready);
        assert!(body.can_reuse(
            &body.skeleton,
            Some(body.graph.as_ref().unwrap().project()),
            "rig",
            false
        ));
        body.sample(0.0, &vfs, &mut anims);
        assert!(body.drawn && body.weapon_out && !body.ready);
        assert!(event(&mut body, "DrawDone", &mut anims));
        body.sample(0.0, &vfs, &mut anims);
        assert!(body.drawn && body.weapon_out && body.ready);
        assert!(event(&mut body, "Unequip", &mut anims));
        body.drawn = false;
        body.ready = false;
        body.sample(0.0, &vfs, &mut anims);
        assert!(body.weapon_out); // Mid-sheathe equipment stays in hand.
        assert!(event(&mut body, "SheathDone", &mut anims));
        body.sample(0.0, &vfs, &mut anims);
        assert!(!body.drawn && !body.weapon_out && !body.ready);
    }

    #[test]
    fn equipment_refresh_requires_the_same_rig_character_and_skeleton_and_cancels_shots() {
        let vfs = vfs::Vfs::new("/tmp/vrm-synthetic-player-no-assets", &[]);
        let mut anims = crate::world::animation::AnimationLibrary::default();
        let mut body = equipment_body();
        let project = body.graph.as_ref().unwrap().project().clone();
        assert!(!body.can_reuse(&Arc::new(Skeleton::default()), Some(&project), "rig", false));
        assert!(!body.can_reuse(&body.skeleton, Some(&project), "other", false));
        assert!(!body.can_reuse(&body.skeleton, Some(&project), "rig", true));
        assert!(!body.can_reuse(&body.skeleton, None, "rig", false));
        let other = equipment_body();
        assert!(!body.can_reuse(
            &body.skeleton,
            Some(other.graph.as_ref().unwrap().project()),
            "rig",
            false
        ));
        body.drawn = true;
        body.weapon_out = true;
        body.ready = true;
        for pending in [false, true] {
            body.bow.drawing = !pending;
            body.bow.shot = pending.then_some((0.8, 0.0));
            body.combat_events.arrow_release = true;
            body.cancel_equipment_attack(false, &vfs, &mut anims);
            assert!(!body.bow.drawing && body.bow.shot.is_none());
            assert!(!body.combat_events.arrow_release);
            assert_eq!(body.bow.update(true, true, 0.0), None);
            assert!(body.drawn && body.weapon_out && body.ready);
        }
    }

    #[test]
    fn bow_requires_accepted_draw_and_release() {
        let mut bow = PlayerBow::default();
        assert!(!bow.release(1.0, || panic!("no draw to release")));
        assert!(!bow.start(|| false));
        assert!(!bow.drawing);
        assert!(bow.start(|| true));
        assert!(!bow.start(|| panic!("already drawing")));
        assert!(!bow.release(1.0, || false));
        assert!(!bow.drawing);
        assert!(bow.shot.is_none());
        assert!(!bow.start(|| panic!("rejected release still quarantines outgoing events")));
        assert_eq!(bow.update(true, true, 0.0), None);
    }

    #[test]
    fn bow_releases_once_then_drains_duplicates_before_next_draw() {
        let mut bow = PlayerBow::default();
        assert!(bow.start(|| true));
        assert!(bow.release(0.75, || true));
        assert_eq!(bow.update(false, true, 0.1), None);
        assert_eq!(bow.update(true, true, 0.1), Some(0.75));
        assert_eq!(bow.update(true, true, 0.3), None);
        assert!(!bow.start(|| panic!("duplicate extends quiet interval")));
        bow.update(false, true, BOW_RELEASE_QUIET);
        assert!(bow.start(|| true));
        // An unexpected old notification during drawing cannot arm a shot.
        assert_eq!(bow.update(true, true, 0.0), None);
        assert!(!bow.release(1.0, || panic!("outgoing event still in quarantine")));
        assert_eq!(bow.update(true, true, 0.0), None);
    }

    #[test]
    fn bow_cancellation_and_missing_event_never_fire() {
        for cancel_during_draw in [false, true] {
            let mut bow = PlayerBow::default();
            assert!(bow.start(|| true));
            if !cancel_during_draw {
                assert!(bow.release(1.0, || true));
            }
            assert_eq!(bow.update(true, false, 0.1), None);
            assert!(!bow.drawing);
            assert!(bow.shot.is_none());
            assert_eq!(bow.update(true, true, 0.0), None);
        }
        let mut bow = PlayerBow::default();
        assert!(bow.start(|| true));
        assert!(bow.release(1.0, || true));
        assert_eq!(bow.update(false, true, BOW_RELEASE_TIMEOUT), None);
        assert!(bow.shot.is_none());
        assert_eq!(bow.update(true, true, 0.0), None);
    }

    #[test]
    fn bow_notification_wins_over_timeout_crossed_in_same_update() {
        let mut bow = PlayerBow::default();
        assert!(bow.start(|| true));
        assert!(bow.release(0.8, || true));
        assert_eq!(bow.update(true, true, BOW_RELEASE_TIMEOUT + 0.1), Some(0.8));
        assert_eq!(bow.update(true, true, 0.0), None);
    }

    #[test]
    fn bow_clip_trigger_flows_through_player_event_collector() {
        use havok::behavior::runtime::{Instance, Shared};
        use havok::behavior::{
            BehaviorGraph, ClipMode, Generator, Project, State, Transition, Trigger,
        };

        // Entirely synthetic: idle -> draw -> release, with two identical
        // notifications in the release clip (as body-part layers can produce).
        let state = |id, transitions| State {
            id,
            name: format!("s{id}"),
            generator: Some(id as usize + 1),
            transitions,
            enter_events: vec![],
            exit_events: vec![],
        };
        let clip = |name: &str, triggers| Generator::Clip {
            name: name.into(),
            animation: name.into(),
            mode: ClipMode::SinglePlay,
            speed: 1.0,
            enforced_duration: 0.0,
            user_fraction: 0.0,
            triggers,
            crop_start: 0.0,
            crop_end: 0.0,
            start_time: 0.0,
        };
        let notification = Trigger {
            time: 0.2,
            from_end: false,
            event: 2,
            payload: None,
        };
        let graph = BehaviorGraph::new(
            "SyntheticBow",
            Some(0),
            vec![
                Generator::StateMachine {
                    name: "Root".into(),
                    start: 0,
                    start_variable: None,
                    start_mode: Default::default(),
                    sync_variable: None,
                    change_event: None,
                    states: vec![
                        state(0, vec![Transition::new(0, 1, None)]),
                        state(1, vec![Transition::new(1, 2, None)]),
                        state(2, vec![]),
                    ],
                    wildcards: vec![],
                },
                clip("Idle", vec![]),
                clip("Draw", vec![]),
                clip("Release", vec![notification.clone(), notification]),
            ],
            ["bowAttackStart", "attackRelease", "ArRoWrElEaSe"]
                .map(String::from)
                .to_vec(),
        );
        let mut graph = Instance::new(
            Shared::new(Arc::new(Project {
                graphs: vec![("synthetic".into(), graph)],
                character: None,
            })),
            1,
        );
        let mut durations = |_: &str| Some(1.0);
        graph.update(0.0, &mut durations);
        let mut bow = PlayerBow::default();
        assert!(bow.start(|| graph.handle_event("bowAttackStart", &mut durations)));
        assert!(bow.release(0.9, || graph.handle_event("attackRelease", &mut durations)));
        let mut releases = Vec::new();
        for dt in [0.1, 0.11, 0.1] {
            graph.update(dt, &mut durations);
            let mut events = PlayerCombatEvents::default();
            for event in graph.take_raised() {
                events.record(&event.event, event.payload.as_deref());
            }
            releases.push(bow.update(events.arrow_release, true, dt));
        }
        assert_eq!(releases, [None, Some(0.9), None]);
    }
}
