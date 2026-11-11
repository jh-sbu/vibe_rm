# Natives that were stubs while test mods used them (audit, 2026-11-10)

Prompted by `SetDontMove` and `SetRestrained`, which did nothing in our
engine while ten batches used them to hold actors (fixed 2026-11-10,
`known_ambiguities/actor-holds.md`).

## Method

- The vanilla natives each test mod calls: identifiers in its Papyrus
  sources (`autotest21` to `autotest24` and the files they include) and the
  `method` / `global` calls of the Rust-built batches, matched against
  `vrm-tool pex-natives`.
- For every commit that touched a batch (its module, its sources, its
  `research/` file: built, run, results, comparison), the dispatcher as it
  was then (`src/script/natives.rs`, `src/debug_natives.rs`) was parsed and
  each native the batch calls classed as missing (falls through to the
  default None / 0), a no-op arm, a constant return, or implemented.
- Limits: an arm that does something but ignores some of its arguments on
  one path (the player's `MoveTo` dropping its offsets, fixed today) isn't
  caught. Arms that read no argument at all although the native takes some
  are only `Enable` and `Disable` (their fade flag), harmless here.

## Findings

Everything flagged is implemented now, except `Quest.IsStarting` (still
False; autotest21's QuestStates read "not starting" right after `Start` in
SE, which agrees). `Topic.Add` was a false match (the loggers' own `Add`).

Natives missing when a batch was built, and implemented from its results
(nothing to redo: the comparisons were made against the engine's code and
say "ours has no ..."): autotest2's `IsRunning`, `SendAssaultAlarm`,
`SetPlayerTeammate`, `EffectShader.Stop`, `VisualEffect.Stop`,
`CanFastTravelToMarker`; autotest4/6/11/12's `GetCurrentPackage` and
autotest4's `EvaluatePackage`; autotest6 to 9 and 11's `Is3DLoaded`
(constant True); `Delete` (autotest to 9); `FastTravel` (autotest13, 14,
21); autotest21/22's `HasLOS`, `StartSneaking`, `SetUnconscious`,
`PushActorAway`; ragdolls' four ragdoll natives.

Setup natives stubbed for a batch's whole life:

| native | stub | batches |
| --- | --- | --- |
| `SetRestrained` | no-op 2026-10-13 to 11-10 | autotest3, autotest4, autotest5 |
| `SetDontMove` | no-op to 2026-11-10 | autotest5, autotest17, autotest21 to autotest24 |

The game honoured them, so SE's readings stand as readings. What they
touch:

- Our side: comparisons were made against the code, not by running the
  batches here, except autotest24's ViewCone (run here 2026-11-10 with the
  observer drifting; `research/autotest24.md` corrected).
- SE readings taken on the held actor's own behaviour, which the hold may
  have shaped (others read a free actor beside a held target, or static
  values):
  - Wards (autotest22, autotest23): the Vigilant that didn't ward was held
    (`known_ambiguities/wards.md`).
  - HostileSpell (autotest21): the held target of a script cast read
    combat state 2, searching; a free one might close in.
  - AlertFlag (autotest21): the 0.6 s sheathe after `DrawWeapon` was read
    on a held NPC; autotest17's held commoner didn't draw at all
    (`plans/animation-unblocked.md`).
  - CombatRegen (autotest4): the restrained fighter "neither fought"; both
    it and its enemy 1500 units off were held, so this says nothing about
    restrained actors fighting.
