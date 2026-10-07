//! Scenes (`SCEN`): phases, the quest aliases acting in them and their actions.
//!
//! Subrecords come in runs: the scene's flags, then each phase between a pair of
//! `HNAM` markers (name, start conditions, `NEXT`, completion conditions, `NEXT`),
//! the actors (`ALID`, `LNAM`, `DNAM`), each action between an `ANAM` type and an
//! empty `ANAM`, and last the owning quest (`PNAM`) and the scene's own conditions.

use crate::{FormId, LoadedRecord};

pub mod flags {
    /// `FNAM`
    pub const BEGIN_ON_QUEST_START: u32 = 0x1;
    pub const STOP_QUEST_ON_END: u32 = 0x2;
    pub const REPEAT_CONDITIONS_WHILE_TRUE: u32 = 0x8;
    pub const INTERRUPTIBLE: u32 = 0x10;
}

pub mod actor_flags {
    /// `LNAM`
    pub const NO_PLAYER_ACTIVATION: u32 = 0x1;
    pub const OPTIONAL: u32 = 0x2;
    pub const RUN_ONLY_SCENE_PACKAGES: u32 = 0x4;
    /// `DNAM` (behaviour)
    pub const DEATH_END: u32 = 0x2;
    pub const COMBAT_PAUSE: u32 = 0x4;
    pub const COMBAT_END: u32 = 0x8;
    pub const DIALOGUE_PAUSE: u32 = 0x10;
    pub const DIALOGUE_END: u32 = 0x20;
}

pub mod action_flags {
    /// Action `FNAM`
    pub const FACE_TARGET: u32 = 1 << 15;
    pub const LOOPING: u32 = 1 << 16;
    pub const HEADTRACK_PLAYER: u32 = 1 << 17;
}

/// A condition as stored: the `CTDA` bytes and its `CIS1` / `CIS2` strings.
#[derive(Debug, Clone, Default)]
pub struct RawCondition {
    pub ctda: Vec<u8>,
    pub cis1: Option<String>,
    pub cis2: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct Phase {
    pub name: String,
    pub start: Vec<RawCondition>,
    pub completion: Vec<RawCondition>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Actor {
    /// Reference alias id in the owning quest.
    pub alias: u32,
    pub flags: u32,
    pub behaviour: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ActionKind {
    /// Say a line of the topic (none: only head tracking), looping every
    /// `loop_min..loop_max` seconds when flagged.
    Dialogue { topic: FormId, headtrack: Option<u32>, loop_min: f32, loop_max: f32, emotion: u32, emotion_value: u32 },
    /// Run the first of these packages whose conditions pass.
    Package { packages: Vec<FormId> },
    Timer { seconds: f32 },
}

#[derive(Debug, Clone)]
pub struct Action {
    pub kind: ActionKind,
    pub name: String,
    /// Alias id of the acting actor (timers: -1 or unused).
    pub actor: i32,
    pub index: u32,
    pub flags: u32,
    pub start_phase: u32,
    pub end_phase: u32,
}

#[derive(Debug, Clone, Default)]
pub struct Scene {
    pub id: FormId,
    pub editor_id: String,
    pub flags: u32,
    pub phases: Vec<Phase>,
    pub actors: Vec<Actor>,
    pub actions: Vec<Action>,
    pub quest: FormId,
    pub conditions: Vec<RawCondition>,
}

impl Scene {
    pub fn actor(&self, alias: u32) -> Option<&Actor> {
        self.actors.iter().find(|a| a.alias == alias)
    }
}

#[derive(PartialEq)]
enum Part {
    Head,
    /// In a phase: before the first `NEXT`, between, after the second.
    Phase(u8),
    Actors,
    Action,
    Tail,
}

/// Which list the last condition went to (its `CIS1` / `CIS2` follow it).
#[derive(Clone, Copy)]
enum CondList {
    Start,
    Completion,
    Scene,
}

pub fn parse(rec: &LoadedRecord<'_>, id: FormId) -> Option<Scene> {
    if rec.tag().0 != *b"SCEN" {
        return None;
    }
    let mut s = Scene { id, editor_id: rec.editor_id().unwrap_or_default(), ..Default::default() };
    let mut part = Part::Head;
    // The action being read, and whether its ENAM was seen (a later SNAM is a
    // timer's seconds, not the start phase).
    let mut action: Option<(Action, bool)> = None;
    let mut last: Option<CondList> = None;
    for sr in rec.subrecords() {
        let tag = &sr.tag.0;
        let u32_of = || sr.data.get(0..4).map(|b| u32::from_le_bytes(b.try_into().unwrap())).unwrap_or(0);
        let f32_of = || f32::from_bits(u32_of());
        match (&part, tag) {
            (Part::Head, b"FNAM") => s.flags = u32_of(),
            (Part::Head, b"HNAM") => {
                s.phases.push(Phase::default());
                part = Part::Phase(0);
            }
            (Part::Phase(_), b"HNAM") => part = Part::Head,
            (Part::Phase(_), b"NAM0") => {
                if let Some(p) = s.phases.last_mut() {
                    p.name = sr.zstring();
                }
            }
            (Part::Phase(n), b"NEXT") => part = Part::Phase(n + 1),
            (Part::Phase(n), b"CTDA") if *n < 2 => {
                let p = s.phases.last_mut()?;
                let (list, which) = if *n == 0 { (&mut p.start, CondList::Start) } else { (&mut p.completion, CondList::Completion) };
                list.push(RawCondition { ctda: sr.data.to_vec(), ..Default::default() });
                last = Some(which);
            }
            (Part::Tail, b"CTDA") => {
                s.conditions.push(RawCondition { ctda: sr.data.to_vec(), ..Default::default() });
                last = Some(CondList::Scene);
            }
            (Part::Phase(_) | Part::Tail, b"CIS1" | b"CIS2") => {
                let list = match last {
                    Some(CondList::Start) => s.phases.last_mut().map(|p| &mut p.start),
                    Some(CondList::Completion) => s.phases.last_mut().map(|p| &mut p.completion),
                    Some(CondList::Scene) => Some(&mut s.conditions),
                    None => None,
                };
                if let Some(c) = list.and_then(|l| l.last_mut()) {
                    let v = Some(sr.zstring());
                    if tag == b"CIS1" {
                        c.cis1 = v;
                    } else {
                        c.cis2 = v;
                    }
                }
            }
            (Part::Head | Part::Actors, b"ALID") => {
                part = Part::Actors;
                s.actors.push(Actor { alias: u32_of(), ..Default::default() });
            }
            (Part::Actors, b"LNAM") => {
                if let Some(a) = s.actors.last_mut() {
                    a.flags = u32_of();
                }
            }
            (Part::Actors, b"DNAM") => {
                if let Some(a) = s.actors.last_mut() {
                    a.behaviour = u32_of();
                }
            }
            (Part::Head | Part::Actors | Part::Tail, b"ANAM") if sr.data.len() >= 2 => {
                part = Part::Action;
                let kind = match u16::from_le_bytes([sr.data[0], sr.data[1]]) {
                    0 => ActionKind::Dialogue { topic: FormId(0), headtrack: None, loop_min: 0.0, loop_max: 0.0, emotion: 0, emotion_value: 0 },
                    1 => ActionKind::Package { packages: Vec::new() },
                    _ => ActionKind::Timer { seconds: 0.0 },
                };
                action = Some((Action { kind, name: String::new(), actor: -1, index: 0, flags: 0, start_phase: 0, end_phase: 0 }, false));
            }
            (Part::Action, b"ANAM") => {
                s.actions.extend(action.take().map(|(a, _)| a));
                part = Part::Tail;
            }
            (Part::Action, _) => {
                let Some((a, ended)) = action.as_mut() else { continue };
                match (tag, &mut a.kind) {
                    (b"NAM0", _) => a.name = sr.zstring(),
                    (b"ALID", _) => a.actor = u32_of() as i32,
                    (b"INAM", _) => a.index = u32_of(),
                    (b"FNAM", _) => a.flags = u32_of(),
                    (b"SNAM", ActionKind::Timer { seconds }) if *ended => *seconds = f32_of(),
                    (b"SNAM", _) => a.start_phase = u32_of(),
                    (b"ENAM", _) => {
                        a.end_phase = u32_of();
                        *ended = true;
                    }
                    (b"PNAM", ActionKind::Package { packages }) => packages.push(rec.fid(FormId(u32_of()))),
                    (b"DATA", ActionKind::Dialogue { topic, .. }) => *topic = rec.fid(FormId(u32_of())),
                    (b"HTID", ActionKind::Dialogue { headtrack, .. }) => *headtrack = Some(u32_of()).filter(|&h| h as i32 >= 0),
                    (b"DMAX", ActionKind::Dialogue { loop_max, .. }) => *loop_max = f32_of(),
                    (b"DMIN", ActionKind::Dialogue { loop_min, .. }) => *loop_min = f32_of(),
                    (b"DEMO", ActionKind::Dialogue { emotion, .. }) => *emotion = u32_of(),
                    (b"DEVA", ActionKind::Dialogue { emotion_value, .. }) => *emotion_value = u32_of(),
                    _ => {}
                }
            }
            (Part::Tail, b"PNAM") => s.quest = rec.fid(FormId(u32_of())),
            _ => {}
        }
    }
    Some(s)
}
