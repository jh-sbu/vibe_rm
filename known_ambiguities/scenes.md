# Scenes

Implemented in `src/scene.rs` (runtime) and `crates/esp/src/scene.rs` (`SCEN`
records); `vrm-tool scenes <data> [scene]` dumps one scene or counts over all
(`SGE=1`: the scenes that begin with start-game-enabled quests).

## What is known

- Record layout (xEdit's definitions, checked against the data): scene flags
  (`FNAM`), phases between `HNAM` markers (name, start conditions, `NEXT`,
  completion conditions, `NEXT`, editor width), actors (`ALID` alias, `LNAM`
  flags, `DNAM` behaviour flags), actions between an `ANAM` type and an empty
  `ANAM` (`SNAM` start phase, `ENAM` end phase, then a timer's seconds in a
  second `SNAM`; packages `PNAM`; dialogue topic `DATA`, head track alias
  `HTID`, looping delay `DMIN` / `DMAX`), then the owning quest (`PNAM`, after
  the actions' own) and the scene's conditions.
- Flag meanings from CommonLibSSE: scene 0x1 begin on quest start, 0x2 stop
  quest on end, 0x8 repeat conditions while true, 0x10 interruptible; actor
  `LNAM` 0x1 no player activation, 0x2 optional, 0x4 run only scene packages;
  `DNAM` 0x2 death end, 0x4 / 0x8 combat pause / end, 0x10 / 0x20 dialogue
  pause / end, 0x40 / 0x80 observe-combat pause / end; action 0x8000 face
  target, 0x10000 looping, 0x20000 head track player.
- Fragments (`VMAD` after the scripts): a byte, flags (1 begin, 2 end), the
  script; each of begin / end as a byte, script, function; then phase
  fragments: flags (1 start, 2 completion), phase, four bytes, script, function.
- Runtime rules from the Creation Kit's documentation: phases in order, a
  phase whose start conditions fail is skipped; a phase ends when its
  completion conditions pass, or when every action ending in it is complete
  (dialogue said, package "done", timer out). Of 3496 package actions, those
  that alone end their phase are travel (201), force greet (35), shout, magic
  and sit; sandbox and follow never do, consistent with packages that never
  finish not ending phases.
- Phases use `IsSceneActionComplete` 910 times; `IsScenePlaying` (packages),
  `IsInScene` (quests), `IsScenePackageRunning` are implemented too.

## Confirmed in the game (`test-mod autotest4`, 2026-10-13)

- `Start` with the owning quest stopped, or its actor's alias empty, played
  nothing; with the alias filled a scene of one 3 s timer played and was
  over 4 s later.
- A scene whose only phase's start conditions never pass was over at once.
- Killing the actor of a scene whose actor ends it on death ended it within
  a second; a scene flagged to stop its quest on end stopped it.

## Choices made without a source

- With completion conditions, the phase waits for them alone (actions ending
  there may still be running and are stopped); MorthalInitialScene's phase 0
  only has head tracking actions and waits on "Aslfur within 1000 of the player".
- Speakers and package actors must be loaded: a scene whose actors are
  elsewhere waits for them (their scene packages still move them off-screen,
  through whereabouts). Off-screen scenes in the game may play on regardless.
- `Start` fails when the scene's conditions fail; `ForceStart` ignores them.
  Both take actors from other scenes, which stop (in the game a scene
  started with its actor in another played; whether that one stopped then or
  only at the later `ForceStart` wasn't seen).
- A travel / escort package is done when the actor stands in the package's
  place within its radius + 150 units; sit / sleep when in the furniture; force
  greet once the conversation it opened has ended. Procedures not run here
  (shout, use magic, activate, hover, orbit, own trees mapped to "hold") count
  as done after a second so scenes don't stall on them.
- Dialogue actions without a topic only head track: complete at once, the
  head tracking lasting until their end phase ends. Lines without a voice file
  last a reading time (0.32 s a word, at least 2 s).
- `Stop` runs the scene's end fragment, like ending normally. "Stop quest on
  end" stops the quest; "repeat while true" starts the scene again next frame
  if its conditions still pass.
- Subtitles show for lines within 1500 units of the player; barks are quiet
  while scene lines are being said, and scene actors don't bark.

## Open questions

- Unknown scene flag 0x4 (on 1394 scenes) and action `LNAM`.
- Observe-combat pause / end, death pause; "interruptible"; "run only scene
  packages" (scene packages already run ahead of all others).
- Whether looping dialogue repeats the same INFO or picks again (picked again
  here).
