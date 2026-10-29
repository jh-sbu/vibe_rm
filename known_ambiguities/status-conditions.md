# Platform, race, item and ownership status conditions

Implemented in `src/condition/mod.rs` (`function_value`); tests in
`src/condition/status_tests.rs`.

## Platform and gore

`IsWin32` (521), `IsXBox` (309), `IsPS3` (520) and `IsGoreDisabled` (547).

### What is known

- UESP Skyrim:Console (oldid 3628513, https://en.uesp.net/w/index.php?oldid=3628513):
  `IsWin32` "Returns true if the game is running on a Windows environment",
  `IsXBox` and `IsPS3` the same for their platforms.

### Unconfirmed choices

- This is a PC build, so `IsWin32` holds and the console checks fail. Whether
  Skyrim SE on Windows reports `IsWin32` true is documented only by the
  console wiki's description of the function, not observed.
- `IsGoreDisabled` has no description on UESP. Nothing here sets a gore
  setting, so it is always false (gore shown).

## GetIsPlayableRace

`GetIsPlayableRace` (254) is true when the subject's race has the `RACE` `DATA`
flag 0x1 ("Playable" in xEdit's `wbRACE`).

### What is known

- UESP Skyrim Mod:Mod File Format/Raw Function Data (oldid 3411473) gives
  `Ref.GetIsPlayableRace()`; no page describes its result.

### Unconfirmed choices

- The race is the one templates give (`race_of`, as `SameRace` reads it), not
  the base record's own `RNAM` as `GetIsRace` does. They differ only for
  template-inheriting NPCs.
- A subject with no race (not an actor) fails the condition.

## GetKeywordItemCount

`GetKeywordItemCount` (465) totals the counts of every carried item whose base
record lists the keyword (`KWDA`).

### What is known

- UESP Skyrim Mod:Mod File Format/Raw Function Data (oldid 3411473) gives
  `Ref.GetKeywordItemCount(Keyword)`; its result is not described.

### Unconfirmed choices

- It counts stacks, not stack kinds: two keyworded swords and three keyworded
  arrows are 5. The name and `GetItemCount` suggest this, nothing confirms it.
- Equipped items are carried items and count; so do items in a reference's
  starting inventory before it changes, as `GetItemCount` reads them.
- Whether a keyword list, rather than one keyword, is accepted is unknown; the
  parameter is a single keyword.

## HasTwoHandedWeaponEquipped and HasBoundWeaponEquipped

`HasTwoHandedWeaponEquipped` (636) and `HasBoundWeaponEquipped` (706, one
casting-source parameter).

### What is known

- UESP Skyrim:Console (oldid 3628513) lists both without descriptions; the
  raw function data (oldid 3411473) gives `Ref.HasTwoHandedWeaponEquipped()`.
- The parameter of `HasBoundWeaponEquipped` is xEdit's `ptCastingSource`
  (0 left hand, 1 right hand, 2 voice, as CommonLibSSE's `CastingSource`).
- Bound weapons are `WEAP` `DNAM` flags2 bit 0x2000 (xEdit's "Bound Weapon").

### Unconfirmed choices

- "Two-handed" is a weapon in the right hand whose animation type is
  greatsword or battleaxe / warhammer (5, 6). Whether bows, crossbows or
  staves count is unknown; they are left out because they are not melee
  two-handers and the animation types are named separately.
- A two-handed weapon is the right-hand item (`weapon_in_hand`, where the
  game places it), whether drawn or sheathed: "equipped" is not "drawn".
- `HasBoundWeaponEquipped` fails for the voice source and for any value other
  than 0-2. The hand is the engine's held slot, so a bound weapon the Bound
  archetype is still conjuring counts once it is in the hand.

## IsOwner, IsCellOwner and IsInMyOwnedCell

`IsOwner` (278, an owner), `IsCellOwner` (280, a cell and an owner) and
`IsInMyOwnedCell` (146).

### What is known

- UESP Skyrim Mod:Mod File Format/Raw Function Data (oldid 3411473) gives
  `Ref.IsOwner(Owner(optional))`, `Ref.IsCellOwner(Cell, Owner(optional))` and
  `Ref.IsInMyOwnedCell()`; the console list (Skyrim:Console, oldid 3628513)
  describes `IsOwner` and `IsCellOwner` as "get the ownership of the ref". The
  matching setters (`SetOwnership`, `SetCellOwnership`) take the same optional
  owner.
- An owner is the `XOWN` of the reference, else of its cell (`ref_owner`,
  shared with theft and trespass), or what a script's `SetActorOwner` /
  `SetFactionOwner` set.

### Unconfirmed choices

- The Oblivion and Morrowind forms of these functions ask whether the *calling
  actor* owns the thing; the Skyrim description reads as a question about the
  reference's own owner. This follows the Skyrim text: `IsOwner(x)` is true when
  the subject reference's owner is `x`; `IsCellOwner(cell, x)` when the cell's
  owner is `x`. Vanilla data has not been surveyed for which reading it
  assumes.
- An omitted owner names the player (the optional argument of the setters
  means "the player"): the player's base `0x7` or the player faction.
- Owner comparison is exact: an NPC owner is not matched by a reference of
  that NPC, and a faction owner is not matched by a member.
- `IsInMyOwnedCell` is the actor owning the cell it stands in, directly, by
  base record, or through a faction it is a member of (rank 0 or better). For
  the player only interior cells are known; an exterior cell counts as not
  owned.

## GetXPForNextLevel

`GetXPForNextLevel` (530) is the character XP the player's next level up costs,
`fXPLevelUpBase + fXPLevelUpMult * level` for the level they will be at.

### What is known

- UESP Skyrim:Leveling (oldid 3636185) gives the level-up cost formula and the
  game settings; the function name is listed on Skyrim:Console (oldid 3628513)
  without a description.

### Unconfirmed choices

- The result is the whole cost, not the XP still missing (`cost - level_xp`).
  The name reads either way; the whole cost matches `level_progress`'s
  denominator. Level ups waiting count towards the level used, as the engine's
  own meter does.
- The subject is ignored; only the player levels.

## IsPCSleeping and IsPC1stPerson

`IsPCSleeping` (175) and `IsPC1stPerson` (391), both about the player.

### What is known

- UESP Skyrim:Console (oldid 3628513) describes `IsPC1stPerson` as "Is the
  player playing in 1st person mode"; `IsPCSleeping` has no description.

### Unconfirmed choices

- `IsPCSleeping` holds from the moment the sleep menu's hours start passing
  (the same moment `OnSleepStart` is sent) until the menu closes. Choosing
  hours while only waiting is not sleeping. Whether the original also counts
  the bed's enter and exit animations, or the menu being open before OK, is
  unknown.
- `IsPC1stPerson` is the opposite of the third-person camera. The original's
  other camera states (vanity, free, horse, dragon, furniture) are not
  modelled, so none of them are "first person".

## Location comparisons and loaded locations

`HasSameEditorLocAsRef` (180) and `IsInSameCurrentLocAsRef` (603) (a reference
and a keyword), `GetInCurrentLocFormList` (444), `IsLocationLoaded` (625) and
`IsLocAliasLoaded` (626). The alias forms of the first two were already
supported and share their code.

### What is known

- UESP Skyrim:Console (oldid 3628513) lists all of these without descriptions;
  the raw function data (oldid 3411473) gives the parameters.
- The reference forms use the same comparison as the alias forms: the two
  locations are equal, or their parents carrying the keyword are.

### Unconfirmed choices

- `GetInCurrentLocFormList` is true when the subject's current location is, or
  lies inside, any location on the list. Whether a list entry's parents count
  the other way round is unknown.
- `IsLocationLoaded` is true when any loaded cell's location is, or lies inside,
  the location. The original may mean the location's marker or its persistent
  data being in memory rather than a cell; the exterior cells counted are those
  of the player's worldspace. An unloaded interior never counts.
- A null reference parameter in `HasSameEditorLocAsRef` or
  `IsInSameCurrentLocAsRef` names nothing and fails.

## GetFactionCombatReaction

`GetFactionCombatReaction` (410, two factions) is the group combat reaction the
first faction lists towards the second: 0 neutral, 1 enemy, 2 ally, 3 friend.

### What is known

- UESP Skyrim Mod:Mod File Format/Raw Function Data (oldid 3411473) gives
  `Ref.GetFactionCombatReaction(Faction, Faction)`. The reaction values are the
  `FACT` `XNAM` "Group Combat Reaction" field that the combat code already
  reads (`faction_reaction`).

### Unconfirmed choices

- Only the first faction's `XNAM` list is read; the answer need not be mutual.
  The AI's own faction test takes the strongest reaction between two lists of
  factions in both directions of membership, which this does not.
- A faction towards itself is an ally (2), as the AI treats shared membership;
  a pair with no entry is neutral (0).
- `GetFactionRelation` (449, an actor) stays unsupported: nothing public says
  what it returns.

## Location ref-type counts and alias keyword data

`GetRefTypeAliveCount` (592), `GetRefTypeDeadCount` (591),
`GetLocAliasRefTypeAliveCount` (601), `GetLocAliasRefTypeDeadCount` (600) and
`GetKeywordDataForAlias` (608). Helper: `location_population`
(`src/locations.rs`); tests in `src/condition/status_tests.rs`.

### What is known

- UESP Skyrim Mod:Mod File Format/Raw Function Data (oldid 3411473) gives
  `Location.GetRefTypeAliveCount(LocationRefType)`, `GetRefTypeDeadCount`, the
  `GetLocAlias…` forms taking a location alias, and
  `LocAlias.GetKeywordDataForAlias(Keyword)`; no page describes the results.
- Vanilla uses: 601 appears 81 times, all in quest conditions
  (`vrm-tool ctda-uses`); 592 five times in dialogue; 591, 600 and 608 never.

### Unconfirmed choices

- The population is the references the location lists with that ref type
  (`LCTN` `ACSR` / `LCSR`, and references' own `XLRT`), those of its child
  locations included, as `LocationHasRefType` reads them. Whether the game
  counts child locations is unknown.
- A reference is dead when `is_dead` says so: killed in play, loaded dead, or
  recorded dead. Actors placed dead in the plugin and never loaded count as
  alive. Disabled references count as alive. References that aren't actors
  (a static with a ref type) count as alive.
- An empty location alias counts 0 of either, and its keyword data reads 0.
- `GetKeywordDataForAlias` reads the data `SetKeywordData` stored for the
  aliased location, as `GetKeywordDataForLocation` does.

## Player, clock and package-input conditions

`IsPlayerInRegion` (117), `GetInZone` (445), `MenuMode` (36),
`GetPlayerControlsDisabled` (98), `IsTimePassing` (265), `GetRealHoursPassed`
(716), `GetSecondsPassed` (12) and `IsNullPackageData` (611). Tests in
`src/condition/status_tests.rs` and `src/ai/package_data_tests.rs`.

### What is known

- UESP Skyrim Mod:Mod File Format/Raw Function Data (oldid 3411473) gives the
  signatures: `IsPlayerInRegion(Region)`, `Ref.GetInZone(EncounterZone)`,
  `MenuMode(Integer (Optional))`, `GetPlayerControlsDisabled` with eight
  optional integers, `IsTimePassing()`, `GetRealHoursPassed()`,
  `GetSecondsPassed()`, `Ref.IsNullPackageData(PackageData (Possibly Null))`.
  UESP Skyrim:Console (oldid 3628513) lists the controls as "movement,
  fighting, pov, looking, sneaking, menu, ..." (0/1 each) and describes none
  of the others.
- Vanilla uses (`vrm-tool ctda-uses`): `IsPlayerInRegion` 42 times, all in
  the survival mode's `Survival_RegionInfoSpell` (the region's weather name
  against 1 or 0); `IsNullPackageData` twice, in `PatrolAndHunt`, on the
  package input `HorseMarker` (once against 0, once against 1). The rest are
  never used.
- `GetRealHoursPassed` is the Papyrus `Game.GetRealHoursPassed`, which the
  engine already answers from its real-time clock.

### Unconfirmed choices

- The player's regions are their cell's `XCLR`, the cell being the loaded
  grid's centre for an exterior, as weather reads them (`cell_regions`).
  Regions a cell lies in only through a region's own area data are not
  considered. An interior with no `XCLR` is in none.
- `GetInZone` reads the subject's own `XEZN`, else its cell's. Locations and
  the zone's own location / owner data are not consulted.
- `MenuMode` with no parameter (0) is true while a menu that pauses the game
  is open (the engine's `in_menu_mode`, so dialogue is not menu mode). A
  parameter naming an Oblivion menu type is unsupported: no source maps those
  numbers to Skyrim's menus, and nothing in vanilla calls it.
- `GetPlayerControlsDisabled` has only two parameters in the condition table:
  movement and fighting. Selected controls must all be disabled; with none
  selected it asks whether any control is. Whether the game ands or ors the
  selection, and what no selection means, is not documented.
- `IsTimePassing` is true while the world runs, and while waiting or
  sleeping (the clock advances from inside the menu). The game may treat
  other states (a scene's camera, the dialogue menu) as time not passing.
- `GetSecondsPassed` is the real seconds of the latest update. The Oblivion
  function measured the time since a script last ran; what a condition sees
  is unknown.
- `IsNullPackageData` is 1 when the package has no input of that index, or
  an input with nothing set (a reference or location that couldn't be read);
  a bool, number, or any readable value is not null. Whether an input naming
  a null reference counts as null is unknown.

### Left unsupported

- `GetCurrentAIPackage` (110) and `GetCurrentAIProcedure` (143): no source
  gives the numbers they return. The one vanilla use (`CheckCorpse`) compares
  the procedure against 6, which names no procedure we can find.
- `IsWaiting` (111): UESP lists the name only, and no source says which state
  it reports.

## Record-data and legacy conditions

`GetScale` (24), `GetIsCreature` (64), `GetFactionRelation` (449),
`GetFactionRankDifference` (60), `GetWeaponAnimType` (108), `IsCarryable` (487),
`GetTargetHeight` (720), `GetActivatorHeight` (680), `GetClothingValue` (41),
`GetDisease` (39), `IsActorUsingATorch` (306), `GetInCellParam` (230) and
`GetCannibal` (227). Tests in `src/condition/status_tests.rs`.

### What is known

- UESP Skyrim:Console (oldid 3628513) and Skyrim talk:Console/Archive 1
  (oldid 2308857) list the names; only `GetTargetHeight` has a description:
  "Get the delta on z between two references". The raw function data (oldid
  3411473) gives the signatures, including `GetFactionRankDifference(Faction,
  Actor)`, `GetInCellParam(Cell, ObjectReferenceID)` and
  `GetFactionRelation(Actor)`.
- Vanilla uses (`vrm-tool ctda-uses`):
  - `GetWeaponAnimType` compares against the weapon records' animation types:
    5 and 6 (two-handers) and 1 and 2 in dialogue, 7 (bows) and 12 (the
    crossbow reload idles) in attack-release idles. The values are the
    `WEAP` `DNAM` animation type byte.
  - `GetFactionRelation` is compared with 1 (`!= 1`), `< 2` and `<= 1`,
    with the player as the actor or a null actor, so its result is numbered
    like the group combat reaction (1 enemy, 2 ally).
  - `GetTargetHeight(null)` is within +-20 or +-35 in the killmove idles, so
    its sign matters little there. `GetActivatorHeight` is `<= 40` for the
    low pick-up idles and `>= 41` for the standing ones.
  - `IsCarryable` is asked of the target in pick-up idles (`== 1`).
  - `GetDisease` against 1 on the player (Windhelm's hellos, an ambient quest).
  - `GetClothingValue` is `>= 12` (Delphine in MQ201) and `>= 1000` (a
    surgeon's greeting, DLC1).
  - `GetCannibal` is 1 in the cannibal feeding idle.
- `ActorTypeCreature` is the keyword 0x13795 (checked in Skyrim.esm).

### Unconfirmed choices

- `GetScale` is a loaded actor's scale, else a placed reference's `XSCL`
  (1 with none).
- `GetIsCreature` is a race with `ActorTypeCreature`. Oblivion's function
  asked for a creature record; Skyrim has none.
- `GetFactionRelation` is the strongest reaction (3 friend, 2 ally, 1 enemy,
  0 neutral) of any member faction of the subject towards any of the actor's,
  as combat's hostility reads them; a shared faction is an ally. Ranks below 0
  are not membership. A null actor is taken to be the player (the dialogue
  uses), where the geometry conditions take the combat target.
- `GetFactionRankDifference` is the subject's rank less the actor's, -1 for
  a non-member, so two non-members differ by 0.
- `GetWeaponAnimType` is the animation type of the weapon in the right hand,
  else the left, 0 with none. A spell in hand is not a type here (the idles
  compare it against numbers we can't map to magic).
- `IsCarryable` is an item kind (`ALCH`, `AMMO`, `APPA`, `ARMO`, `BOOK`,
  `INGR`, `KEYM`, `MISC`, `NOTE`, `SCRL`, `SLGM`, `WEAP`) or a light with the
  "can be carried" flag (`DATA` 0x2). Whether the game also refuses some
  of these (quest items, items too heavy) is unknown.
- `GetTargetHeight` is the other reference's z less the subject's; the sign
  is a guess. A null reference is the subject's combat target.
- `GetActivatorHeight` is the z of the context's target (the idle's, or the
  activated reference) less the subject's, in world units. Nothing here picks
  those pick-up idles yet, so no condition context supplies the target.
- `GetClothingValue` is the sum of the base values of worn armor records of
  the clothing type (`BOD2` armor type 2). Oblivion's function may have
  returned something other than gold value; the thresholds above (12, 1000)
  don't settle it.
- `GetDisease` is true while any disease spell (type Disease) has an effect on
  the subject, whatever its source.
- `IsActorUsingATorch` is `IsTorchOut`'s test (a torch in the left hand).
- `GetInCellParam(cell, reference)` is true when the reference named in the
  second parameter is in the cell; whether the game asks about the subject
  instead is unknown, and no vanilla record uses it.
- `GetCannibal` is false: nobody eats corpses yet (as `GetVampireFeed`).

### Left unsupported

No source or vanilla use gives these a Skyrim meaning: `GetIsCreatureType`
(Oblivion's creature types have no Skyrim record field), `GetIsAlignment`,
`GetIsClassDefault`, `GetClassDefaultMatch`, `GetPersuasionNumber`,
`GetTotalPersuasionNumber`, `CanHaveFlames`, `HasFlames` and `IsWaterObject`
("Water condition used for drinking animation"; nothing says what makes an
object one).


## Gait and unconsciousness

`IsRunning` (287) and `GetUnconscious` (242), in `src/condition/mod.rs`
(`function_value`). Engine helper: `actor_running` (`src/ai/mod.rs`). Test in
`src/condition/status_tests.rs`.

### What is known

- UESP Skyrim:Console (oldid 3628513) lists both names; only
  `GetUnconscious` has a description: "Returns 1 if is unconscious". The raw
  function data (oldid 3411473) gives `IsRunning()` with no parameters.

### Unconfirmed choices

- `IsRunning` is true for the player while moving with the run toggle on
  (sprinting included), and for a loaded NPC moving faster than the midpoint
  of its movement type's walk and run speeds, the rule footsteps use. Whether
  the game reads the graph's speed or the movement input is not documented.
- `GetUnconscious` is true while the actor is bleeding out. Skyrim's
  unconscious state (the old Oblivion flag, also used by knock-out) has no
  public description beyond the one line, and no other source of it exists
  here. Nothing in vanilla's conditions calls either function.

## IsRidingMount

`IsRidingMount` (327) reports false while mounted actors are not implemented,
matching the existing vanilla `Actor.IsOnMount` native in this engine.

Evidence: the installed Skyrim.esm `CharacterMoveStart` (000959FA) and
`CharacterMoveStop` (000959FB) both require `IsRidingMount == 0`; the same check
gates humanoid turning. The on-foot behavior graph accepts `moveStart` and
enters `MT_Locomotion_State`. Treating the query as unsupported rejects even
the comparison with zero, leaving a moving player in the standing state.
These are asset and project-engine observations, not Skyrim SE measurements.

Local UESP searches for `IsRidingMount` and `IsOnMount`, including all namespaces,
found no explanatory pages. Choice: answer unmounted explicitly until the
mounted-actor system exists, without relaxing the evaluator's refusal of other
unsupported conditions. Mount transitions and their query timing remain part
of ROADMAP's riding work.

Verification (2026-10-29): the installed-assets player-body regression first
failed because ActionMoveStart resolved to no event. With the explicit unmounted
answer, it enters locomotion at walk/run speeds and returns to standing on stop.
All 9 player-body tests and 31 status-condition tests pass with ignored fixtures
enabled; the workspace suite passes (786 tests, 371 ignored). Offscreen checks
in VibeRM's WhiterunBanneredMare confirm rendered walk/run strides and separate
locomotion/standing graph states. Captures and logs remain under
`/tmp/vibe-player-animation/`; Skyrim SE itself was not run.
