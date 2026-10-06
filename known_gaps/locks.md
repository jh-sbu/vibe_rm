# Locks, keys and locking homes

Implemented in `src/locks.rs`. Based on public documentation (UESP file formats
and location pages, CK wiki descriptions) and the game's data files.

## What is known

- `XLOC` on a door or container reference (UESP *Mod File Format/REFR*): level
  (0 / 1 novice, 25 apprentice, 50 adept, 75 expert, 100 master, 255 requires key),
  three unknown bytes, the key (`KEYM`), flags (0x04 leveled), padding.
  `vrm-tool locks` tallies them: about 1,500 locked references in the base game and DLC.
- On a load door pair, the lock sits on one side only, often the interior side
  (Sven and Hilde's house, the Riverwood Trader).
- Package `PKDT` flags 0x40 "At Package Start Unlock Doors" and 0x80 "On Package
  Change Unlock Doors" (UESP *Mod File Format/PACK*). The Sleep templates have
  "Lock Doors?" and "Warn Before Locking?" inputs.
- UESP location pages: shops are open (unlocked) by day and locked at night; some
  homes are locked all the time (Sven and Hilde's house, novice lock, key carried by
  both).

## What the implementation does

- A reference with `XLOC` starts locked. Scripts (`Lock`, `SetLockLevel`), keys and
  packages change that; `IsLocked`, `GetLockLevel`, conditions `GetLocked` /
  `GetLockLevel` read it.
- A load door pair shares its lock, but the player can always leave an interior
  into the open.
- The player opens a locked door or container with its key in the inventory (it
  stays unlocked). Otherwise a notification names the lock level.
- Each whereabouts refresh (every 20 s, for every persistent actor) compares
  actors' packages with the previous refresh. When a package with 0x40 starts or
  one with 0x80 ends, the actor's home doors unlock; when a sleep package with
  "Lock Doors?" starts, they lock (locks applied after unlocks).
- Home doors: the load doors (both sides, those with a lock) of the interior where
  the actor's lock-doors sleep package puts it (or its editor cell).

## Gaps

1. **Lockpicking.** No minigame, lockpicks, skill or perks; without the key the
   door stays shut. Leveled locks use their authored level.
2. **Which doors a package locks.** "Home" is a guess: the sleep location's cell.
   The game may use ownership (`XOWN` on doors and cells), the location
   (`LCTN`), or the package's own location.
3. **Locked pairs.** Whether the game shares a lock across a load door pair, and
   whether one can always leave from inside, is inferred from where the data puts
   locks and from play reports.
4. **Warn before locking.** Sleepers don't ask the player to leave; the player
   isn't put out.
5. **Timing.** Locks change on whereabouts refreshes (20 s real time), not
   exactly when a package starts.
6. **NPCs.** NPCs pass through locked doors whatever keys they carry; `Lock`'s
   "as owner" argument is ignored; no trespassing or crime for opening a lock.
7. **Animated doors.** NPCs open locked animated (non-load) doors as they walk
   through.
