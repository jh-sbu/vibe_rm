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
    pub objectives_failed: HashSet<i32>,
    /// Displayed objectives' text, with the aliases as they were then.
    pub objective_texts: HashMap<i32, String>,
    pub completed: bool,
    pub failed: bool,
    /// Log entries added by its stages, oldest first.
    pub log: Vec<String>,
    /// Its quest added banner was shown.
    pub announced: bool,
    /// Tracked: the journal's active quests (`Quest.SetActive`), whose
    /// objectives the HUD and map mark. Quests start untracked: the main
    /// quest's stage fragments track each next one themselves.
    pub active: bool,
    /// When the journal last changed for it (`ScriptState::journal_seq`).
    pub journal_seq: u64,
    /// Reference alias fills: alias id -> reference.
    pub aliases: HashMap<u32, FormId>,
    /// When each alias was filled (`ScriptState::fill_seq`).
    pub filled_at: HashMap<u32, u64>,
    /// The Story Manager event that started it.
    pub event: Option<crate::story::StoryEvent>,
}

/// A HUD notification with an identity independent of the game clock.
pub struct Notification {
    pub id: u64,
    pub text: String,
    pub at: f64,
    /// Seconds on the unpaused real-time clock; MESG notifications use TNAM.
    pub duration: f64,
}

/// Existing duration for notifications without an authored MESG display time.
pub const NOTIFICATION_SECONDS: f64 = 6.0;

impl Notification {
    pub fn is_active(&self, now: f64) -> bool {
        now - self.at < self.duration
    }

    /// The fallback HUD keeps its existing fade, bounded by the display time.
    pub fn alpha(&self, now: f64) -> f32 {
        if !self.is_active(now) {
            return 0.0;
        }
        ((self.duration - (now - self.at)) / self.duration.min(1.5)).clamp(0.0, 1.0) as f32
    }
}

/// Script-visible world state and queues.
#[derive(Default)]
pub struct ScriptState {
    pub notifications: Vec<Notification>,
    notification_seq: u64,
    pub timers: Vec<Timer>,
    pub quests: HashMap<FormId, QuestState>,
    /// Containers the player has taken something from (Chests Looted).
    pub looted: HashSet<FormId>,
    pub globals: HashMap<FormId, f32>,
    pub disabled: HashMap<FormId, bool>,
    pub blocked_activation: HashSet<FormId>,
    /// Locks set by scripts and packages, and keys used, over the references' own
    /// (`XLOC`); lock levels set by scripts.
    pub locked: HashMap<FormId, bool>,
    pub lock_levels: HashMap<FormId, u8>,
    /// Cell public/private overrides from doors locked or unlocked as owner.
    pub cell_public: HashMap<FormId, bool>,
    /// Actor values scripts changed, by actor and value index (`actor_values`), and
    /// by name the ones outside the known values.
    pub actor_values: HashMap<(FormId, u32), crate::actor_values::Modifiers>,
    pub other_actor_values: HashMap<(FormId, String), f32>,
    /// `AddInventoryEventFilter`: the items (or form lists) an object hears
    /// `OnItemAdded` / `OnItemRemoved` for; none set means all.
    pub inventory_filters: HashMap<ObjectId, HashSet<FormId>>,
    /// Events raised by natives, delivered after the current VM run.
    pub pending_events: Vec<(ObjectId, String, Vec<Value>)>,
    /// References whose `GetOpenState` scripts wait on, answered once the
    /// frame's scripts have run (`open_state_key`).
    pub open_state_queries: Vec<FormId>,
    /// Quest fragments to run: (quest, stage).
    pub pending_stages: Vec<(FormId, u16)>,
    /// Quests started whose scripts are to be attached (`Engine::init_quest`).
    pub pending_quest_inits: Vec<FormId>,
    /// Objects that already received OnInit.
    pub initialized: HashSet<ObjectId>,
    /// Real seconds the game ran, menu mode left out (timers, `Wait`).
    pub real_time: f64,
    /// Real seconds since launch, menu mode included (`WaitMenuMode`,
    /// `GetCurrentRealTime`).
    pub wall_time: f64,
    /// Bumped whenever a quest starts or stops or an alias fill changes (actors'
    /// alias packages follow).
    pub alias_gen: u64,
    /// Counts alias fills: which is the latest.
    pub fill_seq: u64,
    /// Set when a native changed enable state; the engine re-syncs visibility.
    pub visibility_dirty: bool,
    pub warned: HashSet<String>,
    /// Bumped whenever a quest's journal entry changes.
    pub journal_seq: u64,
    /// The journal's miscellaneous objectives are untracked (its Toggle
    /// Active on them all).
    pub misc_objectives_hidden: bool,
    /// The player's statistics (`Game.IncrementStat`, the journal's Stats
    /// page) by name.
    pub stats: HashMap<String, i32>,
    /// Spell, school and shout usage for the journal's named statistics.
    pub magic_usage: crate::stats::MagicUsage,
    /// Melee weapon attacks and bow releases for Favorite Weapon.
    pub weapon_usage: crate::stats::WeaponUsage,
    /// The forms, aliases and effects registered for sleep
    /// (`RegisterForSleep`: `OnSleepStart`, `OnSleepStop`).
    pub sleep_listeners: Vec<ObjectId>,
    /// References' and cells' owners set by scripts (`SetActorOwner`,
    /// `SetFactionOwner`; `None` clears it), over the data's.
    pub owners: HashMap<FormId, Option<FormId>>,
    /// Factions scripts put actors (references) in or took them out of, over
    /// their records' lists: the actor's rank there, -1 for out
    /// (`AddToFaction`, `SetFactionRank`, `RemoveFromFaction`).
    pub faction_changes: HashMap<FormId, HashMap<FormId, i8>>,
    /// The player's teammates (`SetPlayerTeammate`) and whether the player
    /// can ask each for favors.
    pub teammates: HashMap<FormId, bool>,
}

impl ScriptState {
    pub fn notify(&mut self, s: impl Into<String>) {
        self.notify_for(s, NOTIFICATION_SECONDS);
    }

    pub(crate) fn notify_for(&mut self, s: impl Into<String>, duration: f64) {
        let s = s.into();
        log::info!("notification: {s}");
        self.notification_seq += 1;
        self.notifications.push(Notification {
            id: self.notification_seq,
            text: s,
            at: self.real_time,
            duration,
        });
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

    fn call_native(
        &mut self,
        class: &str,
        func: &str,
        this: Option<&Value>,
        args: &[Value],
    ) -> papyrus::NativeResult {
        natives::call(self.engine, class, func, this, args)
    }

    fn is_native_type(&self, obj: ObjectId, class: &str) -> bool {
        match obj {
            ObjectId::Form(id) => {
                let actual = self.engine.native_class(FormId(id));
                types::native_is_a(actual, class)
            }
            ObjectId::Alias { .. } => matches!(class, "alias" | "referencealias" | "locationalias"),
            ObjectId::Effect(_) => class == "activemagiceffect",
        }
    }
}

#[cfg(test)]
mod notification_tests {
    use super::*;

    #[test]
    fn notifications_at_zero_and_on_a_paused_clock_keep_distinct_ids() {
        let mut state = ScriptState::default();
        state.notify("first");
        let delivered = state.notifications[0].id;
        assert!(delivered > 0);
        state.notify("second");
        assert_eq!(state.notifications[0].at, state.notifications[1].at);
        let pending: Vec<_> = state
            .notifications
            .iter()
            .filter(|n| n.id > delivered)
            .map(|n| n.text.as_str())
            .collect();
        assert_eq!(pending, ["second"]);
        let last = state.notifications[1].id;
        state.notifications.clear();
        state.notify("after pruning");
        assert!(state.notifications[0].id > last);
    }
}

#[cfg(test)]
#[path = "actor_natives_tests.rs"]
mod actor_natives_tests;
