//! Scripted carriage passengers and physical horse tethers.
use super::*;
use rapier3d::prelude::{ImpulseJointHandle, RigidBodyHandle};

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
pub(crate) struct Hitch {
    anchor: RigidBodyHandle,
    cart: RigidBodyHandle,
    joint: ImpulseJointHandle,
}

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
        let rotation = self.ref_rotation(actor) - self.ref_rotation(vehicle);
        self.passengers.insert(
            actor,
            Passenger {
                vehicle,
                offset: transform.inverse().transform_point3(pos),
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
            self.physics.world.remove_impulse_joint(h.joint);
            self.physics.remove_body(h.anchor);
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

    /// Rebuild constraints lazily after a cell load. The cart's NIF wheel and
    /// steering constraints continue to be simulated by the ordinary loose path.
    pub(crate) fn update_tethers(&mut self) {
        use rapier3d::prelude::{GenericJointBuilder, JointAxesMask, RigidBodyBuilder};
        for cart in self.hitches.keys().copied().collect::<Vec<_>>() {
            let h = &self.hitches[&cart];
            if !self.tethers.contains_key(&cart)
                || self.physics.world.bodies.get(h.cart).is_none()
                || self.physics.world.impulse_joints.get(h.joint).is_none()
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
            if let Some(h) = self.hitches.get(&cart) {
                if let Some(anchor) = self.physics.world.bodies.get_mut(h.anchor) {
                    anchor.set_next_kinematic_translation(point);
                }
                if log::log_enabled!(target: "carts", log::Level::Trace)
                    && let Some(m) = self.vehicle_transform(cart)
                {
                    let inv = horse.inverse();
                    log::trace!(
                        target: "carts",
                        "{cart} in {}'s frame: hitch target {:.0?}, cart at {:.0?}, facing {:.2?}",
                        t.horse,
                        t.horse_point,
                        inv.transform_point3(m.w_axis.truncate()),
                        inv.transform_vector3(m.transform_vector3(Vec3::Y)).normalize_or_zero()
                    );
                    let up = m.transform_vector3(Vec3::Z).normalize_or_zero();
                    let hitch = m.transform_point3(t.cart_point);
                    let at = m.w_axis.truncate();
                    log::trace!(
                        target: "carts",
                        "{cart} tilt {:.0} deg, hitch {:.0} from the horse's ({:.0} up), {:.0} above ground",
                        up.angle_between(Vec3::Z).to_degrees(),
                        (hitch - point).truncate().length(),
                        hitch.z - point.z,
                        at.z - self.ground_height(at.truncate()).unwrap_or(f32::NAN),
                    );
                }
                continue;
            }
            let Some(l) = self
                .cells
                .values()
                .flat_map(|rt| &rt.loose)
                .find(|l| l.ref_id == cart)
            else {
                continue;
            };
            // The cart's own (root, heaviest) body. Not the front-most body
            // origin: that was a front wheel, a hair ahead of the bar, which
            // the horse then dragged by its axle. Nor the 100 kg bar: pulling
            // 12000 kg through its hinge tore the joints apart.
            let Some(body) = l.bodies.get(l.root) else {
                continue;
            };
            let cart_body = body.handle;
            log::debug!(
                target: "carts",
                "{cart} hitched by its body at {:.0?}",
                body.frame.w_axis.truncate()
            );
            let Some((world, _)) = self.physics.body_pose(cart_body) else {
                continue;
            };
            let hitch = l.transform.transform_point3(t.cart_point);
            let local = world.inverse().transform_point3(hitch);
            let anchor = self
                .physics
                .world
                .insert_body(RigidBodyBuilder::kinematic_position_based().translation(point));
            // The horse pulls the hitch along the ground; its height is left
            // to the cart's wheels. The bar only steers, so a hitch held at
            // the horse's height levers the cart over where the road's slope
            // changes between the horse and the wheels.
            let joint = self.physics.world.insert_impulse_joint(
                anchor,
                cart_body,
                GenericJointBuilder::new(JointAxesMask::LIN_X | JointAxesMask::LIN_Y)
                    .local_anchor2(local)
                    .contacts_enabled(false),
            );
            self.hitches.insert(
                cart,
                Hitch {
                    anchor,
                    cart: cart_body,
                    joint,
                },
            );
        }
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
    fn passengers_keep_relative_offsets_save_and_detach_without_restarting_packages() {
        let (_f, mut e) = fixture();
        loaded_actor(&mut e);
        e.location = crate::engine::Location::Interior(CELL);
        let before = e.actor_ref(ACTOR).unwrap().exec.pos;
        e.set_vehicle(ACTOR, Some(MARKER));
        let p = e.passengers[&ACTOR];
        assert_eq!(p.offset, before - Vec3::X * 3000.);
        e.moved_refs.insert(MARKER, Vec3::X * 3500.);
        e.sync_passengers();
        assert_eq!(
            e.actor_ref(ACTOR).unwrap().exec.pos,
            before + Vec3::X * 500.
        );
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
    fn tether_creates_a_real_constraint_rebuilds_after_unload_and_releases() {
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
        let old = e.hitches[&MARKER].anchor;
        e.actor_mut(ACTOR).unwrap().exec.pos.x += 100.;
        e.update_tethers();
        for _ in 0..60 {
            e.physics.step(1. / 60.);
        }
        let pose = e.physics.body_pose(body).unwrap().0.w_axis;
        assert!(pose.x > 3050., "pulled along: {pose}");
        // Only along the ground: nothing under it, it falls as the horse
        // goes on above.
        let anchor = e
            .physics
            .body_pose(e.hitches[&MARKER].anchor)
            .unwrap()
            .0
            .w_axis;
        assert!(pose.z < anchor.z - 100., "held up: {pose} under {anchor}");
        e.clear_vehicle_bodies();
        assert!(e.physics.world.bodies.get(old).is_none());
        e.update_tethers();
        assert_eq!(e.hitches.len(), 1);
        e.tether_to_horse(MARKER, None);
        assert!(e.hitches.is_empty());
        assert!(e.tethers.is_empty());
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
        assert!((e.away[&passenger].pos - Vec3::new(3400., 10., 100.)).length() < 0.01);
    }
}
