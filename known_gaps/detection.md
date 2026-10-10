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
  attack them detects them by. With no such actor in range they are full
  again at once (after the alert wait). Actors have them too, kept while an
  enemy is alert to them or fights them.
- Detection of the player is worked out four times a second, of actors and
  the detection states once a second; enemies look once a second as before.
  Fights start, and are joined, only within 4000 units (no source).
- **Alert**: a calm actor that would attack someone it detects (above 0) but
  doesn't find yet becomes alert to the one it detects most: weapons out, it
  walks to where they were, following them while it still detects them.
  Damage doesn't alert: a hit starts a fight, as before.
- **Lost**: a fighter whose target has gone undetected (0 or less) for
  `fCombatStealthPointRegenDetectedEventWaitTime` (10 s; the wait is that
  setting by its name) stops fighting and searches where it last detected
  it. A target that dies or unloads ends the fight. Fights no longer end by
  distance.
- **Searching** (both): at the spot, it moves to random places on the navmesh
  within 512 units of it every 3 to 8 seconds (invented: the CK wiki's
  "Combat Search" page wasn't found). It attacks once it finds the target
  (stealth points gone, or past the threshold) and gives up once it doesn't
  detect it and the target's stealth points are full; or at once if the
  target dies or it no longer would attack it.
- Topics: `NormalToCombat`, `AlertToCombat`, `LostToCombat` on starting a
  fight; `NormalToAlert`, `CombatToLost` on starting to search; `AlertIdle`
  / `LostIdle` every `fCombatDetectionDialogueIdleMin` to `MaxElapsedTime`
  seconds while searching; `AlertToNormal` / `LostToNormal` on giving up.
  `CombatToNormal` isn't said.
- The sneak eye opens with the most any actor detects the player by against
  the combat threshold, or with the stealth points lost, whichever is more.

## Open questions

- **Weapons are silent, by decision.** Swinging a weapon (all but daggers)
  should make action sound through the weapon's detection sound level
  (`WEAP` `DNAM`) and `iSoundLevelLoud` / `Normal` / `Silent` / `VeryLoud`.
  Only `iSoundLevelSilent` (10) is in Skyrim.esm; the other values live in
  the executable and no public source gives them. Rather than invent them,
  attacks make no sound until a source turns up.
- **Detection events**: projectile impacts alerting those who hear them (the
  same unknown sound levels decide how loud they are).
- Spells and shouts as action sounds: no magic yet.
- `GetIsAlerted` / `SetAlert` (the alert flag scripts set) aren't tied to the
  Alert state; `fSneakAlertMod` (0) is left out.
- The "Combat Search" behaviour itself (where searchers look, for how long)
  is unsourced; see above.
- `fSneakLightMoveMult`, `fSneakLightRunMult`, `fSneakCombatMod`,
  `fSneakStealthboyMult`, `iCombatStealthPointSneakDetectionThreshold`,
  `fSneakNoticedMin`: not in the plugins, defaults unknown; left out.
- `fPlayerDetectionSneakBase` / `Mult`, `fPlayerDetectActorValue`,
  `fSneakActionMult` (unused without action sounds), `fSneakFlyingDistanceMult`,
  `fSneakAmbushNonTargetMod`: their roles are unknown.
- Perks: Stealth's "harder to detect" cuts the observer's skill factor
  (`known_gaps/perks.md`); muffle (Muffled Movement, Silence, the Muffle
  spell) scales movement noise and invisibility hides the target from sight
  (`known_gaps/magic.md`). Not blindness, nor spells and shouts as action
  sounds.
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
- Combat searches through load doors: a target that leaves the cell unloads
  from the fight's view and the fight ends.
