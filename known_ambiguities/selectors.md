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

## Further public and Skyrim SE asset findings

The Skyrim community's [2018 variable-binding discussion](https://forums.nexusmods.com/topic/6539001-need-help-for-binding-variable-in-hkx-xml/)
does show a mod author's XML selector with `indexSelector`,
`selectedIndexCanChangeAfterActivate` and `generatorChangedTransitionEffect`.
The author reports testing index bindings in-game. This is Skyrim-specific
evidence for the XML authoring vocabulary, but does not identify the installed
SE asset layout or settle whether that XML was generated for another version.

Read-only inspection of installed SE `meshes/actors/character/behaviors/0_master.hkx`
found `RootBoneSwitch_MSG` and `DualWieldShieldBoneRootMSG` with their generator
array at 0x48 and selected index at 0x58. Their names begin immediately at 0x60;
there is no serialized effect/index-selector pointer between the index and name.
This rules out simply reading the later-version pointer fields at those offsets
for these assets. It does not establish the meaning of the remaining bytes at
0x59..0x5F, nor the engine's defaults for omitted fields. No additional selector
fields or blends were implemented based on that uncertainty. Only game asset
data was inspected; no engine binary data was examined or committed.
