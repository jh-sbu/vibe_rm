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

## Confirmed in the game (`test-mod autotest5`, 2026-10-13)

- With completion conditions, the phase waits for them alone: a phase with
  a 20 s timer ended (and its scene with it) within 0.32 s of its completion
  condition coming true, and at once when it already was; a scene of a 5 s
  timer and no conditions lasted 5 s. (MorthalInitialScene's phase 0 only
  has head tracking actions and waits on "Aslfur within 1000 of the
  player".)
- A scene whose only phase's start conditions fail was never playing.

## Choices made without a source
- Speakers must be loaded: a line waits for its speaker to load. Package
  actors needn't be: one away runs its scene package where it is and its
  action finishes there (`known_ambiguities/actor-execution.md`, "Scenes").
  `VRM_SCENES_WAIT_FOR_LOADED` has package actions wait for their actors to
  load instead, as they used to. Whether the game plays scenes on with
  their actors unloaded, and how far, is untested (`test-mod autotest15`
  asks, not run yet).
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

## Native save policy and remaining continuity

The optional `SCNE` section in native `.vrs` saves retains the current phase and
elapsed time for timer-only scenes. `SCLF` is an adjunct that leaves the `SCNE`
layout unchanged and retains begun, stopping and paused flags, phase and each
action's waiting/running/done/stopped state for all running scenes. Running
timer actions also keep elapsed time in `SCLF`, including timers in mixed
scenes. Completed actions remain complete, and a begun scene resumes without
replaying its begin or phase-start fragment. Loading replaces existing scene
runs; saves without these sections clear them. This policy does not establish
original `.ess` scene layouts or Skyrim SE save/load behavior.

Timer action actor aliases are reconstructed from quest aliases; explicit
action actor identities are stored in the optional `SCAB` adjunct, including
an explicitly empty binding so an alias fill after saving cannot retarget an
already-bound action. Active package ownership is restored by re-expanding the
authored package records and is installed before the saved cell attaches actors;
the package structs themselves are not serialized. The ownership elapsed clock
is retained, including while a completed package action waits for a phase's
completion condition. Actor procedure, animation, and movement progress remain
separate and may be reconstructed by the runtime. Legacy saves without `SCAB`
continue to resolve actors from current quest aliases. The optional `SDLG`
section retains a speaking action's selected topic and INFO, the current
response's position and remaining suffix by response position, plus its
remaining subtitle deadline. Response numbers are not used as identities
because they need not be unique. Loading rebuilds the INFO from plugin records
and does not rerun its begin fragment. The active audio handle isn't restored
or replayed; the saved relative time is restored against the new runtime clock,
and a saved zero-remaining deadline advances on the next update. Voice playback
position and duration are not preserved, while a remaining queued response
starts normally. Saves without `SDLG`
don't contain this continuation and may select a new INFO. The optional `SLRP`
section retains a looping dialogue action's already chosen remaining wait
between lines, including a zero-remaining deadline; it does not reroll the
interval during loading. It applies only to a running looping action between
INFO lines. The optional `SRSQ` section preserves the pending scene-restart
queue as an ordered list, including repeated entries. Applying it only restores
the queue; the next normal scene update attempts each start with the usual
quest, alias and condition checks. Legacy saves clear this queue. The optional
`SFGT` section retains a greeted actor marker only while a scene-owned force-greet
package is active, so the package can complete after load without reopening the
greet. The conversation itself is not saved: a load discards any live conversation
without running goodbye, INFO-end or arrest-completion side effects. If a save
was taken mid-greet, its start-time completion marker is retained, so the greet
is treated as accepted/completed after loading rather than continued from its
current line. This is a native policy, not verified Skyrim SE save behavior.

Before applying a native snapshot, scene records and their action references are
checked against the loaded plugin definitions, including phase/action bounds,
dialogue topics and child INFO membership, plus consistency between the optional
scene adjuncts. Restored action clocks must produce finite, nonnegative start
times, and dialogue/repeat deadlines must remain finite. Rejected scene data is
validated before the live game is unloaded. These checks guard this runtime's
snapshot format; they do not document Skyrim's `.ess` validation behavior.
