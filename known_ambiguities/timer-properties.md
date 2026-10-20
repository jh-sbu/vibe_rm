# Timer modifier scalar deadline inputs

`hkbTimerModifier.alarmTimeSeconds` now resolves graph-variable and read-only
scalar character-property bindings at the runtime's existing elapsed-time
comparison. This closes a retained-but-ignored control-modifier input; it does
not establish original-engine lifecycle parity.

## Public evidence

The local UESP index returned no matches for `hkbTimerModifier`. The public,
community-authored [Skyrim Behavior Editor timer reader/writer](https://github.com/BrannigansLaw/Skyrim-Behavior-Editor-/blob/7b7167cb18e28f435a64069acbf61eca49a07be3/src/hkxclasses/behavior/modifiers/hkbtimermodifier.cpp)
names `alarmTimeSeconds`, the modifier's `variableBindingSet`, and its alarm
event/payload. The [header at the same revision](https://github.com/BrannigansLaw/Skyrim-Behavior-Editor-/blob/7b7167cb18e28f435a64069acbf61eca49a07be3/src/hkxclasses/behavior/modifiers/hkbtimermodifier.h)
represents the deadline as a real value. These establish the member vocabulary,
not how the original engine evaluates bound deadlines. The public community
[Souls Modding binding guide](https://www.soulsmodding.com/doku.php?id=topic%3Ahavok_behavior_editing#hkbvariablebindingsets)
describes character properties as constant member inputs in binding sets; it
concerns another game's Havok version. Existing Skyrim asset-backed binding
layout findings are recorded in [character-properties.md](character-properties.md).

This extension uses that existing parsed binding path. No additional game
assets were inspected or added. No SDK mirrors, leaked documents or engine
binary data were used as implementation evidence.

## Supported behavior and unresolved timing

- Names resolve case-insensitively against the current character's scalar
  properties. Properties remain read-only and outside graph variables. Existing
  graph-variable input precedence is preserved.
- A missing character, absent scalar property or non-finite input retains the
  authored deadline. A non-finite variable input does not fall through to a
  duplicate property. Finite nonpositive deadlines fire on the first update,
  as authored nonpositive deadlines already did. These are project policies,
  not established original-engine invalid-input behavior.
- The timer compares its accumulated active time with the resolved deadline
  every update. Changing a graph variable can extend the deadline or cause an
  already elapsed deadline to fire on the next update, including a zero-time
  update. Elapsed time is not reset by an input change. Whether Skyrim samples
  this input at activation, every frame or at another boundary remains unknown.
- Existing lifecycle rules are preserved: one alarm per activation, with the
  original event/payload; a new activation resets and rearms the timer; a disabled
  modifier pauses elapsed time and firing. Original-engine disabled-time,
  reactivation and output-binding semantics remain unverified.

Synthetic runtime tests exercise separate character deadlines, case-insensitive
lookup, read-only properties, event/payload preservation, one-shot and reentry
behavior, enable pauses, live variable precedence, absent/non-finite fallbacks,
and zero/negative deadlines. They verify this supported input path and existing
runtime policies, not Skyrim engine parity. No vanilla Skyrim use of a timer
deadline character-property binding is claimed.
