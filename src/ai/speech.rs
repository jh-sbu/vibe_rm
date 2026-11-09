//! Authored package dialogue, sharing scene response playback and gestures.
use esp::FormId;

use super::{
    exec::{ActorExec, Outcome},
    package::{GreetTopic, Input, Node, Package},
    procedure::Path,
};
use crate::{
    dialogue::{Info, Response, Topic},
    engine::{Engine, PLAYER_REF},
    scene::SceneLine,
};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Owner {
    pub(crate) package: FormId,
    pub(crate) started_at: f64,
    pub(crate) key: String,
    pub(crate) path: Path,
    pub(crate) revision: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SavedSpeech {
    pub actor: FormId,
    pub owner: Owner,
    pub topic: FormId,
    pub info: FormId,
    pub response: u32,
    pub listener: Option<FormId>,
    pub look: bool,
    pub finish: bool,
    pub force_subtitle: bool,
}

pub(crate) struct Speech {
    owner: Owner,
    pub(crate) line: SceneLine,
    topic: Topic,
    info: Info,
    queue: std::collections::VecDeque<(usize, Response)>,
    pub(crate) listener: Option<FormId>,
    pub(crate) look: bool,
    finish: bool,
    pub(crate) paused_at: Option<f64>,
}

fn owner(x: &ActorExec, path: &Path) -> Option<Owner> {
    let run = x.run.as_ref().filter(|r| r.outcome == Outcome::Running)?;
    let c = run.cursor.as_ref()?;
    c.active.contains(path).then(|| Owner {
        package: run.id,
        started_at: run.started_at,
        key: c.tree_key.clone(),
        path: path.clone(),
        revision: c.revision,
    })
}

impl Engine {
    pub(crate) fn saved_package_speech(&self) -> Vec<SavedSpeech> {
        let mut out: Vec<_> = self
            .barks
            .packages
            .iter()
            .filter(|(actor, s)| s.finish || self.owns_speech(**actor, &s.owner))
            .map(|(&actor, s)| SavedSpeech {
                actor,
                owner: s.owner.clone(),
                topic: s.topic.id,
                info: s.info.id,
                response: s.line.response_index as u32,
                listener: s.listener,
                look: s.look,
                finish: s.finish,
                force_subtitle: s.line.force_subtitle,
            })
            .collect();
        out.sort_by_key(|s| s.actor);
        out
    }

    pub(crate) fn validate_saved_package_speech(
        &self,
        states: &[SavedSpeech],
    ) -> anyhow::Result<()> {
        for s in states {
            anyhow::ensure!(
                crate::dialogue::topic(&self.lo, s.topic).is_some()
                    && self.lo.topic_infos(s.topic).contains(&s.info),
                "save package dialogue has an invalid topic/INFO"
            );
            let info = crate::dialogue::info(&self.lo, s.info)
                .ok_or_else(|| anyhow::anyhow!("save package dialogue has a missing INFO"))?;
            anyhow::ensure!(
                (s.response as usize) < info.responses.len(),
                "save package dialogue has an invalid response"
            );
        }
        Ok(())
    }

    pub(crate) fn restore_package_speech(&mut self, states: Vec<SavedSpeech>) {
        self.barks.packages.clear();
        let now = self.scripts.real_time;
        for s in states {
            let (Some(topic), Some(info)) = (
                crate::dialogue::topic(&self.lo, s.topic),
                crate::dialogue::info(&self.lo, s.info),
            ) else {
                continue;
            };
            let Some(response) = info.responses.get(s.response as usize) else {
                continue;
            };
            let line = SceneLine {
                speaker: s.actor,
                name: self.form_name(s.actor),
                text: response.text.clone(),
                voice: None,
                ends_at: now,
                response_index: s.response as usize,
                force_subtitle: s.force_subtitle,
            };
            let queue = info
                .responses
                .iter()
                .cloned()
                .enumerate()
                .skip(s.response as usize + 1)
                .collect();
            self.barks.packages.insert(
                s.actor,
                Speech {
                    owner: s.owner,
                    line,
                    topic,
                    info,
                    queue,
                    listener: s.listener,
                    look: s.look,
                    finish: s.finish,
                    paused_at: Some(now),
                },
            );
        }
    }
    pub(crate) fn discard_package_dialogue(&mut self) {
        let speeches = std::mem::take(&mut self.barks.packages);
        for (actor, s) in speeches {
            if let (Some(audio), Some(voice)) = (&self.audio, s.line.voice) {
                audio.stop(voice);
            }
            self.end_talking_gestures(actor);
        }
    }
    fn owns_speech(&self, actor: FormId, speech: &Owner) -> bool {
        self.actor_ref(actor)
            .map(|a| &a.exec)
            .or_else(|| self.away.get(&actor))
            .and_then(|x| owner(x, &speech.path))
            .as_ref()
            == Some(speech)
    }

    fn finish_speech_procedure(&mut self, actor: FormId, speech: &Owner) {
        if !self.owns_speech(actor, speech) {
            return;
        }
        let x = if let Some(key) = self.actor_cells.get(&actor).copied() {
            self.cells
                .get_mut(&key)
                .and_then(|rt| rt.actors.iter_mut().find(|a| a.ref_id == actor))
                .map(|a| &mut a.exec)
        } else {
            self.away.get_mut(&actor)
        };
        if let Some(x) = x {
            if let Some(c) = x.run.as_mut().and_then(|r| r.cursor.as_mut()) {
                if let Some(progress) = c.states.get_mut(&speech.path) {
                    progress.spoken = true;
                }
                c.finish(&speech.path);
            }
            x.next_eval = 0.;
        }
    }

    pub(crate) fn is_package_speaking(&self, actor: FormId) -> bool {
        self.barks
            .packages
            .get(&actor)
            .is_some_and(|s| s.paused_at.is_none())
    }

    pub(crate) fn package_dialogue_target(&self, actor: FormId) -> Option<FormId> {
        self.barks.packages.get(&actor).and_then(|s| s.listener)
    }

    fn ready_dialogue(&self) -> Vec<(FormId, Owner, Package, Node)> {
        let mut ready = vec![];
        for a in self.cells.values().flat_map(|rt| &rt.actors) {
            if a.dead
                || a.exec.pinned
                || a.exec.combat.is_some()
                || a.exec.bleeding.is_some()
                || a.exiting.is_some()
                || self.ai_off.contains(&a.ref_id)
            {
                continue;
            }
            let Some(p) = a.exec.current.and_then(|i| a.exec.packages.get(i)) else {
                continue;
            };
            let Some(tree) = p.tree.as_ref() else {
                continue;
            };
            let Some(c) = a.exec.run.as_ref().and_then(|r| r.cursor.as_ref()) else {
                continue;
            };
            for path in &c.active {
                let Some(n) = tree
                    .at(path)
                    .filter(|n| matches!(n.procedure.as_str(), "Say" | "DialogueActivate"))
                else {
                    continue;
                };
                let Some(owner) = owner(&a.exec, path) else {
                    continue;
                };
                if n.procedure == "DialogueActivate" {
                    let effective = p.for_procedure(Some(n));
                    if !a.exec.goal.is_some_and(|g| self.arrived(a, &effective, g)) {
                        continue;
                    }
                }
                ready.push((a.ref_id, owner, p.clone(), n.clone()));
                break;
            }
        }
        ready.sort_by_key(|r| r.0);
        ready
    }

    pub(crate) fn update_package_dialogue(&mut self) {
        let now = self.scripts.real_time;
        let mut ids: Vec<_> = self.barks.packages.keys().copied().collect();
        ids.sort();
        for actor in ids {
            let mut s = self.barks.packages.remove(&actor).unwrap();
            let same = self.owns_speech(actor, &s.owner);
            if self.is_dead(actor) || self.is_knocked(actor) || (!same && !s.finish) {
                if let (Some(audio), Some(voice)) = (&self.audio, s.line.voice) {
                    audio.stop(voice);
                }
                self.end_talking_gestures(actor);
                continue;
            }
            if self.actor_ref(actor).is_none() {
                if s.paused_at.is_none() {
                    s.paused_at = Some(now);
                    if let (Some(audio), Some(voice)) = (&self.audio, s.line.voice.take()) {
                        audio.stop(voice);
                    }
                }
                self.barks.packages.insert(actor, s);
                continue;
            }
            if s.paused_at.take().is_some() {
                if let Some(response) = s.info.responses.get(s.line.response_index).cloned() {
                    let force = s.line.force_subtitle;
                    s.line = self.play_spoken_response(
                        actor,
                        s.listener,
                        &s.topic,
                        s.info.id,
                        response,
                        s.line.response_index,
                    );
                    s.line.force_subtitle = force;
                }
            }
            let done = match (s.line.voice, &self.audio) {
                (Some(v), Some(a)) => !a.is_playing(v),
                _ => now >= s.line.ends_at,
            };
            if !done {
                self.barks.packages.insert(actor, s);
                continue;
            }
            if same {
                if let Some((index, response)) = s.queue.pop_front() {
                    let force = s.line.force_subtitle;
                    s.line = self.play_spoken_response(
                        actor, s.listener, &s.topic, s.info.id, response, index,
                    );
                    s.line.force_subtitle = force;
                    self.barks.packages.insert(actor, s);
                    continue;
                }
            }
            self.run_info_fragment(&s.info, false, actor);
            self.end_talking_gestures(actor);
            if same {
                self.finish_speech_procedure(actor, &s.owner);
            }
        }
        if !self.ai_enabled {
            return;
        }
        for (actor, owner, p, n) in self.ready_dialogue() {
            if self.barks.packages.contains_key(&actor)
                || self.is_scene_speaking(actor)
                || self.is_barking(actor)
                || self
                    .conversation
                    .as_ref()
                    .is_some_and(|c| c.npc_ref == actor)
            {
                continue;
            }
            let flag = |i, fallback| match n.values.get(i) {
                Some(Input::Bool(b)) => *b,
                _ => fallback,
            };
            let target = n.values.iter().find_map(|v| {
                if let Input::Target(t) = v {
                    Some(*t)
                } else {
                    None
                }
            });
            let listener = self.package_target_of(actor, &p.for_procedure(Some(&n)), target);
            if n.procedure == "DialogueActivate" {
                let Some(target) = listener.filter(|r| *r != actor && !self.is_dead(*r)) else {
                    continue;
                };
                if target == PLAYER_REF {
                    if self.conversation.is_some() || self.menu.is_some() || self.player_dead() {
                        continue;
                    }
                    let greeting = self.greet_line(actor, GreetTopic::Subtype(*b"HELO"));
                    self.open_conversation(actor, greeting);
                } else {
                    if self.actor_ref(target).is_none() {
                        continue;
                    }
                    if !self
                        .current_place_of(actor)
                        .zip(self.current_place_of(target))
                        .is_some_and(|(a, b)| super::schedule::same_space(a.0, b.0))
                    {
                        continue;
                    }
                    if flag(2, false) {
                        if let Some(a) = self.actor_mut(actor) {
                            a.face = Some((target, true));
                        }
                    }
                    let mut event = crate::story::StoryEvent::new(b"ADIA");
                    event.refs = [actor, target];
                    event.locs[0] = self.ref_current_location(actor).unwrap_or_default();
                    self.send_story_event(event);
                }
                self.finish_speech_procedure(actor, &owner);
                continue;
            }
            let Some(Input::Topic(topic)) = n.values.first() else {
                self.finish_speech_procedure(actor, &owner);
                continue;
            };
            let topics = match topic {
                GreetTopic::Topic(id) => {
                    crate::dialogue::topic(&self.lo, *id).into_iter().collect()
                }
                GreetTopic::Subtype(sub) => self.bark_topics(sub),
            };
            let chosen = topics.into_iter().find_map(|t| {
                self.select_info_for(&t, actor, listener.unwrap_or(PLAYER_REF))
                    .map(|i| (t, i))
            });
            let Some((topic, info)) = chosen else {
                self.finish_speech_procedure(actor, &owner);
                continue;
            };
            self.run_info_fragment(&info, true, actor);
            let mut queue: std::collections::VecDeque<_> =
                info.responses.iter().cloned().enumerate().collect();
            let Some((index, response)) = queue.pop_front() else {
                self.run_info_fragment(&info, false, actor);
                self.finish_speech_procedure(actor, &owner);
                continue;
            };
            let mut line =
                self.play_spoken_response(actor, listener, &topic, info.id, response, index);
            line.force_subtitle = flag(4, false);
            self.barks.packages.insert(
                actor,
                Speech {
                    owner,
                    line,
                    topic,
                    info,
                    queue,
                    listener,
                    look: flag(3, false),
                    finish: flag(2, false),
                    paused_at: None,
                },
            );
        }
    }
}

#[cfg(test)]
#[path = "speech_tests.rs"]
mod tests;
