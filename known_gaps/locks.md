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
- Lockpicking (UESP *Skyrim:Lockpicking*): the pick moves along a 180° arc; the
  sweet spot is `60 × 2^-d × (0.82 + 0.6 × skill / 100)` degrees (d 1 novice .. 5
  master), partial zones either side `(26 − 4d) × (0.775 + 1.5 × skill / 100)`
  degrees, where the lock turns part way, more nearer the spot. A pick breaks
  after 2 / 1 / 0.75 / 0.5 / 0.25 s of strain, × (1 + 0.5 × skill / 100). The
  settings agree: `fLockpickSkillSweetSpotMult` 0.006, `fPartialPick<Level>`
  22 / 18 / 14 / 10 / 6, `fLockpickSkillPartialPickBase` 0.775 and `Mult` 0.015.
- Who owns locked load doors (`vrm-tool locks`): most belong, on one side or the
  other, to a faction (the household or shop's) by the door's own `XOWN` or its
  cell's; a few to an NPC; 57 to nobody.
- Packages take NPCs through locked load doors they neither own nor have the key
  to: Falk Firebeard's `FalkBrylingSecretMeeting2x2` puts him in Bryling's house
  (locked at night, owned by `SolitudeBrylingsHouseFaction`, which he isn't in, key
  `SolitudeBrylingsHouseKey`, which he doesn't carry) from 2 to 4 at night, and
  UESP describes the visit. Severio Pelagia's `WhiterunSeverioDrunkenHuntsman20x3`
  names Belethor's General Goods, shut at 8 in the evening.
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
  stays unlocked). Without it, a lock that doesn't need the key is picked, if the
  player has lockpicks: the sweet spot is placed at random, the lock turns towards
  the most the pick's place allows (`most_turn`: linear across the partial zones,
  5% outside), and strain at less than a full turn wears out the pick. Picked open,
  the door or container opens. The skill is the player record's (`NPC_` `DNAM`).
- Each whereabouts refresh (every 20 s, for every persistent actor) compares
  actors' packages with the previous refresh. When a package with 0x40 starts or
  one with 0x80 ends, the actor's home doors unlock; when a sleep package with
  "Lock Doors?" starts, they lock (locks applied after unlocks).
- Animated (non-load) doors open for walkers who get past their lock: with the
  key, or owning it (the door's `XOWN`, else its cell's: the actor or one of its
  factions). So cages, cells and gates in dungeons and prisons, and the player's
  rented room, stay shut; Companions open their own rooms.
- Load doors don't stop actors: whereabouts move them through locked ones as
  their packages say (see Falk above).
- Home doors: the load doors (both sides, those with a lock) of the interior where
  the actor's lock-doors sleep package puts it (or its editor cell).

## Gaps

1. **Lockpicking.** No perks, skill gain (`fSkillUsageLockPick*`,
   `iXPRewardPickLock*`), enchantments or potions; no race skill bonuses in the
   skill used. How the turn falls off across the partial zone, the turning speed,
   whether strain carries over between attempts with one pick, and the "Auto
   Attempt" option are guesses or missing. Leveled locks use their authored level.
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
6. **NPCs and load doors.** What lets NPCs through locked load doors isn't known:
   whether every package can, only packages naming a place inside, or something
   in the doors' or quests' data. Ownership ranks (`XRNK`) are ignored, and keys
   or factions that quest aliases give aren't counted. `Lock`'s "as owner"
   argument is ignored; no trespassing or crime for opening a lock.
7. **Animated doors.** The navmesh doesn't know a door is locked: actors walk up to
   a locked door they can't open and push against it instead of going round.
