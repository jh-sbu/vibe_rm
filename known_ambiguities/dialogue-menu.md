# The dialogue menu (dialoguemenu.swf)

Implemented in `src/swf_ui/dialogue_menu.rs`. Sources: the movie's
ActionScript (`DialogueMenu`: the `SetSpeakerName`, `ShowDialogueText`,
`HideDialogueText`, `NotifyVoiceReady`, `PopulateDialogueList`,
`ShowDialogueList`, `StartHideMenu` and `Cancel` callbacks; the
`TopicClicked(index)`, `SkipText`, `CloseMenu` and `FadeDone` calls; the
list's entries as `text, topicIsNew, topicIndex` triples ending with the topic
to select, -1 for none; `ALLOW_PROGRESS_DELAY` 750 ms; `DialogueCenteredList`
greying entries that aren't new).

Response selection and the shared Rumors entry are described, with sources
and unresolved arbitration rules, in [dialogue-selection.md](dialogue-selection.md).
Entries retain the INFO selected when the list was built; clicking does not
select a new random response. The pooled Rumors entry uses a stable logical
identity so that the last-clicked option remains selected when its
underlying topic changes; RUMO topics among an INFO's linked choices keep
their own.

## Unconfirmed: choices made

- **When it is told what.** A line starting: `ShowDialogueText` and at once
  `NotifyVoiceReady` (the game's voice may take longer to be ready). A line
  ending: `HideDialogueText`. The speaker done: `PopulateDialogueList` and
  `ShowDialogueList`. When the topics come without a line said since the last
  click (no greeting, a topic with no lines), `NotifyVoiceReady` is sent
  then, or the menu would take no clicks.
- **New topics.** A topic is new (white) unless the line (`INFO`) it would
  get now has been said to the player before; heard ones are grey.
- **`ShowDialogueList(newList, hideExit)`.** `newList` is true when the
  topics differ from the last ones listed (they slide in, the clicked topic
  copied above them), false for the same topics again (they fade in). The
  exit button is never hidden.
- **The selected topic.** When the list comes back, the topic last clicked,
  if it is in the list.
- **No Goodbye topic.** Tab (the movie's own) and its exit button leave, and
  Esc, as the game's Cancel: during the greeting it skips the line.
- **The mouse.** The menu acts on only every second `onMouseDown` it gets
  (`iMouseDownExecutionCount`), as if the game delivered two a press; Ruffle
  delivers one, so the count is reset before each press.
