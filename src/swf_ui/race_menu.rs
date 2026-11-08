//! Character creation's race menu as the game shows it (`racesex_menu.swf`,
//! `plans/character-creation.md` step 6).
//!
//! The game fills the movie through `GameDelegate` callbacks: the categories
//! (`SetCategoriesList`: label, filter bit), the races (`SetRaceList`: name,
//! description, current), the sliders (`SetOptionSliders`: label, filter,
//! callback, minimum, maximum, position, interval, id) and the name and race
//! shown. The movie answers `ChangeRace(index)` once the race list rests,
//! each slider's callback with its position and id, `ZoomPC` when the Head
//! category is entered or left, `ConfirmDone` from Done, and `ChangeName`
//! from the name popup. The sliders, their labels and the categories past
//! Race, Body and Head are provisional (`known_ambiguities/chargen.md`).

use std::time::Duration;

use ui_swf::{GameCall, Menu, UiSwf, Value};

use crate::chargen::{self, SliderKind};
use crate::engine::Engine;
use esp::FormId;

use super::{Buttons, num, text};

const R: &str = "_root.RaceSexMenuBaseInstance.RaceSexPanelsInstance";

/// The callback every slider names (the movie calls back whatever name it
/// was given).
const SLIDER_CALLBACK: &str = "ChangeSliderValue";

/// How long the menu takes to fade out after a name is given (its
/// `FadeOut` timeline; chosen).
const FADE_OUT: Duration = Duration::from_millis(600);

pub(super) struct RaceMenu {
    pub menu: Menu,
    pub limited: bool,
    races: Vec<FormId>,
    /// Counting down once named, then closed.
    closing: Option<Duration>,
}

impl RaceMenu {
    pub fn open(
        ui: &UiSwf,
        size: (u32, u32),
        engine: &mut Engine,
        limited: bool,
        buttons: std::sync::Arc<Buttons>,
    ) -> anyhow::Result<Self> {
        let menu = ui.open("racesex_menu.swf", size)?;
        super::set_button_mapping(&menu, &buttons);
        menu.set_responder(move |call, _| super::respond(&buttons, call));
        menu.invoke("_root.InitExtensions", &[]);
        menu.invoke("_root.SetPlatform", &[num(0.0), Value::Bool(false)]);
        if limited {
            menu.set(&format!("{R}.bLimitedMenu"), Value::Bool(true));
        }
        let mut m = RaceMenu {
            menu,
            limited,
            races: chargen::playable_races(&engine.lo),
            closing: None,
        };
        let categories: Vec<Value> = chargen::CATEGORIES
            .iter()
            .flat_map(|&(label, flag)| [text(label), num(flag as f32)])
            .collect();
        m.menu.callback("SetCategoriesList", &categories);
        let name = engine
            .actor_name(crate::engine::PLAYER_REF)
            .unwrap_or_default();
        m.menu.callback("SetNameText", &[text(&name)]);
        m.send_races(engine);
        m.send_sliders(engine);
        m.menu.callback("ShowTextEntry", &[Value::Bool(false)]);
        Ok(m)
    }

    fn current_race(engine: &Engine) -> FormId {
        engine
            .player_appearance
            .as_ref()
            .map_or(FormId::NULL, |a| a.face.race)
    }

    /// The races, the player's marked, and its name below.
    fn send_races(&mut self, engine: &Engine) {
        let current = Self::current_race(engine);
        let mut args = Vec::new();
        for &r in &self.races {
            args.push(text(&chargen::race_name(&engine.lo, r)));
            args.push(text(&chargen::race_description(&engine.lo, r)));
            args.push(num(if r == current { 1.0 } else { 0.0 }));
        }
        self.menu.callback("SetRaceList", &args);
        self.menu.callback(
            "SetRaceText",
            &[text(&chargen::race_name(&engine.lo, current))],
        );
    }

    /// The sliders for the appearance as it is.
    fn send_sliders(&mut self, engine: &Engine) {
        let Some(a) = engine.player_appearance.as_ref() else {
            return;
        };
        let tri = engine.player_face_tri();
        let mut args = Vec::new();
        for s in chargen::sliders(&engine.lo, a, tri.as_ref()) {
            args.extend([
                text(&s.label),
                num(s.filter as f32),
                text(SLIDER_CALLBACK),
                num(s.min),
                num(s.max),
                num(s.position),
                num(s.interval),
                num(s.kind.id() as f32),
            ]);
        }
        self.menu.callback("SetOptionSliders", &args);
    }

    /// The menu's calls this answers; false for those it leaves to the
    /// menus' shared ones.
    pub fn answer(&mut self, engine: &mut Engine, call: &GameCall) -> bool {
        let number = |i: usize| match call.args.get(i) {
            Some(Value::Number(n)) => Some(*n as f32),
            _ => None,
        };
        match call.name.as_str() {
            "ChangeRace" => {
                let race = number(0).and_then(|i| self.races.get(i.max(0.0) as usize).copied());
                if let Some(race) = race
                    && race != Self::current_race(engine)
                    && let Some(a) = engine.player_appearance.clone()
                {
                    let mut next = chargen::race_start(&engine.lo, race, a.face.female, 0);
                    next.face.weight = a.face.weight;
                    engine.set_player_appearance(next);
                    self.send_sliders(engine);
                    self.menu.callback(
                        "SetRaceText",
                        &[text(&chargen::race_name(&engine.lo, race))],
                    );
                }
                self.menu.callback("HideLoadingIcon", &[]);
            }
            SLIDER_CALLBACK => {
                let (Some(value), Some(kind)) = (
                    number(0),
                    number(1).and_then(|id| SliderKind::from_id(id as u32)),
                ) else {
                    return true;
                };
                let Some(mut a) = engine.player_appearance.clone() else {
                    return true;
                };
                if chargen::apply_slider(&engine.lo, &mut a, kind, value) {
                    engine.set_player_appearance(a);
                    // A new sex or preset changes what the others offer.
                    if matches!(kind, SliderKind::Sex | SliderKind::Preset) {
                        self.send_sliders(engine);
                    }
                }
            }
            "ZoomPC" => engine.race_menu_zoom(matches!(call.args.first(), Some(Value::Bool(true)))),
            "ConfirmDone" => self.ask_name(),
            "ChangeName" => match call.args.first() {
                Some(Value::String(name)) if !name.trim().is_empty() && self.closing.is_none() => {
                    let name = name.trim().to_owned();
                    log::info!("race menu: named {name:?}");
                    self.menu.callback("SetNameText", &[text(&name)]);
                    engine.player_name = Some(name);
                    self.menu.callback("FadeOut", &[]);
                    self.closing = Some(FADE_OUT);
                }
                // Cancelled, or the popup gone after a name.
                _ => {}
            },
            "SetAllowTextInput" | "ShowVirtualKeyboard" => {}
            _ => return false,
        }
        true
    }

    /// The name popup, as Done brings it up.
    fn ask_name(&self) {
        self.menu.callback("ShowTextEntry", &[Value::Bool(true)]);
        self.menu.callback("ShowTextEntryField", &[]);
    }

    /// Close once faded out.
    pub fn update(&mut self, engine: &mut Engine, dt: Duration) {
        if let Some(left) = self.closing {
            if left <= dt {
                self.closing = None;
                engine.close_race_menu();
            } else {
                self.closing = Some(left - dt);
            }
        }
    }

    /// The game's keys when the name popup isn't up: Done (`R`, as the
    /// movie's own button art shows it) and the categories (A and D,
    /// provisional; the movie asks the game for `moveCategoriesUp`).
    pub fn key(&self, code: winit::keyboard::KeyCode) -> bool {
        use winit::keyboard::KeyCode;
        if self.menu.typing() || self.closing.is_some() {
            return false;
        }
        match code {
            KeyCode::KeyR => self.ask_name(),
            KeyCode::KeyD => {
                self.menu.callback("moveCategoriesUp", &[]);
            }
            KeyCode::KeyA => {
                self.menu.invoke(&format!("{R}.moveCategoriesDown"), &[]);
            }
            _ => return false,
        }
        true
    }
}
