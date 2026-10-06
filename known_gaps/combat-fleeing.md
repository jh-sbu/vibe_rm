# Combat fleeing (confidence and threat ratio)

Implemented in `src/ai/threat.rs`, with the flee movement in `src/ai/combat.rs`
(`flee_step`). This page records what the implementation is based on and what
public sources leave open. Everything here comes from public documentation, the
CommonLibSSE headers and the game's data files. The executable has not been
disassembled (the project keeps a clean-room stance).

## What is known

**The model (GECK, Fallout 3 / New Vegas; the same engine family).** Confidence is
not a health threshold. It is compared with a *threat ratio*:

- Combat strength = estimated DPS × current health / (1 − min(armor share, 0.99))
  (GECK *GetThreatRatio*: `EstimatedDPS × Health / (1 − min(fArmorScalingFactor ×
  DamageResist / 100, 0.99))`).
- Threat ratio = my side's strength / the enemies' strength (the *group threat ratio*).
  Above 1, "I will live longer than you".
- A fighter flees when the group threat ratio is below its confidence value.
- Flee checks run every `fCombatThreatRatioUpdateTime` seconds (5 in Skyrim.esm).
- "Actors never decide to flee until they have taken some damage."
- A random confidence modifier between `fCombatConfidenceModifierMin` and `Max` is
  rolled once per fight, not for Foolhardy actors.
- Fallout's flee behaviour: run for a door, cover or open ground. At the flee
  position the actor waits; taking damage, or the target coming within
  `fCombatFleeBoostConfidenceTargetRadius` (512), "boosts confidence" and makes it
  re-evaluate. Otherwise it waits `fCombatFleeWaitTime` (60 s).

**Skyrim-specific evidence that the same model is used.**

- Skyrim.esm sets `fConfidenceCautious` 0.375, `fConfidenceAverage` 0.15,
  `fConfidenceBrave` 0.0375, `fCombatConfidenceModifierMin` −0.25 and `Max` 1.0,
  and `fCombatThreatRatioUpdateTime` 5. These are ratio-sized values, not health
  shares.
- The CK wiki's AI Data Tab describes the confidence levels as "flee unless stronger
  than the threat", "flee if outmatched" and "only if severely outmatched": a strength
  comparison. Cowardly actors "NEVER engage in combat".
- CommonLibSSE `CombatState` has `isFleeing`, `confidenceModifier`, `threatValue`,
  `strengthUpdateTimer`, `threatRatioUpdateTimer` and `fleeDialogueTimer`.
  `CombatGroup::CombatMember` has `threatValue` and `groupStrengthUpdateTimer`, and
  `CombatGroup` counts `fleeCount` / `fightCount`. So Skyrim keeps a per-fight
  confidence modifier, per-actor and per-group-member strengths on their own timers,
  and a threat ratio timer.
- Condition functions `GetThreatRatio` (477) and `IsFleeing` (329) exist in Skyrim.
- Community reports (UESP *Skyrim:Combat* and its talk page): hurt NPCs cry "I yield!"
  or "I submit!" and flee, and rejoin the fight once they have recovered. Fleeing
  enemies are not "active" combatants (killmoves trigger on them as the last enemy).

## What the implementation does

- Strength per actor = (weapon or unarmed base damage / attack interval) × health /
  (1 − armor reduction), refreshed every `fCombatStrengthUpdateTime`. The player's
  uses their weapon (or race unarmed damage), health and armor.
- Sides: everyone fighting my enemies, or fought by them, is on my side, and the
  other way round. The player is a member while anyone fights them.
- Check timer: first check at a random point within `fCombatThreatRatioUpdateTime`,
  then every interval. Also immediately when a fleeing actor is hurt, or when its
  threat comes within 512 units.
- Flee when hurt (cowards: always), and group ratio < `fConfidence<Level>` + modifier.
  The modifier is added, rolled uniformly per fight, and not rolled for Foolhardy
  actors.
- Fleeing: runs to the navmesh place near by furthest from the threat, until
  `fCombatFleeDistanceInterior` / `Exterior` away, then waits facing it. After
  `fFleeIsSafeTimer` seconds there, it leaves the fight. Cornered with the threat
  in reach, it fights. When a check finds the ratio back at or above its threshold,
  it stops fleeing and fights again.
- A `FLEE` dialogue line when it starts to flee. Cowards don't draw.
- `GetThreatRatio` (subject strength / parameter's strength) and `IsFleeing`.

## Gaps

1. **DPS estimate.** How Skyrim estimates DPS is undocumented: which weapon, skill
   and perk scaling, attack speed, power attacks, magic and shouts. We use base
   damage / a fixed 2 s interval (2.6 s for bows). The player is treated the same
   way.
2. **Armor term.** The GECK formula uses `fArmorScalingFactor × DamageResist`. We use
   our own armor reduction share (Skyrim's armor rating formula), which may differ
   from what Skyrim feeds into the strength.
3. **Confidence modifier: added to what, and how.** The GECK calls it a "+/-
   adjustment to their Confidence rating". We add it to the confidence *value*.
   With Skyrim.esm's range (−0.25 to 1.0) an Average actor's threshold can rise from
   0.15 to 1.15, so it may flee merely even fights. Other readings fit the asymmetric
   range just as well: a multiplier (`value × (1 + mod)`), or an offset to the
   confidence *level*. Whether the roll is uniform is also unknown.
4. **Group membership.** Which actors count towards a side is unknown: a radius, line
   of sight, detection, or only the formal `CombatGroup` (shared targets / allied
   factions). We count every loaded fighter linked through targets. We count
   fleeing fighters as zero strength for others but their own full strength for
   themselves; the game's handling is unknown. The 2 s group strength timer
   (`fCombatGroupCombatStrengthUpdateTime`) is not modelled separately.
5. **Per-actor (personal) check.** Fallout has a "Flee Based On Personal Survival"
   flag (personal health vs. average enemy DPS, both checks must pass). Skyrim's
   combat style has no such flag in `CSTY` `DATA` (dueling, flanking, dual wield).
   Whether Skyrim always, never or sometimes uses a personal check is unknown.
6. **The damage requirement.** "Some damage" is taken as health below maximum. The
   real test may be a recent hit, a minimum amount, or the "last highest damage
   received" a community summary mentions. Cowards are exempted on the CK wiki's
   "never engage" description.
7. **Check timing.** Sources disagree on whether checks come every
   `fCombatThreatRatioUpdateTime` or at random intervals within it. We randomise only
   the first.
8. **Confidence boost.** Fallout boosts confidence when a fleeing actor is hurt or
   approached; the amount is undocumented and Skyrim has no
   `fCombatFleeBoostConfidence*` settings. We only re-check.
9. **Where to flee.** Skyrim's settings suggest the same priorities as Fallout plus
   allies: doors (`fCombatFleeUseDoorChance`, `fCombatFleeDoorDistanceMax`,
   `fCombatFleeInitialDoorRestrictChance`, restrict times), cover
   (`fCombatFleeCoverMinDistance`, `fCombatFleeCoverSearchRadius`), allies
   (`fCombatFleeAllyDistanceMin` / `Max`, `fCombatFleeAllyRadius`), keeping clear of
   the target (`fCombatFleeTargetAvoidRadius`, `fCombatFleeTargetGatherRadius`).
   Their rules aren't documented. We only run to open ground away from the threat.
10. **Ending a flee.** `fFleeIsSafeTimer` is used as "seconds at the flee distance
    before leaving the fight"; the real meaning is undocumented. Which distance
    settings apply to combat fleeing is a guess: `fCombatFleeDistance*`, rather than
    `fFleeDistance*` / `fFleeDoneDistance*`, which Skyrim.esm sets (5000 / 3000) and
    which may belong to the Flee package procedure and Fear effects
    (`Actor::InitiateFlee(fleeFromDist, fleeToDist)`).
11. **Attackers' side.** `fCombatDetectionFleeingLostRemoveTime` (enemies forget a
    fleeing target lost for that long) and `fCombatLowFleeingTargetHitPercent` are
    not modelled. Attackers chase a fleeing target until `LOSE_DISTANCE`.
12. **Flee dialogue.** `fCombatDialogueFleeMinElapsedTime` / `Max` and
    `fCombatDialogueFleeDistanceMult` (with `CombatState::fleeDialogueTimer`) suggest
    repeated flee lines on a timer with a distance condition. We say one line when
    the flee starts.
13. **Health regeneration in combat.** `fCombatHealthRegenRateMult` is 0, yet players
    report fled NPCs coming back "once they have recovered enough health". The
    mechanism is unclear: regeneration after leaving combat, potions, or the ratio
    shifting as the player is hurt. We regenerate only out of combat.
14. **Threat avoidance out of combat.** Confidence also governs avoiding threats
    outside combat (`fCombatAvoidThreatsChance`, `AVTH` dialogue). Not implemented.
15. **Yielding.** `fCombatYieldTime` / `fCombatYieldRetryTime` (the player yielding to
    NPCs) are not implemented.

## Values taken from the game's own settings table

These settings are not in Skyrim.esm. Their defaults were read from the data of the
game's settings table (names and default values only; no code was read), consistent
with published Skyrim settings lists. Replace them with documented values if any turn
up:

| Setting | Default |
| --- | --- |
| `fConfidenceCowardly` | 1000 (also the GECK's value) |
| `fConfidenceFoolhardy` | 0 (also the GECK's value) |
| `fCombatStrengthUpdateTime` | 1.0 |
| `fCombatGroupCombatStrengthUpdateTime` | 2.0 (not used yet) |
| `fCombatFleeDistanceExterior` / `Interior` | 4096 / 2048 |
| `fFleeIsSafeTimer` | 30 |

## Sources

- GECK: *Confidence*, *GetThreatRatio*, *Combat Flee*, *fCombatThreatRatioUpdateTime*
  (geckwiki.com; summaries via search, as the site blocks direct fetches)
- Fallout Wiki: *Confidence* (fallout.wiki/wiki/Confidence)
- CK wiki: *AI Data Tab*, *FCombatThreatRatioUpdateTime*, *GetThreatRatio* (ck.uesp.net)
- UESP: *Skyrim:Combat*, *Skyrim talk:Combat/Archive 1*, *Skyrim:NPCs*,
  *Skyrim:Fear (effect)*
- CommonLibSSE-NG: `RE/C/CombatState.h`, `RE/C/CombatGroup.h`, `RE/C/CombatController.h`,
  `RE/A/Actor.h` (`InitiateFlee`)
- Skyrim.esm game settings (`vrm-tool gmst`)
