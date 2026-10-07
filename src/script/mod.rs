//! Papyrus integration: attaching scripts to forms, dispatching events and
//! implementing native functions against the engine.

pub mod natives;
pub mod types;
pub mod vmad;

use std::collections::{HashMap, HashSet};

use esp::FormId;
use papyrus::{ObjectId, Value};

use crate::engine::Engine;

#[derive(Debug, Clone)]
pub struct Timer {
    pub at: f64,
    /// Game-time timers fire in game hours rather than real seconds.
    pub game_time: bool,
    pub obj: ObjectId,
    pub script: String,
    pub event: &'static str,
    pub repeat: Option<f64>,
}

#[derive(Debug, Clone, Default)]
pub struct QuestState {
    pub running: bool,
    pub stage: u16,
    pub done: HashSet<u16>,
    pub objectives_displayed: HashSet<i32>,
    pub objectives_completed: HashSet<i32>,
    pub completed: bool,
    /// Reference alias fills: alias id -> reference.
    pub aliases: HashMap<u32, FormId>,
}

/// Script-visible world state and queues.
#[derive(Default)]
pub struct ScriptState {
    pub notifications: Vec<(String, f64)>,
    pub message_boxes: Vec<String>,
    pub timers: Vec<Timer>,
    pub quests: HashMap<FormId, QuestState>,
    pub globals: HashMap<FormId, f32>,
    pub disabled: HashMap<FormId, bool>,
    pub blocked_activation: HashSet<FormId>,
    /// Locks set by scripts and packages, and keys used, over the references' own
    /// (`XLOC`); lock levels set by scripts.
    pub locked: HashMap<FormId, bool>,
    pub lock_levels: HashMap<FormId, u8>,
    /// Actor values scripts changed, by actor and value index (`actor_values`), and
    /// by name the ones outside the known values.
    pub actor_values: HashMap<(FormId, u32), crate::actor_values::Modifiers>,
    pub other_actor_values: HashMap<(FormId, String), f32>,
    /// `AddInventoryEventFilter`: the items (or form lists) an object hears
    /// `OnItemAdded` / `OnItemRemoved` for; none set means all.
    pub inventory_filters: HashMap<ObjectId, HashSet<FormId>>,
    /// Events raised by natives, delivered after the current VM run.
    pub pending_events: Vec<(ObjectId, String, Vec<Value>)>,
    /// Quest fragments to run: (quest, stage).
    pub pending_stages: Vec<(FormId, u16)>,
    /// Objects that already received OnInit.
    pub initialized: HashSet<ObjectId>,
    pub real_time: f64,
    /// Bumped whenever a quest starts or stops or an alias fill changes (actors'
    /// alias packages follow).
    pub alias_gen: u64,
    /// Set when a native changed enable state; the engine re-syncs visibility.
    pub visibility_dirty: bool,
    pub warned: HashSet<String>,
}

impl ScriptState {
    pub fn notify(&mut self, s: impl Into<String>) {
        let s = s.into();
        log::info!("notification: {s}");
        self.notifications.push((s, self.real_time));
    }
}

/// The VM's view of the engine.
pub struct EngineHost<'a> {
    pub engine: &'a mut Engine,
}

impl papyrus::Host for EngineHost<'_> {
    fn load_script(&mut self, name: &str) -> Option<Vec<u8>> {
        self.engine.vfs.read(&format!("scripts/{name}.pex"))
    }

    fn call_native(&mut self, class: &str, func: &str, this: Option<&Value>, args: &[Value]) -> papyrus::NativeResult {
        natives::call(self.engine, class, func, this, args)
    }

    fn is_native_type(&self, obj: ObjectId, class: &str) -> bool {
        match obj {
            ObjectId::Form(id) => {
                let actual = self.engine.native_class(FormId(id));
                types::native_is_a(actual, class)
            }
            ObjectId::Alias { .. } => matches!(class, "alias" | "referencealias"),
            ObjectId::Effect(_) => class == "activemagiceffect",
        }
    }
}
