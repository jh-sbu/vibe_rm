# State-machine start-mode and synchronization inputs

`crates/havok/src/behavior/{mod.rs,runtime.rs}` now resolves scalar bindings
for `hkbStateMachine.startStateMode` and `syncVariableIndex` when the machine
activates. This extends the existing default, synchronized and random start
algorithms; it does not implement chooser starts.

## Evidence and provenance

The community-authored [Skyrim Behavior Editor state-machine class](https://github.com/BrannigansLaw/Skyrim-Behavior-Editor-/blob/master/src/hkxclasses/behavior/generators/hkbstatemachine.cpp)
lists the start-mode enum in default, sync, random, chooser order, retains
`syncVariableIndex` independently of the selected mode, uses -1 as its initial
index, and reads/writes both members and a variable binding set. The public
[Souls Modding binding guide](https://www.soulsmodding.com/doku.php?id=topic%3Ahavok_behavior_editing#hkbvariablebindingsets)
describes variables and constant character properties as binding inputs to
object members. That guide concerns another game's Havok version. These
sources support extending member input resolution; they do not establish
Skyrim's original runtime lifecycle or a vanilla use of these bindings.

The reader already consumed the authored mode at 0x86 and synchronization
index at 0x7C. It now retains the index even for authored default/random
modes, allowing a bound mode to select the existing sync path. No new binary
layout is inferred. Only public community sources and invented test data
were used. No game assets, SDK mirrors, engine executable/DLL data,
disassembly or other engine binary data were examined or added.

## Supported behavior and unresolved questions

- Modes 0, 1 and 2 select the existing default, sync and seeded-random paths.
  Chooser mode 3, fractional, non-finite and other out-of-range inputs retain
  the authored mode. Chooser layout and selection remain unresolved as
  described in [`selectors.md`](selectors.md).
- A synchronization index addresses the active graph's local variable table,
  which the runtime maps to its project-wide table. The bound value is an
  index, not a state ID; the selected variable supplies the state ID. Index
  -1 disables synchronization. Fractional, non-finite, negative values other
  than -1 and indices outside the graph's variable table retain the authored
  index. A sync mode with no index uses the existing default start path.
- Missing characters, missing names and non-scalar properties preserve
  authored inputs. Property names resolve case-insensitively. Graph-variable
  inputs take precedence, including malformed values: an invalid variable
  does not fall through to a duplicate property. Properties remain read-only
  and outside the animation-variable namespace.
- Mode and index are resolved once per machine activation. Sync activations
  keep their resolved output target through transitions; changes to bindings
  while active do not redirect state writes. Re-entry reads the new inputs.
  The existing sync write timing remains: transitions publish to that graph
  variable after activating the destination, while activation only reads it.
- Nested transition targets, explicit `startStateId` bindings and mode-based
  selection retain their existing precedence, in that order. Invalid state
  IDs retain the existing fallback to the first authored state. Random-state
  selection and transition algorithms are unchanged.

The original engine's support for these bound members, sampling/update timing,
invalid/duplicate binding policy, synchronization output ordering, enum and
integer conversions, and random selection remain unverified. The policies
above define this implementation's supported input path and safeguards, not
proven engine parity. Runtime property overrides and scalar-property
evaluation in the static event-to-clip planner remain unsupported.

Synthetic tests cover retained independent indices and graph-local binding
names, default/sync/random property inputs, distinct characters, read-only
properties, missing/non-scalar inputs, invalid enum/index fallbacks, -1,
graph-variable precedence, graph-local remapping across references, unchanged
explicit-start precedence, activation-only selection and sync-output targeting,
and resampling after re-entry. They validate this implementation, not the
unresolved original-engine behavior.
