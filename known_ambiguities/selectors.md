# Behavior selectors' own blends

Still a roadmap item; no selector blending was implemented in this pass.

The current reader handles `hkbManualSelectorGenerator`'s generators and
selected index, with variable bindings choosing the child. Changing the
index immediately deactivates the old child and activates the new one.

The community-authored [Souls Modding Havok behavior editing guide](https://www.soulsmodding.com/doku.php?id=topic%3Ahavok_behavior_editing)
shows `selectedIndexCanChangeAfterActivate`, `indexSelector`, and
`generatorChangedTransitionEffect` on a selector in a different game's
Havok version. That does not establish these fields' existence, serialized
layout, defaults, or behavior in Skyrim SE. In particular it does not
settle whether Skyrim selectors have their own authored cross-fade or
whether a surrounding state machine supplies the blend. No Havok SDK
source mirrors or engine binaries were used to resolve this.

Before implementing the roadmap item, inspect Skyrim-authored selector
assets for an appropriate member/schema or find Skyrim-specific public
community documentation. If supported, activation-only selection, blending
interruption, trigger forwarding while blending and invalid-index behavior
will also need evidence or observable game tests. The immediate-switch
implementation remains a compatibility approximation until then.
