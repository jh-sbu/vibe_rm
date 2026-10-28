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
    filter: Filter,
    pub callbacks: Vec<(String, Vec<Value>)>,
}
impl System {
    pub fn new(engine: &Engine) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            all: engine.save_list(),
            visible: Vec::new(),
            current: engine.scripts.character_id,
            save_disabled: engine.saving_disabled() || engine.player_dead(),
            filter: Filter::All,
            callbacks: Vec::new(),
        }))
    }
    fn selected(&self, row: &SaveListing) -> bool {
        match &self.filter {
            Filter::All => true,
            Filter::Character(id) => row.metadata.as_ref().is_some_and(|m| m.character == *id),
            Filter::Legacy(name) => row.metadata.is_none() && &row.name == name,
        }
    }
    pub fn respond(&mut self, call: &GameCall, movie: &mut Movie<'_, '_>) -> Option<Vec<Value>> {
        match call.name.as_str() {
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
                let rows = self.visible.iter().map(row).collect();
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
                None => (Filter::Legacy(row.name.clone()), "?".into()),
            };
            if !characters.iter().any(|(f, _)| f == &pair.0) {
                characters.push(pair);
            }
        }
        characters
    }
}
fn row(save: &SaveListing) -> Value {
    let m = save.metadata.as_ref();
    let seconds = m.map_or(0, |m| m.play_seconds);
    object([
        (
            "text",
            text(&m.map_or_else(|| save.name.clone(), |m| m.display_text(&save.name))),
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
            text(&m.map_or_else(|| "?".into(), |m| format!("{} UTC", m.written))),
        ),
        ("corrupt", Value::Bool(save.corrupt)),
        ("obsolete", Value::Bool(save.obsolete)),
    ])
}
