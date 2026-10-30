//! The loading menu as the game shows it (`loadingmenu.swf`, "Loading
//! Menu"): the hint text bottom right and the level meter top right, over
//! the loading screen's scene (`research/ui/loading-screen.md`).
//!
//! The game calls the menu's own `InitExtensions`, then `FadeInMenu` once
//! the screen is to come up and `FadeOutMenu` as it goes. On `FadeInMenu`
//! the menu asks `RequestPlayerInfo` (answered `SetLevelProgress(level,
//! percent, showMeter)`) and `RequestLoadingText` (`SetLoadingText(text)`);
//! its text fader asks for a new text every 15.5 s on its own. Both answers
//! are synchronous, from what the engine left in [`Shared`].

use std::sync::{Arc, Mutex};

use ui_swf::{GameCall, Menu, UiSwf, Value};

use crate::engine::Engine;
use crate::loading::Phase;

use super::{num, text};

const M: &str = "_root.Menu_mc";

/// What the menu's calls are answered with.
#[derive(Default)]
struct Shared {
    level: u16,
    /// Towards the next level, 0 to 100.
    percent: f32,
    /// Whether the meter shows (not from the main menu, with no player).
    meter: bool,
    /// The next hint, taken by the menu's next request.
    next_text: Option<String>,
}

pub(super) struct LoadingMenu {
    pub menu: Menu,
    shared: Arc<Mutex<Shared>>,
    faded_in: bool,
    faded_out: bool,
}

impl LoadingMenu {
    pub fn open(ui: &UiSwf, size: (u32, u32), engine: &mut Engine) -> anyhow::Result<Self> {
        let menu = ui.open("loadingmenu.swf", size)?;
        let shared = Arc::new(Mutex::new(Shared {
            level: engine.skills.level,
            percent: engine.level_progress() * 100.0,
            meter: engine.loading_shows_level(),
            next_text: engine.next_loading_text(),
        }));
        let answers = shared.clone();
        menu.set_responder(move |call, _| respond(&answers, call));
        menu.invoke(&format!("{M}.InitExtensions"), &[]);
        Ok(LoadingMenu {
            menu,
            shared,
            faded_in: false,
            faded_out: false,
        })
    }

    /// Follows the loading state: fades in as the screen comes up and out
    /// as it goes; keeps a next hint ready.
    pub fn update(&mut self, engine: &mut Engine) {
        let phase = engine.loading.as_ref().map(|l| l.phase);
        if !self.faded_in && phase != Some(Phase::FadeOut) {
            self.faded_in = true;
            self.menu.invoke(&format!("{M}.FadeInMenu"), &[]);
        }
        if !self.faded_out && phase == Some(Phase::ScreenOut) {
            self.faded_out = true;
            self.menu.invoke(&format!("{M}.FadeOutMenu"), &[]);
        }
        let mut shared = self.shared.lock().unwrap();
        if shared.next_text.is_none() {
            shared.next_text = engine.next_loading_text();
        }
    }

    /// The menu's calls this answers after the fact (none need it: its
    /// requests are answered as they are made).
    pub fn answer(&mut self, _engine: &mut Engine, call: &GameCall) -> bool {
        matches!(
            call.name.as_str(),
            "RequestPlayerInfo" | "RequestLoadingText"
        )
    }
}

fn respond(shared: &Mutex<Shared>, call: &GameCall) -> Option<Vec<Value>> {
    let mut shared = shared.lock().unwrap();
    match call.name.as_str() {
        "RequestPlayerInfo" => Some(vec![
            num(shared.level as f32),
            num(shared.percent),
            Value::Bool(shared.meter),
        ]),
        "RequestLoadingText" => Some(match shared.next_text.take() {
            Some(t) => vec![text(&t)],
            None => vec![Value::Undefined],
        }),
        _ => None,
    }
}
