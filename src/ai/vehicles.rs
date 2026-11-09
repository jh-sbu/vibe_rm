//! Scripted carriage passengers and physical horse tethers.
use super::*;
use glam::Vec2;
use rapier3d::prelude::RigidBodyHandle;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Passenger {
    pub vehicle: FormId,
    pub offset: Vec3,
    pub rotation: Vec3,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Tether {
    pub horse: FormId,
    /// Position of the hitch in the horse's frame and in the cart reference.
    pub horse_point: Vec3,
    pub cart_point: Vec3,
    pub rotation: Vec3,
}
/// A loaded cart hitched to its horse: its bodies are keyframed, the cart
/// trailing the hitch with its wheels on the ground (`update_tethers`).
pub(crate) struct Hitch {
    /// The cart's bodies with their frames in the cart reference (scale
    /// included), and for a wheel its radius and how far it has turned.
    bodies: Vec<(RigidBodyHandle, Mat4, Option<(f32, f32)>)>,
    /// Where its wheels touch the ground, in the cart reference's frame.
    contacts: Vec<Vec3>,
    /// The rear axle's distance behind the hitch, and where it is (world):
    /// the trailer's memory of which way it points.
    reach: f32,
    rear: Vec3,
    /// The ground's slope across and along the cart, eased in.
    slope: Vec2,
}

/// Wheels: the bodies reaching within this of the lowest (game units).
const WHEEL_SPAN: f32 = 15.0;
/// How far above the hitch and below it to look for the ground.
const ABOVE: f32 = 300.0;
const BELOW: f32 = 600.0;
/// The steepest a hitched cart leans (rise over run), and how much of the
/// way to the ground's slope it goes each frame (no source).
const MAX_SLOPE: f32 = 0.8;
const SLOPE_EASE: f32 = 0.2;

impl Engine {
    fn vehicle_transform(&self, r: FormId) -> Option<Mat4> {
        if let Some(l) = self
            .cells
            .values()
            .flat_map(|rt| &rt.loose)
            .find(|l| l.ref_id == r)
        {
            return Some(l.transform);
        }
        let (pos, rotation) = self
            .execution(r)
            .filter(|_| self.actor_ref(r).is_none())
            .map(|x| (x.pos, Vec3::new(0., 0., x.heading)))
            .unwrap_or((self.ref_position(r)?, self.ref_rotation(r)));
        Some(Mat4::from_rotation_translation(
            crate::world::records::rotation_from_euler(rotation),
            pos,
        ))
    }

    pub(crate) fn set_vehicle(&mut self, actor: FormId, vehicle: Option<FormId>) {
        let Some(vehicle) = vehicle.filter(|&v| v != actor && !v.is_null()) else {
            self.passengers.remove(&actor);
            let capsule = self.actor_mut(actor).and_then(|a| {
                a.attachment = None;
                a.exec.next_eval = 0.;
                a.capsule
            });
            if let Some(c) = capsule {
                self.physics.set_enabled(&[c], true);
            }
            return;
        };
        if !self.is_actor_ref(actor)
            || (self.passengers.contains_key(&vehicle)
                || self.passengers.values().any(|p| p.vehicle == actor))
        {
            return;
        }
        let Some(transform) = self.vehicle_transform(vehicle) else {
            return;
        };
        let Some(pos) = self.ref_position(actor) else {
            return;
        };
        // On a cart, the root goes to the cart's origin, facing its way: the
        // seat is in the idle the scripts play next (`IdleCartPrisonerASway`'s
        // clip has the body 200 back and 139 up from the root, seat B to D and
        // the driver's elsewhere). MQ101 seats its other cart's prisoners
        // from where they stand beside the player's.
        let (offset, rotation) = if self.is_actor_ref(vehicle) {
            (
                transform.inverse().transform_point3(pos),
                self.ref_rotation(actor) - self.ref_rotation(vehicle),
            )
        } else {
            (Vec3::ZERO, Vec3::ZERO)
        };
        log::debug!(target: "carts", "{actor} rides {vehicle} at {offset:.0?}");
        self.passengers.insert(
            actor,
            Passenger {
                vehicle,
                offset,
                rotation,
            },
        );
        self.dismount(actor);
    }

    pub(crate) fn tether_to_horse(&mut self, cart: FormId, horse: Option<FormId>) {
        self.remove_hitch(cart);
        self.tethers.remove(&cart);
        let Some(horse) = horse.filter(|&h| h != cart && self.is_actor_ref(h)) else {
            return;
        };
        let Some(cart_transform) = self.vehicle_transform(cart) else {
            return;
        };
        let Some(horse_transform) = self.vehicle_transform(horse) else {
            return;
        };
        // The vanilla moving cart has an articulated FrontBar. Use its forward
        // tip as a hitch, preserving the placed relation to the horse.
        let cart_point = Vec3::new(0., 180., 53.);
        let point = cart_transform.transform_point3(cart_point);
        self.tethers.insert(
            cart,
            Tether {
                horse,
                horse_point: horse_transform.inverse().transform_point3(point),
                cart_point,
                rotation: self.ref_rotation(cart) - self.ref_rotation(horse),
            },
        );
    }

    fn remove_hitch(&mut self, cart: FormId) {
        if let Some(h) = self.hitches.remove(&cart) {
            for (body, ..) in h.bodies {
                self.physics.set_body_keyframed(body, false);
            }
        }
    }

    pub(crate) fn clear_vehicle_bodies(&mut self) {
        let actors = self.passengers.keys().copied().collect::<Vec<_>>();
        for actor in actors {
            if let Some(a) = self.actor_mut(actor) {
                a.attachment = None;
                if let Some(c) = a.capsule {
                    self.physics.set_enabled(&[c], true);
                }
            }
        }
        for cart in self.hitches.keys().copied().collect::<Vec<_>>() {
            self.remove_hitch(cart);
        }
    }

    /// Hitched carts follow their horses: each frame the hitch goes where the
    /// horse holds it, the rear axle trails it (a trailer's track), and the
    /// cart sits on the ground under its wheels. Its bodies are keyframed
    /// while hitched (`known_ambiguities/carriages.md`); hitches are made
    /// lazily after a cell load and let go when the tether goes.
    pub(crate) fn update_tethers(&mut self) {
        for cart in self.hitches.keys().copied().collect::<Vec<_>>() {
            let h = &self.hitches[&cart];
            if !self.tethers.contains_key(&cart)
                || h.bodies
                    .iter()
                    .any(|(b, ..)| self.physics.world.bodies.get(*b).is_none())
            {
                self.remove_hitch(cart);
            }
        }
        let mut carts = self.tethers.keys().copied().collect::<Vec<_>>();
        carts.sort();
        for cart in carts {
            let t = self.tethers[&cart];
            if self.is_disabled(cart) || self.is_disabled(t.horse) || self.is_dead(t.horse) {
                self.remove_hitch(cart);
                continue;
            }
            let Some(horse) = self.vehicle_transform(t.horse) else {
                continue;
            };
            let point = horse.transform_point3(t.horse_point);
            if !self.hitches.contains_key(&cart) && !self.make_hitch(cart, &t) {
                continue;
            }
            self.pull_cart(cart, &t, point);
        }
    }

    /// Keyframe a loaded cart's bodies and find its wheels.
    fn make_hitch(&mut self, cart: FormId, t: &Tether) -> bool {
        let Some(l) = self
            .cells
            .values()
            .flat_map(|rt| &rt.loose)
            .find(|l| l.ref_id == cart)
        else {
            return false;
        };
        let transform = l.transform;
        let scale = transform.to_scale_rotation_translation().0.x;
        let unscale = Mat4::from_scale(Vec3::splat(1. / scale.max(1e-3)));
        // Each body's bounds in the cart's frame; wheels are the bodies
        // reaching lowest, besides the cart's own.
        let parts: Vec<(RigidBodyHandle, Mat4, Option<(Vec3, Vec3)>, bool)> = l
            .bodies
            .iter()
            .enumerate()
            .map(|(i, b)| {
                let local = self.physics.body_local_bounds(b.handle).map(|(lo, hi)| {
                    let to_cart = b.frame * unscale;
                    let (a, c) = (to_cart.transform_point3(lo), to_cart.transform_point3(hi));
                    (a.min(c), a.max(c))
                });
                (b.handle, b.frame, local, i == l.root)
            })
            .collect();
        let lowest = parts
            .iter()
            .filter_map(|(.., bounds, _)| bounds.map(|(lo, _)| lo.z))
            .fold(f32::INFINITY, f32::min);
        let mut contacts = Vec::new();
        let bodies = parts
            .iter()
            .map(|&(h, frame, bounds, root)| {
                let wheel = bounds.filter(|(lo, _)| !root && lo.z < lowest + WHEEL_SPAN);
                let wheel = wheel.map(|(lo, hi)| {
                    let mid = (lo + hi) * 0.5;
                    contacts.push(Vec3::new(mid.x, mid.y, lo.z));
                    (mid.z - lo.z, 0.)
                });
                (h, frame, wheel)
            })
            .collect::<Vec<_>>();
        if contacts.len() < 3 {
            // No wheels to speak of: the corners of the cart's underside.
            let (lo, hi) = parts
                .iter()
                .filter_map(|(.., bounds, _)| *bounds)
                .reduce(|(a, b), (c, d)| (a.min(c), b.max(d)))
                .unwrap_or((Vec3::splat(-1.), Vec3::splat(1.)));
            contacts = vec![
                Vec3::new(lo.x, lo.y, lo.z),
                Vec3::new(hi.x, lo.y, lo.z),
                Vec3::new(lo.x, hi.y, lo.z),
                Vec3::new(hi.x, hi.y, lo.z),
            ];
        }
        let rear_y = contacts.iter().map(|c| c.y).fold(f32::INFINITY, f32::min);
        for (h, ..) in &bodies {
            self.physics.set_body_keyframed(*h, true);
        }
        log::debug!(
            target: "carts",
            "{cart} hitched to {}: {} wheels, rear axle {:.0} behind the hitch",
            t.horse,
            bodies.iter().filter(|b| b.2.is_some()).count(),
            t.cart_point.y - rear_y
        );
        self.hitches.insert(
            cart,
            Hitch {
                bodies,
                contacts,
                reach: t.cart_point.y - rear_y,
                rear: transform.transform_point3(Vec3::new(0., rear_y, 0.)),
                slope: Vec2::NAN,
            },
        );
        true
    }

    /// Bring a hitched cart after its horse's hitch point (world).
    fn pull_cart(&mut self, cart: FormId, t: &Tether, point: Vec3) {
        let Some(scale) = self
            .cells
            .values()
            .flat_map(|rt| &rt.loose)
            .find(|l| l.ref_id == cart)
            .map(|l| l.transform.to_scale_rotation_translation().0.x)
        else {
            return;
        };
        let h = &self.hitches[&cart];
        // The rear axle trails the hitch at its distance.
        let along = (point - h.rear).truncate();
        let Some(fwd) = along.try_normalize() else {
            return;
        };
        let reach = h.reach * scale;
        let rear = point - (fwd * reach).extend(0.);
        let travelled = (rear - h.rear).truncate().length();
        let right = Vec2::new(fwd.y, -fwd.x);
        // Level first: where the wheels are, and the ground under them.
        let origin = point.truncate() - (right * t.cart_point.x + fwd * t.cart_point.y) * scale;
        let samples: Vec<(Vec3, f32)> = h
            .contacts
            .iter()
            .map(|c| {
                let at = origin + (right * c.x + fwd * c.y) * scale;
                let above = point.z + ABOVE;
                let z = self
                    .physics
                    .fixed_below(at.extend(above), ABOVE + BELOW)
                    .map_or(point.z, |d| above - d);
                (*c * scale, z)
            })
            .collect();
        // The plane through them (least squares, in the cart's level frame).
        let n = samples.len() as f32;
        let mean = samples
            .iter()
            .fold((Vec3::ZERO, 0.), |(p, z), (c, g)| (p + *c, z + g));
        let (mc, mz) = (mean.0 / n, mean.1 / n);
        let (mut sxx, mut syy, mut sxy, mut sxz, mut syz) = (0., 0., 0., 0., 0.);
        for (c, z) in &samples {
            let (dx, dy, dz) = (c.x - mc.x, c.y - mc.y, z - mz);
            sxx += dx * dx;
            syy += dy * dy;
            sxy += dx * dy;
            sxz += dx * dz;
            syz += dy * dz;
        }
        let det = sxx * syy - sxy * sxy;
        let target = if det.abs() > 1e-3 {
            Vec2::new((sxz * syy - syz * sxy) / det, (syz * sxx - sxz * sxy) / det)
        } else {
            Vec2::ZERO
        }
        .clamp(Vec2::splat(-MAX_SLOPE), Vec2::splat(MAX_SLOPE));
        // A stone under one wheel doesn't throw it over at once.
        let slope = if h.slope.is_nan() {
            target
        } else {
            h.slope + (target - h.slope) * SLOPE_EASE
        };
        let (roll, pitch) = (slope.x, slope.y);
        let x = (right.extend(0.) + Vec3::Z * roll).normalize();
        let y0 = (fwd.extend(0.) + Vec3::Z * pitch).normalize();
        let z = x.cross(y0).normalize();
        let y = z.cross(x);
        let turn = glam::Mat3::from_cols(x, y, z);
        // Its wheels' undersides on that plane.
        let level = turn * mc;
        let at = origin.extend(mz - level.z);
        let transform = Mat4::from_cols(
            (x * scale).extend(0.),
            (y * scale).extend(0.),
            (z * scale).extend(0.),
            at.extend(1.),
        );
        let unscale = Mat4::from_scale(Vec3::splat(1. / scale.max(1e-3)));
        let h = self.hitches.get_mut(&cart).unwrap();
        h.rear = rear;
        h.slope = slope;
        let mut moves = Vec::with_capacity(h.bodies.len());
        for (body, frame, wheel) in h.bodies.iter_mut() {
            let spin = match wheel {
                Some((radius, angle)) => {
                    *angle += travelled / (*radius * scale).max(1.);
                    Mat4::from_rotation_x(-*angle)
                }
                None => Mat4::IDENTITY,
            };
            moves.push((*body, transform * *frame * unscale * spin));
        }
        for (body, m) in moves {
            self.physics.move_keyframed(body, m);
        }
        log::trace!(
            target: "carts",
            "{cart} at {:.0?}, pitch {:.2}, roll {:.2}",
            at,
            pitch,
            roll
        );
    }

    /// Away carts retain their hitch relation without simulating wheel bodies.
    pub(super) fn sync_away_vehicles(&mut self, horse: FormId) {
        let Some((place, pos, heading)) = self
            .execution(horse)
            .and_then(|x| Some((x.place?, x.pos, x.heading)))
        else {
            return;
        };
        let carts = self
            .tethers
            .iter()
            .filter(|(_, t)| t.horse == horse)
            .map(|(&r, &t)| (r, t))
            .collect::<Vec<_>>();
        for (cart, t) in carts {
            if self
                .cells
                .values()
                .any(|rt| rt.refs.contains(&cart) || rt.loose.iter().any(|l| l.ref_id == cart))
            {
                continue;
            }
            let horse_turn = crate::world::records::rotation_from_euler(Vec3::new(0., 0., heading));
            let rotation = Vec3::new(0., 0., heading) + t.rotation;
            let turn = crate::world::records::rotation_from_euler(rotation);
            let point = pos + horse_turn * t.horse_point;
            let cart_pos = point - turn * t.cart_point;
            self.move_ref(cart, horse, cart_pos, rotation);
            let passengers = self
                .passengers
                .iter()
                .filter(|(_, p)| p.vehicle == cart)
                .map(|(&r, &p)| (r, p))
                .collect::<Vec<_>>();
            for (actor, p) in passengers {
                if actor == PLAYER_REF {
                    continue;
                }
                let pos = cart_pos + turn * p.offset;
                if self.actor_ref(actor).is_some() {
                    self.move_ref(actor, cart, pos, rotation + p.rotation);
                } else if self.run_away(actor) {
                    self.put_away(actor, place, pos);
                }
            }
        }
    }

    pub(super) fn actor_node_transform(&self, actor: FormId, name: &str) -> Option<Mat4> {
        let a = self.actor_ref(actor)?;
        let bone = a.skeleton.find(name)?;
        let bind = a.skeleton.model_space(&[]);
        let key = self.actor_cells.get(&actor)?;
        let index = self
            .cells
            .get(key)?
            .actors
            .iter()
            .position(|a| a.ref_id == actor)?;
        let inst = self.scene.cells.get(key)?.actors.get(index)?;
        Some(
            a.transform()
                * *inst.pose.get(bone)?
                * bind.get(bone)?.inverse()
                * Mat4::from_translation(bind[bone].w_axis.truncate()),
        )
    }

    pub(crate) fn sync_passengers(&mut self) {
        let mut passengers = self
            .passengers
            .iter()
            .map(|(&r, &p)| (r, p))
            .collect::<Vec<_>>();
        passengers.sort_by_key(|p| p.0);
        for (actor, p) in passengers {
            if self.is_dead(actor)
                || self.is_disabled(actor)
                || self.is_disabled(p.vehicle)
                || (self.is_actor_ref(p.vehicle) && self.is_dead(p.vehicle))
            {
                self.set_vehicle(actor, None);
                continue;
            }
            let Some(vehicle) = self.vehicle_transform(p.vehicle) else {
                continue;
            };
            let talon = self
                .flights
                .get(&p.vehicle)
                .filter(|f| f.grabbed == Some(actor))
                .and_then(|_| self.actor_node_transform(p.vehicle, "NPC RLegFoot"));
            let transform = talon.unwrap_or(
                vehicle
                    * Mat4::from_rotation_translation(
                        crate::world::records::rotation_from_euler(p.rotation),
                        p.offset,
                    ),
            );
            let pos = transform.w_axis.truncate();
            if actor == PLAYER_REF {
                self.player.position =
                    pos + Vec3::Z * (self.physics.player_half_height + self.physics.player_radius);
                self.player.vertical_velocity = 0.;
                self.camera.position = self.player.eye();
            } else if let Some(a) = self.actor_mut(actor) {
                a.exec.pos = pos;
                a.exec.heading = self::heading(transform);
                a.attachment = Some(transform);
                a.velocity = Vec3::ZERO;
                a.halt(3600.);
                if let Some(c) = a.capsule {
                    self.physics.set_enabled(&[c], false);
                }
                if let Some(&key) = self.actor_cells.get(&actor)
                    && let Some(i) = self.cells[&key]
                        .actors
                        .iter()
                        .position(|a| a.ref_id == actor)
                    && let Some(inst) = self
                        .scene
                        .cells
                        .get_mut(&key)
                        .and_then(|c| c.actors.get_mut(i))
                {
                    inst.transform = transform;
                }
            } else if let Some((place, _)) = self.current_place_of(p.vehicle) {
                if self.away.contains_key(&actor) {
                    self.put_away(actor, place, pos);
                }
            }
        }
    }
}
fn heading(m: Mat4) -> f32 {
    let forward = m.y_axis;
    forward.x.atan2(forward.y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::exec::tests::{ACTOR, CELL, MARKER, fixture, loaded_actor};

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn cart_passengers_ride_at_its_origin_save_and_detach_without_restarting_packages() {
        let (_f, mut e) = fixture();
        loaded_actor(&mut e);
        e.location = crate::engine::Location::Interior(CELL);
        // Wherever it stands, its seat is the cart's origin (the idle seats it).
        e.set_vehicle(ACTOR, Some(MARKER));
        let p = e.passengers[&ACTOR];
        assert_eq!((p.offset, p.rotation), (Vec3::ZERO, Vec3::ZERO));
        e.moved_refs.insert(MARKER, Vec3::X * 3500.);
        e.sync_passengers();
        assert_eq!(e.actor_ref(ACTOR).unwrap().exec.pos, Vec3::X * 3500.);
        let file = crate::save::state::Snapshot::capture(&e).write();
        assert_eq!(
            crate::save::state::Snapshot::read(&file)
                .unwrap()
                .continuity
                .passengers,
            e.passengers
        );
        let here = e.actor_ref(ACTOR).unwrap().exec.pos;
        e.set_vehicle(ACTOR, None);
        e.moved_refs.insert(MARKER, Vec3::X * 4000.);
        e.sync_passengers();
        assert_eq!(e.actor_ref(ACTOR).unwrap().exec.pos, here);
        assert!(e.actor_ref(ACTOR).unwrap().attachment.is_none());
        e.set_vehicle(PLAYER_REF, Some(MARKER));
        // The synthetic fixture has no player NPC record: exercise the dynamic
        // keyword through a valid passenger entry, as a vanilla player supplies.
        e.passengers.insert(
            PLAYER_REF,
            Passenger {
                vehicle: MARKER,
                offset: Vec3::Z * 100.,
                rotation: Vec3::ZERO,
            },
        );
        assert!(e.has_keyword(PLAYER_REF, FormId(0xDD631)));
        e.set_vehicle(PLAYER_REF, None);
        assert!(!e.has_keyword(PLAYER_REF, FormId(0xDD631)));
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn tether_keyframes_the_cart_after_its_horse_rebuilds_after_unload_and_releases() {
        use rapier3d::prelude::{ColliderBuilder, RigidBodyBuilder};
        let (_f, mut e) = fixture();
        loaded_actor(&mut e);
        e.location = crate::engine::Location::Interior(CELL);
        let body = e
            .physics
            .world
            .insert_body(RigidBodyBuilder::dynamic().translation(Vec3::X * 3000.));
        e.physics
            .world
            .insert_collider(ColliderBuilder::ball(10.).mass(10.).build(), Some(body));
        e.cells
            .get_mut(&crate::render::CellKey::Interior(CELL))
            .unwrap()
            .loose
            .push(crate::loose::Loose {
                ref_id: MARKER,
                bodies: vec![crate::loose::LooseBody {
                    handle: body,
                    frame: Mat4::IDENTITY,
                    world: Mat4::from_translation(Vec3::X * 3000.),
                    instances: vec![],
                }],
                root: 0,
                transform: Mat4::from_translation(Vec3::X * 3000.),
                start: Mat4::IDENTITY,
                moved: false,
                lost: 0,
                age: 10,
                fresh: false,
            });
        e.tether_to_horse(MARKER, Some(ACTOR));
        e.update_tethers();
        assert_eq!(e.hitches.len(), 1);
        let keyframed = |e: &Engine| e.physics.world.bodies[body].is_kinematic();
        assert!(keyframed(&e));
        let before = e.physics.body_pose(body).unwrap().0.w_axis;
        // Ahead (the cart faces +Y), as a trailer: it comes on behind.
        e.actor_mut(ACTOR).unwrap().exec.pos.y += 100.;
        for _ in 0..3 {
            e.update_tethers();
            e.physics.step(1. / 60.);
        }
        let pose = e.physics.body_pose(body).unwrap().0.w_axis;
        assert!(
            (pose.y - before.y - 100.).abs() < 10. && (pose.x - before.x).abs() < 10.,
            "pulled along: {pose} from {before}"
        );
        // Nothing under it: it keeps to the hitch's height, it doesn't fall.
        assert!((pose.z - before.z).abs() < 100., "{pose} from {before}");
        e.clear_vehicle_bodies();
        assert!(e.hitches.is_empty() && !keyframed(&e));
        e.update_tethers();
        assert_eq!(e.hitches.len(), 1);
        assert!(keyframed(&e));
        e.tether_to_horse(MARKER, None);
        assert!(e.hitches.is_empty());
        assert!(e.tethers.is_empty());
        assert!(!keyframed(&e), "let go to the simulation");
    }
}

#[cfg(test)]
mod away_tests {
    use super::*;
    use crate::ai::exec::tests::{ACTOR, BASE, CELL, MARKER, fixture, loaded_actor};
    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn away_horse_moves_its_cart_and_passenger_after_unloading() {
        let (_f, mut e) = fixture();
        loaded_actor(&mut e);
        e.location = crate::engine::Location::Interior(CELL);
        let passenger = FormId(0xFF000820);
        e.created_refs.refs.insert(
            passenger,
            crate::created::Created {
                base: BASE,
                position: Vec3::new(3000., 10., 100.),
                rotation: Vec3::ZERO,
                origin: None,
                place: Some(Place::Interior(CELL)),
                container: None,
                location: None,
                actor: true,
                count: 1,
                owner: None,
            },
        );
        e.set_vehicle(passenger, Some(MARKER));
        e.tether_to_horse(MARKER, Some(ACTOR));
        e.unload_all();
        e.location = crate::engine::Location::Nowhere;
        let before = e.ref_position(MARKER).unwrap();
        e.away.get_mut(&ACTOR).unwrap().pos.x += 400.;
        e.sync_away_vehicles(ACTOR);
        let after = e.ref_position(MARKER).unwrap();
        assert!((after - before - Vec3::X * 400.).length() < 0.01);
        assert!((e.away[&passenger].pos - after).length() < 0.01);
    }
}
