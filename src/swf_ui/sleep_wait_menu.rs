//! The sleep / wait menu as the game shows it (`sleepwaitmenu.swf`): "Wait
//! how long?" or "Rest how long?", the date and time, and the hours slider.
//!
//! The game calls the menu's own functions (`SetPlatform`, `SetSleeping`,
//! `SetCurrentTime`: it registers no callbacks); the menu answers `OK(hours)`,
//! having disabled its controls, or `Cancel`. While the hours pass the menu
//! stays up, its clock following the time (`crate::sleep`).

use ui_swf::{GameCall, Menu, UiSwf, Value};

use crate::engine::Engine;

use super::{Buttons, num, text};

const M: &str = "_root.SleepWaitMenu_mc";

pub(super) struct SleepWaitMenu {
    pub menu: Menu,
    /// The time last shown.
    time: String,
}

impl SleepWaitMenu {
    pub fn open(
        ui: &UiSwf,
        size: (u32, u32),
        engine: &Engine,
        sleeping: bool,
        buttons: std::sync::Arc<Buttons>,
    ) -> anyhow::Result<Self> {
        let menu = ui.open("sleepwaitmenu.swf", size)?;
        menu.set_responder(move |call, _| super::respond(&buttons, call));
        menu.invoke(&format!("{M}.InitExtensions"), &[]);
        menu.invoke(&format!("{M}.SetPlatform"), &[num(0.0), Value::Bool(false)]);
        menu.invoke(&format!("{M}.SetSleeping"), &[Value::Bool(sleeping)]);
        // The date fits the field as the game's does (Ruffle's glyphs come
        // out a little wider).
        menu.set(&format!("{M}.CurrentTime.textAutoSize"), text("shrink"));
        let mut m = SleepWaitMenu {
            menu,
            time: String::new(),
        };
        m.update(engine);
        Ok(m)
    }

    /// The clock, as the hours pass.
    pub fn update(&mut self, engine: &Engine) {
        let time = engine.date_text();
        if time != self.time {
            self.menu
                .invoke(&format!("{M}.SetCurrentTime"), &[text(&time)]);
            self.time = time;
        }
    }

    /// The menu's calls this answers; false for those it leaves to the
    /// menus' shared ones.
    pub fn answer(&mut self, engine: &mut Engine, call: &GameCall) -> bool {
        match call.name.as_str() {
            "OK" => {
                let hours = match call.args.first() {
                    Some(Value::Number(n)) => n.max(0.0) as u32,
                    _ => 0,
                };
                engine.start_waiting(hours);
            }
            "Cancel" => {
                if engine.waiting.is_none()
                    && matches!(engine.menu, Some(crate::items::Menu::SleepWait { .. }))
                {
                    engine.menu = None;
                }
            }
            _ => return false,
        }
        true
    }

    /// Esc: as the menu's Cancel (not while the hours pass).
    pub fn close(&self) {
        self.menu.invoke(&format!("{M}.onCancelPress"), &[]);
    }
}
