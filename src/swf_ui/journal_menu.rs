//! The journal as the game shows it (`quest_journal.swf`): the Quests page
//! (the quests, a quest's latest log entry and its objectives, toggling
//! which are tracked), the Stats page and the bottom bar's date and level.
//! The System page connects native saves, preferences, remapping and the
//! PC help manual through `journal_system`.
//!
//! The game opens it with `InitExtensions`, `SetPlatform` and the
//! `RestoreSavedSettings` callback (the tab to show). The pages then ask:
//! `RequestQuestsData` hands over the quest list, which the game fills
//! before answering (saved quest, misc quests listed, misc tracked, show on
//! map allowed); `RequestObjectivesData` wants the miscellaneous
//! objectives in the selected entry; `updateStats` and `RequestPlayerInfo`
//! are answered with flat lists. Closing (Tab in the menu, or the game's
//! `StartCloseMenu`) comes back as `CloseMenu(tab, quest, instance)`, kept
//! for next time. What is guessed is in `known_ambiguities/journal.md`.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};

use esp::FormId;
use ui_swf::{GameCall, Menu, UiSwf, Value};

use crate::engine::Engine;
use crate::journal::{ObjectiveState, QUEST_TYPE_MISC};
use crate::stats::StatValue;

use super::{Buttons, num, text};

/// The menu's root clip (`Quest_Journal`) and the Quests page's title list.
const M: &str = "_root.QuestJournalFader.Menu_mc";
const TITLES: &str = "_root.QuestJournalFader.Menu_mc.QuestsFader.Page_mc.TitleList";

/// Where the journal was left (`CloseMenu`'s arguments), to open there again.
#[derive(Debug, Clone)]
pub(super) struct Saved {
    tab: Value,
    quest: Value,
    instance: Value,
}

impl Default for Saved {
    fn default() -> Self {
        Saved {
            tab: num(0.0),
            quest: Value::Undefined,
            instance: Value::Undefined,
        }
    }
}

pub(super) struct JournalMenu {
    pub menu: Menu,
    /// Toggle Active in the menu: (quest, now tracked), for the engine with
    /// the next update. `FormId::NULL` is the miscellaneous objectives'.
    toggles: Arc<Mutex<Vec<(FormId, bool)>>>,
    system: Arc<Mutex<super::journal_system::System>>,
    captured: Option<crate::input_map::Button>,
    controls_generation: u64,
    buttons: Arc<Buttons>,
}

fn object(fields: impl IntoIterator<Item = (&'static str, Value)>) -> Value {
    Value::Object(
        fields
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect::<BTreeMap<_, _>>(),
    )
}

fn form(id: FormId) -> Value {
    Value::Number(id.0 as f64)
}

/// What the menu asks for, ready before it asks.
struct Data {
    /// The quests' title list entries (the misc ones aside), latest first.
    quests: Vec<Value>,
    /// The miscellaneous objectives, for `RequestObjectivesData`.
    misc: Vec<Value>,
    misc_hidden: bool,
    /// Each listed quest's tracking, as the menu toggles it.
    active: HashMap<u32, bool>,
    stats: Vec<Value>,
    player: Vec<Value>,
}

impl Data {
    fn of(engine: &mut Engine) -> Data {
        let mut quests = Vec::new();
        let mut misc = Vec::new();
        let mut active = HashMap::new();
        for j in engine.journal() {
            active.insert(j.id.0, j.active);
            let name = engine.replace_text_tags(&j.name, Some(j.id));
            if j.kind == QUEST_TYPE_MISC {
                // The running ones' objectives still to do, under one entry.
                if j.completed || j.failed {
                    continue;
                }
                for (_, text_, _) in j
                    .objectives
                    .iter()
                    .rev()
                    .filter(|o| o.2 == ObjectiveState::Shown)
                {
                    misc.push(object([
                        ("text", text(text_)),
                        ("formID", form(j.id)),
                        ("instance", num(0.0)),
                        ("active", Value::Bool(j.active)),
                        ("completed", Value::Bool(false)),
                        ("failed", Value::Bool(false)),
                    ]));
                }
                continue;
            }
            let done = j.completed || j.failed;
            // The latest objectives first.
            let objectives: Vec<Value> = j
                .objectives
                .iter()
                .rev()
                .map(|(_, text_, state)| {
                    let shown = *state == ObjectiveState::Shown;
                    object([
                        ("text", text(text_)),
                        ("active", Value::Bool(shown && j.active && !done)),
                        (
                            "completed",
                            Value::Bool(*state == ObjectiveState::Completed),
                        ),
                        ("failed", Value::Bool(*state == ObjectiveState::Failed)),
                    ])
                })
                .collect();
            quests.push(object([
                ("text", text(&name)),
                ("formID", form(j.id)),
                ("instance", num(0.0)),
                ("type", num(j.kind as f32)),
                ("active", Value::Bool(j.active && !done)),
                ("completed", Value::Bool(done)),
                // The latest log entry.
                ("description", text(j.log.last().map_or("", |s| s.as_str()))),
                ("objectives", Value::List(objectives)),
            ]));
        }
        // (name, value, category, unused) each.
        let mut stats = Vec::new();
        for &(name, category) in crate::stats::STATS {
            stats.push(text(name));
            stats.push(match engine.stat(name) {
                StatValue::Count(n) => num(n as f32),
                StatValue::Name(s) => text(&s),
            });
            stats.push(num(category as f32));
            stats.push(num(0.0));
        }
        let level = engine.skills.level + engine.skills.pending as u16;
        let player = vec![
            text(&engine.date_text()),
            num(level as f32),
            num(engine.level_progress() * 100.0),
        ];
        Data {
            quests,
            misc,
            misc_hidden: engine.scripts.misc_objectives_hidden,
            active,
            stats,
            player,
        }
    }
}

impl JournalMenu {
    /// Opens the journal on the tab and quest it was left at.
    pub fn open(
        ui: &UiSwf,
        size: (u32, u32),
        engine: &mut Engine,
        saved: &Saved,
        buttons: Arc<Buttons>,
    ) -> anyhow::Result<Self> {
        engine.capture_save_thumbnail();
        let menu = ui.open("quest_journal.swf", size)?;
        let mut data = Data::of(engine);
        let toggles: Arc<Mutex<Vec<(FormId, bool)>>> = Default::default();
        let mut saved_quest = saved.clone();
        // Nothing saved yet: the latest quest still running, else the
        // miscellaneous ones (formID 0). The completed quests' divider has
        // no formID, so undefined would pick it.
        if saved_quest.quest == Value::Undefined {
            saved_quest.quest = data
                .quests
                .iter()
                .find_map(|q| match q {
                    Value::Object(q) if q.get("completed") == Some(&Value::Bool(false)) => {
                        q.get("formID").cloned()
                    }
                    _ => None,
                })
                .unwrap_or(num(0.0));
            saved_quest.instance = num(0.0);
        }
        let system = super::journal_system::System::new(engine);
        let system_data = system.clone();
        let toggled = toggles.clone();
        let live_buttons = buttons.clone();
        super::set_button_mapping(&menu, &buttons);
        menu.set_responder(move |call, movie| {
            if let Some(answer) = system_data.lock().unwrap().respond(call, movie) {
                return Some(answer);
            }
            match call.name.as_str() {
                // The list is the menu's: the quests go into it, then the answer.
                "RequestQuestsData" => {
                    if !data.quests.is_empty() {
                        movie.invoke(&format!("{TITLES}.entryList.push"), &data.quests);
                    }
                    Some(vec![
                        saved_quest.quest.clone(),
                        saved_quest.instance.clone(),
                        Value::Bool(!data.misc.is_empty()),
                        Value::Bool(!data.misc_hidden),
                        // No map to show it on yet.
                        Value::Bool(false),
                    ])
                }
                // The selected entry is the miscellaneous one: its objectives.
                "RequestObjectivesData" => {
                    movie.set(
                        &format!("{TITLES}.selectedEntry.objectives"),
                        Value::List(data.misc.clone()),
                    );
                    Some(vec![])
                }
                "ToggleQuestActiveStatus" => {
                    let Some(Value::Number(id)) = call.args.first() else {
                        return Some(vec![Value::Bool(false)]);
                    };
                    let id = *id as u32;
                    let now = !data.active.get(&id).copied().unwrap_or(false);
                    data.active.insert(id, now);
                    // The misc objectives' copies of it too, for the next time
                    // the entry is shown.
                    for o in &mut data.misc {
                        if let Value::Object(f) = o
                            && f.get("formID") == Some(&Value::Number(id as f64))
                        {
                            f.insert("active".into(), Value::Bool(now));
                        }
                    }
                    toggled.lock().unwrap().push((FormId(id), now));
                    Some(vec![Value::Bool(now)])
                }
                "ToggleShowMiscObjectives" => {
                    let shown = matches!(call.args.first(), Some(Value::Bool(true)));
                    data.misc_hidden = !shown;
                    toggled.lock().unwrap().push((FormId::NULL, shown));
                    Some(vec![])
                }
                "updateStats" => Some(data.stats.clone()),
                "RequestPlayerInfo" => Some(data.player.clone()),
                // No Creations entry on the System page.
                "ShouldShowMod" => Some(vec![Value::Bool(false)]),
                // Quit lists Quit to Main Menu and Quit to Desktop.
                "RequestIsOnPC" => Some(vec![Value::Bool(true)]),
                _ => super::respond(&buttons, call),
            }
        });
        menu.invoke(&format!("{M}.InitExtensions"), &[]);
        menu.invoke(&format!("{M}.SetPlatform"), &[num(0.0), Value::Bool(false)]);
        // The journal key opens the quests; Esc the tab it was left at.
        let tab = if std::mem::take(&mut engine.journal_last_tab) {
            saved.tab.clone()
        } else {
            num(0.0)
        };
        menu.callback("RestoreSavedSettings", &[tab, Value::Bool(false)]);
        Ok(JournalMenu {
            menu,
            toggles,
            system,
            captured: None,
            controls_generation: engine.input_map.read().unwrap().generation,
            buttons: live_buttons,
        })
    }

    /// The tracking toggled in the menu, to the engine.
    pub fn update(&mut self, engine: &mut Engine) {
        let generation = engine.input_map.read().unwrap().generation;
        if generation != self.controls_generation {
            super::set_button_mapping(&self.menu, &self.buttons);
            self.controls_generation = generation;
        }

        self.system.lock().unwrap().save_disabled =
            engine.saving_disabled() || engine.player_dead();
        let callbacks = std::mem::take(&mut self.system.lock().unwrap().callbacks);
        for (name, args) in callbacks {
            self.menu.callback(&name, &args);
        }

        for (q, active) in std::mem::take(&mut *self.toggles.lock().unwrap()) {
            if q == FormId::NULL {
                engine.scripts.misc_objectives_hidden = !active;
            } else {
                engine.set_quest_active(q, active);
            }
        }
    }

    /// The menu's calls this answers; false for those it leaves to the
    /// menus' shared ones.
    pub fn answer(&mut self, engine: &mut Engine, call: &GameCall, saved: &mut Saved) -> bool {
        match call.name.as_str() {
            "OptionChange" => {
                if let (Some(id), Some(Value::Number(value))) =
                    (super::journal_system::index(call), call.args.get(1))
                {
                    let result = if id >= super::journal_system::AUDIO_ID {
                        engine.set_audio_volume(
                            (id > super::journal_system::AUDIO_ID)
                                .then(|| FormId((id - super::journal_system::AUDIO_ID - 1) as u32)),
                            *value as f32,
                        )
                    } else {
                        engine.change_option(id, *value as f32)
                    };
                    if let Err(e) = result {
                        log::warn!("option: {e}");
                    }
                    self.system.lock().unwrap().settings = engine.settings.clone();
                    self.system.lock().unwrap().audio = super::journal_system::audio_rows(engine);
                }
            }
            "SaveControls" => {
                let result = engine.input_map.read().unwrap().save();
                self.menu.callback("SettingsSaved", &[]);
                if let Err(error) = result {
                    engine.debug_message_box(format!("Could not save controls: {error}"));
                }
            }
            "SaveSettings" => {
                let result = engine.save_settings();
                self.menu.callback("SettingsSaved", &[]);
                if let Err(error) = result {
                    engine.debug_message_box(format!("Could not save settings: {error}"));
                }
            }
            "DeleteSave" => {
                if let Some(i) = super::journal_system::index(call) {
                    let listing = self.system.lock().unwrap().visible.get(i).cloned();
                    if let Some(save) = listing {
                        match engine.delete_save(&save.name) {
                            Ok(()) => {
                                let mut system = self.system.lock().unwrap();
                                system.visible.remove(i);
                                system.all.retain(|s| s.name != save.name);
                            }
                            Err(e) => {
                                engine.debug_message_box(format!("Could not delete save: {e:#}"))
                            }
                        }
                    }
                }
            }
            "IsOKtoLoad" => {
                let listing = super::journal_system::index(call)
                    .and_then(|i| self.system.lock().unwrap().visible.get(i).cloned());
                match listing {
                    Some(s) if !s.corrupt && !s.obsolete => {
                        self.menu.callback("ConfirmOKToLoad", &[]);
                    }
                    Some(s) => engine.debug_message_box(if s.corrupt {
                        engine
                            .gmst_string("sSaveGameCorrupt")
                            .unwrap_or_else(|| "This save is corrupt.".into())
                    } else {
                        engine
                            .gmst_string("sLoadingObsoleteSave")
                            .unwrap_or_else(|| {
                                "This save was made with a different plugin list.".into()
                            })
                    }),
                    None => engine.debug_message_box("This save is no longer available.".into()),
                }
            }
            "LoadGame" => {
                if let Some(save) = super::journal_system::index(call)
                    .and_then(|i| self.system.lock().unwrap().visible.get(i).cloned())
                    .filter(|s| !s.corrupt && !s.obsolete)
                {
                    engine.pending_save_load = Some(save.name);
                    engine.menu = None;
                }
            }
            "SaveGame" => {
                let index = super::journal_system::index(call).unwrap_or(0);
                let name = if index == 0 {
                    engine.new_save_name()
                } else {
                    self.system
                        .lock()
                        .unwrap()
                        .visible
                        .get(index - 1)
                        .map(|s| s.name.clone())
                        .ok_or_else(|| anyhow::anyhow!("save slot no longer exists"))
                };
                match name.and_then(|n| engine.save_game(&n)) {
                    Ok(_) => self.system.lock().unwrap().all = engine.save_list(),
                    Err(e) => engine.debug_message_box(format!("Could not save: {e:#}")),
                }
            }
            "QuickSave" => {
                engine.request_save(false);
                engine.flush_save_requests();
                self.system.lock().unwrap().all = engine.save_list();
            }
            "PrepSaveGameScreenshot" => {
                let index = match call.args.first() {
                    Some(Value::Number(n)) => *n as i32,
                    _ => -1,
                };
                let thumbnail = if index < 0 {
                    engine.save_thumbnail.clone()
                } else {
                    self.system
                        .lock()
                        .unwrap()
                        .visible
                        .get(index as usize)
                        .and_then(|s| s.metadata.as_ref())
                        .and_then(|m| m.thumbnail.clone())
                };
                // A blank fallback clears an image from the previous highlighted save.
                let fallback = crate::save::metadata::Thumbnail {
                    width: 1,
                    height: 1,
                    rgba: vec![0, 0, 0, 255],
                };
                let image = thumbnail.as_ref().unwrap_or(&fallback);
                self.menu.set_host_image(
                    "BGSSaveLoadHeader_Screenshot",
                    image.width,
                    image.height,
                    &image.rgba,
                );
                self.menu.callback("ScreenshotReady", &[]);
            }
            "CloseMenu" => {
                let arg = |i: usize| call.args.get(i).cloned().unwrap_or(Value::Undefined);
                *saved = Saved {
                    tab: arg(0),
                    quest: arg(1),
                    instance: arg(2),
                };
                self.update(engine);
                if let Err(error) = engine.save_settings() {
                    log::warn!("settings: {error}");
                }
                if engine.input_map.read().unwrap().generation > 0 {
                    if let Err(error) = engine.input_map.read().unwrap().save() {
                        log::warn!("controls: {error}");
                    }
                }
                if engine.menu == Some(crate::items::Menu::Journal) {
                    engine.menu = None;
                }
            }
            // The tab shown, kept with `CloseMenu`; showing a target on the
            // map (not offered) and the System page's saving, loading and
            // settings, which aren't built. There is no main menu to quit to.
            "SetVersionText" => self.menu.set(
                "_root.QuestJournalFader.Menu_mc.SystemFader.Page_mc.VersionText.text",
                text(concat!("VibeRM ", env!("CARGO_PKG_VERSION"))),
            ),
            "RememberCurrentTabIndex" | "ShowTargetOnMap" => {}
            "QuitToMainMenu" => log::warn!("QuitToMainMenu requires startmenu.swf support"),
            "QuitToDesktop" => engine.quit_requested = true,
            _ => return false,
        }
        true
    }

    /// Capture before ordinary menu or developer-key handling, including
    /// the matching key-up so it cannot activate a newly rebound menu row.
    pub fn capture(&mut self, event: &winit::event::WindowEvent) -> bool {
        use winit::{
            event::{ElementState, WindowEvent},
            keyboard::PhysicalKey,
        };
        let (button, pressed) = match event {
            WindowEvent::KeyboardInput { event, .. }
                if let PhysicalKey::Code(code) = event.physical_key =>
            {
                (
                    crate::input_map::Button::Key(code),
                    event.state == ElementState::Pressed,
                )
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let Some(code) = crate::input_map::mouse_code(*button) else {
                    return false;
                };
                (
                    crate::input_map::Button::Mouse(code),
                    *state == ElementState::Pressed,
                )
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let n = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => *y,
                    winit::event::MouseScrollDelta::PixelDelta(p) => p.y as f32,
                };
                (
                    crate::input_map::Button::Mouse(if n > 0. { 8 } else { 9 }),
                    true,
                )
            }
            _ => return false,
        };
        self.capture_button(
            button,
            pressed,
            matches!(event, WindowEvent::MouseWheel { .. }),
        )
    }
    pub fn capture_button(
        &mut self,
        button: crate::input_map::Button,
        pressed: bool,
        wheel: bool,
    ) -> bool {
        if self.captured == Some(button) {
            if !pressed {
                self.captured = None;
            }
            return true;
        }
        let pending = self.system.lock().unwrap().remapping.clone();
        let Some(name) = pending else {
            return false;
        };
        if !pressed {
            return true;
        }
        let input = self.system.lock().unwrap().input.clone();
        let success = input.write().unwrap().remap(&name, button);
        self.system.lock().unwrap().remapping = None;
        self.captured = if wheel { None } else { Some(button) };
        self.menu.set(
            super::journal_system::MAPPINGS,
            Value::List(super::journal_system::mapping_rows(&input.read().unwrap())),
        );
        self.menu
            .callback("FinishRemapMode", &[Value::Bool(success)]);
        true
    }

    /// Esc or the journal key: the menu closes as on Tab (`CloseMenu`).
    pub fn close(&self) {
        self.menu.callback("StartCloseMenu", &[]);
    }
}
