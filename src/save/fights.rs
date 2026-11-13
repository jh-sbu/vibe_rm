//! Loaded actors' fights and searches (optional `FGHT`): an engine-owned
//! layout, not a claim about how Skyrim SE saves combat. Each entry is set
//! again once its actor's cell has loaded, without barks or
//! `OnCombatStateChanged` (the reported state is set to match).

use std::collections::HashSet;

use esp::FormId;
use glam::Vec3;

use super::codec::{Corrupt, Reader, Result, Writer};
use super::file::SaveFile;
use crate::ai::combat::{Combat, SavedFight};
use crate::detection::{SavedSearch, Search};
use crate::engine::{Engine, PLAYER_REF};

/// An actor's fight and search as saved.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SavedEngagement {
    pub actor: FormId,
    pub fight: Option<SavedFight>,
    pub search: Option<SavedSearch>,
}

/// Living loaded actors' fights and searches, with those still waiting for
/// their cells after a load.
pub(crate) fn capture(e: &Engine) -> Vec<SavedEngagement> {
    let mut out: Vec<_> = e.world_state.pending_fights.values().copied().collect();
    for a in e.cells.values().flat_map(|rt| &rt.actors) {
        if a.dead || (a.exec.combat.is_none() && a.exec.search.is_none()) {
            continue;
        }
        out.retain(|s| s.actor != a.ref_id);
        out.push(SavedEngagement {
            actor: a.ref_id,
            fight: a.exec.combat.as_ref().map(Combat::saved),
            search: a.exec.search.as_ref().map(Search::saved),
        });
    }
    out.sort_by_key(|s| s.actor);
    out
}

fn put_opt_vec3(w: &mut Writer, v: Option<Vec3>) {
    w.bool(v.is_some());
    if let Some(v) = v {
        w.vec3(v);
    }
}

pub(crate) fn write(values: &[SavedEngagement], file: &mut SaveFile) {
    if values.is_empty() {
        return;
    }
    let mut w = Writer::default();
    w.len(values.len());
    for s in values {
        w.form(s.actor);
        w.bool(s.fight.is_some());
        if let Some(f) = &s.fight {
            w.form(f.target);
            w.bool(f.fleeing);
            for v in [
                f.confidence_mod,
                f.threat_check,
                f.flee_distance,
                f.safe,
                f.unseen,
            ] {
                w.f32(v);
            }
            put_opt_vec3(&mut w, f.last_seen);
            w.form(f.ally_killed.unwrap_or(FormId::NULL));
        }
        w.bool(s.search.is_some());
        if let Some(x) = &s.search {
            w.form(x.target);
            w.vec3(x.at);
            w.bool(x.lost);
            w.f32(x.unseen);
            w.bool(x.fled);
        }
    }
    file.add(b"FGHT", w.0);
}

fn finite(v: f32) -> Result<f32> {
    if v.is_finite() {
        Ok(v)
    } else {
        Err(Corrupt("FGHT value isn't finite".into()))
    }
}

fn target(r: &mut Reader, actor: FormId) -> Result<FormId> {
    let t = r.form()?;
    if t.is_null() || t == actor {
        return Err(Corrupt("FGHT target".into()));
    }
    Ok(t)
}

pub(crate) fn read(file: &SaveFile) -> Result<Vec<SavedEngagement>> {
    if file.sections.iter().filter(|(t, _)| t == b"FGHT").count() > 1 {
        return Err(Corrupt("duplicate FGHT".into()));
    }
    let Some(bytes) = file.section(b"FGHT") else {
        return Ok(vec![]);
    };
    let mut r = Reader::new(bytes);
    let mut out: Vec<SavedEngagement> = Vec::new();
    for _ in 0..r.count(6)? {
        let actor = r.form()?;
        if actor.is_null() || actor == PLAYER_REF || out.last().is_some_and(|p| p.actor >= actor) {
            return Err(Corrupt("FGHT actors".into()));
        }
        let fight = if r.bool()? {
            let target = target(&mut r, actor)?;
            let fleeing = r.bool()?;
            let mut v = [0.0; 5];
            for x in &mut v {
                *x = finite(r.f32()?)?;
            }
            let last_seen = if r.bool()? {
                let p = r.vec3()?;
                if !p.is_finite() {
                    return Err(Corrupt("FGHT last seen".into()));
                }
                Some(p)
            } else {
                None
            };
            Some(SavedFight {
                target,
                fleeing,
                confidence_mod: v[0],
                threat_check: v[1],
                flee_distance: v[2],
                safe: v[3],
                unseen: v[4],
                last_seen,
                ally_killed: r.opt_form()?,
            })
        } else {
            None
        };
        let search = if r.bool()? {
            let target = target(&mut r, actor)?;
            let at = r.vec3()?;
            if !at.is_finite() {
                return Err(Corrupt("FGHT search place".into()));
            }
            Some(SavedSearch {
                target,
                at,
                lost: r.bool()?,
                unseen: finite(r.f32()?)?,
                fled: r.bool()?,
            })
        } else {
            None
        };
        if fight.is_none() && search.is_none() {
            return Err(Corrupt("FGHT entry with neither".into()));
        }
        out.push(SavedEngagement {
            actor,
            fight,
            search,
        });
    }
    r.finish()?;
    Ok(out)
}

impl Engine {
    /// Set saved fights and searches again: now for loaded actors, when
    /// their cells load for the rest. Dead actors' are dropped.
    pub(crate) fn restore_saved_fights(&mut self, values: Vec<SavedEngagement>) {
        let mut drawn = HashSet::new();
        for s in values {
            if self.actor_ref(s.actor).is_none() {
                self.world_state.pending_fights.insert(s.actor, s);
                continue;
            }
            let idle_line = self.detection_idle_wait();
            let Some(a) = self.actor_mut(s.actor) else {
                continue;
            };
            if a.dead {
                continue;
            }
            a.exec.search = s.search.as_ref().map(|x| Search::restored(x, idle_line));
            a.exec.combat = s.fight.as_ref().map(Combat::restored);
            a.strength_in = 0.0;
            a.combat_reported = a.combat_state().0;
            let humanoid = a.graph.as_ref().is_some_and(|g| g.project().humanoid());
            let coward = a.stats.confidence == 0;
            self.combat_group_strengths.remove(s.actor);
            if let Some(x) = &s.search {
                self.search_towards(s.actor, x.at);
            }
            if s.fight.is_some() && !humanoid {
                if !coward && let Some(g) = self.actor_mut(s.actor).and_then(|a| a.graph.as_mut()) {
                    g.send_event("combatStanceStart");
                }
            } else if s.search.is_some() || !coward {
                drawn.insert(s.actor);
            }
        }
        for actor in drawn {
            self.draw_weapon(actor, true);
        }
    }
}
