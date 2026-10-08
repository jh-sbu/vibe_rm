# Detection (stealth)

Implemented in `src/detection.rs`; console `detect` lists each loaded actor's
detection of the player, whether it sees them, and the player's light level and
stealth points.

## What is known

- The formula (CK wiki "Detection", Skyrim's page, archived 2020; the GECK's
  Fallout 3 page is the same apart from Perception in place of Sneak; UESP
  Skyrim:Sneak gives it with Skyrim's numbers):
  `fSneakBaseValue + attenuation * (sound + visual + observer skill) +
  (observer skill - target skill)`, attenuation
  `((fSneakMaxDistance - distance) / fSneakMaxDistance) ^
  fSneakDistanceAttenuationExponent` (2).
- Sound: movement `(fSneakEquippedWeightBase 12 + fSneakEquippedWeightMult
  0.5 * armor weight) * fSneakRunningMult` when running, 0 standing still;
  action `ActionSound * fSneakActionMult`; times `fSneakSoundsMult`, and
  `fSneakSoundLosMult` (0.3) without line of sight.
- Visual: 0 if the target can't be seen; `(fDetectionSneakLightMod + light
  level) * fSneakLightMult`, times `1 + moving * fSneakLightMoveMult + running
  * fSneakLightRunMult`, `(100 - blindness) / 100` and `fSneakStealthboyMult`
  when invisible.
- Observer skill: `fSneakPerceptionSkillMin + (Max - Min) * Sneak / 100`,
  times `1 + alert * fSneakAlertMod + sleeping * fSneakSleepBonus +
  not combat target * fSneakCombatMod`.
- Stealth points (CK wiki "Stealth Points"): detection above 0 by an
  aggressive actor drains them (faster the higher), below 0 refills them
  (faster the lower); none refill for `fCombatStealthPointRegenAlertWaitTime`
  (10 s) after someone becomes alert; at 0 the target is detected; detection
  above `iCombatStealthPointDetectionThreshold` (25; or the sneak variant
  when sneaking) detects at once; always 0 while enemies fight you. They
  count for "aggro" detection only, not for being caught stealing.
- Detection states Normal / Alert / Combat / Lost and detection events (CK
  wiki "Detection").
- `GetLightLevel` (Papyrus) is 0 to 150. `GetDetected`, `IsDetectedBy`.
- Skyrim.esm: `fSneakBaseValue` -15, `fSneakMaxDistance` 2500,
  `fSneakExteriorDistanceMult` 2.1, `fSneakLightMult` 0.33,
  `fSneakLightExteriorMult` 0.5, `fDetectionSneakLightMod` 15,
  `fSneakSkillMult` 0.5, `fSneakRunningMult` 2, `fSneakActionMult` 2,
  `fSneakSoundLosMult` 0.3, `fCombatStealthPointDrainMult` 3,
  `fCombatStealthPointRegenMult` 0.2, `fCombatStealthPointRegenMin` 5,
  `iLightLevelInteriorMod` 25, `iLightLevelMax` 300,
  `fPlayerDetectionSneakBase` 10, `fPlayerDetectionSneakMult` 0.4,
  `fPlayerDetectActorValue` -30, `iSoundLevelSilent` 10.

## Choices made without a source

- **Target skill**: the target's Sneak times `fSneakSkillMult` while
  sneaking, 0 otherwise. The sources name a "target skill factor" without
  saying how it is made; `fSneakSkillMult` is used for it by its name.
- **Exteriors**: the reach is `fSneakMaxDistance * fSneakExteriorDistanceMult`
  and the light term is multiplied by `fSneakLightExteriorMult`, read from
  their names.
- **Light level**: the luminance of the scene's ambient (directional ambient
  averaged), its sun (outdoors only when nothing shades the point towards it;
  an interior's directional light always) and the point lights reaching the
  point with nothing in between (the renderer's falloff), times 100, at most
  150. The game's scale, and what `iLightLevelInteriorMod` and
  `iLightLevelMax` do, are unknown.
- **Sight**: a view cone of 190 degrees (invented) from 110 units above the
  feet (as for finding bodies), with nothing in between. Sleepers (in a bed)
  see nothing; their skill is otherwise unchanged (`fSneakSleepBonus` is 0).
- Defaults the plugins lack, from UESP / CK wiki:
  `fSneakEquippedWeightBase` 12, `fSneakEquippedWeightMult` 0.5,
  `fSneakDistanceAttenuationExponent` 2,
  `fCombatStealthPointRegenAlertWaitTime` 10.
- Stealth points drain by `detection * fCombatStealthPointDrainMult` a second
  and refill by `max(-detection * fCombatStealthPointRegenMult,
  fCombatStealthPointRegenMin)` a second, from the most any actor who would
  attack the player detects them by. With no such actor in range they are
  full again at once (after the alert wait).
- Only the player has stealth points. An NPC is detected by another as soon as
  the detection value is above 0.
- Detection of the player is worked out four times a second; enemies look
  once a second as before. Fights still start, and are joined, only within
  combat's 4000 units (no source), so that a faint detection outdoors (the
  reach is 5250) doesn't start a fight that ends at once.
- The sneak eye opens with the most any actor detects the player by against
  the combat threshold, or with the stealth points lost, whichever is more.

## Open questions

- **Action sounds**: weapons swung (all but daggers), spells and shouts. The
  weapon's detection sound level (`WEAP` `DNAM`) maps through
  `iSoundLevelLoud` / `Normal` / `Silent` / `VeryLoud`; only `Silent` (10) is
  in Skyrim.esm, so attacks make no sound yet.
- **Detection events**: projectile impacts alerting those who hear them.
- **Alert and Lost states**: alerted actors searching towards what they
  noticed, searching for a target lost, `fSneakAlertMod`, the transition
  topics (`NormalToAlert`...), `GetIsAlerted`. Combat ends by distance as
  before, not by losing detection.
- `fSneakLightMoveMult`, `fSneakLightRunMult`, `fSneakCombatMod`,
  `fSneakStealthboyMult`, `iCombatStealthPointSneakDetectionThreshold`,
  `fSneakNoticedMin`: not in the plugins, defaults unknown; left out.
- `fPlayerDetectionSneakBase` / `Mult`, `fPlayerDetectActorValue`,
  `fSneakActionMult` (unused without action sounds), `fSneakFlyingDistanceMult`,
  `fSneakAmbushNonTargetMod`: their roles are unknown.
- Perks (Stealth "harder to detect", Muffled Movement, Silence), muffle and
  invisibility effects, blindness: no magic or perks yet.
- The light level follows the weather's colours as the renderer uses them:
  outdoors at midnight (moonlight and night ambient) comes out at about 60% of
  noon in Riverwood (78 against 127). Whether the game's is that bright at
  night is unverified.
- The observer-minus-target skill term isn't attenuated, so an observer with a
  high Sneak (Riverwood's Stump, Sneak 50: 35 at 5100 units) detects a player who
  isn't sneaking anywhere within reach, through walls: a crime witness from
  afar. That is how the published formula reads; whether the game limits it
  otherwise is unknown.
- Finding bodies still uses its own distance (1000 units) and a clear line; the
  formula is for actors.
- Being hit: the CK wiki says it alerts the victim; a hit starts a fight
  as before.
