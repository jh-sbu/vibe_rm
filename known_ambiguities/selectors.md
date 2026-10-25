# Behavior selectors' own blends

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

Even if Skyrim selectors blend, activation-only selection, blending
interruption, trigger forwarding while blending and invalid-index behavior
are unestablished. The immediate switch is a compatibility approximation.

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

## Chooser start-state audit

The public community-authored [CommonLibSSE-NG state-machine header](https://github.com/CharmedBaryon/CommonLibSSE-NG/blob/main/include/RE/H/hkbStateMachine.h)
identifies chooser start mode as 3 and the `startStateChooser` pointer at 0x60.
A read-only audit of 423 behavior HKX assets under `/behaviors/` in the installed
SE `Skyrim - Animations.bsa`, `Skyrim - Meshes0.bsa`, and `Skyrim - Meshes1.bsa`
found 3,602 state machines: 3,220 default, 184 sync and 198 random; none used
chooser mode or had a non-null chooser pointer, and no chooser object classes
occurred. All 581 manual selectors had zero bytes at 0x59..0x5F. These findings
do not prove what those selector bytes mean or that chooser mode cannot be used
by mods or other assets. Chooser object layouts, selection rules and lifecycle
remain unresolved; the reader/runtime still approximates mode 3 as default.
No chooser or selector-blend implementation was inferred from this absence.
Only asset data and public community source were examined, with no engine
binary inspection and no asset copies added to the repository.
