//! The training menu as the game shows it (`trainingmenu.swf`): the skill,
//! its progress meter, the trainer's rank, the sessions this level, the
//! price and the player's gold.
//!
//! The menu has no functions for its contents: the game fills its card's
//! text fields and drives its meter (`Components.Meter`) itself. It answers
//! `Train` and `Exit` (its buttons, E and Tab). A session fills the meter;
//! the menu empties it once full (its `onEnterFrame`), and it fills again to
//! the skill's progress in its new level.

use ui_swf::{GameCall, Menu, UiSwf, Value};

use crate::engine::{Engine, PLAYER_REF};
use esp::FormId;

use super::{Buttons, num, text};

const M: &str = "_root.TrainingMenuObj";

/// Where the meter is in a session's fill.
#[derive(Clone, Copy, PartialEq)]
enum Fill {
    /// Still.
    Still,
    /// Filling to full, then emptied by the menu.
    Full,
    /// Filling to the skill's progress again.
    Progress,
}

pub(super) struct TrainingMenu {
    pub menu: Menu,
    pub trainer: FormId,
    fill: Fill,
    /// What the card last showed.
    shown: Vec<String>,
}

impl TrainingMenu {
    pub fn open(
        ui: &UiSwf,
        size: (u32, u32),
        engine: &Engine,
        trainer: FormId,
        buttons: std::sync::Arc<Buttons>,
    ) -> anyhow::Result<Self> {
        let menu = ui.open("trainingmenu.swf", size)?;
        super::set_button_mapping(&menu, &buttons);
        menu.set_responder(move |call, _| super::respond(&buttons, call));
        menu.invoke(&format!("{M}.SetPlatform"), &[num(0.0), Value::Bool(false)]);
        let mut m = TrainingMenu {
            menu,
            trainer,
            fill: Fill::Still,
            shown: Vec::new(),
        };
        m.update(engine);
        m.menu.invoke(
            &format!("{M}.SkillMeter.SetPercent"),
            &[num(m.progress(engine))],
        );
        Ok(m)
    }

    /// The skill's progress to its next level, in percent.
    fn progress(&self, engine: &Engine) -> f32 {
        let Some(t) = engine.trainer(self.trainer) else {
            return 0.0;
        };
        let i = (t.skill - esp::actor_value::FIRST_SKILL) as usize;
        let need = engine.skill_xp_needed(t.skill);
        if need <= 0.0 {
            return 0.0;
        }
        (engine.skills.xp[i] / need * 100.0).clamp(0.0, 100.0)
    }

    /// The card, when it changed, and the meter while it moves.
    pub fn update(&mut self, engine: &Engine) {
        if let Some(t) = engine.trainer(self.trainer) {
            let level = engine.av_base(PLAYER_REF, t.skill);
            let (had, allowed) = engine.training_sessions();
            let card = vec![
                engine.skill_name(t.skill).to_uppercase(),
                engine.trainer_rank(t),
                format!("{had}/{allowed}"),
                engine.training_cost(level).to_string(),
                engine.gold(PLAYER_REF).to_string(),
            ];
            if card != self.shown {
                let fields = [
                    "SkillName",
                    "TrainerSkill",
                    "TimesTrained",
                    "TrainCost",
                    "CurrentGold",
                ];
                for (field, value) in fields.iter().zip(&card) {
                    self.menu.set(&format!("{M}.{field}.text"), text(value));
                }
                self.shown = card;
            }
        }
        let meter = format!("{M}.SkillMeter");
        let number = |v: Value| match v {
            Value::Number(n) => n,
            _ => 0.0,
        };
        match self.fill {
            Fill::Still => return,
            // The menu empties it once it is full.
            Fill::Full => {
                let current = number(self.menu.get(&format!("{meter}.CurrentPercent")));
                let target = number(self.menu.get(&format!("{meter}.TargetPercent")));
                if current <= 0.0 && target <= 0.0 {
                    let p = self.progress(engine);
                    self.menu
                        .invoke(&format!("{meter}.SetTargetPercent"), &[num(p)]);
                    self.fill = Fill::Progress;
                }
            }
            Fill::Progress => {
                let current = number(self.menu.get(&format!("{meter}.CurrentPercent")));
                let target = number(self.menu.get(&format!("{meter}.TargetPercent")));
                if current >= target {
                    self.fill = Fill::Still;
                    return;
                }
            }
        }
        self.menu.invoke(&format!("{meter}.Update"), &[]);
    }

    /// The menu's calls this answers; false for those it leaves to the
    /// menus' shared ones.
    pub fn answer(&mut self, engine: &mut Engine, call: &GameCall) -> bool {
        match call.name.as_str() {
            "Train" => match engine.train(self.trainer) {
                Ok(()) => {
                    self.menu
                        .invoke(&format!("{M}.SkillMeter.SetTargetPercent"), &[num(100.0)]);
                    self.fill = Fill::Full;
                }
                Err(e) => engine.scripts.notify(e),
            },
            "Exit" => {
                if engine.menu == Some(crate::items::Menu::Training(self.trainer)) {
                    engine.menu = None;
                }
            }
            _ => return false,
        }
        true
    }

    /// Accept (E, Enter) trains, Cancel (Tab, Esc) leaves: as the menu's
    /// buttons.
    pub fn key(&self, code: winit::keyboard::KeyCode) -> bool {
        use winit::keyboard::KeyCode;
        let click = match code {
            KeyCode::KeyE | KeyCode::Enter | KeyCode::NumpadEnter => "OnAcceptClick",
            KeyCode::Tab | KeyCode::Escape => "OnExitClick",
            _ => return false,
        };
        self.menu
            .invoke(&format!("{M}.{click}"), &[Value::Undefined]);
        true
    }
}
