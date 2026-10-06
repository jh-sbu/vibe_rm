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
            Some(&level) => Some(Lock { level, ..own.unwrap_or(Lock { level, key: None, leveled: false }) }),
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
    pub(crate) fn door_lock_for_player(&self, door: FormId, partner: Option<FormId>) -> Option<FormId> {
        let Some(partner) = partner else { return self.is_locked(door).then_some(door) };
        let outside = |r: FormId| self.lo.cell_of_ref(r).and_then(|c| self.lo.cell(c)).is_some_and(|i| i.world.is_some());
        if outside(partner) && !outside(door) {
            return None;
        }
        [door, partner].into_iter().find(|&r| self.is_locked(r))
    }

    /// The player activating a locked door or container: the key opens it for good;
    /// otherwise it stays shut (there's no lockpicking yet). True when it opens.
    pub(crate) fn player_unlock(&mut self, r: FormId) -> bool {
        if !self.is_locked(r) {
            return true;
        }
        let lock = self.lock_of(r).unwrap_or(Lock { level: 0, key: None, leveled: false });
        if let Some(key) = lock.key.filter(|&k| self.item_count(PLAYER_REF, k) > 0) {
            self.set_locked(r, false);
            log::info!("{r} opened with {key} ({})", self.form_name(key));
            return true;
        }
        let msg = if lock.level == Lock::NEEDS_KEY {
            self.gmst_string("sImpossibleLock").unwrap_or_else(|| "Requires key".into())
        } else {
            let level = self.gmst_string(lock_level_setting(lock.level)).unwrap_or_default();
            format!("{} ({level})", self.gmst_string("sLocked").unwrap_or_else(|| "Locked".into()))
        };
        self.scripts.notify(msg);
        false
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
        let packages = self.npc_packages_cached(npc);
        // Sleeping near its editor location (or elsewhere unnamed): its own cell.
        let editor = self.lo.cell_of_ref(achr).map(Place::Interior);
        let place = packages.iter().find(|p| p.lock_doors).and_then(|p| self.package_place(achr, p).map(|p| p.0).or(editor));
        let Some(Place::Interior(cell)) = place.filter(|p| matches!(p, Place::Interior(c) if self.lo.cell(*c).is_some_and(|i| i.world.is_none()))) else {
            return Vec::new();
        };
        let Some(idx) = self.lo.cell(cell) else { return Vec::new() };
        let mut out = Vec::new();
        for &r in idx.persistent.iter().chain(&idx.temporary) {
            let Some(rec) = self.lo.get(r) else { continue };
            let rf = records::reference(&rec);
            let Some((partner, _, _)) = rf.teleport else { continue };
            for d in [r, partner] {
                if self.lo.get(d).is_some_and(|rec| rec.get(b"XLOC").is_some()) && !out.contains(&d) {
                    out.push(d);
                }
            }
        }
        out
    }

    /// Lock and unlock homes as actors' packages change: unlocking when a package
    /// that unlocks doors starts or one that unlocks them on change ends, then
    /// locking as sleepers who lock their doors turn in (so a household stays
    /// locked while any of it sleeps).
    pub(crate) fn package_door_locks(&mut self, before: &std::collections::HashMap<FormId, Option<FormId>>) {
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
            let Some(npc) = self.lo.get(achr).map(|rec| records::reference(&rec).base) else { continue };
            let packages = self.npc_packages_cached(npc);
            let find = |id: Option<FormId>| id.and_then(|id| packages.iter().find(|p| p.id == id));
            let (old, new) = (find(old), find(new));
            let opens = old.is_some_and(|p| p.unlock_on_change) || new.is_some_and(|p| p.unlock_at_start);
            let shuts = new.is_some_and(|p| p.lock_doors);
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
