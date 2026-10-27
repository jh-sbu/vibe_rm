# hkbRotateCharacterModifier

The reader now retains the authored character-rotation settings, and
`Instance::character_rotations` exposes running modifiers with supported scalar
inputs resolved, and `Instance::character_turn(dt)` turns them into a yaw. The
engine adds that yaw to the frame's root-motion yaw (`world/behavior.rs`), which furniture consumes. Ordinary NPCs and the player now let active rotate
modifiers own yaw, without adding the AI/controller turn a second time.

## Public and asset evidence

- The community-authored [Skyrim Behavior Editor class](https://github.com/BrannigansLaw/Skyrim-Behavior-Editor-/blob/master/src/hkxclasses/behavior/modifiers/hkbrotatecharactermodifier.cpp)
  reads and writes `enable`, `degreesPerSecond`, `speedMultiplier` and
  `axisOfRotation`. Its [editing UI](https://github.com/BrannigansLaw/Skyrim-Behavior-Editor-/blob/master/src/ui/hkxclassesui/behaviorui/modifiers/rotatecharactermodifierui.cpp)
  identifies the first three as bool/real/real and the axis as a vector4, and
  permits variable or character-property bindings on each. These establish the
  authoring vocabulary and types, not the original engine's update algorithm.
- Read-only inspection of installed Skyrim SE **game assets only**, using
  `vrm-tool hkb-obj <Data> meshes/actors/character 0_master.hkx
  hkbRotateCharacterModifier 10`, found `Default_RotateModifier` with enable at
  0x48, rate at 0x50, multiplier at 0x54, and aligned vector at 0x60. The
  separation between the two scalar fields and vector agrees with their public
  types. The reader uses those observed offsets and retains all four vector
  components. The object's trailing runtime-sized storage is not interpreted.
- `vrm-tool hkb-vars <Data> meshes/actors/character turn` identified a
  `degreesPerSecond` binding to `TurnDelta` in `0_master.hkx`,
  `blockbehavior.hkx`, and `mt_behavior.hkx`. Thus authored constant rates alone
  would lose an installed asset's actual input. The meaning of `TurnDelta` as
  supplied by the original engine is not established by this binding alone.

No original game engine binaries, SDK source mirrors, or leaked documents were
examined. No game assets or copies of asset data were added to the repository.
Tests use invented packfile contents and graphs.

## Supported subset and unresolved behavior

- Active settings follow the existing runtime's modifier traversal and
  event-driven activation/deactivation. Authored enable, scalar variable and
  character-property bindings to enable/rate/multiplier are resolved. A binding
  to an unsupported member (including the vector axis), or an unavailable scalar
  input, omits the modifier rather than presenting stale authored input. The
  active-settings accessor neither multiplies rates nor normalizes/interprets
  the axis.
- Chosen step: counter-clockwise yaw per update = `degreesPerSecond *
  speedMultiplier` in radians times `dt` times the vertical (Z) component of the
  normalized axis. All vanilla instances inspected use axis (0,0,1) with
  rate 90 (bound to `TurnDelta` in the three character behaviours) and
  multiplier 1, so the axis space, sign convention (positive = counter-clockwise,
  right-handed Z up) and the multiplier as a plain factor are guesses that the
  data does not contradict. Horizontal axis components are ignored (no pitch or
  roll of the actor); a zero axis turns nothing; W is unused. Whether the engine
  applies the product per frame as a rotation delta (as here) or the rate is
  a target rotation speed folded into the animation-driven yaw is unconfirmed.
- Contributions of several running modifiers are summed, added to (not blended
  with) the weighted-average clip yaw. Branch weights are not applied.
- Composition with clip root motion, actor-facing controls, generator transforms,
  and ordering when multiple rotation modifiers run remain unresolved. The
  current active-modifier traversal does not establish branch weights, masks,
  outgoing transition participation, or original enable-gate lifecycle behavior.
  While a modifier is active, AI/player facing supplies the desired turn,
  clamped to the movement type rate, as TurnDelta in degrees per second
  (counter-clockwise positive). The controller suspends direct heading changes
  and consumes character_turn(dt) once, clamped to the requested step so it
  cannot overshoot. Normal controller turning resumes without a modifier.
  A modifier activated during an update takes ownership on the next update;
  the already applied controller turn is not applied again.

- The AI and player body already supply `TurnDelta`, but its original-engine
  units and meaning relative to this modifier remain unverified.

Synthetic tests cover alignment-sensitive field reads, all four axis components,
signed scalar values, authored disable, live scalar bindings, event gates,
character-property inputs and variable precedence, unavailable scalar properties,
and vector variable/property omission. They verify this implementation's data
handling, not original-engine rotation parity.

Installed-asset fixture correction: the one-handed standing draw path in
weapequip.hkx does not activate a rotate modifier. After drawing, the
standing Unequip path in 0_master.hkx does (`1HM_Unequip_ModifierGenerator`).
The integration fixture therefore draws and then sheathes before checking
modifier-owned turning, exact requested rate and no overshoot. The plan's
suggestion to check the active modifier during that draw alone was inaccurate.
