# Leveled lists: AddForm and Revert

Public pages consulted 2026-10-24 (Papyrus Index / community CK wiki text,
summarized):
- https://papyrus.bellcube.dev/skyrimse/script/leveleditem/function/addform/
  (`AddForm(Form, int level, int count)`; works since 1.9.32; merchants that were
  talked to within about 48 hours don't show new additions because their lists
  are evaluated when the barter menu opens)
- .../leveledactor/function/addform/ and .../leveledspell/function/addform/
  (`AddForm(Form, int level)`)
- .../leveleditem/function/revert/ (removes all script-added forms)
- `refs/CommonLibSSE-NG` `TESLeveledList`: `numEntries` is a `u8`.

Implementation (`src/leveled.rs`; inference where marked):
- Additions follow the plugin's `LVLO` entries in the order added and take part
  in the same level filtering as plugin entries. They reach the resolvers
  through `Globals::leveled` (`world/actor.rs`), so item rolls, leveled actors
  (including leveled templates) and leveled spells all see them.
- `Revert` drops only the script additions. Duplicates are kept. Actor and spell
  lists store count 1. Only `LVLI` / `LVLN` / `LVSP` records take entries.
- Total capped at 255 (plugin plus added); an `AddForm` past it is ignored
  (inference: the count field cannot grow).
- Rolls already made (inventories, spawned actors, rolled spells) don't change;
  only later rolls see additions, matching the merchant caveat above.
- Saved as optional `LVAD`; legacy saves load none.

Tests: synthetic LVLI/LVLN/LVSP records; payload and legacy-load tests.
