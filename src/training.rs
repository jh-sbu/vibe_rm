//! Training: a trainer (`Game.ShowTrainingMenu`, from the trainers' dialogue)
//! raises one of the player's skills a level for gold, up to the level its
//! class teaches to, five times a player level (UESP *Skyrim:Trainers*).
//! The gold goes into the trainer's inventory. Choices:
//! `known_ambiguities/skills.md`.

use esp::FormId;
use esp::actor_value as av;

use crate::ai::combat::{gmst_f32, gmst_i32};
use crate::crime::GOLD;
use crate::engine::{Engine, PLAYER_REF};

/// Sessions allowed each player level (UESP: "up to five times per level";
/// the setting isn't in Skyrim.esm).
const SESSIONS_PER_LEVEL: u32 = 5;

/// Added to every session's price: UESP's formulas (`10b + 50` up to 50,
/// `30b + 50` past it, `50b + 50` past 75 for one session from skill `b`)
/// are this plus `fTrainingMultCost` x `b`, times `iTrainingExpertCost` or
/// `iTrainingMasterCost` past 50 and 75. Not in Skyrim.esm.
const BASE_COST: f32 = 50.0;

/// What a trainer teaches: the skill (an actor value) and the level it
/// teaches up to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Trainer {
    pub skill: u32,
    pub max: u8,
}

impl Engine {
    /// What `actor` trains, from its class (`CLAS` `DATA`: the skill taught
    /// at byte 4, from One-handed; the most it teaches at byte 5).
    pub fn trainer(&self, actor: FormId) -> Option<Trainer> {
        let t = self.templates_of(actor)?;
        let class = t.form(&self.lo, crate::world::template::STATS, b"CNAM")?;
        let d = self.lo.get(class)?.get(b"DATA")?.to_vec();
        let (skill, max) = (*d.get(4)? as u32, *d.get(5)?);
        let skill = av::FIRST_SKILL + skill;
        (skill <= av::LAST_SKILL && max > 0).then_some(Trainer { skill, max })
    }

    /// The price of a session from skill level `level`.
    pub fn training_cost(&self, level: f32) -> i32 {
        let tier = if level > 75.0 {
            gmst_i32(&self.lo, "iTrainingMasterCost", 5)
        } else if level > 50.0 {
            gmst_i32(&self.lo, "iTrainingExpertCost", 3)
        } else {
            1
        };
        (BASE_COST + gmst_f32(&self.lo, "fTrainingMultCost", 10.0) * tier as f32 * level).round()
            as i32
    }

    /// Sessions the player has had this level, and how many are allowed.
    pub fn training_sessions(&self) -> (u32, u32) {
        (self.skills.trained, SESSIONS_PER_LEVEL)
    }

    /// The trainer's rank as the menu names it: `sJourneyman` ("Common") up
    /// to 50, `sExpert` up to 75, `sMaster` past it.
    pub fn trainer_rank(&self, t: Trainer) -> String {
        let (setting, fallback) = match t.max {
            0..=50 => ("sJourneyman", "Common"),
            51..=75 => ("sExpert", "Expert"),
            _ => ("sMaster", "Master"),
        };
        self.gmst_string(setting).unwrap_or_else(|| fallback.into())
    }

    /// A session with `trainer`: the skill a level up, its price in gold to
    /// the trainer. Refused, in the game's words, when the skill is as high
    /// as the trainer teaches (`sCanNotTrainHigher`), the player has had
    /// this level's sessions (`sCanNotTrainAnymore`) or hasn't the gold
    /// (`sInsufficientGoldToTrain`).
    pub fn train(&mut self, trainer: FormId) -> Result<(), String> {
        let Some(t) = self.trainer(trainer) else {
            return Err(format!("{trainer} doesn't train"));
        };
        let level = self.av_base(PLAYER_REF, t.skill);
        let refuse = |e: &Engine, setting: &str, fallback: &str| {
            Err(e.gmst_string(setting).unwrap_or_else(|| fallback.into()))
        };
        if level >= t.max as f32 {
            return refuse(
                self,
                "sCanNotTrainHigher",
                "This NPC cannot help you train any higher.",
            );
        }
        if self.skills.trained >= SESSIONS_PER_LEVEL {
            return refuse(
                self,
                "sCanNotTrainAnymore",
                "You must level up to train more.",
            );
        }
        let cost = self.training_cost(level);
        if self.gold(PLAYER_REF) < cost {
            return refuse(
                self,
                "sInsufficientGoldToTrain",
                "You have insufficient gold to train.",
            );
        }
        self.remove_item(PLAYER_REF, GOLD, cost, Some(trainer));
        self.skills.trained += 1;
        self.increment_stat("Training Sessions", 1);
        log::info!(
            "{trainer} trains {} from {level} for {cost} gold ({} this level)",
            av::NAMES[t.skill as usize],
            self.skills.trained
        );
        self.increment_skill(t.skill);
        Ok(())
    }

    /// `Game.ShowTrainingMenu`: the training menu, for a trainer.
    pub fn show_training_menu(&mut self, trainer: FormId) {
        if self.trainer(trainer).is_none() {
            log::warn!("ShowTrainingMenu: {trainer} has no skill to train");
            return;
        }
        self.menu = Some(crate::items::Menu::Training(trainer));
    }
}
