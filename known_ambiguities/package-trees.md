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
0 or 1; integers and floats produce a floating-point condition value. Both
loaded actor selection and persistent actors' schedule selection supply the
candidate package to the condition context. A template branch therefore reads
the candidate's values, rather than the template's values or the actor's
previously selected package. Sparse indices are not ordinal positions or
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
  numeric values used as subjects still fail. Calls outside a package context
  also fail rather than guessing the actor's current package.

Synthetic tests cover sparse indices, types, invalid inputs, indices beyond a
byte, cross-plugin index preservation and concrete-package values in expanded
branches. An optional headless renderer test exercises the condition evaluator's
branch selection and missing-data failures without loading game assets.

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
