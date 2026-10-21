# Blender synchronization master

The runtime retains `hkbBlenderGenerator.indexOfSyncMasterChild` with its
authored signed index and resolves whole-member graph-variable or scalar
character-property inputs. Variable bindings take precedence over duplicate
properties. The editor identifies the member as an integer input, exposes
variable/property binding, and initializes it to `-1` in
[community source](https://github.com/BrannigansLaw/Skyrim-Behavior-Editor-/blob/master/src/hkxclasses/behavior/generators/hkbblendergenerator.cpp)
([binding UI](https://github.com/BrannigansLaw/Skyrim-Behavior-Editor-/blob/master/src/ui/hkxclassesui/behaviorui/generators/blendergeneratorui.cpp)).

The installed Skyrim SE behavior assets were read through the repository's VFS
and Packfile readers without examining engine binaries. An audit of 18 behavior
HKX paths found 588 blender generators: 574 contain `-1` in bytes `0x58..0x59`,
and 14 contain valid child indices (0 or 1 in two-child blenders, or 0 in
four-child blenders). No other or out-of-range values appeared. The following
u16 at `0x5A` contains blender flags. Together these records corroborate a
signed i16 index at offset `0x58`, the `-1` sentinel, and valid explicit indices.
No game assets are included in the repository.

## Implemented policy and limits

For synchronized blenders, a valid authored or bound index chooses which active
child raises clip triggers while all synchronized children follow the shared
phase. `-1` selects automatically; automatic selection chooses the heaviest
active child with a positive resolved weight, preferring non-additive children.
If every active positive-weight child is additive, the heaviest additive child
is a last-resort trigger source so events are not all suppressed. Missing
inputs retain the authored index. Authored or bound out-of-range and
zero-weight choices use automatic selection; fractional/non-finite bindings
retain the authored index, while an exact `-1` binding requests automatic
selection. Direct and nested synchronized blenders share this selector.
Non-synchronized blends keep
their existing independent trigger advancement, and weighted cycle duration
keeps the prior calculation.

The editor and asset records establish the field representation and authored
sentinel, but do not document the engine's trigger-selection algorithm. The
heaviest-child automatic rule follows this runtime's prior behavior. Explicit
master selection, additive fallback, and nested event routing are bounded
implementation policies verified by synthetic tests, not claims of Skyrim SE
runtime parity. Bit-level bindings are excluded because this is a whole integer
input. The local asset audit inspected only behavior HKX asset data; no engine
executable/DLL, disassembly, or SDK material was examined or committed.
