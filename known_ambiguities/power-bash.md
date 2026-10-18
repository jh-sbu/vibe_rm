# Player power bashes

Implemented in `src/ai/combat.rs`; synthetic action regressions are in
`src/ai/combat_bash_tests.rs`. Only public descriptions and installed game plugin
assets were examined. No game engine binary was examined, and no
game assets are included in the repository.

## Evidence

- Players report holding attack while blocking to power bash after acquiring
  the perk, instead of tapping for an ordinary bash: the original observations
  in [this 2012 discussion](https://www.gamesas.com/how-you-perform-different-types-power-attacks-t281171.html)
  and [this 2023 report](https://www.reddit.com/r/skyrim/comments/17gs6mg/).
- The [Archery Behavior Overhaul author's description](https://www.nexusmods.com/skyrimspecialedition/mods/192051)
  describes adding power bashes for bows and crossbows by holding bash, as other
  weapons already do. This supports preserving the ordinary-bash-only bow case;
  crossbows are not implemented by this change.
- Reading the installed base plugins with `vrm-tool esp-dump` on 2026-10-18:
  `PowerBashPerk` (`PERK 00058F67`, Skyrim.esm) has no entry-point section.
  `PowerBash` (`IDLE 000E8452`, Dawnguard.esm override) requests `bashPowerStart`
  and requires `HasPerk PowerBashPerk == 1` and `GetIsID Player == 1`.
  The implementation evaluates the idle's authored conditions using the normal
  condition evaluator, so overriding those conditions changes eligibility.
  A missing idle fails eligibility. This avoids inventing an entry point or
  treating the Block skill requirement as a substitute for perk ownership.
- `NordRace`'s `ATKD` / `ATKE` pairs in the installed plugins include
  `bashStart` (bash flag) and `bashPowerStart` (bash and power flags), with their
  own damage, stagger and stamina multipliers. The implementation uses the
  player's race/NPC attack data rather than copying these values. The existing
  `fStaminaBashBase` / `fStaminaPowerBashBase` settings provide the base costs.

## Implementation policy and remaining ambiguities

1. **Hold timing.** Public observations establish tap versus hold but no exact
   time threshold. We reuse the existing 0.35 s power-attack threshold, measured
   in simulation time. A power bash executes once when it is crossed; releasing
   earlier executes one ordinary bash. Nonpositive time does not advance a bash
   hold. Exact threshold, frame ordering and engine input buffering are unknown.
   Actors without power-bash eligibility retain the existing immediate ordinary
   bash on press; their release does not add an attack.
2. **Cancellation.** A pending bash remembers its shield and weapon. Releasing
   block cancels it; changing that equipment, losing idle eligibility, death,
   conversation or disabled fighting cancels it on the next relevant execution
   check. A canceled bash never becomes a weapon swing or bow shot. Equipment
   and eligibility are checked both on release and in the held-input update.
   This is an explicit input policy, not a measured engine cancellation rule.
3. **Insufficient stamina and attack selection.** The first affordable authored
   power-bash attack is preferred. If none is affordable or present, it falls
   back to the first affordable ordinary bash, consistent with this project's
   existing melee/NPC attack policy. An ordinary request never upgrades to a
   power bash. Exact engine fallback and weighting among several custom bash
   attacks remain unknown. If neither is affordable, no stamina is spent and no
   bash lands.
4. **Hit timing.** Player bashes still land immediately using the existing
   raycast; making them land on the animation graph's HitFrame remains a roadmap
   item. The chosen attack's damage and stagger multipliers and power flag reach
   the existing bash-hit path, including `OnHit`'s power/bash flags. The existing
   bash damage formula and perk entry-point order are unchanged. Whether rejected
   graph events should suppress a bash, detailed recovery and stagger rules,
   and input during a preceding swing remain unconfirmed. This change does not
   claim matching animation timing.
5. **Other equipment.** Dual wielding does not supply a guard, and sheathed
   weapons cannot bash. Existing shield and weapon support is retained; torch
   bashes and their fire effects remain outside this implementation. Race/NPC
   custom attack data may supply different bash animations and multipliers.
