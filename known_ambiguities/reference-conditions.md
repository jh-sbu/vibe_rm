# Reference-state condition functions

Implemented in `src/condition/mod.rs` on the engine's state: `GetDestroyed`
(203), `GetIsObjectType` (432), `IsWeaponInList` (398), `GetDefaultOpen` (215)
and `IsUnlockedDoor` (718). Engine helpers: `is_destroyed`
(`src/world/destruction.rs`), `weapon_of` (`src/ai/combat.rs`), `is_locked`
(`src/locks.rs`). Tests: `src/condition_reference_tests.rs` and
`get_destroyed_follows_the_destroy_stage_of_the_reference` in
`src/destruction_tests.rs`.

## What is known

- `GetIsObjectType`'s parameter is xEdit's `wbFormTypeEnum` (TES5Edit's
  `Core/wbDefinitionsTES5.pas`, `wbFormTypeEnum`, used by `ptFormType`), not
  CommonLibSSE's `RE::FormType`. The vanilla data agrees: the raw values are 1
  (armor, the dialogue about dropped armor), 12 (weapons, the "that's my sword"
  dialogue), 6 (light, the torch idle) and 13 (actor, the monk scene quests). In
  CommonLibSSE's numbering those would be 26 (ARMO), 41 (WEAP), 31 (LIGH) and 43
  (NPC_). The table in `form_type_tag` maps the enum to record types; its names
  are xEdit's.
- `GetDefaultOpen` is documented as "Returns 1 if this object is open by
  default" (`Skyrim:Console`, oldid 3628513; `Skyrim Mod:Mod File Format/Raw
  Function Data`, oldid 3411473). UESP's REFR page (oldid 2354868) says the
  zero-length `ONAM` subrecord, which comes with `XACT` = 0x0d, appears when
  "Open by Default" is set. The engine reads that marker on the placed record.
- `GetDestroyed` has no description on UESP. The destructible stage flags are
  documented on UESP's DEST field page (oldid 2334527): 0x04 "Destroy Object".
  CommonLibSSE's `TESForm` flags name `kDestroyed` (1 << 23) without saying what
  sets it.
- `IsUnlockedDoor` has no description on UESP. Vanilla uses it only in three
  player perks' conditions (`PlayerWerewolfFeed`, `DLC1PlayerWerewolfSavageFeeding`,
  `DLC1VampireActivationBlocker`), on the subject, which is not a door.

## Unconfirmed choices

- `GetDestroyed` is true when the reference's current stage, or any stage
  before it in the authored order, has the `DSTD` 0x04 flag. Damage can carry on
  through later stages, so a destroyed reference that takes more damage stays
  destroyed. Whether the game sets its destroyed state from the flag, or from
  something else, is not documented. Vanilla uses it only as `GetDestroyed == 0`
  in the `CWPatrolAndGuardContestedHold` packages (no destructible), so the choice
  doesn't change any vanilla result we can see. `SetDestroyed` (204) is not
  implemented, so a script can't set this state yet.
- `GetIsObjectType` compares the base object's record type, and a subject with
  no base (or a base whose type the table doesn't list) is not that type.
  Vanilla never passes a value the table omits.
- `IsWeaponInList` tests the weapon in the right hand (`weapon_of`, the weapon
  the actor's swings use). Whether a weapon in the left hand counts is
  unconfirmed; the vanilla uses (`BYOHAdoption` packages) don't tell.
- `IsUnlockedDoor` is true for a door with no lock or with a lock that is now
  open (`is_locked` is false; scripts' locks and unlocks count). It is false for
  anything that isn't a door, so the werewolf feeding perks' `== 0` checks pass
  for the player. Whether the game treats a door opened by a key or a pick
  differently from one that was never locked is not documented.
- `GetDefaultOpen` is false for references the engine made itself, which have no
  record to carry `ONAM`. Placed references follow their record. `SetDefaultOpen`
  (216) is not implemented.
