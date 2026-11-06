//! Authored Shout procedures, using SHOU words and the normal magic pipeline.
use super::*;
use package::{Input, Target};

/// Combat actions are body state and are dropped with combat on unloading.
#[derive(Debug, Clone)]
pub(super) struct CombatVoice {
    pub word: crate::shouts::ShoutWord,
    pub target: FormId,
    pub elapsed: f32,
    pub count: u32,
    pub concentration: bool,
    pub charge: f32,
}

impl Engine {
    pub(super) fn update_package_shouts(&mut self) {
        let mut actors = self.actor_cells.keys().copied().collect::<Vec<_>>();
        actors.sort();
        for actor in actors {
            if self.is_disabled(actor)
                || self
                    .actor_ref(actor)
                    .is_some_and(|a| a.exec.bleeding.is_some())
            {
                continue;
            }
            if let Some((p, path, n, c)) = self.active_procedure(actor, "Shout") {
                let shout = match n.values.get(1) {
                    Some(Input::Target(Target::Object(s)))
                    | Some(Input::Target(Target::Ref(s))) => Some(*s),
                    _ => None,
                };
                let target = match n.values.get(2) {
                    Some(Input::Target(t)) => self.package_target_of(actor, &p, Some(*t)),
                    _ => None,
                };
                let Some((shout, target)) = shout.zip(target) else {
                    continue;
                };
                let Some(word) = self
                    .shout(shout)
                    .and_then(|s| s.words.iter().rev().find(|w| !w.spell.is_null()).cloned())
                else {
                    continue;
                };
                let Some((pos, aim)) = self.ref_position(actor).zip(self.ref_position(target))
                else {
                    continue;
                };
                let eye = pos + Vec3::Z * 110.;
                let aim = aim + Vec3::Z * if self.is_actor_ref(target) { 64. } else { 0. };
                let mut state = c.states.get(&path).cloned().unwrap_or_default();
                if !state.spoken {
                    if self.voice_recovery_time(actor) > 0. {
                        continue;
                    }
                    let effective = p.for_procedure(Some(&n));
                    if let Some((place, dest)) = self.package_place(actor, &effective) {
                        if !self
                            .current_place_of(actor)
                            .is_some_and(|(p, _)| schedule::same_space(p, place))
                            || pos.truncate().distance(dest.truncate())
                                > effective.location.map_or(96., |l| l.radius.max(96.))
                        {
                            continue;
                        }
                    }
                    if matches!(n.values.get(3), Some(Input::Bool(true))) {
                        let delta = aim - eye;
                        if self
                            .physics
                            .raycast_excluding(
                                eye,
                                delta.normalize_or_zero(),
                                delta.length(),
                                actor,
                            )
                            .is_some_and(|(d, r)| d + 16. < delta.length() && r != Some(target))
                        {
                            continue;
                        }
                    }
                    self.equip_shout(actor, shout);
                    self.send_actor_graph_event(actor, "shoutStart");
                    state.spoken = true;
                    state.elapsed = 0.;
                }
                if state.spoken
                    && state.count == 0
                    && self
                        .actor_ref(actor)
                        .and_then(|a| a.graph.as_ref())
                        .is_some_and(|g| {
                            !g.active_states()
                                .iter()
                                .any(|s| s.to_ascii_lowercase().contains("shout"))
                        })
                {
                    self.send_actor_graph_event(actor, "shoutStart");
                }
                let item = self.magic_item(word.spell);
                let concentration = item
                    .as_ref()
                    .is_some_and(|i| i.cast_type == crate::spells::CONCENTRATION);
                let charge = item.as_ref().map_or(1., |i| i.charge_time.max(1.)) as f64;
                if !matches!(n.values.get(4), Some(Input::Bool(true))) {
                    let delta = aim - eye;
                    if let Some(a) = self.actor_mut(actor) {
                        a.exec.heading = delta.x.atan2(delta.y);
                    }
                }
                if let Some(a) = self.actor_mut(actor) {
                    a.look_at = Some(aim);
                }
                let ticks = if state.elapsed < charge {
                    0
                } else if concentration {
                    (((state.elapsed - charge).min(3.) / crate::casting::CONCENTRATION_TICK as f64)
                        .floor() as u32
                        + 1)
                    .min(12)
                } else {
                    1
                };
                if ticks > state.count {
                    if state.count == 0 {
                        self.send_actor_graph_event(actor, "shoutRelease");
                        self.set_voice_recovery_time(
                            actor,
                            word.recovery
                                * self
                                    .actor_value(actor, esp::actor_value::SHOUT_RECOVERY_MULT)
                                    .max(0.),
                        );
                    }
                    let share = if concentration {
                        crate::casting::CONCENTRATION_TICK
                    } else {
                        1.
                    };
                    for _ in state.count..ticks {
                        self.cast_from(
                            actor,
                            word.spell,
                            share,
                            eye,
                            (aim - eye).normalize_or_zero(),
                            false,
                        );
                    }
                    state.count = ticks;
                }
                let done = state.count > 0 && (!concentration || state.elapsed >= charge + 3.);
                if let Some(x) = self.execution_mut(actor)
                    && let Some(cursor) = x.run.as_mut().and_then(|r| r.cursor.as_mut())
                {
                    cursor.states.insert(path.clone(), state);
                    if done {
                        cursor.finish(&path);
                        x.next_eval = 0.;
                    }
                }
                if done {
                    self.send_actor_graph_event(actor, "shoutStop");
                }
            }
        }
    }
    pub(super) fn update_combat_shouts(&mut self, dt: f32) {
        let mut actors = self.actor_cells.keys().copied().collect::<Vec<_>>();
        actors.sort();
        for actor in actors {
            let target = self
                .actor_ref(actor)
                .filter(|a| !a.dead && a.exec.bleeding.is_none())
                .and_then(|a| a.exec.combat.as_ref())
                .filter(|c| !c.fleeing)
                .map(|c| c.target);
            let Some(target) = target.filter(|_| {
                self.ai_enabled && !self.ai_off.contains(&actor) && !self.is_disabled(actor)
            }) else {
                if let Some(a) = self.actor_mut(actor)
                    && a.combat_voice.take().is_some()
                {
                    self.send_actor_graph_event(actor, "shoutStop");
                }
                continue;
            };
            if self.is_dead(target) || self.is_disabled(target) {
                continue;
            }
            let Some((pos, aim)) = self.ref_position(actor).zip(self.ref_position(target)) else {
                continue;
            };
            let eye = self
                .actor_node_transform(actor, "NPC Head")
                .map_or(pos + Vec3::Z * 110., |m| m.w_axis.truncate());
            let aim = aim + Vec3::Z * 64.;
            let pending = self.actor_mut(actor).and_then(|a| a.combat_voice.take());
            let mut voice = if let Some(v) = pending.filter(|v| v.target == target) {
                v
            } else {
                if self.voice_recovery_time(actor) > 0. {
                    continue;
                }
                let weight = self
                    .templates_of(actor)
                    .and_then(|t| t.form(&self.lo, crate::world::template::AI_DATA, b"ZNAM"))
                    .and_then(|r| self.lo.get(r))
                    .and_then(|r| {
                        r.get(b"CSGD")
                            .and_then(|d| d.get(24..28))
                            .map(|d| f32::from_le_bytes(d.try_into().unwrap()))
                    })
                    .unwrap_or(1.);
                if weight <= 0. {
                    continue;
                }
                let mut shouts = self.actor_shouts(actor);
                shouts.sort();
                let selected = shouts.into_iter().find_map(|id| {
                    let word = self
                        .shout(id)?
                        .words
                        .iter()
                        .rev()
                        .find(|w| !w.spell.is_null())?
                        .clone();
                    let item = self.magic_item(word.spell)?;
                    (self.item_hostile(&item) && pos.distance(aim) <= item.range.max(4096.))
                        .then_some((id, word, item))
                });
                let Some((shout, word, item)) = selected else {
                    continue;
                };
                let delta = aim - eye;
                if self
                    .physics
                    .raycast_excluding(eye, delta.normalize_or_zero(), delta.length(), actor)
                    .is_some_and(|(d, r)| d + 16. < delta.length() && r != Some(target))
                {
                    continue;
                }
                self.equip_shout(actor, shout);
                self.send_actor_graph_event(actor, "shoutStart");
                CombatVoice {
                    word,
                    target,
                    elapsed: 0.,
                    count: 0,
                    concentration: item.cast_type == crate::spells::CONCENTRATION,
                    charge: item.charge_time.max(1.),
                }
            };
            voice.elapsed += dt.max(0.);
            let ticks = if voice.elapsed < voice.charge {
                0
            } else if voice.concentration {
                (((voice.elapsed - voice.charge).min(3.) / crate::casting::CONCENTRATION_TICK)
                    .floor() as u32
                    + 1)
                .min(12)
            } else {
                1
            };
            if ticks > voice.count {
                if voice.count == 0 {
                    self.send_actor_graph_event(actor, "shoutRelease");
                    self.set_voice_recovery_time(
                        actor,
                        voice.word.recovery
                            * self
                                .actor_value(actor, esp::actor_value::SHOUT_RECOVERY_MULT)
                                .max(0.),
                    );
                }
                let share = if voice.concentration {
                    crate::casting::CONCENTRATION_TICK
                } else {
                    1.
                };
                for _ in voice.count..ticks {
                    self.cast_from(
                        actor,
                        voice.word.spell,
                        share,
                        eye,
                        (aim - eye).normalize_or_zero(),
                        false,
                    );
                }
                voice.count = ticks;
            }
            let done =
                voice.count > 0 && (!voice.concentration || voice.elapsed >= voice.charge + 3.);
            if done {
                self.send_actor_graph_event(actor, "shoutStop");
            } else if let Some(a) = self.actor_mut(actor) {
                a.combat_voice = Some(voice);
            }
        }
    }
}
