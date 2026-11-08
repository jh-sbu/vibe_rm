//! `TranslateTo` and its kin: a reference moved smoothly to a place and
//! orientation at a speed in units a second, scripts told when it is almost
//! there (`OnTranslationAlmostComplete`, at 90% of the way: the Papyrus
//! index's "default is 90%"), there (`OnTranslationComplete`), or that it
//! couldn't go on (`OnTranslationFailed`). Without a rotation cap the
//! rotation arrives with the position; a cap only slows it (Papyrus index,
//! `TranslateTo`). Choices: `known_ambiguities/translation.md`.

use esp::FormId;
use glam::Vec3;
use papyrus::Value;
use std::f32::consts::TAU;

use crate::engine::Engine;

/// The share of the way at which `OnTranslationAlmostComplete` comes.
const ALMOST: f32 = 0.9;
/// Within this of the target angles (radians), the rotation is there.
const TURNED: f32 = 1e-3;

#[derive(Debug, Clone, PartialEq)]
pub struct Translation {
    from: Vec3,
    to: Vec3,
    /// Euler angles (radians) at the start and at the end.
    rot_from: Vec3,
    rot_to: Vec3,
    speed: f32,
    /// Degrees a second; 0 for no cap.
    max_rotation: f32,
    /// Hermite tangents (start, end) for a spline translation.
    tangents: Option<(Vec3, Vec3)>,
    length: f32,
    travelled: f32,
    rot: Vec3,
    almost_sent: bool,
}

/// An angle difference brought to the shorter way round.
fn wrap(a: f32) -> f32 {
    (a + std::f32::consts::PI).rem_euclid(TAU) - std::f32::consts::PI
}

/// Where a reference turned by `rot` faces (Skyrim's forward is +Y).
fn forward(rot: Vec3) -> Vec3 {
    crate::world::records::rotation_from_euler(rot) * Vec3::Y
}

impl Translation {
    fn point(&self, t: f32) -> Vec3 {
        match self.tangents {
            None => self.from.lerp(self.to, t),
            Some((m0, m1)) => {
                let (t2, t3) = (t * t, t * t * t);
                self.from * (2.0 * t3 - 3.0 * t2 + 1.0)
                    + m0 * (t3 - 2.0 * t2 + t)
                    + self.to * (-2.0 * t3 + 3.0 * t2)
                    + m1 * (t3 - t2)
            }
        }
    }

    fn new(
        from: Vec3,
        rot_from: Vec3,
        to: Vec3,
        rot_to: Vec3,
        speed: f32,
        max_rotation: f32,
        tangent: Option<f32>,
    ) -> Self {
        let mut tr = Translation {
            from,
            to,
            rot_from,
            rot_to,
            speed: speed.max(0.0),
            max_rotation: max_rotation.max(0.0),
            tangents: tangent.map(|m| (forward(rot_from) * m, forward(rot_to) * m)),
            length: 0.0,
            travelled: 0.0,
            rot: rot_from,
            almost_sent: false,
        };
        // The path's length, from 32 steps along it.
        let mut last = from;
        for i in 1..=32 {
            let p = tr.point(i as f32 / 32.0);
            tr.length += p.distance(last);
            last = p;
        }
        tr
    }

    /// The share of the way gone, 0 to 1.
    fn progress(&self) -> f32 {
        if self.length <= f32::EPSILON {
            1.0
        } else {
            (self.travelled / self.length).min(1.0)
        }
    }

    /// One step of `dt` seconds: the new position and angles.
    fn step(&mut self, dt: f32) -> (Vec3, Vec3) {
        self.travelled += self.speed * dt;
        let t = self.progress();
        let wanted = self.rot_from
            + Vec3::new(
                wrap(self.rot_to.x - self.rot_from.x),
                wrap(self.rot_to.y - self.rot_from.y),
                wrap(self.rot_to.z - self.rot_from.z),
            ) * t;
        let mut turn = Vec3::new(
            wrap(wanted.x - self.rot.x),
            wrap(wanted.y - self.rot.y),
            wrap(wanted.z - self.rot.z),
        );
        if self.max_rotation > 0.0 {
            let most = self.max_rotation.to_radians() * dt;
            turn = turn.clamp(Vec3::splat(-most), Vec3::splat(most));
        }
        self.rot += turn;
        (self.point(t), self.rot)
    }

    fn turned(&self) -> bool {
        let d = Vec3::new(
            wrap(self.rot_to.x - self.rot.x),
            wrap(self.rot_to.y - self.rot.y),
            wrap(self.rot_to.z - self.rot.z),
        );
        d.abs().max_element() < TURNED
    }
}

impl Engine {
    /// Start moving `r` to `to`, turned to `rot_to` (radians), replacing a
    /// translation it is on. One not loaded fails at once.
    pub(crate) fn start_translation(
        &mut self,
        r: FormId,
        to: Vec3,
        rot_to: Vec3,
        speed: f32,
        max_rotation: f32,
        tangent: Option<f32>,
    ) {
        let Some(from) = self.ref_position(r).filter(|_| self.translatable(r)) else {
            self.send_script_event(r, "OnTranslationFailed", Vec::new());
            return;
        };
        if !to.is_finite() || !rot_to.is_finite() || !speed.is_finite() || speed <= 0.0 {
            self.send_script_event(r, "OnTranslationFailed", Vec::new());
            return;
        }
        let rot_from = self.ref_rotation(r);
        let tr = Translation::new(from, rot_from, to, rot_to, speed, max_rotation, tangent);
        self.world_state.translations.insert(r, tr);
    }

    /// `StopTranslation`: where it is. No event (`known_ambiguities/translation.md`).
    pub(crate) fn stop_translation(&mut self, r: FormId) {
        self.world_state.translations.remove(&r);
    }

    /// Move each translating reference on by `dt` world seconds.
    pub(crate) fn update_translations(&mut self, dt: f32) {
        if self.world_state.translations.is_empty() || dt <= 0.0 {
            return;
        }
        let ids: Vec<FormId> = self.world_state.translations.keys().copied().collect();
        for r in ids {
            if !self.translatable(r) {
                self.world_state.translations.remove(&r);
                self.send_script_event(r, "OnTranslationFailed", Vec::new());
                continue;
            }
            let Some(tr) = self.world_state.translations.get_mut(&r) else {
                continue;
            };
            let (pos, rot) = tr.step(dt);
            let almost = !tr.almost_sent && tr.progress() >= ALMOST;
            if almost {
                tr.almost_sent = true;
            }
            let done = tr.progress() >= 1.0 && tr.turned();
            self.move_ref(r, r, pos, rot);
            if almost {
                self.send_script_event(r, "OnTranslationAlmostComplete", Vec::new());
            }
            if done {
                self.world_state.translations.remove(&r);
                self.send_script_event(r, "OnTranslationComplete", Vec::new());
            }
        }
    }

    /// In a loaded cell, drawn or not (markers and water planes move too:
    /// `TG08RisingWaterScript`), or a loaded actor.
    fn translatable(&self, r: FormId) -> bool {
        self.actor_cells.contains_key(&r)
            || self.cells.values().any(|rt| rt.refs.contains(&r))
            || self.ref_3d_loaded(r)
    }

    /// Whether a reference is being translated.
    #[cfg(test)]
    pub(crate) fn translating(&self, r: FormId) -> bool {
        self.world_state.translations.contains_key(&r)
    }
}

/// The natives' arguments: degrees to radians.
pub(crate) fn angles(args: &[Value], from: usize) -> Vec3 {
    let f = |i: usize| args.get(i).map_or(0.0, Value::as_float).to_radians();
    Vec3::new(f(from), f(from + 1), f(from + 2))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_straight_translation_reaches_its_end_at_its_speed() {
        let mut tr = Translation::new(
            Vec3::ZERO,
            Vec3::ZERO,
            Vec3::new(100.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            50.0,
            0.0,
            None,
        );
        let (p, r) = tr.step(1.0);
        assert!((p.x - 50.0).abs() < 1e-3 && (r.z - 0.5).abs() < 1e-3);
        let (p, r) = tr.step(1.0);
        assert!((p.x - 100.0).abs() < 1e-3 && (r.z - 1.0).abs() < 1e-3);
        assert!(tr.progress() >= 1.0 && tr.turned());
    }

    #[test]
    fn a_rotation_cap_only_slows_the_turn() {
        // 90 degrees over a 1 s path, capped at 45 a second: there in 2 s.
        let mut tr = Translation::new(
            Vec3::ZERO,
            Vec3::ZERO,
            Vec3::new(100.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 90f32.to_radians()),
            100.0,
            45.0,
            None,
        );
        tr.step(1.0);
        assert!(!tr.turned());
        tr.step(1.0);
        assert!(tr.turned());
    }

    #[test]
    fn angles_turn_the_short_way_round() {
        let mut tr = Translation::new(
            Vec3::ZERO,
            Vec3::new(0.0, 0.0, 350f32.to_radians()),
            Vec3::new(10.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 10f32.to_radians()),
            10.0,
            0.0,
            None,
        );
        let (_, r) = tr.step(0.5);
        assert!(wrap(r.z).abs() < 1e-3, "{}", r.z.to_degrees());
    }

    #[test]
    fn a_spline_starts_and_ends_at_its_points() {
        let tr = Translation::new(
            Vec3::ZERO,
            Vec3::ZERO,
            Vec3::new(100.0, 0.0, 0.0),
            Vec3::ZERO,
            10.0,
            0.0,
            Some(50.0),
        );
        assert_eq!(tr.point(0.0), Vec3::ZERO);
        assert!(tr.point(1.0).distance(Vec3::new(100.0, 0.0, 0.0)) < 1e-3);
        // It leaves along its facing (+Y), so it bows out (an S: both ends
        // face +Y).
        assert!(tr.point(0.25).y > 1.0 && tr.point(0.75).y < -1.0);
        assert!(tr.length > 100.0);
    }
}
