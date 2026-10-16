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
