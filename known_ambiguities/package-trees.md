# Package procedure trees and default package lists

Implemented in `src/ai/package.rs` (`procedure_tree`, `expand`, `npc_packages`),
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
  execution, nested branch switching and inherited/default or dynamically
  changing or global-backed package data still need implementation and evidence.
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

- xEdit names location kind 2 "Near Package Start Location". The engine keeps
  no package start position, so it uses the actor's current position, as force
  greet triggers and travel goals already do; an actor is then always within
  its own "near current" location.
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

The engine reads this from the tree rather than the template's name
(`package::sandbox_stops`): the first repeating `Sequence` / `Random` whose
conditions the package's own data passes (`condition::evaluate_package_data`:
only `GetNumericPackageData`, decided when parsed), and each of its passing
children that sandboxes at a location and waits. Sandbox packages with two
or more such stops get `Package::stops`. All 81 multi-location packages with
two or more locations do (`HodSleepingGiant19x3` has one); every vanilla one
is random. The actor sandboxes about its stop; once within the sandbox area
it starts the stop's clock, and when that has run it gives up what it is
doing and heads for the next.

Choices not settled by public sources:

- The wait is in real seconds. The inputs are named "(seconds)"; DoNothing's
  Wait input is named "ActualSeconds". Real against game seconds (the
  authored 30 to 300 would be 1.5 to 15 real seconds at TimeScale 20) isn't
  established.
- The clock starts on arrival, as the per-location `Sequence` runs Travel
  before its Sandbox + Wait. A `Simultaneous` branch is taken to end when its
  Wait does (Sandbox never completes); that is inferred, not documented.
- The first pass is folded into the rotation: a sequential package starts at
  its first stop, a random one at a random stop (the tree's random first pass
  has no Travel; the actor walks there anyway). Each random pick is another
  stop than the current one; whether `Random` can pick the same child twice
  running is unknown.
- Progress isn't kept: a new package, a cell load or a wait / sleep (the
  actor put back at Location 1 by `jump_to_schedules`) starts over. Unloaded
  actors' whereabouts use Location 1 only.
- "In cell" stops in another cell (Kynesgrove's campfire and inn) aren't
  travelled to: such a stop sandboxes about the actor's editor place, as "in
  cell" locations already do. Per-stop UnlockDoors isn't done.
- The sequential branch's `Simultaneous` nodes for locations 2 to 6 check
  Location 1's time (`GetNumericPackageData` 0x20), as authored; they are
  followed as written. No vanilla package is sequential.

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
