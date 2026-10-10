//! Locked doors and containers: their locks (`XLOC`), keys, sleepers locking their
//! homes for the night and packages unlocking them, and what scripts set.

use esp::FormId;

use crate::ai::schedule::Place;
use crate::engine::{Engine, PLAYER_REF};
use crate::world::records::{self, Lock};

impl Engine {
    /// A reference's own lock (`XLOC`), with any level a script set.
    pub fn lock_of(&self, r: FormId) -> Option<Lock> {
        let own = self.lo.get(r).and_then(|rec| records::reference(&rec).lock);
        match self.scripts.lock_levels.get(&r) {
            Some(&level) => Some(Lock {
                level,
                ..own.unwrap_or(Lock {
                    level,
                    key: None,
                    leveled: false,
                })
            }),
            None => own,
        }
    }

    /// Whether a door or container is locked now: as scripts, packages or a key
    /// left it, else as placed (a reference with a lock starts out locked).
    pub fn is_locked(&self, r: FormId) -> bool {
        match self.scripts.locked.get(&r) {
            Some(&l) => l,
            None => self.lo.get(r).is_some_and(|rec| rec.get(b"XLOC").is_some()),
        }
    }

    pub fn set_locked(&mut self, r: FormId, locked: bool) {
        if self.is_locked(r) != locked {
            log::info!("{r} {}", if locked { "locked" } else { "unlocked" });
        }
        self.scripts.locked.insert(r, locked);
    }

    /// The lock the player must get past to use a door, if it's locked: a load
    /// door and the one on the far side share a lock (which either may hold), but
    /// a locked house or shop always lets the player out into the open.
    pub(crate) fn door_lock_for_player(
        &self,
        door: FormId,
        partner: Option<FormId>,
    ) -> Option<FormId> {
        let Some(partner) = partner else {
            return self.is_locked(door).then_some(door);
        };
        let outside = |r: FormId| {
            self.lo
                .cell_of_ref(r)
                .and_then(|c| self.lo.cell(c))
                .is_some_and(|i| i.world.is_some())
        };
        if outside(partner) && !outside(door) {
            return None;
        }
        [door, partner].into_iter().find(|&r| self.is_locked(r))
    }

    /// Whether an actor gets past a locked door or container: with its key, or when
    /// it (or one of its factions) owns it (`XOWN`, else the cell's owner). Only
    /// animated doors ask: packages take actors through locked load doors that
    /// aren't theirs (`known_gaps/locks.md`).
    pub fn npc_may_open(&self, achr: FormId, r: FormId) -> bool {
        if !self.is_locked(r) {
            return true;
        }
        let Some(npc) = self.base_of(achr) else {
            return false;
        };
        if let Some(owner) = crate::ai::furniture::owner_of(&self.lo, r)
            && (owner == npc
                || self
                    .npc_factions(achr)
                    .iter()
                    .any(|&(f, rank)| f == owner && rank >= 0))
        {
            return true;
        }
        self.lock_of(r)
            .and_then(|l| l.key)
            .is_some_and(|k| self.item_count(achr, k) > 0)
    }

    /// The player activating a locked door or container (`then`: what they
    /// activated, which opens once the lock does): the key opens it for good;
    /// otherwise, with lockpicks, picking it begins. True when it's open now.
    pub(crate) fn player_unlock(&mut self, r: FormId, then: FormId) -> bool {
        if !self.is_locked(r) {
            return true;
        }
        let lock = self.lock_of(r).unwrap_or(Lock {
            level: 0,
            key: None,
            leveled: false,
        });
        if let Some(key) = lock.key.filter(|&k| self.item_count(PLAYER_REF, k) > 0) {
            self.set_locked(r, false);
            self.player_unlocked(r);
            log::info!("{r} opened with {key} ({})", self.form_name(key));
            return true;
        }
        if lock.level == Lock::NEEDS_KEY {
            let msg = self
                .gmst_string("sImpossibleLock")
                .unwrap_or_else(|| "Requires key".into());
            self.scripts.notify(msg);
            return false;
        }
        if self.item_count(PLAYER_REF, LOCKPICK) <= 0 {
            let level = self
                .gmst_string(lock_level_setting(lock.level))
                .unwrap_or_default();
            let msg = self
                .gmst_string("sOutOfLockpicks")
                .unwrap_or_else(|| "You have no lockpicks".into());
            self.scripts.notify(format!("{msg} ({level})"));
            return false;
        }
        self.start_lockpick(r, then, lock.level);
        self.lockpicking_crime(r, then);
        false
    }

    /// Who owns a lock: the reference holding it, the door or container
    /// activated, or a load door's other side (`XOWN`, else their cell's owner:
    /// a house's own door outside often has none).
    fn lock_owner(&self, lock: FormId, then: FormId) -> Option<FormId> {
        let partner = self
            .lo
            .get(then)
            .and_then(|rec| records::reference(&rec).teleport.map(|t| t.0));
        [Some(lock), Some(then), partner]
            .into_iter()
            .flatten()
            .find_map(|r| crate::ai::furniture::owner_of(&self.lo, r))
    }

    /// Starting to pick a lock someone else owns is a crime, whether or not
    /// it opens (UESP Crime: "Lockpicking", 5 gold). There is no crime type of
    /// its own, so it is reported as trespass, whose gold is the same.
    /// In jail, the jail's locks are the escape crime's instead (on opening).
    fn lockpicking_crime(&mut self, lock: FormId, then: FormId) {
        if self.in_jail_with(lock) {
            return;
        }
        let Some(owner) = self
            .lock_owner(lock, then)
            .filter(|&o| self.owned_by_other(o))
        else {
            return;
        };
        let is_faction = self.lo.tag_of(owner).is_some_and(|t| t.0 == *b"FACT");
        let victim = if is_faction {
            None
        } else {
            self.npc_refs_index().get(&owner).copied()
        };
        log::info!("picking {lock}, owned by {owner}");
        self.commit_crime(
            crate::crime::CrimeType::Trespass,
            victim,
            is_faction.then_some(owner),
            0,
        );
    }

    /// Put a lock in front of the player to pick (UESP *Skyrim:Lockpicking*): a sweet
    /// spot somewhere on the pick's half circle, `60 x 2^-difficulty x (0.82 +
    /// fLockpickSkillSweetSpotMult x skill)` degrees wide, with partial zones either
    /// side `fPartialPick<level> x (fLockpickSkillPartialPickBase +
    /// fLockpickSkillPartialPickMult x skill)` wide; a pick lasts 2 / 1 / 0.75 / 0.5 /
    /// 0.25 seconds of strain (novice .. master) x (1 + skill / 200).
    pub fn start_lockpick(&mut self, lock: FormId, then: FormId, level: u8) {
        let difficulty = difficulty(level);
        let skill = self.actor_value(PLAYER_REF, esp::actor_value::LOCKPICKING);
        let gmst = |n: &str, d: f32| crate::ai::combat::gmst_f32(&self.lo, n, d);
        let partial_setting = [
            "fPartialPickVeryEasy",
            "fPartialPickEasy",
            "fPartialPickAverage",
            "fPartialPickHard",
            "fPartialPickVeryHard",
        ][difficulty - 1];
        let sweet = 60.0
            * 0.5f32.powi(difficulty as i32)
            * (0.82 + gmst("fLockpickSkillSweetSpotMult", 0.006) * skill);
        let partial = gmst(partial_setting, 26.0 - 4.0 * difficulty as f32)
            * (gmst("fLockpickSkillPartialPickBase", 0.775)
                + gmst("fLockpickSkillPartialPickMult", 0.015) * skill);
        let durability = [2.0, 1.0, 0.75, 0.5, 0.25][difficulty - 1] * (1.0 + 0.5 * skill / 100.0);
        let room = 90.0 - sweet / 2.0;
        let centre = (self.rand() % 10_000) as f32 / 10_000.0 * 2.0 * room - room;
        log::info!(
            "picking {lock} (difficulty {difficulty}, skill {skill:.0}): sweet spot {sweet:.1} deg at {centre:.1}, partial {partial:.1}, pick lasts {durability:.2} s"
        );
        self.lockpick = Some(Lockpick {
            lock,
            then,
            level,
            centre,
            sweet,
            partial,
            durability,
            pick: 0.0,
            turn: 0.0,
            strain: 0.0,
            turning: false,
            hold: 0.0,
        });
        self.menu = Some(crate::items::Menu::Lockpick);
    }

    /// Turn the lock as far as the pick lets it: all the way opens it; held against
    /// the pick, the pick wears out and snaps, and the lock springs back.
    pub(crate) fn update_lockpick(&mut self, dt: f32) {
        let Some(mut lp) = self.lockpick.take() else {
            return;
        };
        if self.menu != Some(crate::items::Menu::Lockpick) {
            return;
        }
        if lp.hold > 0.0 {
            lp.hold -= dt;
            lp.turning = lp.hold > 0.0;
        }
        let most = lp.most_turn();
        if lp.turning {
            lp.turn = (lp.turn + dt * TURN_SPEED).min(most);
            if lp.turn >= 1.0 {
                log::info!("picked {} open", lp.lock);
                // Lockpicking trains by the lock's difficulty.
                let setting = [
                    ("fSkillUsageLockPickVeryEasy", 2.0),
                    ("fSkillUsageLockPickEasy", 3.0),
                    ("fSkillUsageLockPickAverage", 5.0),
                    ("fSkillUsageLockPickHard", 8.0),
                    ("fSkillUsageLockPickVeryHard", 13.0),
                ][difficulty(lp.level) - 1];
                let xp = crate::ai::combat::gmst_f32(&self.lo, setting.0, setting.1);
                self.use_skill(esp::actor_value::LOCKPICKING, xp);
                self.set_locked(lp.lock, false);
                self.player_unlocked(lp.lock);
                self.menu = None;
                let name = self.form_name(lp.then);
                self.look_target = Some((lp.then, name));
                if let Err(e) = self.activate() {
                    log::error!("activation failed: {e:#}");
                }
                return;
            }
            if lp.turn >= most {
                lp.strain += dt;
                if lp.strain >= lp.durability {
                    self.remove_item(PLAYER_REF, LOCKPICK, 1, None);
                    lp.strain = 0.0;
                    lp.turn = 0.0;
                    lp.hold = 0.0;
                    lp.turning = false;
                    let left = self.item_count(PLAYER_REF, LOCKPICK);
                    log::info!("lockpick broke ({left} left)");
                    let xp =
                        crate::ai::combat::gmst_f32(&self.lo, "fSkillUsageLockPickBroken", 0.25);
                    self.use_skill(esp::actor_value::LOCKPICKING, xp);
                    if left <= 0 {
                        let msg = self
                            .gmst_string("sOutOfLockpicks")
                            .unwrap_or_else(|| "You have no lockpicks".into());
                        self.scripts.notify(msg);
                        self.menu = None;
                        return;
                    }
                }
            }
        } else {
            lp.turn = (lp.turn - dt * TURN_SPEED * 2.0).max(0.0);
        }
        self.lockpick = Some(lp);
    }

    /// A string game setting, from the localised strings.
    pub(crate) fn gmst_string(&self, name: &str) -> Option<String> {
        let rec = self.lo.get(self.lo.find_editor_id(name)?)?;
        let s = self.lo.lstring(&rec, rec.get(b"DATA")?);
        (!s.is_empty()).then_some(s)
    }

    /// The doors of an actor's home: the load doors of the interior where its
    /// sleep package that locks doors puts it, and the doors on the far side of
    /// them, those of them with locks.
    fn home_doors(&mut self, achr: FormId, npc: FormId) -> Vec<FormId> {
        let Some(cell) = self.home_cell(achr, npc) else {
            return Vec::new();
        };
        let Some(idx) = self.lo.cell(cell) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for &r in idx.persistent.iter().chain(&idx.temporary) {
            let Some(rec) = self.lo.get(r) else { continue };
            let rf = records::reference(&rec);
            let Some((partner, _, _)) = rf.teleport else {
                continue;
            };
            for d in [r, partner] {
                if self.lo.get(d).is_some_and(|rec| rec.get(b"XLOC").is_some()) && !out.contains(&d)
                {
                    out.push(d);
                }
            }
        }
        out
    }

    /// An actor's home: the interior where its sleep package that locks doors
    /// puts it.
    fn home_cell(&mut self, achr: FormId, npc: FormId) -> Option<FormId> {
        let packages = self.npc_packages_cached(achr, npc);
        // Sleeping near its editor location (or elsewhere unnamed): its own cell.
        let editor = self.lo.cell_of_ref(achr).map(Place::Interior);
        let place = packages
            .iter()
            .find(|p| p.lock_doors)
            .and_then(|p| self.package_place(achr, p).map(|p| p.0).or(editor));
        match place {
            Some(Place::Interior(c)) if self.lo.cell(c).is_some_and(|i| i.world.is_none()) => {
                Some(c)
            }
            _ => None,
        }
    }

    /// Lock and unlock homes as actors' packages change: unlocking when a package
    /// that unlocks doors starts or one that unlocks them on change ends, then
    /// locking as sleepers who lock their doors turn in (so a household stays
    /// locked while any of it sleeps).
    pub(crate) fn package_door_locks(
        &mut self,
        before: &std::collections::HashMap<FormId, Option<FormId>>,
    ) {
        let changed: Vec<(FormId, Option<FormId>, Option<FormId>)> = self
            .whereabouts
            .package
            .iter()
            .filter(|(a, now)| before.get(*a) != Some(*now))
            .map(|(&a, &now)| (a, before.get(&a).copied().flatten(), now))
            .collect();
        let mut unlock = Vec::new();
        let mut lock = Vec::new();
        for (achr, old, new) in changed {
            let Some(npc) = self.lo.get(achr).map(|rec| records::reference(&rec).base) else {
                continue;
            };
            let packages = self.actor_packages(achr, npc);
            let find = |id: Option<FormId>| id.and_then(|id| packages.iter().find(|p| p.id == id));
            let (old, new) = (find(old), find(new));
            let opens =
                old.is_some_and(|p| p.unlock_on_change) || new.is_some_and(|p| p.unlock_at_start);
            let shuts = new.is_some_and(|p| p.lock_doors);
            // Its home is private while it runs a package that locks doors.
            let home = if shuts {
                self.home_cell(achr, npc)
            } else {
                None
            };
            match home {
                Some(c) => self.crime.private_homes.insert(achr, c),
                None => self.crime.private_homes.remove(&achr),
            };
            if !opens && !shuts {
                continue;
            }
            let doors = self.home_doors(achr, npc);
            if shuts {
                lock.extend(doors);
            } else {
                unlock.extend(doors);
            }
        }
        for d in unlock {
            self.set_locked(d, false);
        }
        for d in lock {
            self.set_locked(d, true);
        }
    }
}

/// The lockpick (`MISC`) and the lockpicking skill's place in an NPC's skills.
pub const LOCKPICK: FormId = FormId(0xA);
/// Share of a full turn the lock makes in a second (tuned by eye).
const TURN_SPEED: f32 = 1.6;

/// A lock being picked.
#[derive(Debug, Clone)]
pub struct Lockpick {
    pub lock: FormId,
    pub then: FormId,
    pub level: u8,
    /// Degrees from straight up (-90 left .. 90 right): the sweet spot's middle and
    /// width, the partial zones' width either side.
    pub centre: f32,
    pub sweet: f32,
    pub partial: f32,
    /// Seconds a pick takes strain before it breaks, and the strain so far.
    pub durability: f32,
    pub strain: f32,
    /// Where the pick is (degrees, as `centre`), how far the lock has turned
    /// (0 .. 1 open), whether the player turns it, and seconds a test turn lasts.
    pub pick: f32,
    pub turn: f32,
    pub turning: bool,
    pub hold: f32,
}

impl Lockpick {
    /// How far the lock turns with the pick where it is: all the way in the sweet
    /// spot, less and less across the partial zones, barely at all outside.
    pub fn most_turn(&self) -> f32 {
        let off = (self.pick - self.centre).abs() - self.sweet / 2.0;
        if off <= 0.0 {
            1.0
        } else if off < self.partial {
            (1.0 - off / self.partial).max(MIN_TURN)
        } else {
            MIN_TURN
        }
    }
}

/// How far a lock turns with the pick nowhere near the spot.
const MIN_TURN: f32 = 0.05;

/// Lock difficulty 1 (novice) .. 5 (master).
fn difficulty(level: u8) -> usize {
    match level {
        0..=1 => 1,
        2..=25 => 2,
        26..=50 => 3,
        51..=75 => 4,
        _ => 5,
    }
}

/// The game setting naming a lock level.
pub fn lock_level_setting(level: u8) -> &'static str {
    match level {
        Lock::NEEDS_KEY => "sLockLevelNameImpossible",
        0..=1 => "sLockLevelNameVeryEasy",
        2..=25 => "sLockLevelNameEasy",
        26..=50 => "sLockLevelNameAverage",
        51..=75 => "sLockLevelNameHard",
        _ => "sLockLevelNameVeryHard",
    }
}
