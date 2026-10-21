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
  * fSneakLightRunMult` (0.01, 0.2 in the game). While the target sneaks and
  the observer is out of combat, multiply this by the observer's Blindness
  actor-value factor `(100 - blindness) / 100`. `fSneakStealthboyMult` when
  invisible.
- Observer skill: `fSneakPerceptionSkillMin + (Max - Min) * Sneak / 100`,
  times `1 + alert * fSneakAlertMod + sleeping * fSneakSleepBonus +
  not combat target * fSneakCombatMod` (-0.4 in the game).
- Stealth points (CK wiki "Stealth Points"): detection above 0 by an
  aggressive actor drains them (faster the higher), below 0 refills them
  (faster the lower); none refill for `fCombatStealthPointRegenAlertWaitTime`
  (10 s) after someone becomes alert; at 0 the target is detected; detection
  above `iCombatStealthPointDetectionThreshold` (25; or the sneak variant, 70
  when sneaking) detects at once; always 0 while enemies fight you. They
  count for "aggro" detection only, not for being caught stealing.
- Detection states Normal / Alert / Combat / Lost and detection events (CK
  wiki "Detection").
- `GetLightLevel` (Papyrus) is 0 to 150. `GetDetected`, `IsDetectedBy`.
- The Skyrim CK wiki documents [`SetAlert(bool abAlerted = true)`](https://ck.uesp.net/w/index.php?title=SetAlert_-_Actor&oldid=15309)
  as setting or clearing an actor's alerted state, and
  [`IsAlerted()`](https://ck.uesp.net/w/index.php?title=IsAlerted_-_Actor&oldid=12200)
  as querying it. The public [SKSE vanilla command list](https://skse.silverlock.org/vanilla_commands.html)
  lists `GetIsAlerted` at 0x105B (condition index 91 after subtracting 0x1000).
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

- **Blindness edges:** UESP's [Skyrim:Sneak](https://en.uesp.net/w/index.php?oldid=3575824)
  (revision 3575824) gives the detection formula and separates visual and sound
  terms. Its [Actor Value Indices](https://en.uesp.net/w/index.php?oldid=3596002)
  (revision 3596002) lists Blindness as actor value 84, a percentage that
  affects spotting a sneaking target out of combat, default 0. The public
  [CK Detection category](https://ck.uesp.net/wiki/Category:Detection) gives the
  visual multiplier `(100 - blindness) / 100`. The engine reads the observer's
  live actor value and scales only vision; it clamps finite values to 0–100
  and treats non-finite values as 0 blindness. It applies the value only while
  the target is sneaking and the observer's `CombatState` is `None`; `Searching`
  and `Fighting` both bypass it. Mapping the source's “out of combat” to that
  engine state and the invalid-value handling are implementation choices, not
  verified Skyrim SE behavior. The console's `sees` flag remains geometric
  visibility (line of sight, view cone, awake, not invisible), independent of
  blindness. At 100 blindness visual contribution is zero, but sound and the
  observer-skill term remain, so this value alone does not guarantee that the
  total detection is zero. The AV index summarizes its maximum as allowing
  detection only through sound; this implementation follows the explicit CK
  factor, which scales vision only, and retains the separate observer-skill
  term. That outcome has not been verified against Skyrim SE.
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
- **Explicit script alert flag**: Papyrus `SetAlert`, `IsAlerted`, and CTDA
  `GetIsAlerted` share a per-actor flag, initially false. An omitted setter
  argument means true; false clears it. A missing receiver does nothing and
  a missing condition subject queries false. Only actor references, including
  the player, created actors and unloaded placed actors, can receive the flag.
  The flag lives in `WorldState`, so unloading or recreating a loaded actor
  runtime does not clear it. This implements the documented set/query contract;
  sharing it with the condition is a choice based on the command name. It
  neither starts nor cancels combat/searches, changes hostility, draws weapons,
  nor modifies detection values. Automatic detection-state transitions leave
  this explicit flag alone until their relationship has a source.

## Open questions

- **Native save continuity.** Optional `.vrs` section `STPT` saves each retained
  player's or actor's current stealth points and regeneration delay, then
  restores them by replacement and invalidates the derived player-observation
  cache for a fresh detection update. This is a bounded engine continuity
  policy, not evidence about Skyrim's `.ess` payload. Searches, combat targets,
  Normal/Alert/Combat/Lost state transitions, elapsed AI execution and observer
  values are not saved; their load-time lifecycle remains open.

- **Weapons are silent, by decision.** Swinging a weapon (all but daggers)
  should make action sound through the weapon's detection sound level
  (`WEAP` `DNAM`) and `iSoundLevelLoud` / `Normal` / `Silent` / `VeryLoud`.
  Only `iSoundLevelSilent` (10) is in Skyrim.esm; the other values live in
  the executable and no public source gives them. Rather than invent them,
  attacks make no sound until a source turns up.
- The "Combat Search" behaviour itself (where searchers look, for how long)
  is unsourced; see above.
- **Alert flag versus detection Alert**: the Skyrim `SetAlert` / `IsAlerted`
  pages do not specify how this flag relates to the Normal / Alert / Combat /
  Lost detection states, weapons drawn, or `GetCombatState`. The setter's CK
  note mentions a player forge/equip-animation problem, but does not define
  the intended animation behavior. No target or search location is supplied
  by the setter. Automatic flag updates, any weapon-posture side effects,
  `SetAlert(false)` during a fight/search, and reset/death/save lifetime need
  evidence; only explicit flag changes and in-memory retention are implemented.
- `fSneakStealthboyMult` and `fSneakNoticedMin` aren't in the plugins and
  read 0 in the game, as a setting that doesn't exist does: left out.
  (`fSneakLightMoveMult` 0.01, `fSneakLightRunMult` 0.2, `fSneakCombatMod`
  -0.4 and `iCombatStealthPointSneakDetectionThreshold` 70, also missing from
  the plugins, were read in the game, `test-mod autotest2`, and are used.)
- `fPlayerDetectionSneakBase` / `Mult`, `fPlayerDetectActorValue`,
  `fSneakActionMult` (unused without action sounds), `fSneakFlyingDistanceMult`,
  `fSneakAmbushNonTargetMod`: their roles are unknown.
- The light level follows the weather's colours as the renderer uses them:
  outdoors at midnight (moonlight and night ambient) comes out at about 60% of
  noon in Riverwood (78 against 127). The game's, by the Riverwood road under
  `SkyrimClear` forced (`test-mod autotest2`, hour: level): 0-4 81.6, 5 81.7,
  6 92.2, 7 102.5, 8 113.1, 9 124.5, 10-15 136.4, 16 136.3, 17 30.7, 18 24.2,
  19 21.7, 20 25.6, 21-23 63.3. Night is about 60% of noon there too, but
  the game's scale is about 7% higher, and its evenings dip far below the
  night (the sun behind the hills to the west still counted as the light's
  source?): not modelled.
- The observer-minus-target skill term isn't attenuated, so an observer with a
  high Sneak (Riverwood's Stump, Sneak 50: 35 at 5100 units) detects a player who
  isn't sneaking anywhere within reach, through walls: a crime witness from
  afar. That is how the published formula reads; whether the game limits it
  otherwise is unknown.
- Being hit: UESP's Sneak page says a sneak attack that doesn't kill leaves
  the victim "looking for you" (searching). A survivor not already fighting
  its attacker searches where the attacker was as it struck (the place is
  chosen), if it would attack it then (after the assault's crime); else it
  fights. What any other unseen hit does isn't published; it starts a fight.
