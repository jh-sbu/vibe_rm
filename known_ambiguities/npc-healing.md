# NPC combat healing

Implemented in `src/casting.rs` and `src/ai/casters.rs`. This is bounded
support for known self-delivered Health restoration spells, with provisional
selection policy; it does not claim the original engine's combat scoring.

## Public evidence and assets

- [Creation Kit: Magic Effect](https://ck.uesp.net/wiki/Magic_Effect)
  documents Self delivery, concentration casting, conditions and actor-value
  modifiers. Recover changes maximum and current Health temporarily;
  non-Recover restoration changes current Health. The implementation excludes
  Recover buffs, detrimental effects and absorb archetypes from healing.
- [A modder's report about AFT](https://forums.nexusmods.com/topic/9759048-best-way-to-force-npcs-to-cast-a-custom-self-buff/)
  (March 2021) describes a mod's scripted, configurable triggers for NPC
  spell use, including half Health. This establishes no vanilla threshold;
  the 50% trigger here is our provisional policy. Native healing thresholds
  remain unresolved.
- Skyrim.esm's GMST records contain `iAICombatRestoreHealthPercentage = 60`
  and `fCombatItemRestoreTimer = 3`. This check read only plugin assets.
  Their use for spell casting, as opposed to inventory restorative items,
  is unresolved, so neither is treated as a proven spell trigger.
- [Public community setting list](https://www.gamesas.com/game-settings-wiki-t202961.html)
  names `fCombatRestoreHealthPercentMin`, `fCombatRestoreHealthPercentMax`,
  `fCombatRestoreStopCastThreshold` and `fCombatRestoreHealthRestrictTime`.
  They are not overrides in the inspected Skyrim.esm.
  [Valravn's author documentation](https://www.nexusmods.com/skyrimspecialedition/mods/53869?tab=docs)
  reports a default of 15 for the restrict time. The names and one default
  do not settle their formula or relationship, so they are not guessed here.

No engine executable, library or binary data was examined. No game assets
are included in the repository or tests.

## Skyrim SE readings

A Vigilant of Stendarr (`HealingLeftHand`, `OakfleshLeftHand`,
`WardLesserLeftHand`) fighting a held target that didn't hurt it, its
health set by script and `HealRate` 0: autotest22 (21–43 magicka) and
autotest23 (300) at 70%, 45% and 20%, autotest24 (330, spells logged) at
90%, 70%, 55%, 45% and 20%, 8 s each. It never cast its heal; it cast
Oakflesh once at the start and then held Lesser Ward. The game's settings:
`iAICombatRestoreHealthPercentage` 60, `fCombatRestoreHealthPercentMin` 0 /
`Max` 0.5, `fCombatRestoreHealthRestrictTime` 15,
`fCombatRestoreStopCastThreshold` 0.25, `fCombatItemRestoreTimer` 3
(`research/autotest23.md`, `autotest24.md`).

autotest27 and autotest28 had the Vigilant struck (`research/autotest27.md`,
`autotest28.md`). With 90 health it warded at 22% against melee and healed
at about 5% in bleedout under fire. With 600: against melee from 70%, 50%
and 30% it never healed in 24 s, even at 0.218 for 8 s with magicka to
spare; against a fire warlock it healed at about 0.16 (for 1.5 s, until its
magicka ran out) and below 0.150 (for 3.9 s, back to about 0.22, then
warded). Out of magicka, it drank a restore-health potion.

## Implemented choices and remaining questions

- A living fighter that is neither fleeing nor bleeding out starts healing
  below 20% of current maximum Health (autotest28: healed at about 0.16 and
  below, never at 0.218 or above; autotest24's held one at 20% didn't).
  Once selected, healing remains preferred until 25% or no affordable
  eligible spell remains (autotest28's stopped near 0.25;
  `fCombatRestoreStopCastThreshold` is 0.25, but its use isn't documented).
  The start between 0.16 and 0.218 is a choice. Unresolved: whether the
  game heals only under spell attack (melee at 0.218 didn't), healing in
  bleedout (autotest27; ours doesn't), drinking potions (ours doesn't),
  healing cooldowns and their interaction with confidence, threat and
  combat style. Healing can precede a weapon or
  bow attack regardless of the style's offensive magic multiplier; a swing or
  guard already under way finishes first, and an interrupted bow is lowered
  before healing and starts a fresh draw afterward.
- Candidates must be ordinary spells with Self delivery and a positive
  non-Recover Health value modifier (including Health as the second value of
  a dual modifier), with no hostile effect. At least one restorative effect's
  spell-entry and effect conditions must currently pass on the caster.
- Among eligible spells, choose the costliest affordable one, as offensive
  selection already does. Concentration requires one second's cost available
  to remain selected, pays per quarter-second landing, and uses the existing
  1.5–3 second channel/cooldown cadence. These are compatibility choices, not
  established native scoring, charge timing or resource-reservation rules.
- Self healing needs no enemy range, line of sight or facing. Cast requests
  retain their chosen spell and recipient through dispatch; live/dead,
  bleedout, fleeing and full-health checks prevent further healing doses.
  Known-spell changes rebuild both offensive and healing candidate lists;
  live effect conditions and costs are evaluated when selecting.

## Verification limits

Synthetic tests cover restoration classification, the provisional health
policy, self-target queuing without enemy geometry requirements, concentration
progression, offensive line-of-sight behavior and cancellation of interrupted
bow draws. The workspace tests passed after review.

A real-assets offscreen gameplay smoke test was unavailable: the sandbox run
failed during renderer initialization, before gameplay, because the fallback
OpenGL adapter allowed 16 sampled textures where the terrain pipeline required
17. No gameplay result, actual self-healing amount or magicka payment was
verified by that attempt; these still need an integration check with a
supported rendering device.
