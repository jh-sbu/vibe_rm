# Further PEX natives

These implementations use public community documentation and existing runtime
systems. They have not been compared experimentally with Skyrim SE. References
and source-versus-inference distinctions are in research/pex-natives/batch*.md.

## Batch 1

Alias game timers inherit our Form registration policy: one timer per object and
event, replacing previous registrations. Exact multiple-script registration
interaction and zero/negative/NaN intervals remain unverified. Ingredient
hostility considers all its effects; whether only learned effects matter is not
specified by the terse public native description. Missing string settings return
an empty string; original diagnostic text is unverified.

## Batch 2

Cell attachment uses membership in the runtime loaded-cell map. Precise Skyrim
attachment/detachment phase timing is unverified. IsStopped uses !running because
our quest start/stop transitions are synchronous; Skyrim's intermediate enabled
and stopping states are not represented. GetKey follows the existing shared
load-door lock policy; unusual pairs with two different keys are unverified.
GetVoiceType inherits the existing actor template precedence.

## Batch 3

HasCommonParent compares strict ancestors on both sides. Whether Skyrim considers
one of the two input locations itself a shared parent is unspecified. HasRefType
counts the location's own references, not its children's (Skyrim SE, autotest21:
see query-natives.md); disabled-reference inclusion remains unverified. Reservations
use running quest reference aliases with Reserves Reference, not furniture use.
Malformed parent cycles terminate at the first repeated location.

## Batch 4

GetAllItemsCount counts distinct items, not copies (Skyrim SE, autotest21: 10 gold
counted 1, then 3 with 3 swords and 2 ingots added). Quest items, unplayable items
and gold are counted like any other: unverified. IsContainerEmpty uses the same rule.
WornHasKeyword includes every equipped item in the runtime equipment list. A drawn
sword counts (Skyrim SE, autotest21); hidden-skin inclusion remains unverified.

## Batch 5

GetCurrentPackage returns None for a loaded actor without a selected package;
unloaded actors report the schedule selection. Skyrim's timing and interrupt
package reporting are unverified. GetDialogueTarget uses the scene's explicit
headtracking target while speaking; headtracking and dialogue listeners need not
coincide in all scenes. Implicit scene listeners, idle pauses between lines and
non-scene ambient dialogue are unresolved and return None. `IsOverEncumbered` is
False for NPCs whatever they carry (Skyrim SE, autotest21: 39 against a carry
weight of 0, and 9 against 8.9). For the player it uses our capacity calculation.

## Batch 6

Dimensions are the base's OBND extents, unscaled and independent of rotation
(Skyrim SE, autotest21: a barrel read 54 x 54 x 80 at scale 1, after SetScale(2) and
turned 45 degrees). GetHeight of an actor read 128 for the player and for an NPC
whose bounds are 86 high, at scale 1 and 2; we return 128 for every actor.
Actors' width and length are their bounds, unverified. Missing/malformed bounds
return zero; reversed bounds clamp to zero. Scale uses our spawn-time actor
height factors; original player scale and race-change timing are unverified.
GetWorldSpace uses our current-placement resolver; unloaded schedule-only travel
and inventory-held objects' worldspace inheritance remain unresolved.

## Batch 7

GetDeadCount is cumulative (Skyrim SE, autotest21): 1 when one was killed, still 1
after Resurrect, 2 when it was killed again, and still 2 after Delete. We count
deaths in play by base (saved in `DCNT`). Placed Starts Dead references never
killed in play also count, which is our choice and unverified. Leveled actor bases follow our existing base resolver.
GetGoldValue uses the existing inventory parser's stored values; dynamically
calculated potion values and non-carryable light values remain unverified.
Conflicting INFO group membership across plugins and duplicate keyword editor
IDs follow the existing load-order index policy.

## Batch 8

IsRunning excludes player sprinting according to the native documentation; the
older condition query includes sprinting. The documented backward-running quirk
is not reproduced because last movement direction is not retained for queries.
NPCs do not yet sprint, so IsSprinting is false for them. IsAlarmed currently
covers guard arrest pursuit; Skyrim's non-guard witness/alarm transitions are
unresolved. IsArrestingTarget uses the current arrest conversation; whether a
pursuing guard also counts before reaching the target is unspecified.
Is3DLoaded uses instantiated render data (including hidden or occluded instances),
not cell attachment. Exact asynchronous creation/destruction timing, persistent
3D kept after detachment and first-person-only body behavior remain unverified.

## Sound instances (Sound.Play and friends)

Instance ids are monotonic here and never reused; whether Skyrim reuses them is
unknown. The voice stays at its start position (it does not follow a moving
source; `Audio` has no position setter). Script-started loops are not saved:
after a load a stored id stops nothing. Inference: SE does not persist sound
handles. **test-mod** wanted: start a looping `Sound.Play`, save, load, and
check whether it is still audible and whether `StopInstance` stops it.

## Music queue (MusicType.Add / Remove)

Music queue and transition choices moved to `known_ambiguities/music.md`.

## Leveled list additions (AddForm / Revert)

Unconfirmed: whether additions survive an SE save (we save them as `LVAD`), what
the game does when a list is already at 255 entries (we ignore the add), and
whether `AddForm` skips duplicates (we keep them). **test-mod** wanted: add an
item to an empty `LeveledItem`, save and load, then roll it through `AddItem`.

## Reference setters (SetOpen / SetDestroyed / SetScale)

Checked in Skyrim SE (autotest21, 2026-11-01): `SetOpen` raises `OnOpen` /
`OnClose` once the door has moved (a door went from state 3 to 1 with `OnOpen`, and
back with `OnClose`). We send them, for every animated door, when its movement
ends, with whoever moved it as `akActionRef` (None for `SetOpen`). Graph-driven
objects send them when the graph says it has opened or closed. `SetDestroyed(true)`
on a destructible web left its stage at -1, kept it enabled and sent no
`OnDestructionStageChanged`, but rebuilt its 3D (`OnCellAttach`, `OnLoad`,
`OnCellLoad`); `SetDestroyed(false)` sent nothing.

Our choices: `SetDestroyed` keeps a flag that `GetDestroyed` reads and the stage
is left alone. What the flag shows (the destroyed model?) and whether it blocks
activation is unknown, so we show nothing different. `ClearDestruction` clears
the flag. Whether `SetScale` on an actor keeps the NPC's and race's heights
multiplied on top is unconfirmed (we do). The player isn't rescaled.

## Reference flags and encounter levels

Unconfirmed: `CalculateEncounterLevel` multipliers. No plugin carries
`fLeveledActorMult*`; we use 0.5 / 0.75 / 1.0 / 1.5 for easy to very hard, and the
rounding is ours. `IgnoreFriendlyHits` is a flat "friendly blows never turn it",
since the engine keeps no friendly-hit count (the number the game tolerates is
unconfirmed). `IsDeleted` for created references is "id gone". **test-mod**
wanted: `CalculateEncounterLevel(0..3)` on a reference in a zone with no limits,
read back at a known player level.

## Actor state (Resurrect / UnequipAll / critical stages)

Unconfirmed: whether a resurrected actor's equipment comes back worn (we keep it
as it died), whether `Resurrect` should refuse an actor placed "starts dead" (we
revive it), whether `UnequipAll` also readies away spells (we leave them), and the
numbering `UnequipItemSlot` takes (we use biped slots 30..61). The critical stage
ends remove the actor by disabling it; the game's ash and goo are not shown.
**test-mod** wanted: `Resurrect` an NPC killed with armor, then `IsEquipped`.

## Hostility and favor flags

Unconfirmed: whether `IsHostileToActor` counts actors that are not loaded or
are dead (we say no), and what `SetDoingFavor` changes besides the flag the
scripts read (we change nothing). `GetFactionReaction` is read as the same
faction combat relation the `GetFactionRelation` condition uses.

## Resets (ObjectReference / Cell / Quest)

Unconfirmed: exactly what `ObjectReference.Reset` restores beyond the location,
inventory and a dead actor (we also restore enable, lock, destruction, open
state and harvest, and leave factions, relationships, owners and alias fills);
whether a cell reset resurrects the actors killed in it and re-enables what
scripts disabled (we do both); whether a quest's text globals are copied when it
starts or when its text first shows them (the latter); and what `Quest.Reset`
does to a running quest and its script variables (we stop it and clear its state
but keep its script). **test-mod** wanted: kill an NPC in a cell, `Cell.Reset`,
leave and return; and `Reset` a disabled reference.

## Game finders, sun and flags

Unconfirmed: whether `FindClosestActor` and the random finders skip dead actors
(we include them), whether they see unloaded or editor positions (we use the loaded
references' current positions), the magnitude of `GetSunPosition*` (we return a
unit direction), the angle that counts as sungazing (12 degrees), the
`CalculateFavorCost` formula (we return the price), and what the controls-disabled
message of `SetInChargen` says. **test-mod** wanted: `GetSunPositionX/Y/Z` at a
known hour, read back through `Debug.Notification`.

## Knowledge, sound categories and settings

Unconfirmed: whether `PlayerKnows` on a magic effect also counts effects learned
other ways than eating or experimenting with an ingredient (we count only those);
how a four-digit year prints in `GameTimeToString` (zero-padded: "0201"); whether
category changes survive saves (we don't save them); whether `SetFrequency` above 1
is accepted (we cap it at 1, as the page says 0..1); and whether `SetINI*` rejects
unknown names (we accept them). **test-mod** wanted: `Utility.GameTimeToString(0.0)`
shown in a message box, and `Mute` then `UnMute` on the music category while music
plays.
