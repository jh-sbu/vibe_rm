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
- More events (CK wiki, CommonLibSSE `BGSAddToPlayerInventoryEvent`):
  player add item (`AIPL`: R1 owner, R2 container, L1 location, F1 item,
  V1 acquire type: 0 none, 1 steal, 2 buy, 3 pickpocket, 4 pick up,
  5 container, 6 dead body); actor hello (`AHEL`: R1 the one greeting, R2
  the one greeted, L1); dead body (`DEAD`: R1 who found it, R2 the body,
  L1); assault (`ASSU`: R1 victim, R2 attacker, L1, V1 crime); change
  relationship rank (`CHRR`: R1, R2, V1 old rank, V2 new).
- Crime events (CK wiki, the Papyrus `OnStory*` events' parameters):
  crime gold (`ADCR`: victim, criminal, faction, gold, crime -1 none, 0
  steal, 1 pickpocket, 2 trespass, 3 attack, 4 murder, 5 escape), arrest
  (`ARRT`: arresting guard, criminal, location, crime), jail (`JAIL`: guard,
  crime group, location, crime gold), escaped jail (`ESJA`: location, crime
  group), served time (`STIJ`: location, crime group, crime gold, days). The
  data's from-event aliases settle `ARRT` R1 guard / R2 criminal and `JAIL`
  L1 the jail's location (`JailQuest`'s `EscapeLocation`). Usage: `ARRT`
  starts `TG00ArrestMonitor` (`TG00` before stage 10) and `DGArrestQuest`
  (criminals other than the player); `JAIL` `JailQuest` (on escaping,
  "Retrieve your possessions" from the hold's prison chest; it ends on
  leaving the hold), `EastmarchJailArenaFightQuest` (Windhelm) and
  `DB03GetArrestedQuest` (its node never passes); `ESJA` `EscapeJailQuest`
  and `EscapeJailAchievementQuest`. No node listens for `ADCR` or `STIJ`.
- Papyrus: a quest started by an event gets `OnStory...` with the event's
  data (`Quest.psc`'s parameter lists; `vrm-tool pex-events <data>
  OnStory` lists the scripts using each: `OnStoryScript` 11 (the
  Companions' radiant quests, Civil War missions, `ClearSkiesQuestScript`),
  `OnStoryKillActor` 3, `OnStoryChangeLocation` 2...). Pay fine
  (`OnStoryPayFine`): criminal, guard, crime group, crime gold.
- Quests' own event conditions: the `QUST` CTDAs after `NEXT` and before the
  stages, checked when a node starts the quest (ADIA 104 quests, SCPT 124,
  CLOC 39...: `OWN=1 vrm-tool story`).
- Relationships (`RELA` `DATA`: parent NPC, child NPC, rank counted from
  lover 0 to archnemesis 8, flags, association type); conditions and Papyrus
  count ranks 4 (lover) to -4, 0 for acquaintances and strangers.
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
- Quests' own event conditions run on the player, like node conditions.
- Item pickup: R1 is a person owner's placed reference (none for a faction
  owner). Whether it is stolen: see "Ownership" below.
- Hellos: sent when an NPC greets the player in passing, before the greeting
  is chosen, so the quests it starts can supply it.
- Assaults: the first blow an actor takes from someone it isn't fighting; a
  crime when it wasn't fighting anyone and keeps the law (a faction that
  tracks crime). Blows to the player send none.
- Bodies: a living, calm humanoid within 1000 units with nothing between
  its eyes and the body (once a second), once for each finder and body.
- Relationships are symmetric, kept on NPC records (`SetRelationshipRank`
  on a reference changes it for its NPC).
- Crime events' members beyond those: the other references R1, R2 in
  order, forms (the faction, the crime group) as F1, values V1, V2 in
  order. The crime group is the crime faction (not its `CRGR` list). The
  arrest event is sent when the player pays or goes to jail through the
  guard who stopped them, for the last crime reported to that faction;
  `ADCR` for crimes witnesses report and escaping, not for scripts'
  `ModCrimeGold`. Location: the player's for an arrest, the jail cell's
  (`XLCN`) for the others.
- The `OnStory...` event is sent right after `OnInit`, before the
  startup stage. Its members follow the parameters' order (references,
  locations, forms and values as R1 / R2, L1 / L2, F1, V1 / V2), the same
  reading as the crime events'. `OnStoryIncreaseSkill`'s skill name has no
  member yet: the skill increase event carries the skill's actor value index
  as V1, and `OnStoryIncreaseLevel` gets the new level as V1.
- Quests started from Papyrus (`Quest.Start`, story events) get their scripts
  and OnInit once the running scripts yield, in the same frame.

## Ownership (stealing)

The owner is the reference's `XOWN`, else its cell's; bodies belong to no
one. Taking something is stealing when:

- a person (an NPC record) owns it: always, whatever their relationship;
- a faction owns it, the faction can own things (`FACT` `DATA` flag 0x8000,
  "can be owner": all 384 owning factions have it, `vrm-tool
  faction-owners`; a faction without it owns nothing), and the item's base
  value is over the faction's favor cap. The cap is the lowest among living
  members ranked friend or better with the player: friend
  `iFavorFriendValue` (25), confidant `iFavorConfidantValue` (50), ally
  `iFavorAllyValue` (100), lover `iFavorLoverValue` (500). Members at
  acquaintance or below don't count, and with none above it there is no
  allowance. Only the base value counts, never the stack's.
- Never for what the player or one of the player's factions owns.

Members are the NPC records listing the faction at rank 0 or more (their
own or their templates'); a member is dead when their placed reference is.

## Open questions

- Ownership: faction membership doesn't change at runtime
  (`AddToFaction` / `RemoveFromFaction`, the player joining a faction), so
  neither does the favor cap; a member that is a generic NPC with several
  references counts as dead only by its first. The required faction rank
  on owned references (`XRNK`) is ignored, so a player in the owning
  faction may take everything. Theft seen by a witness adds crime gold
  (`known_gaps/crime.md`); items aren't marked stolen in the inventory.

- The other events wait for their systems: crafting (`CRFT`), item removal (`REMP`: dropping items), spell cast (`CAST`), new
  voice power (`NVPE`), bribe, intimidate, flatter, lock pick.
- Kill events' crime status (V1) only knows the player's murders; item
  pickups never say bought or pickpocketed (no barter or pickpocketing).
- Hellos between NPCs, and creatures' (dogs') hellos; the real detection
  and distance rules for noticing bodies; assaults on the player.
- Relationship association types (`HasFamilyRelationship`,
  `HasParentRelationship`), secret relationships.
- `WarnIfNoChildQuestStarted`, and which runs on subject in node and quest
  event conditions.
- Story Manager state (last run times, do-all rounds) isn't saved.
