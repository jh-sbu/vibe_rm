//! Optional KNCK continuity for loaded living ragdolls. Recovery clips restart
//! from the saved physical pose; no graph object IDs are serialized.
use super::codec::{Corrupt, Reader, Result, Writer};
use super::file::SaveFile;
use crate::engine::{Engine, PLAYER_REF};
use crate::knock::{Knock, KnockCause, KnockState};
use esp::FormId;
use glam::{Mat4, Vec3};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SavedKnock {
    pub actor: FormId,
    pub state: KnockState,
    pub cause: KnockCause,
    pub down_for: f32,
    pub rest_for: f32,
    pub peak_z: f32,
    pub fall_applied: bool,
    pub script_hold: bool,
    pub getup_requested: bool,
    pub previous_view: Option<bool>,
    pub transform: Mat4,
    pub pose: Vec<Mat4>,
    pub bodies: Vec<Mat4>,
    pub velocities: Vec<(Vec3, Vec3)>,
}
impl SavedKnock {
    fn of(e: &Engine, actor: FormId, k: &Knock, pose: Vec<Mat4>) -> Self {
        Self {
            actor,
            state: k.state,
            cause: k.cause,
            down_for: k.down_for,
            rest_for: k.rest_for,
            peak_z: k.peak_z,
            fall_applied: k.fall_applied,
            script_hold: k.script_hold,
            getup_requested: k.getup_requested,
            previous_view: k.previous_view,
            transform: k.actor_transform,
            pose,
            bodies: e.physics.ragdoll_bodies(&k.ragdoll),
            velocities: e.physics.ragdoll_velocities(&k.ragdoll),
        }
    }
    fn valid(&self) -> bool {
        !self.actor.is_null()
            && self.state != KnockState::Normal
            && self.down_for.is_finite()
            && self.down_for >= 0.0
            && self.rest_for.is_finite()
            && self.rest_for >= 0.0
            && self.peak_z.is_finite()
            && valid_matrix(self.transform)
            && !self.bodies.is_empty()
            && self.bodies.len() <= 256
            && self.pose.len() <= 4096
            && self
                .pose
                .iter()
                .chain(&self.bodies)
                .all(|&m| valid_matrix(m))
            && self.velocities.len() == self.bodies.len()
            && self
                .velocities
                .iter()
                .all(|(v, w)| v.is_finite() && w.is_finite())
    }
}
fn valid_matrix(m: Mat4) -> bool {
    m.is_finite() && m.determinant().is_finite() && m.determinant().abs() > 1e-8
}

pub(crate) fn capture(e: &Engine) -> Vec<SavedKnock> {
    let mut out: Vec<_> = e.world_state.pending_knocks.values().cloned().collect();
    for (key, rt) in &e.cells {
        for (i, a) in rt.actors.iter().enumerate() {
            if let Some(k) = &a.knock {
                if let Some(inst) = e.scene.cells.get(key).and_then(|r| r.actors.get(i)) {
                    out.push(SavedKnock::of(e, a.ref_id, k, inst.pose.clone()));
                }
            }
        }
    }
    if !e.player_dead() {
        if let (Some(k), Some(inst)) = (&e.player_knock, &e.scene.player) {
            out.push(SavedKnock::of(e, PLAYER_REF, k, inst.pose.clone()));
        }
    }
    out.sort_by_key(|k| k.actor);
    out
}

fn put_matrix(w: &mut Writer, m: Mat4) {
    for x in m.to_cols_array() {
        w.f32(x);
    }
}
fn matrix(r: &mut Reader) -> Result<Mat4> {
    let mut values = [0.0; 16];
    for x in &mut values {
        *x = r.f32()?;
    }
    Ok(Mat4::from_cols_array(&values))
}
pub(crate) fn write(values: &[SavedKnock], file: &mut SaveFile) {
    let mut w = Writer::default();
    w.len(values.len());
    for k in values {
        w.form(k.actor);
        w.u8(k.state as u8);
        w.u8(k.cause as u8);
        w.f32(k.down_for);
        w.f32(k.rest_for);
        w.f32(k.peak_z);
        w.bool(k.fall_applied);
        w.bool(k.script_hold);
        w.bool(k.getup_requested);
        w.u8(k.previous_view.map_or(0, |v| if v { 2 } else { 1 }));
        put_matrix(&mut w, k.transform);
        for list in [&k.pose, &k.bodies] {
            w.len(list.len());
            for &m in list {
                put_matrix(&mut w, m);
            }
        }
        w.len(k.velocities.len());
        for &(v, a) in &k.velocities {
            w.vec3(v);
            w.vec3(a);
        }
    }
    file.add(b"KNCK", w.0);
}
pub(crate) fn read(file: &SaveFile) -> Result<Vec<SavedKnock>> {
    if file
        .sections
        .iter()
        .filter(|(tag, _)| tag == b"KNCK")
        .count()
        > 1
    {
        return Err(Corrupt("duplicate KNCK".into()));
    }
    let Some(bytes) = file.section(b"KNCK") else {
        return Ok(vec![]);
    };
    let mut r = Reader::new(bytes);
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for _ in 0..r.count(90)? {
        let actor = r.form()?;
        let state = match r.u8()? {
            5 => KnockState::Queued,
            6 => KnockState::GetUp,
            7 => KnockState::Down,
            _ => return Err(Corrupt("KNCK state".into())),
        };
        let cause = match r.u8()? {
            0 => KnockCause::Push,
            1 => KnockCause::Area,
            2 => KnockCause::Paralysis,
            3 => KnockCause::Script,
            4 => KnockCause::Fall,
            _ => return Err(Corrupt("KNCK cause".into())),
        };
        let (down_for, rest_for, peak_z) = (r.f32()?, r.f32()?, r.f32()?);
        let fall_applied = r.bool()?;
        let (script_hold, getup_requested) = (r.bool()?, r.bool()?);
        let previous_view = match r.u8()? {
            0 => None,
            1 => Some(false),
            2 => Some(true),
            _ => return Err(Corrupt("KNCK camera".into())),
        };
        let transform = matrix(&mut r)?;
        let mut matrices = || -> Result<Vec<Mat4>> {
            let count = r.count(64)?;
            if count > 4096 {
                return Err(Corrupt("KNCK bone count".into()));
            }
            (0..count).map(|_| matrix(&mut r)).collect()
        };
        let pose = matrices()?;
        let bodies = matrices()?;
        let mut velocities = Vec::new();
        for _ in 0..r.count(24)? {
            velocities.push((r.vec3()?, r.vec3()?));
        }
        let k = SavedKnock {
            actor,
            state,
            cause,
            down_for,
            rest_for,
            peak_z,
            fall_applied,
            script_hold,
            getup_requested,
            previous_view,
            transform,
            pose,
            bodies,
            velocities,
        };
        if !seen.insert(actor) || !k.valid() {
            return Err(Corrupt("invalid KNCK entry".into()));
        }
        out.push(k);
    }
    r.finish()?;
    Ok(out)
}

pub(crate) fn validate(values: &[SavedKnock]) -> anyhow::Result<()> {
    let mut seen = HashSet::new();
    anyhow::ensure!(
        values.iter().all(|k| k.valid() && seen.insert(k.actor)),
        "invalid saved ragdolls"
    );
    Ok(())
}

impl Engine {
    pub(crate) fn restore_saved_knocks(&mut self, values: Vec<SavedKnock>) {
        for saved in values {
            let actor = saved.actor;
            if actor != PLAYER_REF && self.actor_ref(actor).is_none() {
                self.world_state.pending_knocks.insert(actor, saved);
                continue;
            }
            // Loaded bodies already passed capability checks when saved. Script
            // cause bypasses recovery thresholds, then the original hold wins.
            if !self.knock_down(actor, Vec3::ZERO, KnockCause::Paralysis) {
                continue;
            }
            let skeleton = if actor == PLAYER_REF {
                self.player_body.as_ref().map(|b| b.skeleton.clone())
            } else {
                self.actor_ref(actor).map(|a| a.skeleton.clone())
            };
            let Some(skeleton) = skeleton else { continue };
            let desc = skeleton.ragdoll.as_ref().unwrap();
            if desc.bodies.len() != saved.bodies.len() || skeleton.bones.len() != saved.pose.len() {
                if actor == PLAYER_REF {
                    self.finish_player_knock();
                } else {
                    self.finish_knock(actor);
                }
                continue;
            }
            let mut k = if actor == PLAYER_REF {
                self.player_knock.take().unwrap()
            } else {
                self.actor_mut(actor).unwrap().knock.take().unwrap()
            };
            self.physics
                .place_ragdoll(&k.ragdoll, &saved.bodies, &saved.velocities);
            k.mapping = crate::world::ragdoll::RagdollPose::new(
                desc,
                &skeleton,
                &saved.pose,
                saved.transform,
            );
            k.state = KnockState::Down;
            k.cause = saved.cause;
            k.down_for = saved.down_for;
            k.rest_for = saved.rest_for;
            k.peak_z = saved.peak_z;
            k.fall_applied = saved.fall_applied;
            k.script_hold = saved.script_hold;
            k.getup_requested = false;
            k.actor_transform = saved.transform;
            k.previous_view = saved.previous_view;
            let recovering = saved.getup_requested || saved.state == KnockState::GetUp;
            if actor == PLAYER_REF {
                if let Some(inst) = self.scene.player.as_mut() {
                    inst.transform = saved.transform;
                    inst.pose = saved.pose.clone();
                }
                self.player_knock = Some(k);
            } else {
                let key = self.actor_cells[&actor];
                let index = self.cells[&key]
                    .actors
                    .iter()
                    .position(|a| a.ref_id == actor)
                    .unwrap();
                if let Some(inst) = self
                    .scene
                    .cells
                    .get_mut(&key)
                    .and_then(|c| c.actors.get_mut(index))
                {
                    inst.transform = saved.transform;
                    inst.pose = saved.pose.clone();
                }
                self.cells.get_mut(&key).unwrap().actors[index].knock = Some(k);
            }
            if recovering {
                self.request_get_up(actor);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip_and_old_saves_and_invalid_bodies() {
        let k = SavedKnock {
            actor: FormId(1),
            state: KnockState::Down,
            cause: KnockCause::Push,
            down_for: 1.0,
            rest_for: 0.4,
            peak_z: 40.0,
            fall_applied: false,
            script_hold: false,
            getup_requested: false,
            previous_view: Some(false),
            transform: Mat4::IDENTITY,
            pose: vec![Mat4::IDENTITY],
            bodies: vec![Mat4::from_translation(Vec3::Z)],
            velocities: vec![(Vec3::X, Vec3::ZERO)],
        };
        let mut file = SaveFile::default();
        write(&[k.clone()], &mut file);
        assert_eq!(read(&file).unwrap(), vec![k.clone()]);
        assert!(read(&SaveFile::default()).unwrap().is_empty());
        file.add(b"KNCK", vec![]);
        assert!(read(&file).is_err());
        let mut k = k;
        k.bodies[0] = Mat4::ZERO;
        assert!(validate(&[k]).is_err());
    }
}
