# Debug, Utility and game-control natives (batch 14)

Natives: `Utility.CaptureFrameRate`, `Start/EndFrameRateCapture`,
`GetAverage/Min/MaxFrameRate`, `GetCurrentMemory`, `GetBudgetCount`,
`GetCurrentBudget`, `OverBudget`, `GetBudgetName`; `Debug.ToggleCollisions`,
`ToggleAI`, `ToggleMenus`, `SetGodMode`, `SetFootIK`, `CenterOnCell(AndWait)`,
`PlayerMoveToAndWait`, `QuitGame`, `TakeScreenshot`, `OpenUserLog`,
`CloseUserLog`, `GetConfigName`, `DebugChannelNotify`, `DumpAliasData`,
`ShowRefPosition`, `dbSendPlayerPosition`, the script, stack and object
profiling functions; `Game.UsingGamepad`, `ServeTime`, `QuitToMainMenu`,
`RequestModel`, `AddAchievement`, `ShakeController`,
`SetAllowFlyingMountLandingRequests`, `ClearTempEffects`,
`SetSunGazeImageSpaceModifier`, `SetSittingRotation`.

## Sources

The Papyrus Index (papyrus.bellcube.dev, consulted 2026-11-09), descriptions
attributed to the Creation Kit wiki (CC BY-SA 4.0):

- `CaptureFrameRate(numFrames)`: a string describing the frame rate over a
  number of frames, "up to about 1K characters", comma separated.
- `GetAverageFrameRate()` and its kin: undescribed beyond their names.
- `SetGodMode(abGodMode)`: "TGM functionality".
- `PlayerMoveToAndWait(asDestRef)`: teleports the player to the named
  reference and doesn't return until they arrive; the float returned isn't
  described.
- `ClearTempEffects()`: clears temporary effects, "such as terrain effects".
- `SetSunGazeImageSpaceModifier(apImod)`: the modifier triggered when the
  player begins gazing at the sun (`IsPlayerSungazing`).
- `SetSittingRotation(afValue)`: the player's sitting camera rotation, in
  degrees, offset from the standard angle; first person only.

## Game-asset observations (`vrm-tool pex-calls`)

- The Debug natives are called only by debug scripts (`screenshot`,
  `CWScript`'s user log, `StatsPlayerPosition`).
- `ServeTime` is the jail bed's (`JailBedScript`); `QuitToMainMenu` and
  `PlayBink` only the E3 demo's finale; `UsingGamepad` and
  `SetSittingRotation(-55)` MQ101 (the cart ride); `ClearTempEffects` MQ206;
  `SetSunGazeImageSpaceModifier` Auriel's Bow (`DLC1AurielsBowPlayerScript`).
- `textures/blood/screenbloodalpha01opt.dds` and the `fBloodSplatter*`
  settings are the screen blood `TriggerScreenBlood` would draw (not done).

Choices are in `known_ambiguities/remaining-natives.md`.
