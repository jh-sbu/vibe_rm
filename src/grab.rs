//! The player grabbing things: holding Activate on a loose object (or a body
//! lying as a ragdoll) picks up the body under the crosshair and carries it
//! in front of the camera; letting go drops it with the way it was moving.
//! A tap activates as before (takes the item, searches the body).

use anyhow::Result;
use esp::FormId;
use glam::Vec3;
use rapier3d::prelude::RigidBodyHandle;

use crate::engine::{Engine, PLAYER_REF};

/// How long Activate is held before it grabs rather than activates (seconds;
/// no source).
const GRAB_HOLD: f32 = 0.3;

/// How far ahead things can be grabbed: the activation reach.
const GRAB_REACH: f32 = 220.0;

/// The nearest the held point is kept to the eye (game units; no source).
const HOLD_NEAREST: f32 = 50.0;

/// The held point this far from where it is pulled (stuck behind something
/// the player walked away from) is let go (game units; no source).
const LET_GO: f32 = 150.0;

/// The fastest a dropped object leaves the hand (game units / s; no source).
const DROP_SPEED: f32 = 700.0;

/// Something the player holds.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Held {
    pub owner: FormId,
    body: RigidBodyHandle,
    /// The point held, in the body's frame, kept `distance` ahead of the eye.
    local: Vec3,
    distance: f32,
    /// The mass carried: the body's and the simulated ones joined to it.
    mass: f32,
}

/// Activate held down, and what the player holds.
#[derive(Default)]
pub(crate) struct Grab {
    /// Seconds Activate has been down over something that can be grabbed.
    pending: Option<f32>,
    pub held: Option<Held>,
}

impl Engine {
    /// Activate pressed: something that can be grabbed waits to see whether
    /// it is held; anything else is activated at once.
    pub fn activate_press(&mut self) -> Result<()> {
        if self.grab.held.is_some() {
            return Ok(());
        }
        if !self.disabled_controls.activate && self.grab_target().is_some() {
            self.grab.pending = Some(0.0);
            return Ok(());
        }
        self.activate()
    }

    /// Activate let go: a tap activates; what is held drops.
    pub fn activate_release(&mut self) -> Result<()> {
        if self.grab.pending.take().is_some() {
            return self.activate();
        }
        self.release_grab();
        Ok(())
    }

    /// The body under the crosshair that can be grabbed: its owner, the body,
    /// where the crosshair meets it and how far that is.
    fn grab_target(&self) -> Option<(FormId, RigidBodyHandle, Vec3, f32)> {
        let (eye, dir) = (self.camera.position, self.camera.forward());
        let (toi, owner, body) = self.physics.raycast_body(eye, dir, GRAB_REACH)?;
        let (owner, body) = (owner?, body?);
        let b = self.physics.world.bodies.get(body)?;
        // Fixed bodies (a sign's bracket, objects held by `SetMotionType`)
        // stay; keyframed ones are grabbed (their `OnGrab` may let them go).
        if b.is_fixed() || owner == PLAYER_REF || self.is_disabled(owner) {
            return None;
        }
        // Living actors aren't grabbed: only the dead lie as ragdolls.
        Some((owner, body, eye + dir * toi, toi))
    }

    /// Grab what is under the crosshair (Activate held, the console's `grab`).
    pub fn start_grab(&mut self) -> Option<FormId> {
        self.grab.pending = None;
        if self.grab.held.is_some() || self.disabled_controls.activate {
            return None;
        }
        let (owner, body, at, toi) = self.grab_target()?;
        let (_, local) = self.physics.body_local_point(body, at)?;
        self.disturb(owner);
        let mass = self.physics.joined_mass(body);
        self.grab.held = Some(Held {
            owner,
            body,
            local,
            distance: toi.max(HOLD_NEAREST),
            mass,
        });
        self.physics.held = Some(body);
        log::info!("player grabs {owner} ({mass:.1} kg)");
        self.send_script_event(owner, "OnGrab", Vec::new());
        Some(owner)
    }

    /// Drop what the player holds, moving as it was (up to `DROP_SPEED`).
    pub fn release_grab(&mut self) -> Option<FormId> {
        let held = self.grab.held.take()?;
        self.physics.held = None;
        self.physics.cap_speed(held.body, DROP_SPEED);
        log::info!("player lets go of {}", held.owner);
        self.send_script_event(held.owner, "OnRelease", Vec::new());
        Some(held.owner)
    }

    /// What the player holds (`GetPlayerGrabbedRef`).
    pub fn grabbed_ref(&self) -> Option<FormId> {
        self.grab.held.map(|h| h.owner)
    }

    /// Before each physics step: Activate held long enough grabs; what is
    /// held is pulled to where the player looks; it drops when out of reach,
    /// gone, disabled, or the player can't hold it (a menu, a conversation,
    /// death, activation disabled).
    pub(crate) fn update_grab(&mut self, dt: f32) {
        if let Some(t) = self.grab.pending.as_mut() {
            *t += dt;
            if *t >= GRAB_HOLD && self.start_grab().is_none() {
                self.grab.pending = None;
            }
        }
        let Some(held) = self.grab.held.as_mut() else {
            return;
        };
        // Again each step: a keyframed body its `OnGrab` lets go weighs
        // nothing until then; joints break.
        held.mass = self.physics.joined_mass(held.body);
        let held = *held;
        let busy = self.menu.is_some()
            || self.conversation.is_some()
            || self.player_dead()
            || self.disabled_controls.activate
            || self.is_disabled(held.owner);
        let target = self.camera.position + self.camera.forward() * held.distance;
        let max_force = self.grab_max_force();
        match self.physics.drive_held(
            held.body,
            held.local,
            target,
            held.mass,
            max_force,
            dt.clamp(1.0 / 240.0, 1.0 / 30.0),
        ) {
            Some(off) if off <= LET_GO && !busy => {}
            Some(off) => {
                log::debug!(
                    "{} slips from the player's grasp ({off:.0} away)",
                    held.owner
                );
                self.release_grab();
            }
            None => {
                // Its body is gone (its cell unloaded, a script took it).
                self.grab.held = None;
                self.physics.held = None;
                self.send_script_event(held.owner, "OnRelease", Vec::new());
            }
        }
    }

    /// The most force the player holds things with (game units): enough to
    /// lift `fZKeyMaxForceWeightHigh` kg; heavier things are dragged.
    fn grab_max_force(&self) -> f32 {
        crate::ai::combat::gmst_f32(&self.lo, "fZKeyMaxForceWeightHigh", 150.0)
            * crate::physics::GRAVITY
    }

    /// Turn the view onto the middle of what `r` owns that moves (the
    /// console's `grab <ref>`); a held test camera turns with it.
    pub fn look_at_bodies(&mut self, r: FormId) -> bool {
        let Some(at) = self.physics.owner_center(r) else {
            return false;
        };
        let d = at - self.camera.position;
        self.camera.yaw = d.x.atan2(d.y);
        self.camera.pitch = (d.z / d.length().max(1.0)).asin();
        if let Some(t) = self.test_camera.as_mut() {
            (t.1, t.2) = (self.camera.yaw, self.camera.pitch);
        }
        true
    }

    /// What the crosshair is on, for the console.
    pub fn crosshair_on(&self) -> String {
        let (eye, dir) = (self.camera.position, self.camera.forward());
        match self.physics.raycast_body(eye, dir, GRAB_REACH) {
            None => "nothing in reach".into(),
            Some((toi, owner, body)) => {
                let kind =
                    body.and_then(|b| self.physics.world.bodies.get(b))
                        .map_or("no body", |b| {
                            if b.is_fixed() {
                                "fixed"
                            } else if b.is_dynamic() {
                                "dynamic"
                            } else {
                                "keyframed"
                            }
                        });
                format!("{owner:?} {toi:.0} ahead ({kind})")
            }
        }
    }

    /// Console `grab`: what the player holds, or grabs now.
    pub fn describe_grab(&self) -> String {
        match self.grab.held {
            Some(h) => {
                let at = self
                    .physics
                    .body_pose(h.body)
                    .map(|(m, _)| m.transform_point3(h.local))
                    .unwrap_or_default();
                format!(
                    "holding {} ({:.1} kg) {:.0} ahead, at {:.0} {:.0} {:.0}",
                    h.owner, h.mass, h.distance, at.x, at.y, at.z
                )
            }
            None => "holding nothing".into(),
        }
    }
}
