# The title sequence (`titles.swf`)

`Game.ShowTitleSequenceMenu`, `StartTitleSequence(asSequenceName)` and
`HideTitleSequenceMenu` run the opening's titles over the cart ride
(`MQ101QuestScript`, `QF_MQ101_0003372B`). Implemented in
`src/swf_ui.rs` (`title_sequence`).

## Sources

- The Papyrus Index (papyrus.bellcube.dev, consulted 2026-11-09):
  `StartTitleSequence` "runs a given animation in the title menu";
  "Sequence1" to "Sequence4" exist.

## Game-asset observations

- `vrm-tool pex-dump ... QF_MQ101_0003372B`: stage 10's fragment calls
  `FadeOutGame(false, true, 14, 15)`, then `ShowTitleSequenceMenu()`, and
  registers `MQ101QuestScript` for an update in 4 s.
- `MQ101QuestScript.OnUpdate` starts "Sequence1", "Sequence2", "Sequence3"
  and "Sequence4" in turn, 5, 12, 5 and 10 s apart, then hides the menu.
- `titles.swf` (Interface archive, disassembled outside the repository with
  the spike harness): the root's frame script stops the clips `sequence1` to
  `sequence4` on their first frames; each clip's first frame stops it. Playing
  `sequence1` from its second frame shows "Bethesda Game Studios" fading in;
  `sequence2` "Presents". Nothing calls back to the game.

## Choices

- `StartTitleSequence("SequenceN")` plays `_root.sequenceN` from its second
  frame (`gotoAndPlay(2)`); the clip returns to its first frame's stop at its
  end. A sequence started while the menu isn't up is dropped.
- The menu draws over the HUD and under the loading and message-box menus.
- Whether SE also accepts other sequence names, or calls into the movie some
  other way, is unknown; the four clips are all the movie has.
