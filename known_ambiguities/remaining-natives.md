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
Is3DLoaded uses instantiated render data (including occluded instances), not cell
attachment, and is False for a disabled reference. Skyrim SE (autotest21,
`research/autotest21.md`, 3DLoaded): a `PlaceAtMe`'d barrel is loaded at once,
`Disable` makes it False at once, and one placed initially disabled is not loaded;
ours agrees (`PlaceAtMe`'s `abInitiallyDisabled` is honoured). Persistent 3D kept
after detachment and first-person-only body behavior remain unverified.

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

Checked in Skyrim SE (autotest21): a dead NPC still has its armor equipped.
`Resurrect` brings it back with its base inventory: a cuirass added and worn by
script was gone (count 0). We roll the inventory again from the record.
`UnequipItemSlot(32)` takes off a cuirass and `(2)` doesn't: biped slots 30..61,
as ours. Still unconfirmed: whether `Resurrect` refuses an actor placed "starts
dead" (we revive it) and whether `UnequipAll` also readies away spells (we leave
them; the test's spell had already been displaced by a sword). The critical stage
ends remove the actor by disabling it; the game's ash and goo are not shown.

## Hostility and favor flags

Unconfirmed: whether `IsHostileToActor` counts actors that are not loaded or
are dead (we say no), and what `SetDoingFavor` changes besides the flag the
scripts read (we change nothing). `GetFactionReaction` is read as the same
faction combat relation the `GetFactionRelation` condition uses.

## Resets (ObjectReference / Cell / Quest)

Checked in Skyrim SE (autotest21): `ObjectReference.Reset` on a plugin container
put it back where it was placed and restored its inventory (three ingots back,
added gold gone). It left its owner and blocked activation as set, and a disabled
container stayed disabled. We do the same, and also restore lock, destruction,
open state and harvest (the lock wasn't tested: the container started locked).
`Quest.Reset` on a running quest stopped it, set stage 0 and cleared the done
stages, as ours. Unconfirmed: whether a cell reset resurrects the actors killed in
it and re-enables what scripts disabled (we do both), and whether a quest's text
globals are copied when it starts or when its text first shows them (the latter).

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
whether
category changes survive saves (we don't save them); whether `SetFrequency` above 1
is accepted (we cap it at 1, as the page says 0..1); and whether `SetINI*` rejects
unknown names (we accept them). **test-mod** wanted: `Mute` then `UnMute` on the
music category while music plays.

`GameTimeToString`: Skyrim SE printed `GameTimeToString(0)` and `(1)` as
`07/16/0201 00:00`, `(1.5)` as `07/16/0201 12:00`, `(31)` as `08/15/0201 00:00`
and `(365)` as `07/15/0202 00:00`, and the current time (0.552) and one day
on (1.552) alike as `07/16/0201 13:14` (autotest21; autotest23,
`research/autotest23.md`). So the year is zero-padded, the month and day count
from 0 (17 Last Seed, month index 7), the time of day is the fraction's, and
the date is the start date moved on by the whole days less one, never before
it. Both saves were in their first day, so whether today's date counts
instead of the start date can't be told: taken as the start date, which fits
every reading. No vanilla script calls it.

## Natives added 2026-11-07

- `SetNoBleedoutRecovery(true)` keeps an actor bleeding out until cleared
  (`GetNoBleedoutRecovery` reads it). Not saved yet.
- `HasLOS` is `GetLineOfSight`'s view-cone line (autotest22 read the two
  alike). `IsTrespassing` is the player's trespass; NPCs never trespass.
- `Actor.SendAssaultAlarm()` is the player assaulting the actor (autotest2:
  its crime faction got the assault's gold, reported by witnesses; the
  `ASSU` story event isn't sent, unobserved). `Faction.SendAssaultAlarm()`
  is an assault on the faction, reported by its members who see the player.
  `SendStealAlarm(player)` is the theft of the object as taking it would be,
  without taking it or an `AIPL`; other thieves do nothing.
- `Game.FastTravel(akDestination)` travels to any reference (a map
  marker's travel anchor for one) with the map's travel time, ignoring the
  player's restrictions and discovery (the carriages go to cities not yet
  found). Whether the game refuses it in combat isn't known.
- `MoveToPackageLocation` puts the actor where its running package puts it
  (as a scheduled placement); a whole-cell location moves nothing.
- `WaitForAnimationEvent(asEventName)` waits for the reference's graph to
  raise the event, with `PlayAnimationAndWait`'s 10 s limit (the game's has
  none), and returns True.
- `AttachAshPile(akAshPileBase)` places an ash pile of that activator, or
  `DefaultAshPile1` (0x1B) with none given, where the actor is; activating
  the pile searches the actor's body (the vanilla piles have no script, so
  the link is the engine's). Not saved yet. The default form is a choice:
  `magicAttachAshPileOnDeath` passes its own property.
- `SetAlpha(afTargetAlpha, abFade)` draws a loaded actor that opaque, by the
  same dithering as the distance fade; 0 neither draws it nor casts a
  shadow. A fade takes a second for the whole way (no source gives its
  length). The player and objects aren't faded; an actor loaded again is
  opaque.
- `MoveToNode(akTarget, asNodeName)` moves to the named node of the
  target's model (case ignored), turned as the node is, by the node's rest
  transform in the model (the weapon racks' pivots); an actor's nodes, or a
  missing node, move nothing.

## Natives added 2026-11-09

Research: `research/pex-natives/actor-batch12-states.md` and
`speech-batch11-checks.md` (choices for the speech checks are in
`speech-checks.md`).

- `IsInKillMove` is False: there are no killmoves yet.
- The player doesn't ride, so `GetPlayersLastRiddenHorse` is None and
  `IsPlayersLastRiddenHorse` False. `IsBeingRidden` is an NPC mounted on it.
- `IsAllowedToFly` reads `SetAllowFlying` (allowed until set). `CanFlyHere`
  adds being out of doors; whether SE also refuses in some exteriors (towns,
  cells flagged in some way) is unknown.
- `InterruptCast` stops the player's charging or held spells, drops those let
  go but not fired, and stops an NPC's hand cast. Script casts from objects
  land at once here, so for objects there is nothing to stop.
- `SetActorCause`'s actor takes the blame for damage and kills the object is
  credited with (`OnHit` aggressor, crimes, the killer). Saved. Whether it also
  makes the actor the owner of the object's explosions and hazards for
  experience or other effects is unknown.
- `StartSneaking` on an NPC overrides its package's stance until called
  again; whether SE's package later takes the stance back is unknown. On the
  player it toggles `sneaking` (not while swimming, in the air or attacking).
- `UnlockOwnedDoorsInCell` unlocks doors (base `DOOR`) placed in the actor's
  current cell that it, or a faction it is in at the owner's rank, owns
  (`XOWN`, else the cell's owner). Load doors' far sides are left alone.
- `SendTrespassAlarm(player)` is a trespass the actor alone witnessed,
  reported to its crime faction. Other criminals do nothing.
- A deferred kill: blows that would kill don't (Papyrus `Kill` neither); the
  last killer is kept, and `EndDeferredKill` kills an actor left at no health
  by it. Saved. Whether `EndDeferredKill` kills at once in SE, or only at the
  next blow, is unverified.
- `ClearArrested` on the player calls off their arrest; on anyone else it
  does nothing (NPCs aren't arrested here).
- `SendVampirismStateChanged` / `SendLycanthropyStateChanged` send
  `OnVampirismStateChanged` / `OnLycanthropyStateChanged` to the actor's
  scripts, its aliases and its effects. Whether other actors' scripts hear it
  ("anyone who cares") is unknown.
- Frame-rate natives read the engine's frame times: `CaptureFrameRate(n)`
  lists the last n rates (up to 200) to one decimal, oldest first; the
  average, least and most cover frames between `StartFrameRateCapture` and
  `EndFrameRateCapture`. The string's exact format in SE is unknown. Budget
  and memory queries give 0 / False / "", profiling does nothing, and user
  logs go to the debug log.
- `SetGodMode` (and the console's `tgm`) only stops damage to the player;
  whether SE's also gives unlimited magicka, stamina and carry weight here is
  left out. `ToggleMenus` (`tm`) hides the SWF HUD and menus.
- `SetFootIK(false)` turns foot placement off for everyone until set again.
- `CenterOnCell(AndWait)` loads the cell at once, so neither waits.
  `PlayerMoveToAndWait` queues the move and returns 0 at once.
- `TakeScreenshot(name)` writes `name.png` (relative to the working
  directory) of the world without the UI, after the frame.
- `UsingGamepad` is False (no gamepad input yet).
- `AddAchievement`, `ShakeController`, `SetAllowFlyingMountLandingRequests`
  and `RequestModel` do nothing: no achievements, rumble or player mounts here,
  and models load with their references.
- `ClearTempEffects` removes the decals blows, arrows and spells left (on the
  world and on bodies) and the impacts' playing effects; whether SE also
  clears placed hazards is unknown.
- `SetSunGazeImageSpaceModifier`'s modifier plays at full strength when the
  player begins gazing at the sun (`IsPlayerSungazing`'s test); none is set
  until a script sets one.
- `SetSittingRotation(deg)`: in first person, the view of a player seated in
  a cart starts that many degrees from the seat's heading, positive to the
  right (the sign is a guess; MQ101 passes -55), and turns with the seat.
  Furniture seats don't use it yet (the player doesn't sit in furniture).
- `ShowFirstPersonGeometry(false)` stops drawing the player's first-person
  body until shown again (saved); the third-person body is unaffected.
- The title sequence: see `research/ui/title-sequence.md`.
- Cell fog overrides, Havok ball-and-socket constraints and dependent
  animated objects: choices in
  `research/pex-natives/cell-batch17-fog-constraints.md`. Dependent animated
  objects have no effect yet; the source doesn't say what they do.
- `ShaderParticleGeometry.Apply` / `Remove`: one script geometry at a time,
  falling indoors unsheltered; choices in
  `research/pex-natives/render-batch18-shader-particles.md`.
- `SendWereWolfTransformation`: the nearest actor that detects the player and
  has a crime faction reports a werewolf crime (the source: "an actor in high
  who can detect the player"); none, nothing.
- `StartVampireFeed` sends `OnVampireFeed(akTarget)` to the feeder's scripts
  at once; the feeding animation isn't played.
- `PlayBink` plays nothing.
