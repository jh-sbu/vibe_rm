//! Shouts: words of power learned (`Game.TeachWord`) and unlocked with
//! dragon souls (`Game.UnlockWord`, the magic menu), the shouts they belong
//! to, the one readied in the voice slot, and the player shouting it.
//!
//! A shout (`SHOU`) has up to three words (`SNAM`: the word, the spell it
//! casts, the recovery time). Holding the voice key (Z) longer speaks more
//! of the shout's words, as many as are unlocked in a row from the first:
//! one word under `fShoutTime1`, two under `fShoutTime2`, three after; the
//! shout goes out when the key comes up, or as soon as the last unlocked word
//! is reached. Its spell is cast as the player's other spells are
//! (`crate::casting`), and the voice then rests for that word's recovery
//! time times `ShoutRecoveryMult`: the HUD's shout meter fills again, and
//! flashes when the key is pressed before it is full.
//!
//! The player has a shout once they know one of its words, or a script gave
//! it (`AddShout`); NPCs have the shouts their record lists (`SPLO`) and
//! those given. Sources: UESP's Shouts and Dragon Shouts pages, the SHOU /
//! WOOP records, the game settings, `hudmenu.swf` and `magicmenu.swf`.
//! Choices: `known_gaps/shouts.md`.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use esp::FormId;
use esp::actor_value as av;

use crate::ai::combat::gmst_f32;
use crate::engine::{Engine, PLAYER_REF};
use crate::spells::VOICE;

/// One of a shout's words: the word of power, the spell it casts, the rest
/// the voice needs after it.
#[derive(Debug, Clone)]
pub struct ShoutWord {
    pub word: FormId,
    pub spell: FormId,
    pub recovery: f32,
}

/// A `SHOU` record as read.
#[derive(Debug, Clone)]
pub struct Shout {
    pub name: String,
    pub description: String,
    pub words: Vec<ShoutWord>,
}

/// A `WOOP` record: the word in the dragon tongue and its meaning.
#[derive(Debug, Clone, Default)]
pub struct Word {
    pub dragon: String,
    pub translation: String,
}

/// Words known and unlocked, shouts given and taken, and the player's voice.
#[derive(Default)]
pub struct Shouts {
    pub known: HashSet<FormId>,
    pub unlocked: HashSet<FormId>,
    added: HashMap<FormId, Vec<FormId>>,
    removed: HashMap<FormId, HashSet<FormId>>,
    /// Seconds before the player can shout again, and the rest's whole length.
    pub recovery: f32,
    pub recovery_total: f32,
    /// Seconds the voice key has been held, while it is.
    held: Option<f32>,
    /// The HUD's shout meter should flash (counted).
    pub meter_flashes: u32,
    shouts: std::cell::RefCell<HashMap<FormId, Option<Arc<Shout>>>>,
    /// Each word's shout, read on first use.
    by_word: std::cell::OnceCell<HashMap<FormId, FormId>>,
}

impl Engine {
    /// A shout record, read once.
    pub fn shout(&self, id: FormId) -> Option<Arc<Shout>> {
        if let Some(s) = self.shouts.shouts.borrow().get(&id) {
            return s.clone();
        }
        let s = self.read_shout(id).map(Arc::new);
        self.shouts.shouts.borrow_mut().insert(id, s.clone());
        s
    }

    fn read_shout(&self, id: FormId) -> Option<Shout> {
        let rec = self.lo.get(id).filter(|r| r.tag().0 == *b"SHOU")?;
        let words = rec
            .subrecords()
            .filter(|s| s.tag.0 == *b"SNAM" && s.data.len() >= 12)
            .map(|s| ShoutWord {
                word: rec.fid(FormId(s.u32(0))),
                spell: rec.fid(FormId(s.u32(4))),
                recovery: s.f32(8),
            })
            .filter(|w| !w.word.is_null())
            .collect();
        let text = |tag: &[u8; 4]| {
            rec.get(tag)
                .map(|x| self.lo.lstring(&rec, x))
                .unwrap_or_default()
        };
        Some(Shout {
            name: text(b"FULL"),
            description: text(b"DESC"),
            words,
        })
    }

    /// A word of power's names.
    pub fn word(&self, id: FormId) -> Word {
        let Some(rec) = self.lo.get(id).filter(|r| r.tag().0 == *b"WOOP") else {
            return Word::default();
        };
        let s = |tag: &[u8; 4]| {
            rec.get(tag)
                .map(|x| self.lo.lstring(&rec, x))
                .unwrap_or_default()
        };
        Word {
            dragon: s(b"FULL"),
            translation: s(b"TNAM"),
        }
    }

    /// The shout a word belongs to.
    fn shout_of_word(&self, word: FormId) -> Option<FormId> {
        self.shouts
            .by_word
            .get_or_init(|| {
                let mut map = HashMap::new();
                for &id in self.lo.ids_of_type(b"SHOU") {
                    if let Some(s) = self.shout(id) {
                        for w in &s.words {
                            map.entry(w.word).or_insert(id);
                        }
                    }
                }
                map
            })
            .get(&word)
            .copied()
    }

    /// The shouts an actor has: the player's for the words they know, an
    /// NPC's from its record, and those scripts gave, less those taken.
    pub fn actor_shouts(&self, actor: FormId) -> Vec<FormId> {
        let mut out: Vec<FormId> = if actor == PLAYER_REF {
            let mut known: Vec<FormId> = self.shouts.known.iter().copied().collect();
            known.sort();
            known
                .into_iter()
                .filter_map(|w| self.shout_of_word(w))
                .collect()
        } else {
            self.actor_spells(actor)
                .into_iter()
                .filter(|&s| self.lo.tag_of(s).is_some_and(|t| t.0 == *b"SHOU"))
                .collect()
        };
        out.extend(self.shouts.added.get(&actor).into_iter().flatten().copied());
        if let Some(r) = self.shouts.removed.get(&actor) {
            out.retain(|s| !r.contains(s));
        }
        let mut seen = HashSet::new();
        out.retain(|s| seen.insert(*s));
        out
    }

    pub fn has_shout(&self, actor: FormId, shout: FormId) -> bool {
        self.actor_shouts(actor).contains(&shout)
    }

    /// `Actor.AddShout`.
    pub fn add_shout(&mut self, actor: FormId, shout: FormId) -> bool {
        if self.shout(shout).is_none() || self.has_shout(actor, shout) {
            return false;
        }
        if let Some(r) = self.shouts.removed.get_mut(&actor) {
            r.remove(&shout);
        }
        if !self.has_shout(actor, shout) {
            self.shouts.added.entry(actor).or_default().push(shout);
        }
        log::info!("{actor} gains the shout {shout}");
        true
    }

    /// `Actor.RemoveShout`.
    pub fn remove_shout(&mut self, actor: FormId, shout: FormId) -> bool {
        if !self.has_shout(actor, shout) {
            return false;
        }
        if let Some(a) = self.shouts.added.get_mut(&actor) {
            a.retain(|&s| s != shout);
        }
        self.shouts.removed.entry(actor).or_default().insert(shout);
        if self.equipped_spell(actor, VOICE) == Some(shout) {
            self.unequip_spell(actor, shout, Some(VOICE));
        }
        true
    }

    /// Whether a form is a shout.
    pub fn is_shout(&self, form: FormId) -> bool {
        self.lo.tag_of(form).is_some_and(|t| t.0 == *b"SHOU")
    }

    /// Readies a shout in the voice slot (`EquipShout`, the magic menu).
    pub fn equip_shout(&mut self, actor: FormId, shout: FormId) {
        if !self.is_shout(shout) {
            return;
        }
        self.equipped_spells.entry(actor).or_default()[VOICE] = Some(shout);
        log::info!("{actor} readies the shout {shout}");
    }

    /// The shout an actor has readied (`GetEquippedShout`).
    pub fn equipped_shout(&self, actor: FormId) -> Option<FormId> {
        self.equipped_spell(actor, VOICE)
            .filter(|&s| self.is_shout(s))
    }

    /// `Game.TeachWord`: the player knows a word (its shout is theirs).
    pub fn teach_word(&mut self, word: FormId) {
        if !self.shouts.known.insert(word) {
            return;
        }
        log::info!("player learns the word {word}");
        self.increment_stat("Words Of Power Learned", 1);
        let shout = self.shout_of_word(word);
        let first = shout.is_some_and(|s| {
            self.shout(s).is_some_and(|sh| {
                sh.words
                    .iter()
                    .filter(|w| self.shouts.known.contains(&w.word))
                    .count()
                    == 1
            })
        });
        if first {
            self.increment_stat("Shouts Learned", 1);
        }
    }

    /// `Game.UnlockWord`: the word can be shouted (it is known too).
    pub fn unlock_word(&mut self, word: FormId) {
        self.teach_word(word);
        if !self.shouts.unlocked.insert(word) {
            return;
        }
        log::info!("player unlocks the word {word}");
        self.increment_stat("Words Of Power Unlocked", 1);
        let Some(shout) = self.shout_of_word(word).and_then(|s| self.shout(s)) else {
            return;
        };
        let unlocked = shout
            .words
            .iter()
            .filter(|w| self.shouts.unlocked.contains(&w.word))
            .count();
        if unlocked == 1 {
            self.increment_stat("Shouts Unlocked", 1);
        }
        if unlocked == shout.words.len() {
            self.increment_stat("Shouts Mastered", 1);
        }
    }

    pub fn is_word_unlocked(&self, word: FormId) -> bool {
        self.shouts.unlocked.contains(&word)
    }

    /// How many of a shout's words the player can speak: those unlocked in a
    /// row from the first.
    pub fn shout_level(&self, shout: &Shout) -> usize {
        shout
            .words
            .iter()
            .take_while(|w| self.shouts.unlocked.contains(&w.word))
            .count()
    }

    /// The next of a shout's words a dragon soul would unlock: known and
    /// not yet unlocked, with those before it unlocked.
    pub fn next_word_to_unlock(&self, shout: FormId) -> Option<FormId> {
        let s = self.shout(shout)?;
        let w = s.words.get(self.shout_level(&s))?;
        self.shouts.known.contains(&w.word).then_some(w.word)
    }

    /// The magic menu's unlock: a dragon soul spent on the shout's next
    /// known word. False (and "Dragon Souls are required...") without a soul
    /// or a word to unlock.
    pub fn spend_dragon_soul(&mut self, shout: FormId) -> bool {
        let Some(word) = self.next_word_to_unlock(shout) else {
            return false;
        };
        if self.actor_value(PLAYER_REF, av::DRAGON_SOULS) < 1.0 {
            let text = self
                .gmst_string("sFailSpendSoul")
                .unwrap_or_else(|| "Dragon Souls are required to unlock shouts.".into());
            self.scripts.notify(text);
            return false;
        }
        self.mod_actor_value(PLAYER_REF, av::DRAGON_SOULS, -1.0);
        self.unlock_word(word);
        true
    }

    /// The voice key went down: speaking starts if the voice has rested
    /// (else its meter flashes). A power readied is used instead.
    pub fn player_voice_press(&mut self) {
        if self.disabled_controls.fighting || self.player_dead() || self.conversation.is_some() {
            return;
        }
        let Some(shout) = self.equipped_shout(PLAYER_REF) else {
            self.player_use_power();
            return;
        };
        if self.shouts.recovery > 0.0 {
            self.shouts.meter_flashes += 1;
            return;
        }
        if self.shout(shout).is_none_or(|s| self.shout_level(&s) == 0) {
            return;
        }
        self.shouts.held = Some(0.0);
    }

    /// The voice key came up: the shout goes out with as many words as it
    /// was held for.
    pub fn player_voice_release(&mut self) {
        if let Some(held) = self.shouts.held.take() {
            self.player_shout(held);
        }
    }

    /// The words a shout held this long speaks, up to `level`.
    fn words_for(&self, held: f32, level: usize) -> usize {
        let t1 = gmst_f32(&self.lo, "fShoutTime1", 0.2);
        let t2 = gmst_f32(&self.lo, "fShoutTime2", 0.9);
        let words = if held < t1 {
            1
        } else if held < t2 {
            2
        } else {
            3
        };
        words.min(level)
    }

    /// Shout the readied shout, held for `held` seconds.
    pub fn player_shout(&mut self, held: f32) {
        let Some(shout) = self.equipped_shout(PLAYER_REF).and_then(|s| self.shout(s)) else {
            return;
        };
        let level = self.shout_level(&shout);
        if level == 0 || self.shouts.recovery > 0.0 {
            return;
        }
        let n = self.words_for(held, level);
        let word = shout.words[n - 1].clone();
        let mult = self
            .actor_value(PLAYER_REF, av::SHOUT_RECOVERY_MULT)
            .max(0.0);
        self.shouts.recovery = word.recovery * mult;
        self.shouts.recovery_total = self.shouts.recovery;
        let words: Vec<String> = shout.words[..n]
            .iter()
            .map(|w| self.word(w.word).dragon)
            .collect();
        log::info!(
            "player shouts {} ({}): {:.0}s to recover",
            shout.name,
            words.join(" "),
            self.shouts.recovery
        );
        self.increment_stat("Times Shouted", 1);
        self.cast(PLAYER_REF, word.spell, 1.0);
    }

    /// The voice resting; the key held long enough for every word unlocked
    /// lets the shout go.
    pub(crate) fn update_shouts(&mut self, dt: f32) {
        if self.shouts.recovery > 0.0 {
            self.shouts.recovery = (self.shouts.recovery - dt).max(0.0);
        }
        let Some(held) = self.shouts.held.as_mut() else {
            return;
        };
        *held += dt;
        let held = *held;
        let level = self
            .equipped_shout(PLAYER_REF)
            .and_then(|s| self.shout(s))
            .map_or(0, |s| self.shout_level(&s));
        let full = match level {
            0 | 1 => 0.0,
            2 => gmst_f32(&self.lo, "fShoutTime1", 0.2),
            _ => gmst_f32(&self.lo, "fShoutTime2", 0.9),
        };
        if held >= full {
            self.shouts.held = None;
            self.player_shout(held);
        }
    }

    /// The HUD's shout meter: 100 when the voice has rested.
    pub fn shout_meter_percent(&self) -> f32 {
        if self.shouts.recovery_total <= 0.0 || self.shouts.recovery <= 0.0 {
            100.0
        } else {
            (1.0 - self.shouts.recovery / self.shouts.recovery_total) * 100.0
        }
    }
}
