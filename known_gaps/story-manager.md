# Story Manager

Implemented in `src/story.rs` (tree, events, event data) and
`crates/esp/src/story.rs` (`SMEN` / `SMBN` / `SMQN`); `vrm-tool story <data>
[event]` counts each event type's nodes, quests and the event members used,
or prints one event's tree.

## What is known

- Records (UESP, CommonLibSSE): parent `PNAM`, previous sibling `SNAM`,
  conditions after `CITC`, `DNAM` node flags (0x1 random, 0x2 warn if no
  child quest started) and quest flags (0x1 do all before repeating, 0x2
  shares event, 0x4 num quests to run), `XNAM` most running at once, event
  node `ENAM` event type, quest node `MNAM` count and `NNAM` quests each with
  `FNAM` flags and `RNAM` hours until reset.
- Traversal (CK wiki): an event node's conditions, then its children top to
  bottom (stacked) or at random; quest nodes start a quest; an event is used
  up by the first node that starts one unless that node shares it.
- Event data: conditions run on event data (`run_on` 7, the member code in the
  last field) and `GetEventData` (function 0 GetIsID, 1 IsInList, 2 GetValue,
  3 HasKeyword; member; form). Aliases filled from the event (`ALFE` + `ALFD`)
  take the member if their conditions accept it.
- Events sent: actor dialogue (`ADIA`: R1 the actor striking up the
  conversation, R2 the other, L1 where; CK wiki) on the `fAISocial*` settings'
  timer, chance and radius; change location (`CLOC`: R1 the player, L1 old,
  L2 new) when the player's location changes; script events (`SCPT`,
  `Keyword.SendStoryEvent(AndWait)`: K1 the keyword, L1, R1, R2, V1, V2);
  kills (`KILL`: R1 victim, R2 killer, L1 where).
- Usage in Skyrim.esm and DLCs: `ADIA` roots 919 quests (every town's
  conversations), `SCPT` 363, `CLOC` 258, `KILL` 15.

## Choices made without a source

- Node conditions run on the player (subject).
- An actor considers a conversation every 10 to 30 s (first one staggered),
  with a 10% chance, with the nearest other free humanoid within 500 units:
  alive, calm, not in a scene or talking to the player; interiors use the
  `...Interior` settings.
- Reset hours count game time since the quest was last started by the Story
  Manager; quests started otherwise don't count.
- "Max concurrent" counts the running quests of the node and those below it.
- `SendStoryEventAndWait` returns whether a quest started, at once.
- Quests started from Papyrus (`Quest.Start`, story events) get their scripts
  and OnInit once the running scripts yield, in the same frame.

## Open questions

- The other events: crafting (`CRFT`), increase level / skill (`LEVL`,
  `SKIL`), arrest / jail (`ARRT`, `JAIL`), assault (`ASSU`), actor hello
  (`AHEL`), item pickup / removal (`AIPL`, `REMP`), spell cast (`CAST`),
  dead body found (`DEAD`), change relationship rank (`CHRR`), bribe,
  intimidate, flatter, lock pick, escape jail...
- Kill events' crime status and relationship values (V1 / V2) aren't set.
- `WarnIfNoChildQuestStarted`, the quest record's own event conditions
  (`QUST` CTDAs after the dialogue ones), and which runs on subject in node
  conditions.
- Story Manager state (last run times, do-all rounds) isn't saved.
