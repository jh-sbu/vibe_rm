//! System-page data that must be supplied synchronously to live movie lists.
use super::{num, text};
use crate::{engine::Engine, save::SaveListing};
use std::sync::{Arc, Mutex};
use ui_swf::{GameCall, Movie, Value};
const S: &str = "_root.QuestJournalFader.Menu_mc.SystemFader.Page_mc";
pub const LIST: &str = "_root.QuestJournalFader.Menu_mc.SystemFader.Page_mc.SaveLoadListHolder.SaveLoadList_mc.entryList";
fn object(fields: impl IntoIterator<Item = (&'static str, Value)>) -> Value {
    Value::Object(fields.into_iter().map(|(k, v)| (k.into(), v)).collect())
}
pub fn index(call: &GameCall) -> Option<usize> {
    match call.args.first() {
        Some(Value::Number(n)) if *n >= 0. && n.is_finite() => Some(*n as usize),
        _ => None,
    }
}
#[derive(Clone, Debug, PartialEq)]
enum Filter {
    All,
    Character(u64),
    Legacy(String),
}
pub struct System {
    pub all: Vec<SaveListing>,
    pub visible: Vec<SaveListing>,
    current: u64,
    pub save_disabled: bool,
    pub settings: crate::settings::Settings,
    pub audio: Vec<Value>,
    pub input: crate::input_map::Shared,
    pub remapping: Option<String>,
    pub help: Vec<crate::system_help::Topic>,
    labels: (String, String),
    filter: Filter,
    pub callbacks: Vec<(String, Vec<Value>)>,
}
impl System {
    pub fn new(engine: &mut Engine) -> Arc<Mutex<Self>> {
        let help = engine.help_topics();
        Arc::new(Mutex::new(Self {
            all: engine.save_list(),
            visible: Vec::new(),
            current: engine.scripts.character_id,
            save_disabled: engine.saving_disabled() || engine.player_dead(),
            settings: engine.settings.clone(),
            audio: audio_rows(engine),
            input: engine.input_map.clone(),
            remapping: None,
            help,
            labels: (
                engine
                    .gmst_string("sMenuDisplayQuicksaveName")
                    .unwrap_or_else(|| "[Quicksave]".into()),
                engine
                    .gmst_string("sMenuDisplayAutosaveName")
                    .unwrap_or_else(|| "[Autosave]".into()),
            ),
            filter: Filter::All,
            callbacks: Vec::new(),
        }))
    }
    fn selected(&self, row: &SaveListing) -> bool {
        match &self.filter {
            Filter::All => true,
            Filter::Character(id) => row.metadata.as_ref().is_some_and(|m| m.character == *id),
            Filter::Legacy(_) => row.metadata.is_none(),
        }
    }
    pub fn respond(&mut self, call: &GameCall, movie: &mut Movie<'_, '_>) -> Option<Vec<Value>> {
        match call.name.as_str() {
            "SetVersionText" => {
                movie.set(
                    &format!("{S}.VersionText.text"),
                    text(concat!("VibeRM ", env!("CARGO_PKG_VERSION"))),
                );
            }
            "RequestIsOnPC" => {
                // The movie hard-codes desktop to list index 1 in both its
                // confirmation and dispatch. Keep index 0 as an inert blank.
                movie.set(
                    &format!("{S}.PCQuitList.entryList"),
                    Value::List(vec![
                        object([("text", text("")), ("disabled", Value::Bool(true))]),
                        object([("text", text("$Desktop")), ("disabled", Value::Bool(false))]),
                    ]),
                );
                movie.set(&format!("{S}.PCQuitList.selectedIndex"), num(1.));
                movie.invoke(&format!("{S}.PCQuitList.UpdateList"), &[]);
                return Some(vec![Value::Bool(true)]);
            }
            "PopulateHelpTopics" => {
                let rows = self
                    .help
                    .iter()
                    .enumerate()
                    .map(|(i, h)| object([("text", text(&h.title)), ("index", num(i as f32))]))
                    .collect();
                movie.set(&format!("{S}.HelpList.entryList"), Value::List(rows));
            }
            "RequestHelpText" => {
                if let Some(h) = index(call).and_then(|i| self.help.get(i)) {
                    movie.set(&format!("{S}.HelpTitleText.text"), text(&h.title));
                    movie.set(&format!("{S}.HelpText.text"), text(&h.body));
                }
            }
            "RequestInputMappings" => {
                movie.set(
                    MAPPINGS,
                    Value::List(mapping_rows(&self.input.read().unwrap())),
                );
            }
            "ResetControlsToDefaults" => {
                self.input.write().unwrap().reset();
            }
            "StartRemapMode" => {
                self.remapping = call.args.first().and_then(|v| {
                    if let Value::String(s) = v {
                        Some(s.clone())
                    } else {
                        None
                    }
                });
            }
            "RequestAudioOptions" => {
                movie.set(
                    &format!("{S}.OptionsListsPanel.OptionsLists.List_mc.entryList"),
                    Value::List(self.audio.clone()),
                );
            }
            "RequestGameplayOptions" | "RequestDisplayOptions" => {
                let list = format!("{S}.OptionsListsPanel.OptionsLists.List_mc.entryList");
                let count = match movie.get(&format!("{list}.length")) {
                    Value::Number(n) => n as usize,
                    _ => 0,
                };
                for i in 0..count {
                    let Value::String(label) = movie.get(&format!("{list}.{i}.text")) else {
                        continue;
                    };
                    if let Some(key) = crate::settings::option_key(&label) {
                        let id = crate::settings::SPECS
                            .iter()
                            .position(|s| s.key == key)
                            .unwrap();
                        for (name, value) in [
                            ("ID", id as f32),
                            (
                                "value",
                                crate::settings::option_value(&self.settings, key, false),
                            ),
                            (
                                "defaultVal",
                                crate::settings::option_value(
                                    &crate::settings::Settings::default(),
                                    key,
                                    true,
                                ),
                            ),
                        ] {
                            movie.set(&format!("{list}.{i}.{name}"), num(value));
                        }
                    }
                }
            }
            "SetSaveDisabled" => {
                for (i, disabled) in [
                    (0, self.save_disabled),
                    (1, self.save_disabled),
                    (2, false),
                    (3, true),
                    (5, false),
                    (7, false),
                ] {
                    movie.set(
                        &format!("{S}.CategoryList.entryList.{i}.disabled"),
                        Value::Bool(disabled),
                    );
                }
            }
            "ShouldShowKinectTunerOption" => return Some(vec![Value::Bool(false)]),
            "UseCurrentCharacterFilter" => self.filter = Filter::Character(self.current),
            "SAVE" | "LOAD" | "CharacterSelected" => {
                if call.name == "CharacterSelected" {
                    let id = index(call).unwrap_or(u32::MAX as usize) as u64;
                    self.filter = if id == u32::MAX as u64 {
                        Filter::All
                    } else {
                        self.characters()
                            .get(id as usize)
                            .map(|(f, _)| f.clone())
                            .unwrap_or(Filter::All)
                    };
                }
                self.visible = self
                    .all
                    .iter()
                    .filter(|s| self.selected(s))
                    .cloned()
                    .collect();
                let rows = self.visible.iter().map(|s| row(s, &self.labels)).collect();
                movie.set(LIST, Value::List(rows));
                let n = num(self.visible.len() as f32);
                self.callbacks.push((
                    "onSaveLoadBatchComplete".into(),
                    vec![Value::Bool(true), n.clone(), n],
                ));
            }
            "PopulateCharacterList" => {
                let mut rows = vec![object([
                    ("text", text("Show all saves")),
                    ("id", Value::Number(u32::MAX as f64)),
                    ("flags", num(0.)),
                ])];
                rows.extend(self.characters().iter().enumerate().map(|(i, (_, name))| {
                    object([
                        ("text", text(name)),
                        ("id", num(i as f32)),
                        ("flags", num(0.)),
                    ])
                }));
                movie.set(LIST, Value::List(rows));
                self.callbacks.push((
                    "onFillCharacterListComplete".into(),
                    vec![Value::Bool(true)],
                ));
            }
            "ForceStopSaveListLoading" => {}
            _ => return None,
        }
        Some(vec![])
    }
    fn characters(&self) -> Vec<(Filter, String)> {
        let mut characters = Vec::new();
        for row in &self.all {
            let pair = match &row.metadata {
                Some(m) => (Filter::Character(m.character), m.name.clone()),
                None => (Filter::Legacy("?".into()), "?".into()),
            };
            if !characters.iter().any(|(f, _)| f == &pair.0) {
                characters.push(pair);
            }
        }
        characters
    }
}
fn row(save: &SaveListing, labels: &(String, String)) -> Value {
    let m = save.metadata.as_ref();
    let seconds = m.map_or(0, |m| m.play_seconds);
    object([
        (
            "text",
            text(&m.map_or_else(
                || save.name.clone(),
                |m| {
                    if save.name.starts_with("quicksave") {
                        labels.0.clone()
                    } else if save.name.starts_with("autosave") {
                        labels.1.clone()
                    } else {
                        m.display_text(&save.name)
                    }
                },
            )),
        ),
        ("fileNum", num(save.file_num() as f32)),
        ("name", text(m.map_or("?", |m| m.name.as_str()))),
        ("raceName", text(m.map_or("?", |m| m.race.as_str()))),
        ("level", num(m.map_or(0, |m| m.level) as f32)),
        (
            "playTime",
            text(&format!(
                "{:02}:{:02}:{:02}",
                seconds / 3600,
                seconds / 60 % 60,
                seconds % 60
            )),
        ),
        (
            "dateString",
            text(&m.map_or_else(
                || "?".into(),
                |m| crate::save::metadata::date_string(m.written),
            )),
        ),
        ("corrupt", Value::Bool(save.corrupt)),
        ("obsolete", Value::Bool(save.obsolete)),
    ])
}
pub const AUDIO_ID: usize = 1000;
pub fn audio_rows(engine: &Engine) -> Vec<Value> {
    let mut categories: Vec<_> = engine.audio_preferences.menus.iter().collect();
    let rank = |name: &str| match name.to_ascii_lowercase().as_str() {
        "audiocategorysfx" => 0,
        "audiocategoryfst" => 1,
        "audiocategoryvocgeneral" => 2,
        "audiocategorymus" => 3,
        _ => 4,
    };
    categories.sort_by_key(|c| (rank(&c.name), c.id.0));
    let mut rows = vec![object([
        ("text", text("$Master")),
        ("ID", Value::Number(AUDIO_ID as f64)),
        (
            "value",
            num(engine.audio_preferences.master(&engine.audio_settings)),
        ),
        ("defaultVal", num(1.)),
        ("movieType", num(0.)),
    ])];
    rows.extend(categories.into_iter().map(|c| {
        let gmst = match c.name.to_ascii_lowercase().as_str() {
            "audiocategorysfx" => Some(("sEffectsVolume", "Effects")),
            "audiocategoryfst" => Some(("sFootstepsVolume", "Footsteps")),
            "audiocategoryvocgeneral" => Some(("sVoiceVolume", "Voice")),
            "audiocategorymus" => Some(("sMusicVolume", "Music")),
            _ => None,
        };
        let name = gmst
            .map(|(key, fallback)| engine.gmst_string(key).unwrap_or_else(|| fallback.into()))
            .unwrap_or_else(|| {
                let full = engine.form_name(c.id);
                if full.is_empty() {
                    c.name.clone()
                } else {
                    full
                }
            });
        object([
            ("text", text(&name)),
            ("ID", Value::Number((AUDIO_ID + 1) as f64 + c.id.0 as f64)),
            (
                "value",
                num(engine
                    .audio_preferences
                    .category(&engine.audio_settings, c.id)
                    .unwrap_or(c.default)),
            ),
            ("defaultVal", num(c.default)),
            ("movieType", num(0.)),
        ])
    }));
    rows
}
pub const MAPPINGS: &str =
    "_root.QuestJournalFader.Menu_mc.SystemFader.Page_mc.InputMappingPanel.List_mc.entryList";
pub fn mapping_rows(map: &crate::input_map::InputMap) -> Vec<Value> {
    map.events()
        .iter()
        .enumerate()
        .filter(|(_, e)| e.remappable[0] || e.remappable[1])
        .map(|(i, e)| {
            let key = map.first(&e.name, 0);
            let mouse = map.first(&e.name, 1);
            let art = key
                .and_then(super::keyboard_art)
                .or_else(|| mouse.and_then(super::mouse_art))
                .unwrap_or_else(|| "UnknownKey".into());
            object([
                ("text", text(&e.name)),
                ("buttonName", text(&art)),
                (
                    "buttonID",
                    Value::Number(key.or_else(|| mouse.map(|m| m + 256)).unwrap_or(255) as f64),
                ),
                ("sortIndex", num(i as f32)),
            ])
        })
        .collect()
}
