//! Arrests: guards (their class's Guard flag) of a crime faction the player
//! owes come after them once a crime is reported there, calling on them to
//! halt (the `PURS` lines), and stop them with the arrest dialogue: the
//! blocking branches of `DialogueCrimeGuards`, whose fragments pay the
//! bounty, send the player to jail or resist (UESP *Skyrim:Crime*). Resisting,
//! walking off, or a bounty past what a faction arrests for makes its guards
//! attack instead (the faction's Arrest and Attack on Sight crime flags).

use std::collections::HashMap;

use esp::FormId;

use crate::crime::CrimeValues;
use crate::engine::{Engine, PLAYER_REF};

/// The bounty from which the guards of a faction flagged "Attack on Sight"
/// attack instead of arresting. No public source gives it; UESP's crime
/// table puts murder (1000) where guards stop asking.
pub const ATTACK_ON_SIGHT_GOLD: i32 = 1000;
/// `IsGuardFaction` (Skyrim.esm).
const IS_GUARD_FACTION: FormId = FormId(0x86EEE);
/// Seconds between a pursuing guard's calls to halt (no source).
const PURSUE_BARK: (f32, f32) = (6.0, 10.0);
/// Guards within this many units of the player hear of a crime reported to
/// their faction (no source).
const ALARM_DISTANCE: f32 = 2048.0;
/// A pursuing guard stops the player within this many units (no source).
const ARREST_DISTANCE: f32 = 160.0;

/// A guard coming to arrest the player for its crime faction
/// (`GetAlarmed`).
#[derive(Debug, Clone, Copy)]
pub struct Pursuit {
    pub faction: FormId,
    bark_in: f32,
}

#[derive(Default)]
pub struct Arrests {
    /// Guards coming to arrest the player.
    pub alarmed: HashMap<FormId, Pursuit>,
    /// Crime factions whose arrest the player resisted: their guards attack
    /// until the bounty is paid.
    pub resisting: std::collections::HashSet<FormId>,
    /// The guard arresting the player in conversation now
    /// (`GetArrestingActor`), and its faction.
    pub arresting: Option<(FormId, FormId)>,
    /// Where to send the player once the VM is done (the far side of a
    /// prison marker; see `send_player_through`).
    pending: Option<FormId>,
    /// The player's jail sentence, while they are in jail.
    pub jailed: Option<Sentence>,
    /// Days the player has spent in jail (`GetDaysInJail`).
    pub days_in_jail: i32,
}

/// A jail sentence: the faction's, so many days, and the prison marker
/// inside the jail (the way out).
#[derive(Debug, Clone, Copy)]
pub struct Sentence {
    pub faction: FormId,
    pub days: i32,
    pub inside: FormId,
    /// The bounty the sentence is for: it stands again if the player escapes
    /// (UESP: "If you do escape, your bounty will remain").
    pub owed: crate::crime::Bounty,
}

/// The lockpick the player keeps in jail (`Lockpick`).
const LOCKPICK: FormId = FormId(0xA);
/// UESP: the sentence is at most seven days, served for any bounty of 700
/// or more; read as a day for each 100 gold.
const MAX_SENTENCE: i32 = 7;

impl Engine {
    /// Whether an actor is a guard: a member of `IsGuardFaction` (the hold
    /// guards, at rank 0; their templates list it at -1) or of a class with
    /// the Guard flag (`CLAS` `DATA`'s last byte, 0x1: the four guard classes
    /// in Skyrim.esm). Which the game reads isn't documented.
    pub fn is_guard(&self, actor: FormId) -> bool {
        if actor == PLAYER_REF {
            return false;
        }
        if self
            .npc_factions(actor)
            .iter()
            .any(|&(f, rank)| f == IS_GUARD_FACTION && rank >= 0)
        {
            return true;
        }
        let Some(class) = self
            .templates_of(actor)
            .and_then(|t| t.form(&self.lo, crate::world::template::TRAITS, b"CNAM"))
        else {
            return false;
        };
        self.lo
            .get(class)
            .and_then(|r| {
                r.get(b"DATA")
                    .filter(|d| d.len() >= 36)
                    .map(|d| d[35] & 1 != 0)
            })
            .unwrap_or(false)
    }

    /// Whether `r` is a guard coming to arrest the player (`GetAlarmed`).
    pub fn is_alarmed(&self, r: FormId) -> bool {
        self.crime.arrests.alarmed.contains_key(&r)
    }

    /// Whether `r` attacks the player for their crimes: a guard of a crime
    /// faction they owe that doesn't arrest (no Arrest flag), whose arrest
    /// they resisted, or whose Attack on Sight bounty they have passed.
    pub(crate) fn crime_hostile(&self, r: FormId) -> bool {
        let Some(f) = self.crime_faction(r) else {
            return false;
        };
        let owed = self.bounty(f).total();
        if owed <= 0 || !self.is_guard(r) {
            return false;
        }
        let Some(v) = CrimeValues::of(&self.lo, f) else {
            return false;
        };
        !v.arrest
            || self.crime.arrests.resisting.contains(&f)
            || (v.attack_on_sight && owed >= ATTACK_ON_SIGHT_GOLD)
    }

    /// A crime was reported to `faction`: its guards hear of it (UESP: the
    /// witnesses report it to the guards) and those near by, awake and not
    /// fighting come to arrest the player, unless the faction doesn't arrest
    /// (then they attack on finding them: `crime_hostile`).
    pub(crate) fn raise_alarm(&mut self, faction: FormId) {
        if !CrimeValues::of(&self.lo, faction).is_some_and(|v| v.arrest) || self.player_dead() {
            return;
        }
        let player = self.ref_position(PLAYER_REF).unwrap_or_default();
        let guards: Vec<FormId> = self
            .cells
            .values()
            .flat_map(|rt| &rt.actors)
            .filter(|a| {
                !a.dead
                    && a.bleeding.is_none()
                    && a.combat.is_none()
                    && a.pos.distance(player) < ALARM_DISTANCE
            })
            .map(|a| a.ref_id)
            .filter(|&r| {
                !self.crime.arrests.alarmed.contains_key(&r)
                    && self.crime_faction(r) == Some(faction)
                    && self.is_guard(r)
                    && self.sit_sleep_state(r, true) != 3.0
            })
            .collect();
        log::debug!("alarm for {faction}: {} loaded guards of it", guards.len());
        for g in guards {
            log::info!("{g} comes to arrest the player for {faction}");
            self.crime.arrests.alarmed.insert(
                g,
                Pursuit {
                    faction,
                    bark_in: 0.0,
                },
            );
        }
    }

    /// Pursuing guards run to the player, calling on them to halt now and
    /// then; the first to reach them, with no conversation going on, stops
    /// them with the arrest dialogue (the blocking greeting that passes,
    /// `GetAlarmed` true for it). Guards stop pursuing once the bounty is
    /// gone, they are fighting, or they are no longer loaded. A guard whose
    /// faction now attacks the player starts the fight.
    pub(crate) fn update_arrests(&mut self, dt: f32) {
        if let Some(door) = self.crime.arrests.pending.take() {
            self.send_player_through(door);
        }
        if self
            .crime
            .arrests
            .jailed
            .is_some_and(|s| self.location != crate::engine::Location::Interior(self.jail_cell(s)))
        {
            self.escape_jail();
        }
        let ids: Vec<(FormId, Pursuit)> = self
            .crime
            .arrests
            .alarmed
            .iter()
            .map(|(&r, &p)| (r, p))
            .collect();
        let player = self.player_feet();
        let mut reached = Vec::new();
        for (r, mut p) in ids {
            let live = self
                .actor_ref(r)
                .filter(|a| !a.dead && a.bleeding.is_none());
            let fighting = live.is_some_and(|a| a.combat.is_some());
            if live.is_none()
                || fighting
                || self.bounty(p.faction).total() <= 0
                || self.player_dead()
            {
                self.calm_guard(r);
                continue;
            }
            if self.crime_hostile(r) {
                self.calm_guard(r);
                self.start_combat(r, PLAYER_REF);
                continue;
            }
            if self.crime.arrests.arresting.is_some_and(|(g, _)| g == r) {
                continue;
            }
            let pos = self.actor_ref(r).map(|a| a.pos).unwrap_or_default();
            if let Some(a) = self.actor_mut_any(r) {
                a.0.pursue(PLAYER_REF, a.1);
            }
            p.bark_in -= dt;
            if p.bark_in <= 0.0 {
                let k = (self.rand() % 1000) as f32 / 1000.0;
                p.bark_in = PURSUE_BARK.0 + (PURSUE_BARK.1 - PURSUE_BARK.0) * k;
                log::debug!(
                    "{r} pursuing the player, {:.0} units off ({:.0} up)",
                    pos.truncate().distance(player.truncate()),
                    player.z - pos.z
                );
                if self.barks.current.is_none() && self.conversation.is_none() {
                    self.bark(r, b"PURS");
                }
            }
            self.crime.arrests.alarmed.insert(r, p);
            // Level with the player, and close.
            let d = pos.truncate().distance(player.truncate());
            if d < ARREST_DISTANCE && (pos.z - player.z).abs() < ARREST_DISTANCE {
                reached.push((d, r, p.faction));
            }
        }
        reached.sort_by(|a, b| a.0.total_cmp(&b.0));
        if let Some(&(_, r, faction)) = reached.first()
            && self.conversation.is_none()
        {
            match self.blocking_greeting(r) {
                Some(greeting) => {
                    log::info!(
                        "{r} stops the player: {} {}",
                        greeting.0.editor_id,
                        greeting.1.id
                    );
                    self.crime.arrests.arresting = Some((r, faction));
                    self.crimes_known(faction);
                    self.open_conversation(r, Some(greeting));
                }
                None => {
                    log::info!("{r} has no arrest dialogue; it gives up");
                    self.calm_guard(r);
                }
            }
        }
    }

    /// A guard stops pursuing the player.
    fn calm_guard(&mut self, r: FormId) {
        self.crime.arrests.alarmed.remove(&r);
        if let Some(a) = self.actor_mut(r) {
            a.end_pursuit();
        }
    }

    /// The loaded actor `r` with the furniture world, to change both.
    fn actor_mut_any(
        &mut self,
        r: FormId,
    ) -> Option<(
        &mut crate::ai::ActorRuntime,
        &mut crate::ai::furniture::FurnitureWorld,
    )> {
        let key = self.actor_cells.get(&r).copied()?;
        let a = self
            .cells
            .get_mut(&key)?
            .actors
            .iter_mut()
            .find(|a| a.ref_id == r)?;
        Some((a, &mut self.furniture))
    }

    /// The player resists arrest (`SetPlayerResistingArrest`, said by the
    /// arresting guard): that faction's guards attack them until the bounty
    /// is paid (UESP: "the guards ... in the area" turn hostile).
    pub fn set_player_resisting_arrest(&mut self, guard: FormId) {
        let Some(f) = self
            .crime_faction(guard)
            .or(self.crime.arrests.arresting.map(|a| a.1))
        else {
            return;
        };
        log::info!("the player resists arrest by {f}");
        self.crime.arrests.resisting.insert(f);
        self.crime.arrests.arresting = None;
        let guards: Vec<FormId> = self
            .crime
            .arrests
            .alarmed
            .iter()
            .filter(|(_, p)| p.faction == f)
            .map(|(&r, _)| r)
            .collect();
        for g in guards.into_iter().chain(std::iter::once(guard)) {
            self.calm_guard(g);
            if self.crime_hostile(g) {
                self.start_combat(g, PLAYER_REF);
            }
        }
    }

    /// The arrest conversation ended. Walking away from it without settling
    /// (the bounty still owed and the guard not letting them go with a
    /// goodbye line) is resisting (UESP: cancelling the dialogue makes the
    /// guards attack). A guard letting them go stops pursuing.
    pub(crate) fn arrest_conversation_ended(&mut self, npc: FormId, goodbye: bool) {
        let Some((guard, f)) = self.crime.arrests.arresting.filter(|a| a.0 == npc) else {
            return;
        };
        self.crime.arrests.arresting = None;
        if self.bounty(f).total() <= 0 || self.crime.arrests.resisting.contains(&f) {
            self.calm_guard(guard);
        } else if goodbye {
            log::info!("{guard} lets the player go");
            self.calm_guard(guard);
        } else {
            self.set_player_resisting_arrest(guard);
        }
    }

    /// The player's crimes against a faction are settled (paid or jailed):
    /// its guards stop pursuing and fighting them.
    pub(crate) fn crimes_settled(&mut self, faction: FormId) {
        self.crime.arrests.resisting.remove(&faction);
        self.crimes_known(faction);
        let guards: Vec<FormId> = self
            .cells
            .values()
            .flat_map(|rt| &rt.actors)
            .filter(|a| a.combat.as_ref().is_some_and(|c| c.target == PLAYER_REF))
            .map(|a| a.ref_id)
            .filter(|&r| self.crime_faction(r) == Some(faction) && self.is_guard(r))
            .collect();
        for g in guards {
            self.end_combat(g);
        }
        let alarmed: Vec<FormId> = self
            .crime
            .arrests
            .alarmed
            .iter()
            .filter(|(_, p)| p.faction == faction)
            .map(|(&r, _)| r)
            .collect();
        for g in alarmed {
            self.calm_guard(g);
        }
    }

    /// The prison marker reference of a crime faction (`JAIL`): its far side
    /// (`XTEL`) is in the jail.
    pub(crate) fn jail_marker(&self, faction: FormId) -> Option<FormId> {
        self.faction_form(faction, b"JAIL")
    }

    /// Send the player through a door or prison marker to its far side, once
    /// the scripts are done (the cell changes).
    pub(crate) fn queue_player_through(&mut self, door: FormId) {
        self.crime.arrests.pending = Some(door);
    }

    fn send_player_through(&mut self, door: FormId) {
        // Taken away: the guard's conversation is over.
        self.end_conversation();
        let Some((dest, pos, rot)) = self
            .lo
            .get(door)
            .and_then(|r| crate::world::records::reference(&r).teleport)
        else {
            log::warn!("{door} leads nowhere");
            return;
        };
        if let Err(e) = self.teleport_through(dest, pos, rot.z) {
            log::warn!("can't send the player through {door}: {e}");
        }
    }

    /// A crime faction's form field `tag` (`JAIL`, `STOL`, `PLCN`, `JOUT`).
    fn faction_form(&self, faction: FormId, tag: &[u8; 4]) -> Option<FormId> {
        let rec = self.lo.get(faction)?;
        let d = rec.get(tag).filter(|d| d.len() >= 4)?;
        Some(rec.fid(FormId(u32::from_le_bytes(d[0..4].try_into().unwrap()))))
            .filter(|f| !f.is_null())
    }

    /// Send the player to a crime faction's jail (`SendPlayerToJail`): the
    /// bounty becomes a sentence (UESP: at most seven days, for 700 gold or
    /// more), stolen goods go to the evidence chest (`STOL`) and, with
    /// `remove_inventory`, everything else to the player's belongings chest
    /// (`PLCN`) but one lockpick; they wear the jail outfit (`JOUT`) and are
    /// put in the cell through the jail's prison marker (`JAIL`).
    pub fn send_player_to_jail(&mut self, faction: FormId, remove_inventory: bool) {
        let Some(jail) = self.jail_marker(faction) else {
            log::warn!("{faction} has no jail");
            return;
        };
        let Some(inside) = self
            .lo
            .get(jail)
            .and_then(|r| crate::world::records::reference(&r).teleport)
            .map(|t| t.0)
        else {
            return;
        };
        let bounty = self.bounty(faction);
        let owed = bounty.total();
        let guard = self.arrested_by(faction);
        let days = (owed / 100).clamp(1, MAX_SENTENCE);
        self.set_crime_gold(faction, 0, true);
        self.set_crime_gold(faction, 0, false);
        self.confiscate_stolen(faction);
        if remove_inventory {
            let chest = self.faction_form(faction, b"PLCN");
            let items = self.inventory_mut(PLAYER_REF).items.clone();
            for (f, n) in items {
                let keep = if f == LOCKPICK { 1 } else { 0 };
                if n > keep {
                    self.remove_stack(PLAYER_REF, f, n - keep, None, chest, None);
                }
            }
        }
        if let Some(outfit) = self.faction_form(faction, b"JOUT") {
            let items: Vec<FormId> = self
                .lo
                .get(outfit)
                .map(|r| {
                    r.subrecords()
                        .filter(|s| s.tag.0 == *b"INAM")
                        .flat_map(|s| {
                            s.data
                                .chunks_exact(4)
                                .map(|c| r.fid(FormId(u32::from_le_bytes(c.try_into().unwrap()))))
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            for item in items {
                self.add_item(PLAYER_REF, item, 1);
                if let Err(e) = self.equip_item(PLAYER_REF, item, true) {
                    log::debug!("jail outfit {item}: {e}");
                }
            }
        }
        self.crimes_settled(faction);
        self.crime.arrests.jailed = Some(Sentence {
            faction,
            days,
            inside,
            owed: bounty,
        });
        log::info!("the player is jailed by {faction} for {days} days ({owed} gold)");
        let place = self.jail_location(inside);
        self.send_jail_event(guard, faction, place, owed);
        self.queue_player_through(jail);
    }

    /// The player gives themselves up to the guard arresting them for
    /// `faction`, if one is: the arrest event. Returns the guard.
    pub(crate) fn arrested_by(&mut self, faction: FormId) -> Option<FormId> {
        let (guard, _) = self.crime.arrests.arresting.filter(|a| a.1 == faction)?;
        let crime = self
            .crime
            .last_crime
            .get(&faction)
            .map_or(-1, |&k| k as i32);
        self.send_arrest_event(guard, faction, crime);
        Some(guard)
    }

    /// Where a jail is: the location of the cell its inner prison marker is in.
    fn jail_location(&self, inside: FormId) -> Option<FormId> {
        self.lo
            .cell_of_ref(inside)
            .and_then(|c| self.cell_location(c))
    }

    /// The jail's interior: where its inner prison marker is.
    fn jail_cell(&self, s: Sentence) -> FormId {
        self.lo.cell_of_ref(s.inside).unwrap_or_default()
    }

    /// The player unlocked a door or container: in jail, unlocking anything
    /// there (the cell door) is escaping (UESP: "unlocking the door to a jail
    /// cell is considered a crime").
    pub(crate) fn player_unlocked(&mut self, lock: FormId) {
        if self.in_jail_with(lock) {
            self.escape_jail();
        }
    }

    /// Whether the player is jailed where `r` is.
    pub(crate) fn in_jail_with(&self, r: FormId) -> bool {
        self.crime
            .arrests
            .jailed
            .is_some_and(|s| self.lo.cell_of_ref(r) == Some(self.jail_cell(s)))
    }

    /// The player escapes jail: the sentence is over unserved, the bounty it
    /// was for stands again with the faction's escape gold on top (UESP:
    /// 100), and its guards near by come for them. Their belongings stay in
    /// the chest.
    pub fn escape_jail(&mut self) {
        let Some(s) = self.crime.arrests.jailed.take() else {
            return;
        };
        log::info!("the player escapes {}'s jail", s.faction);
        let b = self.crime.bounties.entry(s.faction).or_default();
        b.violent += s.owed.violent;
        b.nonviolent += s.owed.nonviolent;
        let gold = CrimeValues::of(&self.lo, s.faction)
            .map_or(100, |v| v.gold(crate::crime::CrimeType::Escape, 0));
        self.crime
            .last_crime
            .insert(s.faction, crate::crime::CrimeType::Escape);
        if gold > 0 {
            self.mod_crime_gold(s.faction, gold, false);
            self.notify_crime_gold(s.faction, gold, "sAddCrimeGold", "bounty added to");
            self.send_crime_gold_event(None, s.faction, gold, crate::crime::CrimeType::Escape);
        }
        let place = self.jail_location(s.inside);
        self.send_escape_jail_event(s.faction, place);
        self.raise_alarm(s.faction);
    }

    /// Whether activating `bed` serves the player's sentence: a bed in the
    /// jail they are in (UESP: "sleep in a cell bed").
    pub fn serves_sentence_in(&self, bed: FormId) -> bool {
        let Some(s) = self.crime.arrests.jailed else {
            return false;
        };
        let sleep = self.furniture.get(bed).is_some_and(|f| {
            f.markers
                .iter()
                .any(|m| m.kind == crate::ai::furniture::Use::Sleep)
        });
        sleep
            && self.lo.cell_of_ref(bed).is_some()
            && self.lo.cell_of_ref(bed) == self.lo.cell_of_ref(s.inside)
    }

    /// Serve the sentence: the days pass, the player gets their belongings
    /// back from the belongings chest and is let out through the prison
    /// marker. Skill progress lost (UESP) isn't kept yet.
    pub fn serve_sentence(&mut self) {
        let Some(s) = self.crime.arrests.jailed.take() else {
            return;
        };
        self.day += s.days as u32;
        self.crime.arrests.days_in_jail += s.days;
        if let Some(chest) = self.faction_form(s.faction, b"PLCN") {
            let items = self.inventory_mut(chest).items.clone();
            for (f, n) in items {
                self.remove_stack(chest, f, n, None, Some(PLAYER_REF), None);
            }
        }
        log::info!("the player serves {} days for {}", s.days, s.faction);
        let place = self.jail_location(s.inside);
        self.send_served_time_event(s.faction, place, s.owed.total(), s.days);
        self.scripts
            .notify(format!("You served {} days in jail.", s.days));
        self.queue_player_through(s.inside);
    }
}
