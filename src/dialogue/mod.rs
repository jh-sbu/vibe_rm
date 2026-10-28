//! Dialogue: topics (DIAL), branches (DLBR), responses (INFO) and conversations.

use esp::{FormId, LoadOrder};

use crate::condition::{self, Condition};
use crate::engine::{Engine, PLAYER_REF};

pub mod barks;
mod social;

pub mod info_flags {
    pub const GOODBYE: u16 = 0x1;
    pub const RANDOM: u16 = 0x2;
    pub const SAY_ONCE: u16 = 0x4;
    pub const RANDOM_END: u16 = 0x20;
    pub const INVISIBLE_CONTINUE: u16 = 0x40;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voice_paths_follow_the_quest_and_topic_length_rules() {
        use esp::writer::{Node, RecordBuilder, write_plugin};

        let cases = [
            // The Bleak Falls campfire scene has no topic editor ID.
            ("MS13BleakFallsBarrowCampfireScene", "", "ms13bleakf_"),
            // A long quest is preserved when the combined length fits.
            ("DialogueGeneric", "", "dialoguegeneric_"),
            ("LongQuestID", "ShortTopic", "longquestid_shorttopic"),
            // Exactly 25 characters preserves both IDs; 26 applies the cuts.
            (
                "ABCDEFGHIJK",
                "ABCDEFGHIJKLMN",
                "abcdefghijk_abcdefghijklmn",
            ),
            (
                "ABCDEFGHIJK",
                "ABCDEFGHIJKLMNO",
                "abcdefghij_abcdefghijklmno",
            ),
            // A short topic does not give a long quest more than 10 characters.
            ("ABCDEFGHIJKLMNOPQRSTUVWX", "AB", "abcdefghij_ab"),
            (
                "Short",
                "ABCDEFGHIJKLMNOPQRSTU",
                "short_abcdefghijklmnopqrst",
            ),
            (
                "ABCDEFGHIJ",
                "ABCDEFGHIJKLMNOP",
                "abcdefghij_abcdefghijklmno",
            ),
        ];
        let dir = std::env::temp_dir().join(format!("vrm-voice-paths-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let quests = cases.iter().enumerate().map(|(i, (quest, _, _))| {
            RecordBuilder::new(b"QUST", FormId(0x900 + i as u32)).zstring(b"EDID", quest)
        });
        std::fs::write(
            dir.join("Voice.esp"),
            write_plugin(
                0,
                &[],
                "test",
                "synthetic",
                0xa00,
                &[
                    Node::top(b"QUST", quests.collect()),
                    Node::top(
                        b"VTYP",
                        vec![
                            RecordBuilder::new(b"VTYP", FormId(0x800))
                                .zstring(b"EDID", "FemaleNord"),
                        ],
                    ),
                    Node::top(b"INFO", vec![RecordBuilder::new(b"INFO", FormId(0x801))]),
                ],
            ),
        )
        .unwrap();
        let lo = LoadOrder::load(&dir, &["Voice.esp".to_string()]).unwrap();
        for (i, (quest, topic, expected)) in cases.iter().enumerate() {
            let (mut t, _) = candidate(1, 2, 0);
            t.quest = FormId(0x900 + i as u32);
            t.editor_id = topic.to_string();
            assert_eq!(
                voice_path(&lo, FormId(0x800), &t, FormId(0x801), 2),
                Some(format!(
                    "sound/voice/voice.esp/femalenord/{expected}_00000801_2.fuz"
                )),
                "quest={quest}, topic={topic}",
            );
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    fn candidate(topic_id: u32, info_id: u32, flags: u16) -> (Topic, Info) {
        (
            Topic {
                id: FormId(topic_id),
                editor_id: String::new(),
                prompt: String::new(),
                priority: 50.0,
                quest: FormId(topic_id + 100),
                branch: FormId(0),
                subtype: *b"RUMO",
            },
            Info {
                id: FormId(info_id),
                flags,
                responses: Vec::new(),
                conditions: Vec::new(),
                choices: Vec::new(),
                prompt: None,
                script: None,
                begin_fragment: None,
                end_fragment: None,
            },
        )
    }

    #[test]
    fn responses_keep_their_speaker_and_listener_idles() {
        use esp::writer::{Node, RecordBuilder, write_plugin};
        let dir = std::env::temp_dir().join(format!("vrm-info-idles-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let trdt = |number: u8| {
            let mut t = [0u8; 24];
            t[12] = number;
            t
        };
        let info = RecordBuilder::new(b"INFO", FormId(0x800))
            .field(b"TRDT", &trdt(1))
            .zstring(b"NAM1", "Over here.")
            .form(b"SNAM", FormId(0x810))
            .form(b"LNAM", FormId(0x811))
            .field(b"TRDT", &trdt(2))
            .zstring(b"NAM1", "Quickly.")
            .field(b"TRDT", &trdt(3))
            .zstring(b"NAM1", "No idles.")
            // The idle of the next response, not this one.
            .form(b"SNAM", FormId(0));
        std::fs::write(
            dir.join("Info.esp"),
            write_plugin(
                0,
                &[],
                "test",
                "synthetic",
                0x820,
                &[Node::top(b"INFO", vec![info])],
            ),
        )
        .unwrap();
        let lo = LoadOrder::load(&dir, &["Info.esp".to_string()]).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        let i = super::info(&lo, FormId(0x800)).unwrap();
        let idles: Vec<_> = i
            .responses
            .iter()
            .map(|r| (r.number, r.speaker_idle, r.listener_idle))
            .collect();
        assert_eq!(
            idles,
            [
                (1, Some(FormId(0x810)), Some(FormId(0x811))),
                (2, None, None),
                // A null idle is none.
                (3, None, None),
            ]
        );
    }

    #[test]
    fn rumor_group_keeps_each_response_across_quests() {
        let group = response_group([
            candidate(1, 11, info_flags::RANDOM),
            candidate(1, 12, info_flags::RANDOM),
            candidate(2, 21, info_flags::RANDOM),
        ]);
        assert_eq!(
            group.iter().map(|(_, i)| i.id.0).collect::<Vec<_>>(),
            [11, 12, 21]
        );
        assert_ne!(group[0].0.quest, group[2].0.quest);
    }

    #[test]
    fn random_end_bounds_the_pool_across_topics() {
        let group = response_group([
            candidate(1, 11, info_flags::RANDOM),
            candidate(2, 21, info_flags::RANDOM | info_flags::RANDOM_END),
            candidate(3, 31, info_flags::RANDOM),
        ]);
        assert_eq!(
            group.iter().map(|(_, i)| i.id.0).collect::<Vec<_>>(),
            [11, 21]
        );
    }

    #[test]
    fn nonrandom_response_terminates_and_is_included() {
        let group = response_group([
            candidate(1, 11, info_flags::RANDOM),
            candidate(2, 21, 0),
            candidate(3, 31, info_flags::RANDOM),
        ]);
        assert_eq!(
            group.iter().map(|(_, i)| i.id.0).collect::<Vec<_>>(),
            [11, 21]
        );
        assert_eq!(
            response_group([candidate(1, 11, 0), candidate(2, 21, 0)]).len(),
            1
        );
        assert!(response_group([]).is_empty());
    }

    #[test]
    fn rumor_prompt_follows_selected_info_and_identity_survives_topic_changes() {
        let (topic, info) = candidate(1, 11, info_flags::RANDOM);
        let default = DialogueOption {
            pooled: true,
            ..DialogueOption::new(topic, info, true, "Localized rumors?")
        };
        assert_eq!(default.prompt, "Localized rumors?");
        let (mut topic, mut info) = candidate(2, 21, info_flags::RANDOM);
        topic.prompt = "Topic prompt".into();
        info.prompt = Some("Give me the latest gossip.".into());
        let selected = DialogueOption {
            pooled: true,
            ..DialogueOption::new(topic, info, false, "Localized rumors?")
        };
        assert_eq!(selected.prompt, "Give me the latest gossip.");
        assert_eq!(selected.info.id, FormId(21));
        assert_eq!(selected.topic.quest, FormId(102));
        assert!(!selected.new);
        assert_eq!(default.id(), selected.id());
    }

    #[test]
    fn linked_rumor_topics_keep_separate_identities() {
        let make = |id| {
            let (topic, info) = candidate(id, id + 10, info_flags::RANDOM);
            DialogueOption::new(topic, info, true, "Rumors?")
        };
        assert_eq!(make(1).id(), OptionId::Topic(FormId(1)));
        assert_ne!(make(1).id(), make(2).id());
    }

    #[test]
    fn custom_topics_with_identical_text_keep_separate_identities() {
        let make = |id| {
            let (mut topic, info) = candidate(id, id + 10, 0);
            topic.subtype = *b"CUST";
            topic.prompt = "Same words".into();
            DialogueOption::new(topic, info, true, "Rumors?")
        };
        assert_ne!(make(1).id(), make(2).id());
        let (mut topic, info) = candidate(3, 13, 0);
        topic.subtype = *b"CUST";
        assert!(
            DialogueOption::new(topic, info, true, "Rumors?")
                .prompt
                .is_empty()
        );
    }
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
    /// The idles the speaker and the listener play with it (`SNAM`, `LNAM`).
    pub speaker_idle: Option<FormId>,
    pub listener_idle: Option<FormId>,
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

/// The pooled rumor entry keeps one identity even when the chosen topic
/// changes quests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionId {
    Topic(FormId),
    Rumors,
}

#[derive(Debug, Clone)]
pub struct DialogueOption {
    pub topic: Topic,
    /// Selected when the menu is built, so its prompt and spoken line agree.
    pub info: Info,
    pub prompt: String,
    pub new: bool,
    /// The top-level rumor entry pooled across quests, as opposed to a RUMO
    /// topic listed on its own among an INFO's linked choices.
    pub pooled: bool,
}

impl DialogueOption {
    pub fn id(&self) -> OptionId {
        if self.pooled {
            OptionId::Rumors
        } else {
            OptionId::Topic(self.topic.id)
        }
    }

    fn new(topic: Topic, info: Info, new: bool, rumor_prompt: &str) -> Self {
        let prompt = info.prompt.clone().unwrap_or_else(|| {
            if topic.prompt.is_empty() && topic.subtype == *b"RUMO" {
                rumor_prompt.to_owned()
            } else {
                topic.prompt.clone()
            }
        });
        Self {
            topic,
            info,
            prompt,
            new,
            pooled: false,
        }
    }
}

/// The first qualifying random group, including its terminating INFO.
/// Cross-quest ordering and weighting for rumors remain provisional; see
/// known_ambiguities/dialogue-selection.md.
fn response_group(infos: impl IntoIterator<Item = (Topic, Info)>) -> Vec<(Topic, Info)> {
    let mut group = Vec::new();
    for (topic, info) in infos {
        let end = info.flags & info_flags::RANDOM == 0 || info.flags & info_flags::RANDOM_END != 0;
        group.push((topic, info));
        if end {
            break;
        }
    }
    group
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
                    speaker_idle: None,
                    listener_idle: None,
                });
            }
            b"SNAM" | b"LNAM" if sr.data.len() >= 4 => {
                let idle = Some(r.fid(sr.form_id(0))).filter(|i| !i.is_null());
                if let Some(c) = cur.as_mut() {
                    if sr.tag.0 == *b"SNAM" {
                        c.speaker_idle = idle;
                    } else {
                        c.listener_idle = idle;
                    }
                }
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
    // xEdit's Export dialogues.pas (InfoFileName): preserve both IDs up to a
    // combined 25 characters. Above that, cut a long quest to 10 and topic to
    // 15; a quest of at most 10 leaves the remaining budget to the topic.
    // https://github.com/Sharlikran/xEdit/blob/master/Edit%20Scripts/Export%20dialogues.pas
    let qlen = quest.chars().count();
    let tlen = topic.editor_id.chars().count();
    let (qlen, tlen) = if qlen + tlen > 25 {
        if qlen > 10 {
            (10, 15)
        } else {
            (qlen, 25 - qlen)
        }
    } else {
        (qlen, tlen)
    };
    let q: String = quest.chars().take(qlen).collect();
    let t: String = topic.editor_id.chars().take(tlen).collect();
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
    /// Menu entries with their already-selected responses.
    pub options: Vec<DialogueOption>,
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

    /// INFOs whose quest and speaker conditions pass, in topic order.
    fn eligible_infos(&mut self, topic: &Topic, npc_ref: FormId) -> Vec<Info> {
        self.eligible_infos_for(topic, npc_ref, PLAYER_REF)
    }

    fn eligible_infos_for(
        &mut self,
        topic: &Topic,
        npc_ref: FormId,
        listener: FormId,
    ) -> Vec<Info> {
        if !topic.quest.is_null()
            && !self
                .scripts
                .quests
                .get(&topic.quest)
                .is_some_and(|q| q.running)
        {
            return Vec::new();
        }
        let mut ctx = self.dialogue_ctx(npc_ref);
        ctx.target = Some(listener);
        if !topic.quest.is_null() {
            let qc = self.quest_dialogue_conditions(topic.quest);
            let mut c = ctx;
            c.quest = Some(topic.quest);
            if !qc.is_empty() && !condition::evaluate(self, &qc, c) {
                return Vec::new();
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
                passing.push(i);
            }
        }
        passing
    }

    fn select_response(&mut self, infos: Vec<(Topic, Info)>) -> Option<(Topic, Info)> {
        let mut group = response_group(infos);
        if group.is_empty() {
            return None;
        }
        let k = (self.rand() % group.len() as u64) as usize;
        Some(group.swap_remove(k))
    }

    pub fn select_info(&mut self, topic: &Topic, npc_ref: FormId) -> Option<Info> {
        self.select_info_for(topic, npc_ref, PLAYER_REF)
    }

    pub(crate) fn select_info_for(
        &mut self,
        topic: &Topic,
        npc_ref: FormId,
        listener: FormId,
    ) -> Option<Info> {
        let infos = self
            .eligible_infos_for(topic, npc_ref, listener)
            .into_iter()
            .map(|i| (topic.clone(), i))
            .collect();
        self.select_response(infos).map(|(_, i)| i)
    }

    fn dialogue_option(&self, topic: Topic, info: Info) -> DialogueOption {
        let new = !self.infos_said.contains(&info.id);
        let fallback = self
            .gmst_string("sTopicSubtypeTextPlayerDialogueRumors")
            .unwrap_or_else(|| "Heard any rumors lately?".into());
        DialogueOption::new(topic, info, new, &fallback)
    }

    /// One RUMO entry across running quests, rather than one entry per branch.
    /// Only topics in a top-level branch take part: Rorik's patrol comments
    /// are RUMO in a branch that isn't, and he offers no rumors in the game.
    fn rumor_option(&mut self, npc_ref: FormId) -> Option<DialogueOption> {
        let mut topics = self.topics_of_subtype(b"RUMO");
        topics.retain(|t| {
            self.lo
                .get(t.branch)
                .and_then(|r| r.get(b"DNAM").and_then(|d| d.first()).copied())
                .is_some_and(|f| f & 0x1 != 0)
        });
        let priority = |t: &Topic| {
            self.lo
                .get(t.quest)
                .and_then(|r| r.get(b"DNAM").and_then(|d| d.get(2)).copied())
                .unwrap_or(0)
        };
        // Quest priority orders the response pool; topic priority orders the
        // menu. FormID is our deterministic fallback for unresolved ties.
        topics.sort_by(|a, b| priority(b).cmp(&priority(a)).then(a.id.0.cmp(&b.id.0)));
        let mut infos = Vec::new();
        for t in topics {
            infos.extend(
                self.eligible_infos(&t, npc_ref)
                    .into_iter()
                    .map(|i| (t.clone(), i)),
            );
        }
        self.select_response(infos).map(|(t, i)| DialogueOption {
            pooled: true,
            ..self.dialogue_option(t, i)
        })
    }

    /// Topics the player can choose at the top level.
    fn top_level_topics(&mut self, npc_ref: FormId) -> Vec<DialogueOption> {
        let branches: Vec<FormId> = self.lo.ids_of_type(b"DLBR").to_vec();
        let mut out: Vec<DialogueOption> = Vec::new();
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
            if t.subtype == *b"RUMO" || out.iter().any(|o| o.topic.id == t.id) {
                continue;
            }
            if let Some(i) = self.select_info(&t, npc_ref) {
                let option = self.dialogue_option(t, i);
                if !option.prompt.is_empty() {
                    out.push(option);
                }
            }
        }
        out.extend(self.rumor_option(npc_ref));
        out.sort_by(|a, b| b.topic.priority.total_cmp(&a.topic.priority));
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
        if self.base_of(npc_ref).is_none() {
            return;
        }
        if let Some(p) = self.crime.arrests.alarmed.get(&npc_ref) {
            self.crime.arrests.arresting = Some((npc_ref, p.faction));
        }
        let greeting = self.blocking_greeting(npc_ref).or_else(|| {
            self.bark_topics(b"HELO").into_iter().find_map(|t| {
                let i = self.select_info(&t, npc_ref)?;
                Some((t, i))
            })
        });
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
                let option = self.dialogue_option(t, i);
                if !option.prompt.is_empty() {
                    opts.push(option);
                }
            }
        }
        if opts.is_empty() {
            opts = self.top_level_topics(npc_ref);
        }
        log::debug!(
            "dialogue options: {:?}",
            opts.iter()
                .map(|o| (&o.prompt, o.topic.id, o.info.id))
                .collect::<Vec<_>>()
        );
        if let Some(c) = self.conversation.as_mut() {
            c.options = opts;
        }
    }

    /// Player picked option `i` (or Goodbye if out of range).
    pub fn choose_topic(&mut self, i: usize) {
        let Some(c) = &self.conversation else { return };
        let Some(option) = c.options.get(i).cloned() else {
            self.end_conversation();
            return;
        };
        if let Some(c) = self.conversation.as_mut() {
            c.queue = option.info.responses.clone();
            c.info = Some((option.topic, option.info));
        }
        self.begin_info();
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
            self.infos_said.insert(info.id);
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
        let idle = r.speaker_idle;
        let listener_idle = r.listener_idle;
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
            voice = audio.play(
                &self.vfs,
                p,
                crate::audio::VoiceParams {
                    volume: crate::world::sound::voice_volume(&self.lo),
                    rate: 1.0,
                    looping: false,
                    position: at,
                    falloff: crate::world::sound::voice_falloff(&self.lo),
                    exempt: true,
                    ..Default::default()
                },
            );
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
        self.line_gesture(npc_ref, idle);
        self.listener_gesture(PLAYER_REF, listener_idle);
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

    /// Drop a transient conversation while applying a save. Conversations are
    /// not part of the snapshot, so don't run goodbye or arrest side effects.
    pub(crate) fn discard_conversation_for_load(&mut self) {
        let Some(conversation) = self.conversation.take() else {
            return;
        };
        if let (Some(line), Some(audio)) = (&conversation.current, &self.audio)
            && let Some(voice) = line.voice
        {
            audio.stop(voice);
        }
        self.end_talking_gestures(conversation.npc_ref);
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
