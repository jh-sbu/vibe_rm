//! The quest journal: stage log entries (`QSDT` / `CNAM`, those whose
//! conditions pass; flags completing or failing the quest), objectives
//! displayed, completed and failed, and the HUD's quest banners.

use esp::FormId;

use crate::engine::{Engine, PLAYER_REF};

/// Quest types (`DNAM`): only quests with a type show in the journal, and the
/// miscellaneous ones share one entry.
pub const QUEST_TYPE_NONE: u8 = 0;
pub const QUEST_TYPE_MISC: u8 = 6;

/// Log entry flags (`QSDT`).
const ENTRY_COMPLETES: u8 = 0x1;
const ENTRY_FAILS: u8 = 0x2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectiveState {
    Shown,
    Completed,
    Failed,
}

/// A quest as the journal shows it.
#[derive(Debug, Clone)]
pub struct JournalQuest {
    pub id: FormId,
    pub name: String,
    pub kind: u8,
    pub completed: bool,
    pub failed: bool,
    /// Log entries, oldest first.
    pub log: Vec<String>,
    /// Displayed objectives by index.
    pub objectives: Vec<(i32, String, ObjectiveState)>,
}

impl Engine {
    /// A quest's type (`DNAM`'s ninth byte).
    pub fn quest_type(&self, q: FormId) -> u8 {
        self.lo
            .get(q)
            .and_then(|r| r.get(b"DNAM").filter(|d| d.len() >= 12).map(|d| d[8]))
            .unwrap_or(QUEST_TYPE_NONE)
    }

    /// A quest's name with its tags replaced.
    pub fn quest_name(&mut self, q: FormId) -> String {
        let name = self
            .lo
            .get(q)
            .and_then(|r| r.get(b"FULL").map(|d| self.lo.lstring(&r, d)))
            .unwrap_or_default();
        self.replace_text_tags(&name, Some(q))
    }

    /// A stage was set: each of its log entries whose conditions pass, in
    /// order, goes into the journal (when it has text) and may complete or fail
    /// the quest. Returns those entries' indices (whose fragments run); none
    /// when the stage has no entries.
    ///
    /// Not only the first passing entry: the game's stages put an unconditional
    /// entry (completing the quest, with a fragment) before conditional ones
    /// holding the text and further fragments (`MQ102` stage 160).
    pub(crate) fn add_log_entries(&mut self, q: FormId, stage: u16) -> Option<Vec<usize>> {
        let rec = self.lo.get(q)?;
        // (flags, conditions, text) of each of the stage's entries.
        let mut entries: Vec<(u8, Vec<crate::condition::Condition>, String)> = Vec::new();
        let mut in_stage = false;
        for sr in rec.subrecords() {
            match &sr.tag.0 {
                b"INDX" => {
                    if in_stage {
                        break;
                    }
                    in_stage = sr.u16(0) == stage;
                }
                b"QOBJ" | b"ALST" | b"ALLS" | b"ANAM" if in_stage => break,
                _ if !in_stage => {}
                b"QSDT" => entries.push((sr.u8(0), Vec::new(), String::new())),
                b"CTDA" => {
                    if let (Some(e), Some(c)) =
                        (entries.last_mut(), crate::condition::parse(&rec, sr.data))
                    {
                        e.1.push(c);
                    }
                }
                b"CIS1" | b"CIS2" => {
                    if let Some(c) = entries.last_mut().and_then(|e| e.1.last_mut()) {
                        let s = Some(sr.zstring().into());
                        if sr.tag.0 == *b"CIS1" {
                            c.string_p1 = s;
                        } else {
                            c.string_p2 = s;
                        }
                    }
                }
                b"CNAM" => {
                    if let Some(e) = entries.last_mut() {
                        e.2 = self.lo.lstring(&rec, sr.data);
                    }
                }
                _ => {}
            }
        }
        drop(rec);
        if entries.is_empty() {
            return None;
        }
        let ctx = crate::condition::Context {
            subject: Some(PLAYER_REF),
            quest: Some(q),
            ..Default::default()
        };
        let mut passed = Vec::new();
        for (index, (flags, conds, text)) in entries.into_iter().enumerate() {
            if !crate::condition::evaluate(self, &conds, ctx) {
                continue;
            }
            passed.push(index);
            if !text.is_empty() {
                let text = self.replace_text_tags(&text, Some(q));
                self.announce_quest(q);
                self.touch_quest(q).log.push(text);
            }
            if flags & ENTRY_COMPLETES != 0 {
                self.complete_quest(q);
            }
            if flags & ENTRY_FAILS != 0 {
                self.fail_quest(q);
            }
        }
        Some(passed)
    }

    /// The quest's state, marked as updated last (the journal lists it first).
    fn touch_quest(&mut self, q: FormId) -> &mut crate::script::QuestState {
        self.scripts.journal_seq += 1;
        let seq = self.scripts.journal_seq;
        let st = self.scripts.quests.entry(q).or_default();
        st.journal_seq = seq;
        st
    }

    /// The first time a quest the journal shows gets an entry or objective:
    /// its banner.
    fn announce_quest(&mut self, q: FormId) {
        let st = self.scripts.quests.entry(q).or_default();
        if st.announced || self.quest_type(q) == QUEST_TYPE_NONE {
            return;
        }
        self.scripts.quests.entry(q).or_default().announced = true;
        let name = self.quest_name(q);
        if !name.is_empty() {
            let sub = self
                .gmst_string("sQuestAddedText")
                .unwrap_or_else(|| "Quest added".into());
            self.banner(name, sub);
        }
    }

    /// `Quest.CompleteQuest`, or a log entry completing it.
    pub fn complete_quest(&mut self, q: FormId) {
        let st = self.touch_quest(q);
        if std::mem::replace(&mut st.completed, true) {
            return;
        }
        self.quest_ended(q, "sQuestCompletedText", "Quest completed");
    }

    /// A log entry failing the quest.
    pub fn fail_quest(&mut self, q: FormId) {
        let st = self.touch_quest(q);
        if std::mem::replace(&mut st.failed, true) {
            return;
        }
        self.quest_ended(q, "sQuestFailed", "Quest FAILED");
    }

    fn quest_ended(&mut self, q: FormId, setting: &str, default: &str) {
        log::info!("quest {q}: {default}");
        if self.quest_type(q) == QUEST_TYPE_NONE {
            return;
        }
        let name = self.quest_name(q);
        if !name.is_empty() {
            let sub = self.gmst_string(setting).unwrap_or_else(|| default.into());
            self.banner(name, sub);
        }
    }

    /// `Quest.SetObjectiveDisplayed`: shown (its text, with the aliases as they
    /// are now, in the HUD) or hidden; `force` shows it again when shown already.
    pub fn set_objective_displayed(&mut self, q: FormId, obj: i32, displayed: bool, force: bool) {
        if !displayed {
            let st = self.touch_quest(q);
            st.objectives_displayed.remove(&obj);
            return;
        }
        let shown = self
            .scripts
            .quests
            .get(&q)
            .is_some_and(|s| s.objectives_displayed.contains(&obj));
        if shown && !force {
            return;
        }
        let raw = self.objective_text(q, obj);
        let text = self.replace_text_tags(&raw, Some(q));
        self.announce_quest(q);
        let st = self.touch_quest(q);
        st.objectives_displayed.insert(obj);
        st.objective_texts.insert(obj, text.clone());
        if !text.is_empty() {
            self.scripts.notify(text);
        }
    }

    /// `Quest.SetObjectiveCompleted`.
    pub fn set_objective_completed(&mut self, q: FormId, obj: i32, completed: bool) {
        let st = self.touch_quest(q);
        let changed = if completed {
            st.objectives_completed.insert(obj)
        } else {
            st.objectives_completed.remove(&obj)
        };
        let text = st.objective_texts.get(&obj).cloned().unwrap_or_default();
        if changed && completed && st.objectives_displayed.contains(&obj) && !text.is_empty() {
            let done = self
                .gmst_string("sHUDCompleted")
                .unwrap_or_else(|| "Completed".into());
            self.scripts.notify(format!("{done}: {text}"));
        }
    }

    /// `Quest.SetObjectiveFailed`.
    pub fn set_objective_failed(&mut self, q: FormId, obj: i32, failed: bool) {
        let st = self.touch_quest(q);
        if failed {
            st.objectives_failed.insert(obj);
        } else {
            st.objectives_failed.remove(&obj);
        }
    }

    /// The quests the journal lists, the latest updated first: the running
    /// and finished ones with a type and something to show.
    pub fn journal(&self) -> Vec<JournalQuest> {
        let mut out: Vec<(u64, JournalQuest)> = Vec::new();
        for (&q, st) in &self.scripts.quests {
            let kind = self.quest_type(q);
            if kind == QUEST_TYPE_NONE || (st.log.is_empty() && st.objectives_displayed.is_empty())
            {
                continue;
            }
            let mut objectives: Vec<(i32, String, ObjectiveState)> = st
                .objectives_displayed
                .iter()
                .map(|&o| {
                    let state = if st.objectives_failed.contains(&o) {
                        ObjectiveState::Failed
                    } else if st.objectives_completed.contains(&o) {
                        ObjectiveState::Completed
                    } else {
                        ObjectiveState::Shown
                    };
                    (
                        o,
                        st.objective_texts.get(&o).cloned().unwrap_or_default(),
                        state,
                    )
                })
                .collect();
            objectives.sort_by_key(|o| o.0);
            let name = self
                .lo
                .get(q)
                .and_then(|r| r.get(b"FULL").map(|d| self.lo.lstring(&r, d)))
                .unwrap_or_default();
            out.push((
                st.journal_seq,
                JournalQuest {
                    id: q,
                    name,
                    kind,
                    completed: st.completed,
                    failed: st.failed,
                    log: st.log.clone(),
                    objectives,
                },
            ));
        }
        out.sort_by(|a, b| b.0.cmp(&a.0));
        out.into_iter().map(|(_, j)| j).collect()
    }
}
