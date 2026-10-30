//! The main menu as the game shows it (`startmenu.swf`) and the credits it
//! opens (`creditsmenu.swf`). The contract is in `research/ui/main-menu.md`.
//!
//! The game sets the menu up with `InitExtensions`, `SetPlatform(0, false)`
//! and the `sendMenuProperties` callback, whose PC values leave out the
//! Bethesda.net login, Creations, mods, add-ons and help. Load shares the
//! journal's save list (`journal_system`). Continue and New are two-step:
//! the menu's `CONTINUE` / `NEW` are answered with a confirmation, and its
//! fade then calls `ContinueLastSavedGame` / `StartNewGame`. A new game
//! isn't there yet: confirming New says so. The choices without a source are in
//! `known_ambiguities/main-menu.md`.

use std::sync::{Arc, Mutex};

use ui_swf::{GameCall, Menu, UiSwf, Value};

use crate::engine::Engine;

use super::{Buttons, journal_system::System, num, text};

/// The `StartMenu` clip.
const M: &str = "_root.MenuHolder.Menu_mc";
/// Its save list's entries (the panel is the journal's `SaveLoadPanel`).
const LIST: &str = "_root.MenuHolder.Menu_mc.SaveLoadPanel_mc.SaveLoadList_mc.entryList";

pub(super) struct MainMenu {
    pub menu: Menu,
    system: Arc<Mutex<System>>,
}

/// `sendMenuProperties`' arguments on PC (`research/ui/main-menu.md`): Quit,
/// whether there are saves (Continue, Load), the version, and no press-start
/// state, upsell, add-ons, help or mods, as SE shows; no Creations either
/// (`deviations/main-menu.md`); logged in, so no login.
fn properties(has_saves: bool) -> Vec<Value> {
    let mut args = vec![
        Value::Bool(true),
        Value::Bool(has_saves),
        text(env!("CARGO_PKG_VERSION")),
    ];
    args.extend((3..9).map(|_| Value::Bool(false)));
    args.push(Value::Bool(true));
    args.extend((10..15).map(|_| Value::Bool(false)));
    args
}

impl MainMenu {
    pub fn open(
        ui: &UiSwf,
        size: (u32, u32),
        engine: &mut Engine,
        buttons: Arc<Buttons>,
    ) -> anyhow::Result<Self> {
        let menu = ui.open("startmenu.swf", size)?;
        let system = System::with_list(engine, LIST);
        let answers = system.clone();
        super::set_button_mapping(&menu, &buttons);
        menu.set_responder(move |call, movie| {
            if let Some(answer) = answers.lock().unwrap().respond(call, movie) {
                return Some(answer);
            }
            super::respond(&buttons, call)
        });
        menu.invoke(&format!("{M}.InitExtensions"), &[]);
        menu.invoke(&format!("{M}.SetPlatform"), &[num(0.0), Value::Bool(false)]);
        let has_saves = !system.lock().unwrap().all.is_empty();
        menu.callback("sendMenuProperties", &properties(has_saves));
        Ok(MainMenu { menu, system })
    }

    /// The menu's calls that wait for the next update: the entries, the
    /// save list's loading calls and the fades.
    pub fn answer(&mut self, engine: &mut Engine, call: &GameCall) -> bool {
        if super::journal_system::answer_load(&self.system, &self.menu, engine, call) {
            return true;
        }
        match call.name.as_str() {
            "CONTINUE" => match engine.continue_target() {
                Some(_) => {
                    let text_ = engine
                        .gmst_string("sConfirmContinue")
                        .unwrap_or_else(|| "Continue from your last saved game?".into());
                    self.menu.callback("ConfirmContinue", &[text(&text_)]);
                }
                None => engine.debug_message_box("No save can be loaded.".into()),
            },
            "ContinueLastSavedGame" => {
                if let Some(save) = engine.continue_target() {
                    engine.pending_save_load = Some(save.name);
                    engine.menu = None;
                } else {
                    self.menu.callback("FadeInMenu", &[]);
                }
            }
            "NEW" => {
                let text_ = engine
                    .gmst_string("sConfirmNew")
                    .unwrap_or_else(|| "Start a new game?".into());
                self.menu.callback("ConfirmNewGame", &[text(&text_)]);
            }
            // Confirmed and faded out: there is no new game to start yet.
            "StartNewGame" => {
                engine.debug_message_box("Starting a new game isn't available yet.".into());
                self.menu.callback("FadeInMenu", &[]);
            }
            "OpenCreditsMenu" => engine.menu = Some(crate::items::Menu::Credits),
            "QuitToDesktop" => engine.quit_requested = true,
            // The last save deleted: Continue goes and Load is disabled.
            "DoDeleteSaveUISanityCheck" => {
                let saves = !engine.save_list().is_empty();
                self.system.lock().unwrap().all = engine.save_list();
                self.menu.callback(
                    "OnDeleteSaveUISanityCheck",
                    &[Value::Bool(saves), Value::Bool(saves)],
                );
            }
            // Notifications, and the entries left out on PC.
            "StartState"
            | "currentState"
            | "fadeOutStarted"
            | "myLog"
            | "FirstTimeOunceDataTransferCheck"
            | "OnDisabledLoadPress"
            | "HELP" => {}
            _ => return false,
        }
        true
    }

    /// Back from the credits: the menu fades in again.
    pub fn fade_in(&self) {
        self.menu.callback("FadeInMenu", &[]);
    }

    /// Delivers the save list's batch callbacks the responder queued.
    pub fn update(&mut self) {
        let callbacks = std::mem::take(&mut self.system.lock().unwrap().callbacks);
        for (name, args) in callbacks {
            self.menu.callback(&name, &args);
        }
    }
}

/// The credits: the text scrolls up and the menu asks to close at its end,
/// on Tab or Esc, or on a click (`CodeObj.closeMenu`).
pub(super) struct Credits {
    pub menu: Menu,
}

impl Credits {
    pub fn open(ui: &UiSwf, size: (u32, u32), engine: &mut Engine) -> anyhow::Result<Self> {
        let menu = ui.open("creditsmenu.swf", size)?;
        // The English credits, as HTML; the second field stays empty.
        let credits = engine
            .vfs
            .read("interface/credits.txt")
            .map(|b| credits_text(&String::from_utf8_lossy(&b)))
            .unwrap_or_default();
        let speed = crate::ai::combat::gmst_f32(&engine.lo, "fCreditsScrollSpeed", 2.0);
        menu.set_responder(move |call, movie| match call.name.as_str() {
            "_root.CodeObj.requestCredits" => {
                if let Some(Value::String(field)) = call.args.first() {
                    movie.invoke(
                        &format!("{field}.SetText"),
                        &[text(&credits), Value::Bool(true)],
                    );
                }
                Some(vec![])
            }
            "_root.CodeObj.getScrollSpeed" => Some(vec![num(speed)]),
            _ => None,
        });
        menu.install_host_object(
            "_root.CodeObj",
            &["requestCredits", "getScrollSpeed", "closeMenu"],
        );
        menu.invoke("_root.SetPlatform", &[num(0.0), Value::Bool(false)]);
        menu.invoke("_root.onCodeObjectInit", &[]);
        Ok(Credits { menu })
    }

    /// Whether the menu asked to close.
    pub fn answer(&mut self, call: &GameCall) -> bool {
        call.name == "_root.CodeObj.closeMenu"
    }
}

/// `credits.txt` for the HTML field. Its logos (`<img src='SkyrimLogo'>` and
/// two more, the movie's library symbols) are left out: Ruffle lays out the
/// lines after an inline image over one another.
fn credits_text(file: &str) -> String {
    file.replace("\r\n", "\n")
        .lines()
        .filter(|l| !l.trim_start().starts_with("<img"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn properties_leave_out_the_online_entries() {
        let args = properties(true);
        assert_eq!(args.len(), 15);
        assert_eq!(args[0], Value::Bool(true), "Quit");
        assert_eq!(args[1], Value::Bool(true), "Continue and Load");
        assert_eq!(args[9], Value::Bool(true), "logged in: no login screen");
        for i in [3, 4, 5, 6, 7, 8, 10, 11, 12, 13, 14] {
            assert_eq!(args[i], Value::Bool(false), "argument {i}");
        }
        assert_eq!(properties(false)[1], Value::Bool(false));
    }

    #[test]
    fn credits_text_drops_the_logos() {
        let text = credits_text(
            "<img src='SkyrimLogo'>\r\n\r\nTITLE\r\nName\r\n<img src='X'>\r\nEnd</font>",
        );
        assert_eq!(text, "\nTITLE\nName\nEnd</font>");
    }
}
