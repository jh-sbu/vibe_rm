# Quest alias fills

Implemented in `src/aliases.rs` (filling), `src/locations.rs` (editor and
current locations, location ref types) and `src/created.rs` (created references). Based on the Creation Kit wiki's *Quest
Alias Tab* and UESP *Mod File Format/QUST* and */LCTN*.

## What is known

- `vrm-tool alias-fills <data>` counts aliases by fill type (all quests, start
  game enabled ones, ones with packages) and the condition functions they use;
  `SHOW="ref conditions"` lists them. Of 16,100 aliases: unique actors
  3,513, forced 3,228, location alias references (`ALFA` + `ALRT`) 2,320, from
  event 2,055, find matching (conditions only) 1,191, created 968, empty
  (script-filled) 983, external (`ALEQ` + `ALEA`) 164; location aliases are
  mostly from events, specific locations and conditions.
- Fills run in record order, so conditions (`GetIsAliasRef`, `GetIsEditorLocAlias`...)
  see the aliases above; a quest whose required alias (no `Optional` flag, `FNAM`
  0x2) finds nothing doesn't start, and `Start()` returns false. All 443 start
  game enabled quests start.
- Find matching reference: persistent references (or, with "In loaded area",
  the loaded cells' references and actors), skipping ones already in the quest
  (unless "Allow reuse"), held by another running quest's reserving alias
  (unless "Allow reserved"), dead or disabled (unless allowed); "Closest" takes
  the one nearest the player, else the first in load order.
- Location ref types come from the locations' `ACSR` / `LCSR` lists and, for most
  (Boss, LocationCenterMarker, CWSoldier... `vrm-tool ref-types`), from the
  references' own `XLRT` in their location (`XLCN`) or their cell's. A location
  alias reference is searched in the location, then in the locations within it.
- Editor locations: the locations' persistent / unique / static reference lists,
  then the reference's `XLCN`, then its cell's (for worldspace-persistent
  references, the exterior cell under them). `GetIsEditorLocation` /
  `GetIsEditorLocAlias` / `GetInCurrentLoc(Alias)` accept locations within the
  one asked for.
- Packages located at a location alias (`PLDT` kind 9) stay about the actor's
  editor place when it is in that location, else go to the location's marker
  (`MNAM`).
- Near alias (`ALNA` the alias, `ALNT` always 0): 355 aliases, all with
  conditions, mostly roadside encounters' travel and scene markers found by
  the trigger that started them (`IsLinkedTo` the trigger alias, `HasRefType
  WETravel`). Filled with the matching reference nearest what the near alias
  holds, among the loaded references when it is loaded, else its cell's and
  (outdoors) the cells around it and the worldspace's persistent ones.
- Conditions with "use aliases" (`CTDA` flag 0x02): a reference or location
  parameter is an alias id of the condition's quest; an empty alias matches
  nothing.
- Stopping a quest empties its aliases.
- Filled reference aliases' `ALFC` factions now contribute temporary actor
  memberships through the shared faction queries. Clear/refill and quest stop
  remove that contribution; script ranks and base membership remain separate.
  Rank/conflict policies and source: [locks.md](locks.md#quest-alias-factions).
- Created references (`ALCO` object, `ALCA` the alias to make it at, high bit
  set for "in" its inventory): 968 aliases, 570 of them NPCs, 293 "in". They get
  form ids from `FF000800` up. Items made in a container go to its inventory;
  actors join the persistent actors' whereabouts at the place they were made
  (spawned at once when it is loaded) and run their packages. The 443 start game
  enabled quests make 94 at startup (notes, rewards, MS09's Geirlund and Vidrald...).
  Papyrus `PlaceAtMe` / `PlaceActorAtMe` and console `placeatme` make them too.
- Created references are cleaned up when their quest stops or its aliases are
  filled again: items made in a container (not the player's) are taken out of
  it, objects made in the world are disabled and forgotten. Ones the player
  has (made in their inventory, or picked up) stay.

## Confirmed in the game (`test-mod autotest4`, 2026-10-13)

Optional aliases of a quest of ours, started by script:

- Forced to a reference disabled first, and unique to an actor killed first
  (neither alias allowing it): empty. Unique to an actor with no placed
  reference: empty, the quest starting all the same.
- Created at the player from an NPC with the PC level mult (x1), at create
  levels easy, medium and very hard: level 1 each (the player's), so the
  create level made no difference to it.
- Matching (in the loaded area) two NPCs placed 500 then 200 units from the
  player: the first placed without "Closest", the nearer with it.

`test-mod autotest8` (2026-10-22), optional aliases of a start-game-enabled
quest added to a save: unique to an NPC whose one reference isn't flagged
persistent (in an unloaded interior, in memory and moving by its package):
empty. Forced to such a reference: filled.

## Open questions

- Clean-up (unconfirmed): no public source we could reach (CommonLibSSE's alias flags, the
  data) says what is deleted and when; the cases are our choice. Actors
  are kept (a created follower or quest giver vanishing on `Stop` would be
  worse than one lingering), the player's items too. Whether the game also
  deletes them when the alias is cleared by script (`Clear`), or when the
  quest completes rather than stops, isn't known.
- What `ALNT` other than 0 would mean, and how far "near" reaches.
- The create level (`ALCL`) is ignored; whether it changes a leveled actor's
  pick isn't known.
- Which of several matching references the game takes without "Closest" (the
  first placed of two in the game; here the first persistent one in load
  order, else the first loaded), and whether it searches anything beyond
  persistent references when "In loaded area" isn't set.
- Unique actors whose reference isn't flagged persistent: the engine fills
  them (Suvaris Atheron's `DialogueWindhelm` alias, which `WindhelmBullyScene`
  needs), but autotest8's stayed empty. Vanilla ones like Suvaris are in
  their location's unique actors (`ACUN`) and their quests start with a new
  game; which of those makes the difference (if either) isn't known.
- Location aliases don't reserve locations; location keyword data is only
  what scripts set (`SetKeywordData`); cleared locations:
  `known_ambiguities/cleared.md`.
