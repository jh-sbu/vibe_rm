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

## Spell-list inheritance

`Sources::spells` and `Sources::perks` read `SPLO` and `PRKR` from exactly the
NPC selected by the `0x8` template flag, following nested flags and the same
seeded leveled-template choice as the other actor parts. A missing list is
empty, including when the selected NPC has a further template whose spell-list
flag is clear. The child's lists do not merge with the selected source's.
Racial spells come separately from the race selected by the traits part; they
do not come from the spell-list source's race unless it also supplies traits.

Public evidence (consulted 2026-10-15):

- The Creation Kit's [Actor documentation](https://ck.uesp.net/w/index.php?title=Category%3AActor%2Ffr)
  describes template checkboxes as selecting the corresponding tab's data;
  unchecked tabs retain manually authored values. This is the French
  translation of the public CK documentation.
- A modder's [description of template inheritance](https://www.reddit.com/r/skyrimrequiem/comments/qvn7xx/)
  explicitly reports that the spell-list flag replaces both spells and perks,
  rather than layering the child's entries over the template's. The
  [Creation Kit editing instructions](https://forums.nexusmods.com/topic/12011978-le-please-someone-teach-me-how-to-add-perks-since-5-years-i-couldnt-find-it/)
  likewise place perks in SpellList and require clearing the template flag to
  edit them. These are community observations, not controlled game tests.
- [xEdit's public Skyrim record definitions](https://github.com/TES5Edit/TES5Edit/blob/dev-4.1.6/Core/wbDefinitionsTES5.pas)
  define `SPLO` as an actor-effect FormID, `PRKR` as a perk FormID plus rank,
  and `SPLO` on races as well as NPCs. Its
  [template flag table](https://github.com/TES5Edit/xEditScripts/blob/master/Scripts/Generate%20Bash%20Tags.pas)
  maps Use Spell List to `8`. These establish storage and flag values, not
  empty-list runtime behavior.

The shared `Engine::actor_spells` path feeds `HasSpell`, NPC offensive spell
selection, NPC shouts and ability synchronization for the player and loaded
living NPCs. Inherited perks' ability entries still enter through
`actor_perks`. Script additions/removals and final spell deduplication remain
applied after the record lists. Synthetic plugin tests cover empty local and
selected lists, replacement, nested flags, independent racial inheritance,
leveled bases/templates, and cross-plugin FormIDs without game assets.

Remaining questions and limits:

- No controlled game observation establishes the case where an unflagged
  NPC's list is empty, or where an inherited source's list is empty but its
  further template has entries. Treating these as empty is an inference from
  the documented tab-selection semantics, not measured compatibility.
- Ability effects currently synchronize on the first magic update, then every
  real second. An NPC loaded between syncs can wait until the next one. Exact
  engine timing relative to loading and script events is not established by
  these sources; this change does not alter that cadence.
- When Skyrim rolls a leveled spell list (on load, reset, first use, or another
  time), and whether reset rolls anew, remain unconfirmed.

## Leveled spell lists

`SPLO` can also name a leveled spell list (`LVSP`; xEdit's definitions:
`LVLD` chance none, `LVLF` flags, `LVLO` level / spell / count). The base
game's NPCs name 29 of them, about 380 NPCs in all (`vrm-tool npc-templates`
lists them): every bandit and ghost mage's destruction spell
(`LSpellBandit01FireFrostShock`...: one of fire, frost or shock), dragon
priests' spells, draugr overlords' 50 / 66% extra spell, the world-encounter
battlemages' and conjurers' hands, vampires' 50% reanimation spells, Miraak's.
Without expansion none of these actors had the spells.

`Engine::actor_spells` now expands them (`world::actor::resolve_spells`), as
leveled items are: "use all" (`0x04`) gives every eligible entry, otherwise
one is picked; "calculate from all levels" (`0x01`) makes every entry up to
the level eligible, otherwise those of the highest level reached; chance of
none can leave nothing; nested lists are followed. Entries' counts are
ignored, and so "each item in count" (`0x02`).

Choices not settled by public sources (research on 2026-10-16 found none, see
the [Skyrim LE report](https://forums.nexusmods.com/topic/9690643-le-how-do-you-add-a-leveled-spell-list-to-an-npc/)
that a copied actor's leveled spell list yields nothing):

- The level is the actor's own (`actor_level`), not the player's as for
  leveled NPCs and items. Only two vanilla lists have entries above level 1
  (`LSpellWEConjurerRighHand` 1 / 10 / 20 / 35, `DLC2MiraakLSpells` up to
  60), so the choice touches little.
- The native runtime uses the actor's form id as the seed and rolls the list
  the first time its spells are asked for (a fight, `HasSpell`...), at its level
  and globals then, keeping the result for the session. Optional `RLSP` save
  data retains that result per
  actor and list, including an empty result, so a changed level or global after
  loading does not reroll it. Older saves without the section clear the cache.
  This is a native `.vrs` policy, not evidence of Skyrim SE save behavior.
- `HasSpell` on the list's own form id is false (the actor has the rolled
  spells, not the list).
- The copied-actor report above (nothing given) isn't followed: vanilla
  actors would lose their spells.

An autotest is planned in `plans/autotest-survey.md` (tier 2).

## Open questions

- A scalar part that isn't templated but is missing from the NPC (no combat style, no
  name) is taken from the template. That avoids blank names, but whether the
  game does it is unverified (Papyrus can't read either back). Factions aren't:
  an NPC templated without the factions flag and listing none of its own is in
  none of its template's (`test-mod autotest2`).
- NPC attack data: read as the attack race's attacks (else the race's), with the
  NPC's own `ATKD` / `ATKE` replacing those of the same event. Whether the NPC's
  list replaces the race's entirely is unverified.
- Actors' `HasKeyword` (Papyrus and the condition) counts the NPC's own
  keywords (through the keywords template) and its race's, not those of
  what it wears or of the magic effects on it. CommonLibSSE shows only that
  `Actor` overrides `HasKeywordHelper`, not what it adds; the vanilla
  conditions give no sign either way (they test item and effect keywords on
  the item or spell in perk tabs, `WornHasKeyword` and
  `HasMagicEffectWithKeyword` exist for the rest, and `Vampire` is a race
  keyword).
