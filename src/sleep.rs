//! Sleeping and waiting: the Wait key or a bed opens the sleep / wait menu
//! (unless the game's reasons forbid it: `sNoWait*` / `sNoSleep*`), and the
//! hours chosen pass one by one while it stays up. Each hour moves the clock
//! and the weather on, runs the magic effects on the player for the hour's
//! seconds, fills the player's health, magicka and stamina and counts the
//! Hours Waiting / Hours Slept statistic. Sleeping tells the scripts
//! registered for it (`RegisterForSleep`: `OnSleepStart`, `OnSleepStop`).
//! Choices: `known_gaps/sleep-wait.md`.

use esp::FormId;
use esp::actor_value as av;
use papyrus::{ObjectId, Value};

use crate::engine::{Engine, PLAYER_REF};
use crate::items::Menu;

/// Real seconds each hour takes to pass while waiting (no source).
const SECONDS_PER_HOUR: f32 = 0.2;
/// Real seconds in a game hour at the timescale of 20.
const HOUR_SECONDS: f32 = 3600.0 / 20.0;

/// Hours passing in the sleep / wait menu.
#[derive(Debug, Clone, Copy)]
pub struct Waiting {
    pub sleeping: bool,
    /// Hours still to pass.
    pub left: u32,
    /// Real seconds until the next.
    next: f32,
}

impl Engine {
    /// Opens the sleep / wait menu: waiting, or sleeping in `bed`. When the
    /// player can't, says why.
    pub fn request_sleep_wait(&mut self, bed: Option<FormId>) {
        let sleeping = bed.is_some();
        if let Some(reason) = self.no_sleep_wait_reason(bed) {
            let (setting, english) = reason;
            let name = format!("sNo{}{setting}", if sleeping { "Sleep" } else { "Wait" });
            let fallback = english.replace("%s", if sleeping { "sleep" } else { "wait" });
            let text = self.gmst_string(&name).unwrap_or(fallback);
            self.scripts.notify(text);
            return;
        }
        self.menu = Some(Menu::SleepWait { sleeping });
    }

    /// Why the player can't wait (or sleep in `bed`) now: the game setting's
    /// name after `sNoWait` / `sNoSleep`, and English for it.
    fn no_sleep_wait_reason(&self, bed: Option<FormId>) -> Option<(&'static str, &'static str)> {
        use crate::ai::combat::CombatState;
        if self.combat_state(PLAYER_REF) != CombatState::None {
            return Some((
                "HostileActorsNear",
                "You cannot %s when enemies are nearby.",
            ));
        }
        if !self.player.grounded && !self.player.noclip {
            return Some(("InAir", "You cannot %s in the air."));
        }
        if !self.crime.arrests.alarmed.is_empty() {
            return Some(("WhileAlarm", "You cannot %s while guards are pursuing you."));
        }
        if self.trespass_warning_level() > 0 {
            return Some(("WarnToLeave", "You cannot %s while being asked to leave."));
        }
        if self.trespassing() {
            return Some(("Trespass", "You cannot %s while trespassing."));
        }
        if self.taking_health_damage() {
            return Some((
                "TakingHealthDamage",
                "You cannot %s while taking health damage.",
            ));
        }
        if let Some(bed) = bed
            && self.ref_owner(bed).is_some_and(|o| self.owned_by_other(o))
        {
            return Some(("InOwnedBed", "You cannot %s in an owned bed."));
        }
        None
    }

    /// Whether a magic effect on the player is taking their health away.
    fn taking_health_damage(&self) -> bool {
        self.magic.effects.iter().any(|x| {
            x.target == PLAYER_REF
                && x.active
                && !x.done
                && self.magic_effect(x.effect).is_some_and(|m| {
                    m.primary == Some(av::HEALTH) && m.has(crate::magic::flags::DETRIMENTAL)
                })
        })
    }

    /// The menu's OK: `hours` start passing (sleeping or not, as the menu
    /// was opened). Scripts registered for sleep hear it start.
    pub fn start_waiting(&mut self, hours: u32) {
        let Some(Menu::SleepWait { sleeping }) = self.menu else {
            return;
        };
        if hours == 0 {
            self.menu = None;
            return;
        }
        if sleeping {
            let now = self.game_days();
            let end = now + hours as f32 / 24.0;
            self.send_sleep_event("OnSleepStart", vec![Value::Float(now), Value::Float(end)]);
        }
        self.waiting = Some(Waiting {
            sleeping,
            left: hours,
            next: SECONDS_PER_HOUR,
        });
    }

    /// While the menu is up: passes the next hour when its time comes, and
    /// closes the menu after the last.
    pub(crate) fn update_waiting(&mut self, dt: f32) {
        let Some(mut w) = self.waiting else {
            return;
        };
        if !matches!(self.menu, Some(Menu::SleepWait { .. })) {
            self.waiting = None;
            return;
        }
        w.next -= dt;
        if w.next > 0.0 {
            self.waiting = Some(w);
            return;
        }
        w.next += SECONDS_PER_HOUR;
        w.left -= 1;
        self.pass_hour(w.sleeping);
        if w.left > 0 {
            self.waiting = Some(w);
            return;
        }
        self.waiting = None;
        self.menu = None;
        if w.sleeping {
            self.send_sleep_event("OnSleepStop", vec![Value::Bool(false)]);
        }
    }

    /// An hour passes for the player.
    fn pass_hour(&mut self, sleeping: bool) {
        self.hour += 1.0;
        if self.hour >= 24.0 {
            self.hour -= 24.0;
            self.day += 1;
        }
        self.update_weather(1.0);
        self.update_magic(HOUR_SECONDS);
        for index in [av::HEALTH, av::MAGICKA, av::STAMINA] {
            self.damage_actor_value(PLAYER_REF, index, -1.0e9);
        }
        let stat = if sleeping {
            "Hours Slept"
        } else {
            "Hours Waiting"
        };
        self.increment_stat(stat, 1);
    }

    fn send_sleep_event(&mut self, event: &str, args: Vec<Value>) {
        for &obj in &self.scripts.sleep_listeners.clone() {
            self.scripts
                .pending_events
                .push((obj, event.to_string(), args.clone()));
        }
    }

    /// `RegisterForSleep` / `UnregisterForSleep`.
    pub fn register_for_sleep(&mut self, obj: ObjectId, on: bool) {
        let listeners = &mut self.scripts.sleep_listeners;
        listeners.retain(|&o| o != obj);
        if on {
            listeners.push(obj);
        }
    }
}
