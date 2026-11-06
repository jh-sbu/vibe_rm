# Package procedure trees and default package lists

Implemented in `src/ai/package.rs` (`procedure_tree`, `expand`; NPC collection is in `schedule::npc_packages`),
with package-input conditions in `src/condition/mod.rs`. Based on the templates'
own trees in the data (`vrm-tool pack-tree <data> <package or template>`) and
which form lists NPCs name (`vrm-tool pack-lists <data>`).

## What is known

- A template's tree follows `XNAM`: each node an `ANAM` (`Stacked`, `Sequence`,
  `Simultaneous`, `Procedure`), its conditions (`CITC` + `CTDA`), a branch's child
  count (`PRCB`), a procedure's name (`PNAM`) and the inputs it takes (`PKC2`, by
  input index), children in pre-order. The input names (`UNAM` + `BNAM`) come
  after the tree.
- In tree conditions, run-on 6 ("package data") names the input run on in the
  CTDA's third parameter; flag `0x08` makes the first parameter an input index.
- Procedures' parameter order, from the single-procedure templates: Sandbox
  (location, allow eating, sleeping, conversation, idle markers, sitting,
  wandering, preferred path only, energy, special furniture), Travel (location,
  ride horse, preferred path), Patrol (start, radius, repeatable, start at
  nearest, ride horse, static pathing), Follow (target, min radius, max radius,
  accompany, ride horse, need LOS).
- `DefaultMasterPackageTemplate`: follow the target (the linked ref by default)
  when it is a living actor (wider for giants, mammoths...), travel to or patrol
  from a `linkWaitPoint` linked ref by `Variable04`, guard and patrol a
  `GuardMarker`, patrol from the target when linked to it, else sandbox at the
  editor location (creatures without furniture).
- Of the NPCs naming a default package list (`DPLT`), 2618 name
  `DefaultMasterPackageList`, 292 `PackageDefDraugr`, 70 the dragons', 62 the
  guards' and so on; most have no packages of their own.

## Numeric package data conditions

`GetNumericPackageData` (condition function 612) reads a concrete package's
authored `Bool`, `Int` or `Float` input by its `UNAM` index. Booleans produce
0 or 1; integers and floats produce a floating-point condition value. The
value is worked out when the package (or each of its expanded template
branches) is parsed and stored on the condition. A template branch therefore
reads the concrete package's values, rather than the template's values or the
actor's previously selected package. Sparse indices are not ordinal positions or
FormIDs. Numeric inputs are not resolved as reference parameters.

Public evidence (consulted 2026-10-17):

- [xEdit's Skyrim definitions](https://github.com/TES5Edit/TES5Edit/blob/dev-4.1.6/Core/wbDefinitionsTES5.pas)
  identify function 612 and an integer first parameter. Its PACK definition
  stores `ANAM` type names, `CNAM` Boolean bytes / 32-bit integers / floats and
  byte-sized `UNAM` indices separately from the procedure tree. This change
  exposes the existing package parser's typed values to conditions.
- A modder's firsthand [Forcegreet tutorial](https://www.gamesas.com/forcegreet-tutorial-t261109.html)
  describes the template's `GetNumericPackageData` check for the Boolean
  "Player must be detected" input: zero means false and selects a different
  procedure-tree branch. This is public CK authoring evidence, not a controlled
  comparison against this project's runtime.
- [SKSE's public vanilla command table](https://skse.silverlock.org/vanilla_commands.html)
  independently labels the argument as numeric package data. It does not
  establish missing-value behavior or procedure lifecycle timing.

Limits and unresolved behavior:

- Missing, nonnumeric, malformed or nonfinite inputs fail the condition,
  including comparisons against zero. Native invalid-input return values are
  not established; failure follows this project's unsupported-condition policy.
- Support is scoped to authored values in package-selection conditions and
  the template branches already exposed by `expand`. Sequence/Simultaneous
  execution and nested branch switching now use the cursor for available
  procedures. Template defaults are now inherited; the vanilla census finds
  no omitted template inputs. No vanilla numeric input setter or global-backed
  input was found (`research/package-data.md`).
- The existing parser interprets `Int` as signed 32-bit, whereas xEdit displays
  its storage as unsigned. Negative values' native interpretation is not
  established; tests of negative inputs check the parser's current convention,
  not native equivalence. Condition values use `f32`, so integers above 2^24
  can lose precision. Native large-integer comparison behavior is unverified.
- The evidenced use checks a numeric input from the package context. Running a
  condition *on* numeric package data (`run_on` 6) is not established by these
  sources. Existing run-on-package-data support resolves reference inputs;
  numeric values used as subjects still fail. Conditions outside a package
  record also fail rather than guessing the actor's current package.

Synthetic tests cover sparse indices, types, invalid inputs, indices beyond a
byte, cross-plugin index preservation and concrete-package values in expanded
branches, including a package with its own tree.

## Package location conditions

`GetWithinPackageLocation` (condition function 325) asks whether its subject is
within the location a package `Location` input names. Its parameter is an input
index whatever the "use package data" flag says ([xEdit's Skyrim
definitions](https://github.com/TES5Edit/TES5Edit/blob/dev-4.1.6/Core/wbDefinitionsTES5.pas):
`ptPackdata`; CommonLibSSE-NG's `TESCondition.h`: `kGetWithinPackageLocation = 325`).
The CK wiki page was not reachable (Cloudflare) when this was written.

The base game masters use it 56 times, all but one in procedure trees: the
ForceGreet, ForceGreetFromSitting, ForceGreetWaitSitting and SayToPlayer
templates (the player, run on through a target input, within the trigger
location; the greeter within the force greet distance of the player),
TravelToRadius / TravelVariableSpeed (travel until within each radius), the
civil war hold-position packages (travel until within the hold location, then
hold) and the Dawnguard disguised vampire packages. Expanded, the masters'
packages carry 1,798 such conditions, every one naming a `Location` input
(near reference 1,171, near current 290, near self 189, near alias 108, near
editor 20, near linked reference 16, in cell 4). Until now every one failed,
so their branches never ran.

The location is worked out when the package is parsed and placed for the actor
whose package it is (the condition context's subject), with the rule force
greet triggers use: within the radius (3D, inclusive) of a reference's,
alias's or linked reference's position, the actor's own position (near self,
near current) or its editor position. "In cell" asks whether the subject is in
that interior cell, or in that exterior cell's grid square; "in location
alias" whether the subject's current location is within the alias's location.

Limits and unresolved behavior:

- xEdit names location kind 2 "Near Package Start Location". New runs retain
  their initial coordinates, and goals and location checks use those. Legacy
  runs with no recorded origin use the actor's current position until the next
  run; the original start cannot be recovered.
- When the package's actor isn't loaded (persistent actors' schedule
  selection), near-style locations can't be placed and the condition fails.
- An empty alias, a missing input or a non-location input fails the condition.
- Whether the native check uses 3D or horizontal distance, and whether an
  exterior "in cell" means the grid square, is unverified.

An autotest for the start position, the distance and the radius's edge is
planned in `plans/autotest-survey.md` (tier 2).

## Multi-location sandboxing

`SandboxMultiLocation` (67 base-game packages) and `SandboxWorkingMultiLocation`
(14) sandbox at up to six locations in turn. Their tree: a root `Sequence` of
Travel and UnlockDoors to Location 1 (only when "Unlock On Arrival?"), a first
pass (a `Sequence` when "Random?" is false, else a `Random`) of Sandbox + Wait
per location, then a branch flagged "Repeat when Complete" (`PRCB` flags bit 0,
per [xEdit's Skyrim definitions](https://github.com/TES5Edit/TES5Edit/blob/dev-4.1.6/Core/wbDefinitionsTES5.pas))
of one `Sequence` per location: Travel, UnlockDoors when asked, then
`Simultaneous` Sandbox and Wait for "Location N Time (seconds)". Each
location's branch is conditioned on its time being above zero.

The procedure cursor now runs this tree directly, including its first pass
and per-stop door procedures; the former precomputed `Package::stops` and
`StopProgress` rotation are removed. Progress is saved with the run and retained
when the body unloads. Time passed in a menu advances a loaded Wait without
respawning the actor at Location 1. Current implementation limits are recorded
in `plans/ai-depth.md` step 2.

Choices not settled by public sources:

- Wait uses real seconds. The inputs say "(seconds)"; DoNothing's input says
  "ActualSeconds". The real/game-time distinction has not been measured.
- A Simultaneous branch ends when its Wait does (Sandbox never completes).
  That follows the tree's apparent purpose, not documented engine timing.
- The authored first pass is now kept distinct from the repeat. It has no
  Travel before the first Sandbox/Wait, so that Wait starts when entered,
  whereas repeating stops first Travel and then start their Wait. The old
  implementation folded this pass into the rotation; the new reading follows
  the authored structure. Whether SE's Sandbox itself delays its simultaneous
  Wait until arrival is not established.
- Random excludes its own previous child when another passes; separate Random
  nodes have independent choice histories. Native repeat-choice rules remain
  unmeasured.
- The sequential branch's Simultaneous nodes for locations 2 to 6 check
  Location 1's time, as authored; the interpreter follows those conditions.

An autotest is planned in `plans/autotest-survey.md` (tier 3).

## Open questions

- Trees with a `Stacked` root are split into branches (first branch whose
  conditions pass); each branch runs as the behaviour of its main procedure
  (patrol over guard in a simultaneous branch).
- Default package lists come after the NPC's own packages, in list order. Whether
  the game consults them only when none of its own packages applies, or in some
  other order, is not settled; with them last both readings agree.
- Patrolling from "Self" walks the actor's own linked reference chain.

## Procedures

- Packages without a template carry their own procedure tree (`ambushSleepPackage`,
  184 NPC package slots: draugr and others sleeping in a linked sarcophagus,
  throne or alcove until their ambush script sets `Variable01`); templates not
  known by name run their tree's main procedure (`HoldPosition` travels and
  stays, `GuardPost` travels to its wait location).
- Creatures use the furniture their package names: their enter / exit events
  come from their own graph's branch of `ActionActivate` (`DraugrActivateFurniture`:
  sarcophagus, throne, slouched seats, alcoves, ground ambushes), whose
  conditions name the furniture as the creature's linked reference
  (`GetIsID` run on linked reference, `IsEnteringInteractionQuick`,
  `GetSleeping`). They start in it on load and get out when the package ends.
- UseWeapon (and UseWeaponMultiTarget / AlreadyHeld): travel to "Use Weapon
  Location", take a carried weapon of the "Weapon Type" (object type 19 melee,
  20 ranged, or a specific weapon) in hand, draw and attack one of the targets
  per barrage ("Min / Max Attacks per Barrage", "Min / Max Pause"); archers loose
  arrows at the target's bounds centre (harmless, they stick), melee swings
  strike nothing. The old weapon comes back afterwards. Per-target trigger
  distances, Aim Only and optional pauses are implemented; remaining lifecycle
  and timing questions are in [use-weapon.md](use-weapon.md).

## Procedure cursor semantics (2026-11-05)

The retained trees and `ai::procedure::Cursor` follow the structure read from
vanilla TravelBackAndForth (`00098EB2`), SandboxMultiLocation (`00068B86`)
and CarryAndDropItem (`00079ABF`), inspected with `pack-tree`. Their flags,
input ordering, condition strings and repeat structure are game-asset evidence;
the cursor's scheduling rules below remain inferences, not SE measurements.

- Sequence advances in order, skipping failing children. A repeat restarts its
  subtree. Progress belongs to the package run, including a scene-suspended
  run, rather than the loaded body.
- Simultaneous ends on its first completing eligible child. A failing child
  does not count as completion. The moving procedure follows the former
  main-procedure priority (Patrol, Follow, Sit, Sleep, Sandbox, Escort, Travel,
  HoldPosition), while Wait only advances its timer. This extends the existing
  Sandbox/Wait inference above; it is still not independently established.
- Stacked rechecks on package evaluation, and switching branches restarts the
  branch left behind. The same PACK remains current and fires no package
  lifecycle event just for a branch or procedure change. Neither the recheck
  cadence nor restart-versus-resume of an old branch has been measured in SE.
- Random excludes its previous child when another eligible child exists, but
  allows a sole eligible child to repeat. Its previous choice survives a
  repeating ancestor's reset. These are choices, not documented random rules.
- Wait uses real seconds, as previously chosen for sandbox stops; a newly
  entered Wait does not reuse the preceding procedure's elapsed time.
- Single Travel and Escort keep the already observed arrival lifecycle: the
  run ends immediately on arrival and remains current until replaced. Other
  procedures in a sequence finishing do not end the PACK. Scenes wait for
  root completion, retaining their existing Sit/Sleep furniture completion.

An SE probe should compare Simultaneous's first/all completion, Random repeat
choices, nested Stacked switches on the evaluation cadence and whether returning
to a previous branch resumes it. Queued in ROADMAP Test plugins. Unfinished
procedure integration and the temporary fallback gate are tracked only in
`plans/ai-depth.md` step 1.

## Menu time and optional sequence inputs (2026-11-05)

Additional asset examination with `pack-tree` found:

- `DriveCart` (`000CDFC4`), using DoNothing (`000654E2`), sets
  `ActualSeconds` to zero and `StopMovement` true. UseWeapon also supplies its
  explicitly named "Max Time spent Attacking (0 = forever)" to Wait.
- StayAtCurrentLocation (`00082571`) selects Wait while
  `IsFurnitureAnimType(1)` passes, otherwise Travel to its starting location.
- ActivateInSequence (`00071C45`) supplies seven Activate leaves. Delphine's
  `00071C46` leaves targets 5 and 6 unset; Blind Cliff's `0008CABD` supplies
  only its first two targets, including an unset final target. The unset values
  are PTDA Object Type zero. xEdit's public `wbObjectTypeEnum` names zero
  "None"; its MPL 2.0 source was consulted for enum facts, without copying code.
- TravelBackAndForthAndGuard (`0009BD86`) runs a repeating Travel pair beside
  Guard. Guard remains skipped as the plan requires.

Choices: Wait with zero seconds holds indefinitely; positive seconds complete
on elapsed real time. A Wait-only phase retains a seat, including beyond the
previous furniture duration. A statically unset Activate target or zero use
count completes without activating anything. A dynamically unresolved alias is
not treated as an empty authored slot. IsNullPackageData recognizes these null
target sentinels. These rules follow the authored uses; exact SE behavior at
malformed and dynamically disappearing inputs still needs observation.

Large wait/sleep advances consume time across phase boundaries on a temporary
clock. Movement precedes the boundary's completion, so a newly entered Wait
receives only subsequent seconds. Papyrus real time stays paused; away movement
deadlines are rebased afterwards. Furniture exit animation is skipped during
the menu advance, as scheduled respawning already skipped it. The graph resets
to its default state and applies the carry offset/instant exit when accepted;
carried objects survive, ordinary idle objects do not. Native menu timing for
these transitions is unmeasured. Queue a probe comparing several short waits
with one long wait through Travel/Wait, doors and a carry sequence.

A sole completed Travel/Escort retains its destination while current, matching
the existing arrival behavior. Composite sequences give way only at root
completion. Synthetic runtime and vanilla data fixtures verify this engine's
chosen rules; they are not original-game measurements.

## Object ID and Object Type resolution (2026-11-05)

The shared selector matches base records using xEdit's public object-type names,
including food's ENIT flag, weapon animation kinds and furniture marker types
already used by the engine. Inventory weapon checks and food checks use this
predicate too. World searches include authored, moved and created references,
exclude disabled/deleted/contained references and stay in the package's space
and search radius. A whole-cell location accepts any position in that cell.

Choices after checking PACK/FURN's public UESP layouts and xEdit's enum (see
`research/package-data.md`): select the nearest suitable reference to the
actor, breaking equal distances by FormID. Preserve Sleep's own-bed, faction,
unowned priority, ownership permissions and marker reservations. A positive
authored search radius is used as given; absent/zero radius uses the existing
768-unit furniture search distance. Generic Object Type furniture selection
uses all usable markers, while Beds/Chairs restrict the marker kind. Existing
furniture interaction still handles approach paths and graph compatibility.

The enum does not establish every aggregate membership rule. "Combat Wearable"
is interpreted as weapons, ammo and light/heavy armor; "Wearable" as armor
including clothing. Spell Range Target includes aimed, target actor and target
location deliveries; Touch and Self use their corresponding delivery values.
"Weapons: None" selects no object. These memberships, native tie breaking and
search distance defaults remain unmeasured in SE. A probe should compare
equidistant targets, ownership, occupied multi-marker furniture, no positive
search radius and the aggregate object categories. Do not treat these choices
as evidence for the blocked location/target kinds.

## Observation, fixed idles and groups (2026-11-06)

Research is in `research/package-observation-groups.md`. A UseIdleMarker leaf
uses only its named furniture/idle marker, with the existing entry/idle/exit
machinery and an infinite hold. A missing or occupied marker waits; it does not
select another chair. KeepAnEyeOn observes only inside ObservationArea: head
tracking while moving, body facing when standing idle. EndPursuitArea is not
interpreted: pursuit semantics remain unread; the implemented choice is to
stop observing once the target leaves the observation area.

SetLookAt/ClearLookAt store an explicit target per actor, including away and
save/load continuity. Explicit requests override package observation. Combat
retains combat tracking. We read abPathingLookAt as permitting an idle body
turn as well as head tracking; false turns only the head. This is a controller
policy, not a verified SE priority or a claim about moving path constraints.

GroupMovement's authored tree selects leaders and successors; stable reference
order gives followers separate distance bands, adding the vanilla
fFollowSpaceBetweenFollowers (192) per earlier follower. SingleFileFollow uses
its authored predecessor/fallback chain with at least one 192-unit spacing.
The stable order and distance-band policy need an SE comparison; the input
radii and setting alone do not specify a geometric formation.
FollowTo finishes following when its target enters the destination and then
runs the authored Travel leaf. TravelCrouchWhenNear uses a standing first leg
and sneaking second leg, as the template's location names describe; no extra
radius is invented. FollowPlayer's repeated MinRadius argument is preserved.

Probe observation across both area boundaries, SetLookAt during locomotion and
scenes, grouped actors approaching a narrow doorway and a leader dying, and
the standing-to-crouched travel transition. These compare the remaining policy
choices without holding the authored functionality back.
