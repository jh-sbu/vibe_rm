//! The player's speech checks: what a bribe costs (`GetBribeAmount`,
//! `<BribeCost>` in dialogue prompts), whether an intimidation succeeds
//! (`WillIntimidateSucceed`, the `GetIntimidateSuccess` condition), the
//! bribed and intimidated flags `FavorDialogueScript` sets, and who the
//! player can talk to (`AllowPCDialogue`, `AllowBleedoutDialogue`).
//!
//! Formulas: UESP *Skyrim:Speech* (revision 3627051), "Bribe Formula" and
//! "Intimidate Formula"; *Skyrim talk:Speech* (revision 2848144),
//! "Intimidate formula". Settings the plugins lack have provisional
//! defaults (`known_ambiguities/speech-checks.md`).

use esp::FormId;
use esp::actor_value as av;

use crate::ai::combat::gmst_f32;
use crate::engine::{Engine, PLAYER_REF};
use crate::perks::ep;

const MORALITY: u32 = 3;
/// Race `DATA` flag "Allow PC Dialogue" (xEdit `wbRACE`).
const RACE_ALLOW_PC_DIALOGUE: u32 = 0x0020_0000;

impl Engine {
    /// `GetBribeAmount`: the gold `actor` takes as a bribe from the player.
    pub fn bribe_amount(&self, actor: FormId) -> i32 {
        let lo = &self.lo;
        let morality =
            self.actor_value(actor, MORALITY) * gmst_f32(lo, "fBribeMoralityMult", 1.0) + 1.0;
        let speech = self.actor_value(PLAYER_REF, av::SPEECHCRAFT)
            - self.actor_value(actor, av::SPEECHCRAFT);
        let base = self.actor_level(actor) as f32 * gmst_f32(lo, "fBribeNPCLevelMult", 1.0)
            + self.actor_level(PLAYER_REF) as f32
            - (speech - 100.0) * gmst_f32(lo, "fBribeSpeechcraftMult", 0.1);
        let cost = base.max(0.0).powf(gmst_f32(lo, "fBribeCostCurve", 1.4))
            * gmst_f32(lo, "fBribeScale", 0.5)
            * morality;
        let cost = self.perk_entry_point(ep::MOD_BRIBE_AMOUNT, PLAYER_REF, &[Some(actor)], cost);
        cost.round().max(0.0) as i32
    }

    /// The `GetBribeSuccess` condition: whether the player carries the bribe.
    pub fn bribe_succeeds(&self, actor: FormId) -> bool {
        self.item_count(PLAYER_REF, crate::crime::GOLD) >= self.bribe_amount(actor)
    }

    /// `WillIntimidateSucceed`: the player's "scariness" (level by the
    /// speech difference, through `Mod Player Intimidation`) against the
    /// NPC's (level by its confidence's multiplier).
    pub fn intimidate_succeeds(&self, actor: FormId) -> bool {
        let lo = &self.lo;
        let speech = (self.actor_value(PLAYER_REF, av::SPEECHCRAFT)
            - self.actor_value(actor, av::SPEECHCRAFT))
            / 100.0;
        let curve = gmst_f32(lo, "fIntimidateSpeechcraftCurve", 1.0);
        let player = self.actor_level(PLAYER_REF) as f32 * (1.0 + speech.max(-1.0)).powf(curve);
        let player = self.perk_entry_point(
            ep::MOD_PLAYER_INTIMIDATION,
            PLAYER_REF,
            &[Some(actor)],
            player,
        );
        let mult = match self.actor_value(actor, av::CONFIDENCE).clamp(0.0, 4.0) as u8 {
            0 => "fIntimidateConfidenceMultCowardly",
            1 => "fIntimidateConfidenceMultCautious",
            2 => "fIntimidateConfidenceMultAverage",
            3 => "fIntimidateConfidenceMultBrave",
            _ => "fIntimidateConfidenceMultFoolhardy",
        };
        let npc = self.actor_level(actor) as f32 * gmst_f32(lo, mult, 1.0);
        npc <= player
    }

    /// Whether the player can talk to `actor`: its race's "Allow PC
    /// Dialogue" unless `AllowPCDialogue` overrode it, and, bleeding out,
    /// only after `AllowBleedoutDialogue(true)`.
    pub fn can_talk_to(&self, actor: FormId) -> bool {
        let allowed = self
            .world_state
            .pc_dialogue
            .get(&actor)
            .copied()
            .unwrap_or_else(|| {
                self.race_of(actor)
                    .is_none_or(|race| self.race_flag(race, RACE_ALLOW_PC_DIALOGUE))
            });
        allowed
            && (!self.is_bleeding_out(actor) || self.world_state.bleedout_dialogue.contains(&actor))
    }

    /// Replace a dialogue prompt's tags for a conversation with `speaker`:
    /// `<BribeCost>`, `<CrimeGold>` (the bounty with the speaker's crime
    /// faction), and the quest's `<Alias=...>` / `<Global=...>` as in
    /// messages. Unknown tags stay as written.
    pub fn dialogue_text(&mut self, text: &str, speaker: FormId, quest: FormId) -> String {
        if !text.contains('<') {
            return text.to_owned();
        }
        let mut out = String::with_capacity(text.len());
        let mut rest = text;
        while let Some(i) = rest.find('<') {
            out.push_str(&rest[..i]);
            rest = &rest[i..];
            let Some(end) = rest.find('>') else { break };
            let tag = &rest[1..end];
            if tag.eq_ignore_ascii_case("bribecost") {
                out.push_str(&self.bribe_amount(speaker).to_string());
            } else if tag.eq_ignore_ascii_case("crimegold") {
                let gold = self
                    .crime_faction(speaker)
                    .map_or(0, |f| self.bounty(f).total());
                out.push_str(&gold.to_string());
            } else {
                let q = (!quest.is_null()).then_some(quest);
                out.push_str(&self.replace_text_tags(&rest[..=end], q));
            }
            rest = &rest[end + 1..];
        }
        out.push_str(rest);
        out
    }
}

#[cfg(test)]
#[path = "speech_checks_tests.rs"]
mod tests;
