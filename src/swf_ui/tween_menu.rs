//! The tween menu as the game shows it (`tweenmenu.swf`): Tab's four ways
//! into the menus (Skills up, Magic left, Items right, Map down) with the
//! date, level and level meter at the bottom.
//!
//! The game opens it with `StartOpenMenuAnim(levelUp, date, level,
//! percent)`; the menu answers `HighlightMenu(n)` and
//! `OpenHighlightedMenu(n)` (1 Skills, 2 Magic, 3 Items, 4 Map), and on Tab
//! `StartCloseMenu`, then `CloseMenu` when it has folded away. The item and
//! magic menus come back to it with `ShowTweenMenu`. Choices:
//! `known_ambiguities/tween-menu.md`.

use ui_swf::{GameCall, Menu, UiSwf, Value};

use crate::engine::Engine;
use crate::items::Menu as Shown;

use super::{Buttons, num, text};

const M: &str = "_root.TweenMenu_mc";

pub(super) struct TweenMenu {
    pub menu: Menu,
    /// Closing: it folds away, then calls `CloseMenu`.
    pub closing: bool,
}

impl TweenMenu {
    pub fn open(
        ui: &UiSwf,
        size: (u32, u32),
        engine: &Engine,
        buttons: std::sync::Arc<Buttons>,
    ) -> anyhow::Result<Self> {
        let menu = ui.open("tweenmenu.swf", size)?;
        menu.set_responder(move |call, _| super::respond(&buttons, call));
        menu.invoke(&format!("{M}.InitExtensions"), &[]);
        let level = engine.skills.level + engine.skills.pending as u16;
        menu.callback(
            "StartOpenMenuAnim",
            &[
                Value::Bool(engine.skills.pending > 0),
                text(&engine.date_text()),
                num(level as f32),
                num(engine.level_progress() * 100.0),
            ],
        );
        Ok(TweenMenu {
            menu,
            closing: false,
        })
    }

    /// The menu's calls this answers; false for those it leaves to the
    /// menus' shared ones.
    pub fn answer(&mut self, engine: &mut Engine, call: &GameCall) -> bool {
        match call.name.as_str() {
            "OpenHighlightedMenu" => {
                let next = match call.args.first() {
                    Some(Value::Number(n)) => match *n as i32 {
                        1 => Some(Shown::Skills),
                        2 => Some(Shown::Magic),
                        3 => Some(Shown::Inventory),
                        // The map isn't built.
                        _ => None,
                    },
                    _ => None,
                };
                if let Some(next) = next
                    && !self.closing
                {
                    engine.menu = Some(next);
                }
            }
            "StartCloseMenu" => self.closing = true,
            "CloseMenu" => {
                if engine.menu == Some(Shown::Tween) {
                    engine.menu = None;
                }
            }
            "HighlightMenu" | "OpenAnimFinished" => {}
            _ => return false,
        }
        true
    }

    /// Esc: it folds away as on Tab.
    pub fn close(&mut self) {
        if self.closing {
            return;
        }
        self.closing = true;
        self.menu.invoke(&format!("{M}.StartCloseMenuAnim"), &[]);
    }
}
