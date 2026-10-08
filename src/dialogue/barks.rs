//! Things NPCs say outside conversations: greeting the player as they pass
//! (`HELO` topics) and idle chatter (`IDLE`), voiced where they stand with a
//! subtitle.

use std::collections::HashMap;

use esp::FormId;
use glam::Vec3;

use super::{Topic, voice_path};
use crate::engine::Engine;

/// How close the player comes before an NPC greets them.
const GREETING_DISTANCE: f32 = 200.0;
/// Seconds before an NPC greets the player again.
const GREETING_INTERVAL: f64 = 120.0;
/// Idle chatter is heard within this distance. Each NPC looks for something to say
/// every so often (seconds, randomised); the lines' own chances (`GetRandomPercent`)
/// keep most of those quiet.
const CHATTER_DISTANCE: f32 = 1200.0;
const CHATTER_INTERVAL: (f64, f64) = (8.0, 20.0);
/// Seconds of quiet after any line before another NPC speaks.
const QUIET_AFTER: f64 = 2.0;

/// A line an NPC says by itself.
#[derive(Debug, Clone)]
pub struct Bark {
    pub speaker: FormId,
    pub name: String,
    pub text: String,
    pub voice: Option<crate::audio::VoiceId>,
    pub ends_at: f64,
}

#[derive(Default)]
pub struct Barks {
    /// The line being said, if any.
    pub current: Option<Bark>,
    /// When each NPC last greeted the player, and when it may chatter next.
    greeted: HashMap<FormId, f64>,
    chatter_at: HashMap<FormId, f64>,
    quiet_until: f64,
    /// Topics by subtype, highest priority first.
    topics: HashMap<[u8; 4], Vec<Topic>>,
}

impl Engine {
    /// Topics of a subtype, highest priority first (cached).
    pub(crate) fn bark_topics(&mut self, sub: &[u8; 4]) -> Vec<Topic> {
        if let Some(t) = self.barks.topics.get(sub) {
            return t.clone();
        }
        let t = self.topics_of_subtype(sub);
        self.barks.topics.insert(*sub, t.clone());
        t
    }

    /// Whether `r` is saying a line by itself.
    pub fn is_barking(&self, r: FormId) -> bool {
        self.barks.current.as_ref().is_some_and(|b| b.speaker == r)
    }

    /// Greet the player or chatter now and then (called every frame).
    pub fn update_barks(&mut self) {
        let now = self.scripts.real_time;
        if let Some(b) = &self.barks.current {
            let done = match (b.voice, &self.audio) {
                (Some(v), Some(a)) => !a.is_playing(v),
                _ => now >= b.ends_at,
            };
            if done {
                let speaker = b.speaker;
                self.barks.current = None;
                self.barks.quiet_until = now + QUIET_AFTER;
                self.end_talking_gestures(speaker);
            }
            return;
        }
        if self.conversation.is_some()
            || now < self.barks.quiet_until
            || !self.ai_enabled
            || !self.scene_lines().is_empty()
        {
            return;
        }
        let player = self.player.position;
        // Humanoids about, nearest first.
        let mut near: Vec<(f32, FormId)> = self
            .cells
            .values()
            .flat_map(|rt| rt.actors.iter())
            .filter(|a| a.graph.as_ref().is_some_and(|g| g.project().humanoid()))
            .map(|a| (a.pos.distance(player), a.ref_id))
            .filter(|(d, _)| *d < CHATTER_DISTANCE)
            .collect();
        near.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (d, r) in near {
            if self.scene_of_actor(r).is_some() {
                continue;
            }
            if d < GREETING_DISTANCE
                && self
                    .barks
                    .greeted
                    .get(&r)
                    .is_none_or(|t| now - t > GREETING_INTERVAL)
            {
                self.barks.greeted.insert(r, now);
                self.send_actor_hello(r, crate::engine::PLAYER_REF);
                if self.bark(r, b"HELO") {
                    return;
                }
            }
            let due = *self
                .barks
                .chatter_at
                .entry(r)
                .or_insert_with(|| now + CHATTER_INTERVAL.0 + (r.0 % 13) as f64);
            if now >= due {
                let wait = CHATTER_INTERVAL.0
                    + (self.rand() % 1000) as f64 / 1000.0
                        * (CHATTER_INTERVAL.1 - CHATTER_INTERVAL.0);
                self.barks.chatter_at.insert(r, now + wait);
                if self.bark(r, b"IDLE") {
                    return;
                }
            }
        }
    }

    /// Say a line from the first topic of `subtype` with an INFO for `speaker`.
    /// True if something was said.
    pub fn bark(&mut self, speaker: FormId, subtype: &[u8; 4]) -> bool {
        let Some(voice_type) = self.actor_voice_type(speaker) else {
            log::trace!("{speaker}: no voice type");
            return false;
        };
        for topic in self.bark_topics(subtype) {
            let Some(info) = self.select_info(&topic, speaker) else {
                log::trace!(
                    "{speaker}: no {} line ({}, quest {} {})",
                    topic.editor_id,
                    topic.id,
                    topic.quest,
                    if self
                        .scripts
                        .quests
                        .get(&topic.quest)
                        .is_some_and(|q| q.running)
                    {
                        "running"
                    } else {
                        "not running"
                    }
                );
                continue;
            };
            // Barks are single lines; multi-line INFOs start with their first.
            let Some(r) = info.responses.first().cloned() else {
                continue;
            };
            let path = voice_path(&self.lo, voice_type, &topic, info.id, r.number);
            let at = self.ref_position(speaker).map(|p| p + Vec3::Z * 110.0);
            let now = self.scripts.real_time;
            let mut voice = None;
            if let (Some(p), Some(audio)) = (&path, self.audio.as_mut())
                && self.vfs.exists(p)
            {
                voice = audio.play(&self.vfs, p, 1.0, false, at, 400.0, 3000.0);
            }
            // Silent lines (and without audio) last a reading time.
            let duration = (r.text.split_whitespace().count() as f64 * 0.32).max(2.0);
            let name = self.form_name(speaker);
            log::info!(
                "{name} ({}): {} [{}]",
                String::from_utf8_lossy(subtype),
                r.text,
                path.unwrap_or_default()
            );
            self.barks.current = Some(Bark {
                speaker,
                name,
                text: r.text,
                voice,
                ends_at: now + duration,
            });
            self.talking_gesture(speaker);
            return true;
        }
        false
    }
}
