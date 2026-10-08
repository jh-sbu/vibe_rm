//! Dialogue: topics (DIAL), branches (DLBR), responses (INFO) and conversations.

use esp::{FormId, LoadOrder};

use crate::condition::{self, Condition};
use crate::engine::{Engine, PLAYER_REF};

pub mod barks;

pub mod info_flags {
    pub const GOODBYE: u16 = 0x1;
    pub const RANDOM: u16 = 0x2;
    pub const SAY_ONCE: u16 = 0x4;
    pub const INVISIBLE_CONTINUE: u16 = 0x20;
}

#[derive(Debug, Clone)]
pub struct Topic {
    pub id: FormId,
    pub editor_id: String,
    pub prompt: String,
    pub priority: f32,
    pub quest: FormId,
    pub branch: FormId,
    pub subtype: [u8; 4],
}

#[derive(Debug, Clone)]
pub struct Response {
    pub number: u8,
    pub text: String,
    /// Emotion type (0 neutral, 1 anger, 2 disgust, 3 fear, 4 sad, 5 happy,
    /// 6 surprise, 7 puzzled) and strength (0..100).
    pub emotion: u32,
    pub emotion_value: u32,
}

#[derive(Debug, Clone)]
pub struct Info {
    pub id: FormId,
    pub flags: u16,
    pub responses: Vec<Response>,
    pub conditions: Vec<Condition>,
    pub choices: Vec<FormId>,
    pub prompt: Option<String>,
    /// Begin/end fragment functions and their script.
    pub script: Option<String>,
    pub begin_fragment: Option<String>,
    pub end_fragment: Option<String>,
}

pub fn topic(lo: &LoadOrder, id: FormId) -> Option<Topic> {
    let r = lo.get(id)?;
    if r.tag().0 != *b"DIAL" {
        return None;
    }
    let fid = |tag: &[u8; 4]| {
        r.get(tag)
            .map(|d| r.fid(FormId(u32::from_le_bytes(d[0..4].try_into().unwrap()))))
            .unwrap_or_default()
    };
    Some(Topic {
        id,
        editor_id: r.editor_id().unwrap_or_default(),
        prompt: r
            .get(b"FULL")
            .map(|d| lo.lstring(&r, d))
            .unwrap_or_default(),
        priority: r
            .get(b"PNAM")
            .map(|d| f32::from_le_bytes(d[0..4].try_into().unwrap()))
            .unwrap_or(50.0),
        quest: fid(b"QNAM"),
        branch: fid(b"BNAM"),
        subtype: r
            .get(b"SNAM")
            .and_then(|d| d.get(0..4))
            .map(|d| d.try_into().unwrap())
            .unwrap_or([0; 4]),
    })
}

pub fn info(lo: &LoadOrder, id: FormId) -> Option<Info> {
    let r = lo.get(id)?;
    let mut out = Info {
        id,
        flags: r
            .get(b"ENAM")
            .map(|d| u16::from_le_bytes([d[0], d[1]]))
            .unwrap_or(0),
        responses: Vec::new(),
        conditions: condition::parse_all(&r),
        choices: Vec::new(),
        prompt: r
            .get(b"RNAM")
            .map(|d| lo.lstring(&r, d))
            .filter(|s| !s.is_empty()),
        script: None,
        begin_fragment: None,
        end_fragment: None,
    };
    let mut cur: Option<Response> = None;
    for sr in r.subrecords() {
        match &sr.tag.0 {
            b"TRDT" => {
                if let Some(c) = cur.take() {
                    out.responses.push(c);
                }
                cur = Some(Response {
                    number: sr.u8(12),
                    text: String::new(),
                    emotion: sr.u32(0),
                    emotion_value: sr.u32(4),
                });
            }
            b"NAM1" => {
                if let Some(c) = cur.as_mut() {
                    c.text = lo.lstring(&r, sr.data);
                }
            }
            b"TCLT" => out.choices.push(r.fid(sr.form_id(0))),
            _ => {}
        }
    }
    if let Some(c) = cur.take() {
        out.responses.push(c);
    }
    // Shared responses (DNAM) live on another INFO.
    if out.responses.is_empty()
        && let Some(d) = r.get(b"DNAM")
        && let Some(shared) = info(
            lo,
            r.fid(FormId(u32::from_le_bytes(d[0..4].try_into().ok()?))),
        )
    {
        out.responses = shared.responses;
    }
    // INFO VMAD: scripts, then fragment data (flags u8, script name, begin/end fragments).
    if let Some(v) = r.get(b"VMAD") {
        parse_info_fragments(v, &mut out);
    }
    Some(out)
}

fn parse_info_fragments(d: &[u8], out: &mut Info) {
    // Skip the script list, then read fragment info.
    let mut p = 0usize;
    let rd16 = |p: &mut usize| -> Option<u16> {
        let v = u16::from_le_bytes(d.get(*p..*p + 2)?.try_into().ok()?);
        *p += 2;
        Some(v)
    };
    let rdstr = |p: &mut usize| -> Option<String> {
        let n = u16::from_le_bytes(d.get(*p..*p + 2)?.try_into().ok()?) as usize;
        *p += 2;
        let s = esp::decode_zstring(d.get(*p..*p + n)?);
        *p += n;
        Some(s)
    };
    let mut f = || -> Option<()> {
        let version = rd16(&mut p)? as i16;
        let format = rd16(&mut p)? as i16;
        let n = rd16(&mut p)?;
        for _ in 0..n {
            rdstr(&mut p)?;
            if version >= 4 {
                p += 1;
            }
            let np = rd16(&mut p)?;
            for _ in 0..np {
                rdstr(&mut p)?;
                let ty = *d.get(p)?;
                p += 1;
                if version >= 4 {
                    p += 1;
                }
                skip_value(d, &mut p, ty, format)?;
            }
        }
        // Fragments
        p += 1; // unknown
        let flags = *d.get(p)?;
        p += 1;
        out.script = Some(rdstr(&mut p)?);
        if flags & 1 != 0 {
            p += 1;
            rdstr(&mut p)?;
            out.begin_fragment = Some(rdstr(&mut p)?);
        }
        if flags & 2 != 0 {
            p += 1;
            rdstr(&mut p)?;
            out.end_fragment = Some(rdstr(&mut p)?);
        }
        Some(())
    };
    let _ = f();
}

fn skip_value(d: &[u8], p: &mut usize, ty: u8, format: i16) -> Option<()> {
    let _ = format;
    match ty {
        1 => *p += 8,
        2 => {
            let n = u16::from_le_bytes(d.get(*p..*p + 2)?.try_into().ok()?) as usize;
            *p += 2 + n;
        }
        3 | 4 => *p += 4,
        5 => *p += 1,
        11..=15 => {
            let n = u32::from_le_bytes(d.get(*p..*p + 4)?.try_into().ok()?) as usize;
            *p += 4;
            for _ in 0..n {
                skip_value(d, p, ty - 10, format)?;
            }
        }
        _ => return None,
    }
    Some(())
}

/// Voice file for a response, following the Creation Kit naming scheme.
pub fn voice_path(
    lo: &LoadOrder,
    npc_voice: FormId,
    topic: &Topic,
    info: FormId,
    response: u8,
) -> Option<String> {
    let (plugin, local) = lo.origin(info)?;
    let vt = lo.get(npc_voice)?.editor_id()?;
    let quest = lo
        .get(topic.quest)
        .and_then(|q| q.editor_id())
        .unwrap_or_default();
    // Quest and topic editor ids share a 25 character budget; the quest keeps at least 10.
    let tlen = topic.editor_id.chars().count();
    let q: String = quest
        .chars()
        .take(25usize.saturating_sub(tlen).max(10))
        .collect();
    let t: String = topic
        .editor_id
        .chars()
        .take(25 - q.chars().count())
        .collect();
    Some(
        format!(
            "sound/voice/{}/{}/{}_{}_{:08x}_{}.fuz",
            plugin, vt, q, t, local, response
        )
        .to_ascii_lowercase(),
    )
}

/// One line being spoken.
#[derive(Debug, Clone)]
pub struct Line {
    pub text: String,
    pub voice: Option<crate::audio::VoiceId>,
    pub ends_at: f64,
    /// The response's emotion type and strength.
    pub emotion: (u32, u32),
}

/// An ongoing conversation with an NPC.
pub struct Conversation {
    pub npc_ref: FormId,
    pub npc: FormId,
    pub name: String,
    pub voice_type: FormId,
    pub options: Vec<(Topic, String)>,
    /// Remaining lines of the current INFO, and the INFO itself.
    pub queue: Vec<Response>,
    pub current: Option<Line>,
    pub info: Option<(Topic, Info)>,
    pub ending: bool,
    pub said_once: std::collections::HashSet<FormId>,
}

impl Engine {
    fn dialogue_ctx(&self, npc_ref: FormId) -> condition::Context {
        condition::Context {
            subject: Some(npc_ref),
            target: Some(PLAYER_REF),
            ..Default::default()
        }
    }

    /// First INFO of a topic whose conditions pass for this speaker.
    pub fn select_info(&mut self, topic: &Topic, npc_ref: FormId) -> Option<Info> {
        if !topic.quest.is_null()
            && !self
                .scripts
                .quests
                .get(&topic.quest)
                .is_some_and(|q| q.running)
        {
            return None;
        }
        let ctx = self.dialogue_ctx(npc_ref);
        if !topic.quest.is_null() {
            let qc = self.quest_dialogue_conditions(topic.quest);
            let mut c = ctx;
            c.quest = Some(topic.quest);
            if !qc.is_empty() && !condition::evaluate(self, &qc, c) {
                return None;
            }
        }
        let ids: Vec<FormId> = self.lo.topic_infos(topic.id).to_vec();
        let mut passing = Vec::new();
        for id in ids {
            let Some(i) = info(&self.lo, id) else {
                continue;
            };
            let mut c = ctx;
            c.quest = Some(topic.quest);
            let pass = condition::evaluate(self, &i.conditions, c);
            if !pass && log::log_enabled!(target: "dialogue_fail", log::Level::Trace) {
                log::trace!(target: "dialogue_fail", "{} {} {}: {}", topic.editor_id, topic.id, i.id, condition::explain(self, &i.conditions, c));
            }
            if pass {
                if log::log_enabled!(log::Level::Trace) {
                    log::trace!(
                        "{} / {}: {}",
                        topic.editor_id,
                        topic.prompt,
                        condition::explain(self, &i.conditions, c)
                    );
                }
                if i.flags & info_flags::RANDOM == 0 {
                    if passing.is_empty() {
                        return Some(i);
                    }
                    break;
                }
                passing.push(i);
            }
        }
        if passing.is_empty() {
            return None;
        }
        let k = (self.rand() % passing.len() as u64) as usize;
        Some(passing.swap_remove(k))
    }

    /// Topics the player can choose at the top level.
    fn top_level_topics(&mut self, npc_ref: FormId) -> Vec<(Topic, String)> {
        let branches: Vec<FormId> = self.lo.ids_of_type(b"DLBR").to_vec();
        let mut out: Vec<(Topic, String)> = Vec::new();
        for b in branches {
            let Some(r) = self.lo.get(b) else { continue };
            let flags = r.get(b"DNAM").map(|d| d[0]).unwrap_or(0);
            if flags & 0x1 == 0 {
                continue;
            }
            let Some(start) = r
                .get(b"SNAM")
                .map(|d| r.fid(FormId(u32::from_le_bytes(d[0..4].try_into().unwrap()))))
            else {
                continue;
            };
            drop(r);
            let Some(t) = topic(&self.lo, start) else {
                continue;
            };
            if let Some(i) = self.select_info(&t, npc_ref) {
                let prompt = i.prompt.clone().unwrap_or_else(|| t.prompt.clone());
                if !prompt.is_empty() && !out.iter().any(|(x, _)| x.id == t.id) {
                    out.push((t, prompt));
                }
            }
        }
        out.sort_by(|a, b| b.0.priority.total_cmp(&a.0.priority));
        out
    }

    /// Topics of a given subtype (e.g. HELO greetings), highest priority first.
    fn topics_of_subtype(&self, sub: &[u8; 4]) -> Vec<Topic> {
        let mut v: Vec<Topic> = self
            .lo
            .ids_of_type(b"DIAL")
            .iter()
            .filter_map(|&d| topic(&self.lo, d))
            .filter(|t| &t.subtype == sub)
            .collect();
        v.sort_by(|a, b| b.priority.total_cmp(&a.priority));
        v
    }

    /// Talk to an NPC: a blocking branch's topic it has a line for comes
    /// first (`DLBR` flag 0x2: the guards' arrest, quests stopping the
    /// player...), else it greets the player with its first `HELO` line. A
    /// guard come to arrest the player starts the arrest.
    pub fn start_conversation(&mut self, npc_ref: FormId) {
        let greeting = self.blocking_greeting(npc_ref).or_else(|| {
            self.bark_topics(b"HELO").into_iter().find_map(|t| {
                let i = self.select_info(&t, npc_ref)?;
                Some((t, i))
            })
        });
        if let Some(p) = self.crime.arrests.alarmed.get(&npc_ref) {
            self.crime.arrests.arresting = Some((npc_ref, p.faction));
        }
        self.open_conversation(npc_ref, greeting);
    }

    /// The starting topic of a blocking branch (`DLBR` `DNAM` 0x2) the
    /// speaker has a line for, the highest priority first.
    pub(crate) fn blocking_greeting(&mut self, npc_ref: FormId) -> Option<(Topic, Info)> {
        let mut starts: Vec<Topic> = Vec::new();
        for &b in self.lo.ids_of_type(b"DLBR") {
            let Some(r) = self.lo.get(b) else { continue };
            if r.get(b"DNAM")
                .and_then(|d| d.first())
                .is_none_or(|f| f & 0x2 == 0)
            {
                continue;
            }
            let Some(start) = r
                .get(b"SNAM")
                .filter(|d| d.len() >= 4)
                .map(|d| r.fid(FormId(u32::from_le_bytes(d[0..4].try_into().unwrap()))))
            else {
                continue;
            };
            starts.extend(topic(&self.lo, start));
        }
        starts.sort_by(|a, b| b.priority.total_cmp(&a.priority));
        starts.into_iter().find_map(|t| {
            let i = self.select_info(&t, npc_ref)?;
            Some((t, i))
        })
    }

    /// Start a conversation with `npc_ref`, opening with `greeting`.
    pub(crate) fn open_conversation(&mut self, npc_ref: FormId, greeting: Option<(Topic, Info)>) {
        let Some(npc) = self.base_of(npc_ref) else {
            return;
        };
        self.talked_to_pc.insert(npc_ref);
        let voice_type = self.actor_voice_type(npc_ref).unwrap_or_default();
        let name = self.form_name(npc_ref);
        let mut conv = Conversation {
            npc_ref,
            npc,
            name,
            voice_type,
            options: Vec::new(),
            queue: Vec::new(),
            current: None,
            info: None,
            ending: false,
            said_once: Default::default(),
        };
        if let Some((t, i)) = greeting {
            conv.queue = i.responses.clone();
            conv.info = Some((t, i));
        }
        self.conversation = Some(conv);
        self.begin_info();
        self.refresh_options();
    }

    fn refresh_options(&mut self) {
        let Some(c) = &self.conversation else { return };
        let npc_ref = c.npc_ref;
        let choices = c
            .info
            .as_ref()
            .map(|(_, i)| i.choices.clone())
            .unwrap_or_default();
        let mut opts = Vec::new();
        for ch in choices {
            if let Some(t) = topic(&self.lo, ch)
                && let Some(i) = self.select_info(&t, npc_ref)
            {
                let prompt = i.prompt.clone().unwrap_or_else(|| t.prompt.clone());
                if !prompt.is_empty() {
                    opts.push((t, prompt));
                }
            }
        }
        if opts.is_empty() {
            opts = self.top_level_topics(npc_ref);
        }
        log::debug!(
            "dialogue options: {:?}",
            opts.iter().map(|o| &o.1).collect::<Vec<_>>()
        );
        if let Some(c) = self.conversation.as_mut() {
            c.options = opts;
        }
    }

    /// Player picked option `i` (or Goodbye if out of range).
    pub fn choose_topic(&mut self, i: usize) {
        let Some(c) = &self.conversation else { return };
        let Some((t, _)) = c.options.get(i).cloned() else {
            self.end_conversation();
            return;
        };
        let npc_ref = c.npc_ref;
        if let Some(info) = self.select_info(&t, npc_ref) {
            if let Some(c) = self.conversation.as_mut() {
                c.queue = info.responses.clone();
                c.info = Some((t, info));
            }
            self.begin_info();
        }
    }

    /// Run an INFO's begin or end fragment, `speaker` saying it.
    pub(crate) fn run_info_fragment(&mut self, info: &Info, begin: bool, speaker: FormId) {
        let (Some(script), Some(func)) = (
            &info.script,
            if begin {
                &info.begin_fragment
            } else {
                &info.end_fragment
            },
        ) else {
            return;
        };
        let speaker = self.object_value(speaker);
        let obj = papyrus::ObjectId::Form(info.id.0);
        let mut vm = std::mem::take(&mut self.vm);
        {
            let mut host = crate::script::EngineHost { engine: self };
            vm.attach(&mut host, obj, script, &[]);
            vm.call_method(&mut host, obj, script, func, vec![speaker]);
        }
        self.vm = vm;
    }

    fn begin_info(&mut self) {
        if let Some((_, info)) = self.conversation.as_ref().and_then(|c| c.info.clone()) {
            let speaker = self
                .conversation
                .as_ref()
                .map(|c| c.npc_ref)
                .unwrap_or_default();
            self.run_info_fragment(&info, true, speaker);
        }
        self.next_line();
    }

    /// Start the next queued response line (voice + subtitle).
    fn next_line(&mut self) {
        let now = self.scripts.real_time;
        let Some(c) = self.conversation.as_mut() else {
            return;
        };
        if c.queue.is_empty() {
            c.current = None;
            let info = c.info.clone();
            let npc_ref = c.npc_ref;
            if let Some((_, i)) = &info {
                self.run_info_fragment(i, false, npc_ref);
                if i.flags & info_flags::GOODBYE != 0 {
                    if let Some(c) = self.conversation.as_mut() {
                        c.ending = true;
                    }
                    return;
                }
            }
            self.refresh_options();
            return;
        }
        let r = c.queue.remove(0);
        let (topic, info_id) = match &c.info {
            Some((t, i)) => (t.clone(), i.id),
            None => return,
        };
        let voice_type = c.voice_type;
        let npc_ref = c.npc_ref;
        let path = voice_path(&self.lo, voice_type, &topic, info_id, r.number);
        let at = self
            .ref_position(npc_ref)
            .map(|p| p + glam::Vec3::Z * 110.0);
        let mut voice = None;
        let words = r.text.split_whitespace().count() as f64;
        let mut duration = (words * 0.32).max(2.0);
        if let (Some(p), Some(audio)) = (&path, self.audio.as_mut())
            && self.vfs.exists(p)
        {
            voice = audio.play(&self.vfs, p, 1.0, false, at, 400.0, 3000.0);
            duration = duration.max(1.0);
        }
        log::info!(
            "{}: {} [{}]",
            self.conversation
                .as_ref()
                .map(|c| c.name.as_str())
                .unwrap_or(""),
            r.text,
            path.unwrap_or_default()
        );
        if let Some(c) = self.conversation.as_mut() {
            c.current = Some(Line {
                text: r.text,
                voice,
                ends_at: now + duration,
                emotion: (r.emotion, r.emotion_value),
            });
        }
        // Gesture along with the line.
        self.talking_gesture(npc_ref);
    }

    pub fn end_conversation(&mut self) {
        let Some(c) = self.conversation.take() else {
            return;
        };
        if let (Some(line), Some(a)) = (&c.current, &self.audio)
            && let Some(v) = line.voice
        {
            a.stop(v);
        }
        self.end_talking_gestures(c.npc_ref);
        // On a goodbye line: the speaker meant it to end.
        let goodbye = c
            .info
            .as_ref()
            .is_some_and(|(_, i)| i.flags & info_flags::GOODBYE != 0);
        self.arrest_conversation_ended(c.npc_ref, goodbye);
    }

    /// Advance the conversation (called every frame).
    pub fn update_conversation(&mut self) {
        let now = self.scripts.real_time;
        let Some(c) = &self.conversation else { return };
        if let Some(line) = &c.current {
            // Voiced lines end with their audio; silent ones after a reading-time estimate.
            let done = match (line.voice, &self.audio) {
                (Some(v), Some(a)) => !a.is_playing(v),
                _ => now >= line.ends_at,
            };
            if done {
                self.next_line();
            }
        } else if c.ending {
            self.end_conversation();
        }
    }

    /// Skip the current line (like clicking during dialogue).
    pub fn skip_line(&mut self) {
        if let Some(c) = &self.conversation
            && let Some(line) = &c.current
        {
            if let (Some(v), Some(a)) = (line.voice, &self.audio) {
                a.stop(v);
            }
            self.next_line();
        }
    }
}
