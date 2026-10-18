# Clip crop binding inputs

`crates/havok/src/behavior/runtime.rs` now resolves scalar bindings for
`hkbClipGenerator.cropStartAmountLocalTime` and `cropEndAmountLocalTime`
when the clip activates. These inputs use the existing generator binding
lookup: graph variables take precedence, character-property names resolve
case-insensitively, and absent or non-scalar properties retain authored crops.
Character properties remain read-only and outside the animation-variable table.

## Public evidence

The community-authored [Souls Modding behavior guide](https://www.soulsmodding.com/doku.php?id=topic%3Ahavok_behavior_editing)
describes character properties as constant inputs to binding sets that set
behavior-object members. Its clip-generator example names both crop members;
its introduction to behavior files describes animation cropping. This concerns
another game's Havok version and establishes the member names and common
binding concept, not Skyrim's precise crop lifecycle.

The existing Skyrim reader already reads authored clip crops at 0x58 and 0x5C
and retains arbitrary scalar generator binding names. This change extends those
existing runtime input reads without adding serialized-layout assumptions.
No installed game assets were inspected for this extension, so a vanilla Skyrim
use of property-bound crops has not been established. Only public community
documentation and invented tests were used; no SDK sources, engine executable,
DLL, disassembly or other engine binary data were examined, and no game assets
or asset bytes were added to the repository.

## Supported behavior and unresolved questions

The resolved crop pair produces the clip's stored offset and usable length.
Forward/reverse samples, user-controlled time, existing clip-local triggers and
synchronized cycle lengths all use that same clip state. World consumers receive
the resolved sample times for pose and root-motion sampling. The extension keeps
the existing `max(0, full length - start crop - end crop)` policy, including zero
usable length when overcropped and existing treatment of finite negative crops.
It does not change trigger-coordinate or root-motion extraction algorithms.

Non-finite bound values fall back individually to the authored member; arithmetic
that would create an infinite usable length falls back to the authored crop pair.
These guards are local robustness choices. Original-engine handling of invalid,
negative or non-finite inputs, missing or duplicate bindings, and crop boundaries
relative to trigger times remains unverified.

Crops are sampled at activation, alongside the existing clip start-time input,
and stay fixed for that activation. Variable changes take effect on reactivation.
Exact original-engine crop update timing is unresolved; no runtime property
override API or mutable crop policy is introduced. The static event-to-clip
planning resolver continues to use authored crops and does not evaluate these
bindings.

Synthetic tests cover distinct character crops, case-insensitive property lookup,
read-only properties, both playback directions, end-trigger timing, user-controlled
sampling, authored fallback, invalid-value guards, zero/negative crop policies,
variable precedence, activation-time sampling and synchronized clips. They verify
the supported implementation path, not original-engine parity.
