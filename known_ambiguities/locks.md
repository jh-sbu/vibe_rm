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
- A load door pair shares one lock, held by the side with `XLOC` (the lower
  form id when both or neither have one): either side reports it and `Lock`,
  `SetLockLevel`, keys, packages and the console's `lock` / `unlock` change it
  for both. The player can always leave an interior into the open.
- The player opens a locked door or container with its key in the inventory (it
  stays unlocked). Without it, a lock that doesn't need the key is picked, if the
  player has lockpicks: the sweet spot is placed at random, the lock turns towards
  the most the pick's place allows (`most_turn`: linear across the partial zones,
  5% outside), and strain at less than a full turn wears out the pick. Picked open,
  the door or container opens. The skill is the player record's (`NPC_` `DNAM`).
- A loaded actor's home doors follow its package as its AI switches to it;
  other persistent actors' follow each whereabouts refresh (every 20 s), which
  compares their packages with the last ones their doors followed. When a
  package with 0x40 starts, or one with 0x80 or "Lock Doors?" ends, the
  actor's home doors unlock, unless another actor whose home they are is in
  a package that locks them; when a sleep package with "Lock Doors?" starts,
  they lock (locks applied after unlocks). That a lock-doors package's end
  opens the home is settled (`test-mod autotest11`: Arcadia's shop, she
  unloaded in it, was locked exactly while her sleep package ran, changing
  as her package switched, whether the clock ran past 0:00 / 8:00 or
  jumped). The two evaluations can pick differently (the
  refresh doesn't see loaded state); each actor's doors follow only one of
  them at a time, so they don't flip between them, but loading or unloading
  can fire a change. The refresh is also due at each half hour of game time,
  so unloaded actors' doors follow their packages as they start (packages
  start on the hour or half hour; unconfirmed: whether the game evaluates
  unloaded actors at their start times or at some coarser cadence).
- Animated (non-load) doors open for walkers who get past their lock: with the
  key, or owning it (the door's `XOWN`, else its cell's: the actor or one of its
  factions). So cages, cells and gates in dungeons and prisons, and the player's
  rented room, stay shut; Companions open their own rooms.
- Load doors don't stop actors: whereabouts move them through locked ones as
  their packages say (see Falk above).
- Home doors: the load doors (both sides, those with a lock) of the interior where
  the actor's lock-doors sleep package puts it (or its editor cell).

## Gaps

### Quest-alias factions and inventory

Filled reference aliases now contribute their `ALFC` factions to the actor's
membership while the quest runs. Shared queries expose them to owned-door
access, player ownership checks, `GetInFaction`, `GetFactionRank`, and other
existing faction consumers. Clearing/refilling an alias or stopping its quest
removes only that contribution; another filled alias can still supply it.
The actor's base factions and scripted ranks are not rewritten. Existing quest
and alias save state reconstructs these memberships after load; no separate
membership state is saved. Direct alias inventory items/keys are also supported;
form-list and leveled-list grants remain pending.

The local [UESP QUST reference, revision 3507775](https://en.uesp.net/w/index.php?oldid=3507775)
documents `ALFC` as repeated faction FormIDs on reference aliases, granted for
the duration of the quest (lines 183 and 352–354). This is public file-format
documentation, not an observed SE lifecycle test. Parsed FormIDs use the quest's
load-order remapping. Null, truncated and duplicate faction entries are ignored;
location aliases do not grant actor factions.

Unresolved rank/conflict policies: `ALFC` encodes no rank in that documented
schema. This implementation grants rank 0, preserves higher base ranks, and
lets explicit script ranks (including removal at -1) take precedence. Alias-only
factions are appended in FormID order for deterministic queries. Whether the
original handles these conflicts, multiple alias-supplied vendor factions,
completed-but-running quests or partially filled startup conditions identically
needs public evidence or game-side observation. Queries follow current fills;
they do not synthesize faction-change events or other activation callbacks.
Synthetic tests exercise quest start/stop, native alias clear/refill, independent
references, CTDA membership, door access, script ranks, player identity and save
restoration. Only public documentation and invented records were used; no engine
binary data or game assets were examined or added for this extension.

Reference-alias `CNTO` entries are granted to the reference's normal inventory
after a successful quest fill or a new `ForceRefTo` assignment. The grant stays
with the reference after `Clear`, reassignment, quest stop, or quest restart; the
same-reference `ForceRefTo` is treated as no new placement and is idempotent.
Clearing and filling again is a new placement, so its permanent grant is added
again. Failed startup applies no grants, and native load restores the saved
inventory and alias state without replaying them. Location aliases, invalid
item forms, and targets other than actors, the player, or containers are
ignored. The runtime currently grants direct inventory items; form lists and
leveled lists are outside this implementation. Counts above `i32::MAX` are
rejected because authored counts are unsigned but runtime counts are signed.
Before granting, the count is capped at the remaining `i32` capacity for that
item, preventing unchecked inventory addition from overflowing. This
representation bound is not a documented Creation Kit limit.

The local [UESP QUST reference, revision 3507775](https://en.uesp.net/w/index.php?oldid=3507775)
says alias inventory is added for the duration of the quest (lines 328–340).
The public [Creation Kit Quest Alias page mirror](https://tesck.ru/index.php/Вкладка_Quest_Alias)
preserves English wording that the inventory grant is permanent and is not
removed when a reference leaves the alias (Alias Data, Inventory; mirror of the
CK page whose original returned HTTP 403). These sources conflict. The
implementation follows the more specific CK alias-tab description, while
removal/transfer edge cases and whether the original re-grants after a clear and
refill remain unverified in Skyrim SE. The tested same-reference force policy is
an implementation choice. Synthetic tests cover startup failure, key use,
malformed and oversized counts, location aliases, failed item lookup, forced
fills, clear/refill, stop/restart, save restoration, and count saturation.

### Scripted owner locking

`ObjectReference.Lock(abLock, abAsOwner)` now honors `abAsOwner`: unlocking a
load door as owner makes its adjoining interior public, and locking makes it
private. Trespass checks read this override before the authored Public Area
flag; a private owned room no longer needs a scheduled sleep package to count
as private. Ordinary calls and non-door references only change the lock.
The overrides persist in an optional `CPUB` native-save section; older saves
without it retain authored/schedule-based access.

The public [Papyrus Index Lock documentation](https://papyrus.bellcube.dev/skyrimse/script/objectreference/function/lock/)
reproduces the CK parameter description and the shipped script's documentation
comment: owner-mode locking changes adjoining-cell access. The separate CK page
returned HTTP 403. The local UESP index had no Lock/ObjectReference title match.
No game assets or engine binaries were examined for this extension.

Implementation choices still needing Skyrim SE observation: an exterior/interior
pair targets the interior from either half; between two interiors it targets the
destination cell. Non-teleport doors have no adjoining cell in the current data
model, so only their lock changes. Interactions among several entrances to one
cell, the exact relationship with Off Limits, and package-driven access changes
after an explicit override remain unresolved. Existing lock creation and open-door
closing behavior are separate gaps; this extension handles the owner argument.
Synthetic tests validate native defaults, door/container discrimination, access
changes, save restoration, old saves and malformed sections, not engine parity.

0. **`XRNK`, the faction rank an owner asks.** Unconfirmed: an owned
   reference's (or its cell's) `XRNK` is read as the lowest rank in the
   owning faction that may use its things freely (xEdit names it the owner's
   faction rank; no public source says what the game does with it). A
   player (or, for an animated door's lock, an actor) in the faction below
   that rank is treated as outside it: taking is stealing, picking a lock is
   a crime, beds and cells are someone else's. -1 or no field asks nothing.
   An owner a script set (`SetFactionOwner`) carries no rank. Ranks come
   from the records' faction lists only.

1. **Lockpicking.** How the turn falls off across the partial zone, the turning
   speed, and whether strain carries over between attempts with one pick are
   guesses. Leveled locks use their authored level. Locksmith's "Set
   Lockpick Starting Arc" (45) is read as the pick starting somewhere in a
   45° arc around the sweet spot's middle (UESP: "starts close to the lock
   opening position"); without it the pick starts straight up. Quick Hands'
   "Mod Lockpicking Crime Chance" (set to 0) is taken as the chance (from 1)
   that picking an owned lock is a crime at all.
2. **Which doors a package locks.** "Home" is a guess: the sleep location's cell.
   The game may use ownership (`XOWN` on doors and cells), the location
   (`LCTN`), or the package's own location.
   **When the owner is away or the player inside**, the game departs from
   "locked while the lock-doors package runs" (`test-mod autotest11`,
   Belethor, 2026-10-22), and the engine doesn't follow it:
   - Belethor out in the city when the test began (sleep package, doors
     locked) and through the night: his doors stayed locked all day, through
     his sleep's end, his sell package and his being moved into the shop
     (UESP *Belethor* notes the shop often not reopening as a bug). Perhaps
     a package only locks the home with its owner there and only unlocks
     what it locked, or unlocking waits on the owner reaching its location.
   - The player inside (sneaking, unseen): his doors unlocked about 13 game
     minutes after 8:00 (he got out of bed; when his package switched isn't
     known), and his sleep at 0:00 and at a jump to 3:00 didn't lock them
     again. Perhaps sleepers don't lock a home with the player in it
     (the Sleep templates' "Warn Before Locking?").
     He also said "You need to leave" and the like while the sneak eye
     showed the player undetected, with no bounty; whether those voiced
     warnings are what "Warn Before Locking?" drives isn't known.
   `test-mod autotest12` tests both on Arcadia: sent to Riverwood as her
   sleep starts, then home, then waking to a lock the script made; and the
   player inside her shop through 0:00 (sneaking, then in plain view),
   then moved out, with her detection of the player and the bounty.
3. **Leaving from inside.** That one can always leave an interior through a
   locked pair is inferred from where the data puts locks and from play
   reports. (That the pair shares one lock is settled: Sven and Hilde's
   outside door, with no lock of its own, reports the inside one's state and
   follows `Lock` on either side; `test-mod autotest`.)
4. **NPCs and load doors.** What lets NPCs through locked load doors isn't known:
   whether every package can, only packages naming a place inside, or something
   in the doors' or quests' data.
