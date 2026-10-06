# NPC templates

Implemented in `src/world/template.rs`. Based on UESP *Mod File Format/NPC_*
(`TPLT`, `ACBS` template flags, `ATKR`), the Creation Kit wiki's *Actor* page
(template data: "Use Traits", "Use Stats"...) and the game's data files.

## What is known

- `ACBS` bytes 18..20 hold the template flags: 0x1 traits, 0x2 stats, 0x4
  factions, 0x8 spell list, 0x10 AI data, 0x20 AI packages, 0x40 (model /
  animation, unused), 0x80 base data, 0x100 inventory, 0x200 script, 0x400 default
  package list, 0x800 attack data, 0x1000 keywords.
- `vrm-tool npc-templates`: about 4,700 templated NPCs, 670 of them on a leveled
  NPC list; 1,040 NPCs name an attack race (`ATKR`) and 124 list attacks of their own.
- Leveled templates are picked with the reference's seed, the same pick as the
  actor's looks.

## Open questions

- A part that isn't templated but is missing from the NPC (no factions, no combat
  style, no name) is taken from the template. That was the engine's earlier
  behaviour here and avoids blank names and factionless bandits, but whether the
  game does it is unverified.
- NPC attack data: read as the attack race's attacks (else the race's), with the
  NPC's own `ATKD` / `ATKE` replacing those of the same event. Whether the NPC's
  list replaces the race's entirely is unverified.
- Actors' `HasKeyword` checks the keywords part and the race (`ActorTypeNPC` and
  the like are race keywords). Keywords added by worn items or magic effects aren't.
- Spell lists and default package lists (`DPLT`) aren't used yet.
