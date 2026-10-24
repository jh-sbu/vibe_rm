# Further PEX natives

These implementations use public community documentation and existing runtime
systems. They have not been compared experimentally with Skyrim SE. References
and source-versus-inference distinctions are in refs/pex-natives/batch*.md.

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
includes descendant locations, following our existing location reference index;
exact descendant and disabled-reference inclusion remains unverified. Reservations
use running quest reference aliases with Reserves Reference, not furniture use.
Malformed parent cycles terminate at the first repeated location.

## Batch 4

GetAllItemsCount sums copies rather than distinct base forms. The public wording
is not precise about that distinction, quest items, unplayable items or gold.
We count all positive inventory quantities; IsContainerEmpty uses the same rule.
WornHasKeyword includes every equipped item in the runtime equipment list; exact
hand-held weapon and hidden-skin inclusion in Skyrim remains unverified.

## Batch 5

GetCurrentPackage returns None for a loaded actor without a selected package;
unloaded actors report the schedule selection. Skyrim's timing and interrupt
package reporting are unverified. GetDialogueTarget uses the scene's explicit
headtracking target while speaking; headtracking and dialogue listeners need not
coincide in all scenes. Implicit scene listeners, idle pauses between lines and
non-scene ambient dialogue are unresolved and return None. Encumbrance uses our
existing NPC capacity calculation, whose precise Skyrim enforcement is unverified.

## Batch 6

Dimensions use authored OBND extents multiplied by effective scale, independent of
rotation or animation. Whether Skyrim uses these bounds or current loaded-3D
bounds, and when it applies scale, remains unresolved. Missing/malformed bounds
return zero; reversed bounds clamp to zero. Scale uses our spawn-time actor
height factors; original player scale and race-change timing are unverified.
GetWorldSpace uses our current-placement resolver; unloaded schedule-only travel
and inventory-held objects' worldspace inheritance remain unresolved.

## Batch 7

GetDeadCount uses actors currently dead (including placed Starts Dead). Whether
Skyrim instead maintains a cumulative counter across respawn, resurrection or
cleanup is unclear from the public descriptions. Deleted created actors are no
longer enumerated. Leveled actor bases follow our existing base resolver.
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

Tie-breaking at equal priority (we take the first queued, and a queued type
over the location's), whether `Add` of an already queued type moves it (we keep
its place), and whether script-added entries survive a save (we save them as
`MUSQ`) are unverified. Fades and ducking are not implemented, and an abrupt
versus graceful change is our reading of the MUSC flags. **test-mod** wanted
for the last two: `Add` a type, save, load and see whether it still plays; and
`Add` two types of equal priority and see which plays.

## Leveled list additions (AddForm / Revert)

Unconfirmed: whether additions survive an SE save (we save them as `LVAD`), what
the game does when a list is already at 255 entries (we ignore the add), and
whether `AddForm` skips duplicates (we keep them). **test-mod** wanted: add an
item to an empty `LeveledItem`, save and load, then roll it through `AddItem`.

## Reference setters (SetOpen / SetDestroyed / SetScale)

Unconfirmed: whether `SetOpen` raises `OnOpen` / `OnClose` (we raise none);
whether `SetDestroyed` runs the stage's explosion and debris and sends
`OnDestructionStageChanged` (we do neither), and which stage the flag implies
(we take the first that destroys, else the last); and whether `SetScale` on an
actor keeps the NPC's and race's heights multiplied on top (we do). The player
isn't rescaled. **test-mod** wanted: `SetDestroyed` on a destructible with
`OnDestructionStageChanged` registered, and `SetScale(2)` on an NPC then
`GetScale`.

## Reference flags and encounter levels

Unconfirmed: `CalculateEncounterLevel` multipliers. No plugin carries
`fLeveledActorMult*`; we use 0.5 / 0.75 / 1.0 / 1.5 for easy to very hard, and the
rounding is ours. `IgnoreFriendlyHits` is a flat "friendly blows never turn it",
since the engine keeps no friendly-hit count (the number the game tolerates is
unconfirmed). `IsDeleted` for created references is "id gone". **test-mod**
wanted: `CalculateEncounterLevel(0..3)` on a reference in a zone with no limits,
read back at a known player level.
