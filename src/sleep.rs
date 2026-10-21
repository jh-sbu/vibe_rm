//! Sleeping and waiting: the Wait key or a bed opens the sleep / wait menu
//! (unless the game's reasons forbid it: `sNoWait*` / `sNoSleep*`), and the
//! hours chosen pass one by one while it stays up. Each hour moves the clock
//! and the weather on, runs the magic effects on the player for the hour's
//! seconds, heals the other actors for them and puts them where their
//! schedules have them now, fills the player's health, magicka and stamina
//! and counts the Hours Waiting / Hours Slept statistic. Sleeping tells the
//! scripts registered for it (`RegisterForSleep`: `OnSleepStart`,
//! `OnSleepStop`). Enemies finding the player after an hour end the wait.
//! Choices: `known_ambiguities/sleep-wait.md`.

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
            && self.ref_owned_by_other(bed)
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
            self.imagespace.start_sleep_fade_out();
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
            if w.sleeping {
                self.imagespace.start_sleep_fade_in();
            }
            return;
        }
        if w.sleeping && !self.imagespace.sleep_fade_is_black() {
            self.waiting = Some(w);
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
        let interrupted = self.enemies_come_upon_player();
        if w.left > 0 && !interrupted {
            self.waiting = Some(w);
            return;
        }
        self.finish_waiting(w.sleeping, interrupted);
    }

    fn finish_waiting(&mut self, sleeping: bool, interrupted: bool) {
        self.waiting = None;
        self.menu = None;
        if sleeping {
            self.send_sleep_event("OnSleepStop", vec![Value::Bool(interrupted)]);
            self.imagespace.start_sleep_fade_in();
        }
    }

    /// After an hour: the loaded actors that would attack the player and
    /// detect them at all now (an hour is long enough to find them) start
    /// the fight, which ends the wait. Whether any did.
    fn enemies_come_upon_player(&mut self) -> bool {
        use crate::ai::combat::ENGAGE_DISTANCE;
        let player = self.player.position;
        let near: Vec<FormId> = self
            .cells
            .values()
            .flat_map(|rt| &rt.actors)
            .filter(|a| !a.dead && a.bleeding.is_none() && a.combat.is_none())
            .filter(|a| a.pos.distance(player) < ENGAGE_DISTANCE)
            .map(|a| a.ref_id)
            .collect();
        let mut found = false;
        for r in near {
            if self.detects(r, PLAYER_REF) && self.would_attack_player(r) {
                log::info!("{r} comes upon the player while they wait");
                found |= self.start_combat(r, PLAYER_REF);
            }
        }
        found
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
        // The other actors' health and stamina come back for the hour, near
        // and away.
        self.update_stamina(HOUR_SECONDS);
        self.heal_away(HOUR_SECONDS);
        if self.ai_enabled {
            self.jump_to_schedules();
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use esp::writer::write_plugin;

    struct Fixture(std::path::PathBuf);

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn engine() -> (Fixture, Engine) {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("vrm-sleep-fade-{}-{stamp}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        std::fs::write(
            dir.join("Sleep.esp"),
            write_plugin(0, &[], "test", "sleep fade tests", 0x800, &[]),
        )
        .unwrap();
        let engine = crate::stats::action_tests::headless_engine(&dir, "Sleep.esp");
        (Fixture(dir), engine)
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic plugin only"]
    fn sleeping_fades_before_hours_and_returns_after_completion_or_close() {
        let (_fixture, mut e) = engine();
        let listener = ObjectId::Form(0x801);
        e.register_for_sleep(listener, true);
        e.menu = Some(Menu::SleepWait { sleeping: true });
        e.start_waiting(1);
        let hour = e.hour;
        assert!(
            e.scripts
                .pending_events
                .iter()
                .any(|(obj, event, _)| { *obj == listener && event == "OnSleepStart" })
        );

        e.update(Default::default(), 0.25, 20.0);
        assert_eq!(e.hour, hour, "sleep hours wait until the view is black");
        assert!((e.imagespace.sleep_fade_amount() - 0.5).abs() < 1e-6);
        e.update(Default::default(), 0.25, 20.0);
        assert_eq!(e.hour, hour);
        assert!(e.imagespace.sleep_fade_is_black());

        e.update(Default::default(), 0.2, 20.0);
        assert_eq!(e.hour, hour + 1.0);
        assert!(e.menu.is_none());
        assert!(e.scripts.pending_events.iter().any(|(obj, event, args)| {
            *obj == listener && event == "OnSleepStop" && args == &[Value::Bool(false)]
        }));
        assert!(e.imagespace.sleep_fade_is_black());
        e.update(Default::default(), 0.25, 20.0);
        assert!((e.imagespace.sleep_fade_amount() - 0.5).abs() < 1e-6);
        e.update(Default::default(), 0.25, 20.0);
        assert_eq!(e.imagespace.sleep_fade_amount(), 0.0);

        // Closing an active sleep menu through another path cancels the wait
        // and fades from its current amount back to the world.
        e.menu = Some(Menu::SleepWait { sleeping: true });
        e.start_waiting(3);
        e.update(Default::default(), 0.2, 20.0);
        assert!(e.imagespace.sleep_fade_amount() > 0.0);
        e.menu = None;
        e.update(Default::default(), 0.1, 20.0);
        assert!(e.waiting.is_none());
        assert!(e.imagespace.sleep_fade_amount() < 1.0);
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic plugin only"]
    fn waiting_and_canceling_hours_do_not_fade_or_send_sleep_events() {
        let (_fixture, mut e) = engine();
        let listener = ObjectId::Form(0x801);
        e.register_for_sleep(listener, true);
        e.menu = Some(Menu::SleepWait { sleeping: true });
        e.start_waiting(0);
        assert!(e.menu.is_none());
        assert!(e.waiting.is_none());
        assert_eq!(e.imagespace.sleep_fade_amount(), 0.0);
        assert!(e.scripts.pending_events.is_empty());

        e.menu = Some(Menu::SleepWait { sleeping: false });
        e.start_waiting(1);
        let hour = e.hour;
        e.update(Default::default(), 0.2, 20.0);
        assert_eq!(e.hour, hour + 1.0);
        assert_eq!(e.imagespace.sleep_fade_amount(), 0.0);
        assert!(e.scripts.pending_events.is_empty());
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic plugin only"]
    fn interrupted_sleep_sends_flag_and_begins_fade_in() {
        let (_fixture, mut e) = engine();
        let listener = ObjectId::Form(0x801);
        e.register_for_sleep(listener, true);
        e.menu = Some(Menu::SleepWait { sleeping: true });
        e.start_waiting(2);
        e.update_sleep_imagespace(crate::imagespace::SLEEP_FADE_SECONDS);
        assert!(e.imagespace.sleep_fade_is_black());

        e.finish_waiting(true, true);
        assert!(e.waiting.is_none());
        assert!(e.menu.is_none());
        assert!(e.scripts.pending_events.iter().any(|(obj, event, args)| {
            *obj == listener && event == "OnSleepStop" && args == &[Value::Bool(true)]
        }));
        e.update_sleep_imagespace(crate::imagespace::SLEEP_FADE_SECONDS * 0.5);
        assert!((e.imagespace.sleep_fade_amount() - 0.5).abs() < 1e-6);
    }
}
