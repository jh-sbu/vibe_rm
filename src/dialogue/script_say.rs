//! `ObjectReference.Say(akTopicToSay, akActorToSpeakAs, abSpeakInPlayersHead)`:
//! a reference says a topic's line, voiced as itself (an actor) or as the
//! actor it speaks as, from where it is or in the player's head
//! (`research/pex-natives/dialogue-batch15-say.md`).

use std::collections::{HashMap, VecDeque};

use esp::FormId;
use glam::Vec3;

use super::{Info, Response, Topic};
use crate::engine::{Engine, PLAYER_REF};
use crate::scene::SceneLine;

/// A line a script had a reference say, and the INFO's responses to come.
pub(crate) struct ScriptSay {
    pub line: SceneLine,
    topic: Topic,
    info: Info,
    queue: VecDeque<(usize, Response)>,
    /// Whose voice and conditions: the speaker when it is an actor, else the
    /// actor it speaks as.
    voice: Option<FormId>,
    in_head: bool,
}

/// The lines scripts started, by speaker.
pub(crate) type ScriptSays = HashMap<FormId, ScriptSay>;

impl Engine {
    /// `Say`: the speaker (or the actor it speaks as) says the topic's first
    /// line its conditions allow; one already being said stops.
    pub(crate) fn script_say(
        &mut self,
        speaker: FormId,
        topic: FormId,
        as_actor: Option<FormId>,
        in_head: bool,
    ) {
        self.stop_script_say(speaker);
        let Some(topic) = super::topic(&self.lo, topic) else {
            return;
        };
        let voice = if self.is_actor_ref(speaker) || speaker == PLAYER_REF {
            Some(speaker)
        } else {
            as_actor
        };
        if voice.is_some_and(|v| self.is_dead(v) || self.is_knocked(v)) {
            return;
        }
        // Without an actor, the conditions run on the reference itself.
        let subject = voice.unwrap_or(speaker);
        let Some(info) = self.select_info_for(&topic, subject, PLAYER_REF) else {
            log::debug!("{speaker} says {}: no line", topic.editor_id);
            return;
        };
        self.run_info_fragment(&info, true, subject);
        let mut queue: VecDeque<(usize, Response)> =
            info.responses.iter().cloned().enumerate().collect();
        let Some((index, response)) = queue.pop_front() else {
            self.run_info_fragment(&info, false, subject);
            return;
        };
        let line = self.say_response(speaker, voice, in_head, &topic, info.id, response, index);
        self.barks.script.insert(
            speaker,
            ScriptSay {
                line,
                topic,
                info,
                queue,
                voice,
                in_head,
            },
        );
    }

    pub(crate) fn stop_script_say(&mut self, speaker: FormId) {
        if let Some(s) = self.barks.script.remove(&speaker) {
            if let (Some(audio), Some(voice)) = (&self.audio, s.line.voice) {
                audio.stop(voice);
            }
            if self.is_actor_ref(speaker) {
                self.end_talking_gestures(speaker);
            }
        }
    }

    /// Whether a script has `r` saying a line.
    pub(crate) fn is_script_saying(&self, r: FormId) -> bool {
        self.barks.script.contains_key(&r)
    }

    /// One response of a scripted line: voiced by `voice`'s voice type, from
    /// the speaker's place (or not placed, in the player's head).
    #[allow(clippy::too_many_arguments)]
    fn say_response(
        &mut self,
        speaker: FormId,
        voice: Option<FormId>,
        in_head: bool,
        topic: &Topic,
        info: FormId,
        r: Response,
        response_index: usize,
    ) -> SceneLine {
        let voice_type = voice
            .and_then(|v| self.actor_voice_type(v))
            .unwrap_or_default();
        let path = super::voice_path(&self.lo, voice_type, topic, info, r.number);
        let at = if in_head {
            None
        } else {
            self.ref_position(speaker).map(|p| p + Vec3::Z * 110.0)
        };
        let params = crate::world::sound::default_params(
            &self.lo,
            &self.audio_defaults,
            b"DDSC",
            b"DOP2",
            at,
        );
        let mut played = None;
        if let (Some(p), Some(audio)) = (&path, self.audio.as_mut())
            && self.vfs.exists(p)
        {
            played = audio.play(&self.vfs, p, params);
        }
        let duration = (r.text.split_whitespace().count() as f64 * 0.32).max(2.0);
        let name = self.form_name(voice.unwrap_or(speaker));
        log::info!("{name} (Say): {} [{}]", r.text, path.unwrap_or_default());
        if self.is_actor_ref(speaker) {
            self.line_gesture(speaker, r.speaker_idle);
        }
        SceneLine {
            speaker,
            name,
            text: r.text,
            voice: played,
            ends_at: self.scripts.real_time + duration,
            response_index,
            force_subtitle: false,
        }
    }

    /// Scripted lines go on to their next response, and end with their
    /// INFO's end fragment (called every frame).
    pub(crate) fn update_script_says(&mut self) {
        let now = self.scripts.real_time;
        let mut speakers: Vec<FormId> = self.barks.script.keys().copied().collect();
        speakers.sort();
        for speaker in speakers {
            let mut s = self.barks.script.remove(&speaker).unwrap();
            if s.voice
                .is_some_and(|v| self.is_dead(v) || self.is_knocked(v))
            {
                if let (Some(audio), Some(voice)) = (&self.audio, s.line.voice) {
                    audio.stop(voice);
                }
                continue;
            }
            let done = match (s.line.voice, &self.audio) {
                (Some(v), Some(a)) => !a.is_playing(v),
                _ => now >= s.line.ends_at,
            };
            if !done {
                self.barks.script.insert(speaker, s);
                continue;
            }
            if let Some((index, response)) = s.queue.pop_front() {
                s.line = self.say_response(
                    speaker, s.voice, s.in_head, &s.topic, s.info.id, response, index,
                );
                self.barks.script.insert(speaker, s);
                continue;
            }
            self.run_info_fragment(&s.info, false, s.voice.unwrap_or(speaker));
            if self.is_actor_ref(speaker) {
                self.end_talking_gestures(speaker);
            }
        }
    }
}
