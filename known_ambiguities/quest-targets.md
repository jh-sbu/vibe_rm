# Quest targets on the compass and world map

Implemented in `src/quest_targets.rs`, with the compass in `src/swf_ui.rs`.
Sources: the data (`QOBJ`, `QSTA` (alias, flags) and the `CTDA` after each
target; load doors' `XTEL`); the CK wiki (quest objectives and targets,
`SetActive`); `hudmenu.swf` (the Quest and QuestDoor frames of the compass
marker, quest markers off the compass held at its nearer edge).

## Unconfirmed: choices made

- **Which targets.** Those of the displayed objectives not completed or
  failed, of running quests the journal tracks (`SetActive`), whose
  conditions pass with the target as the subject and the quest as the
  quest. Targets that are location aliases, or aliases left empty, show
  nothing; nor do disabled references, the player, or what the player
  carries. `QSTA`'s flag 1 (the marker ignoring locks) isn't used: the way
  through doors doesn't look at locks.
- **Where a target is.** A loaded actor where it stands, an actor not
  loaded where its schedule has it, what's in a container where the
  container is, anything else where it was placed or moved to.
- **The way there.** A target in the player's interior or worldspace is
  marked where it is. Elsewhere, the marker is the load door the way there
  starts at: the fewest load doors between interiors and worldspaces (all
  the data's, disabled ones aside), the nearest of the first doors as short.
  The game's own choice (it may weigh distances, or the navmeshes' door
  links) isn't published.
- **Every quest marker** is at full alpha and scale 100, at any distance;
  targets sharing a spot (or a door) get one marker.
- **Enemies.** Loaded actors fighting the player (not fleeing, dying or
  dead) are marked with the Enemy frame, at any distance.

## Research gate: direct location-alias targets

The roadmap's proposal to support location aliases directly as objective
targets is not yet established as Skyrim behavior. The local UESP
[QUST layout, revision 3507775](https://en.uesp.net/w/index.php?oldid=3507775)
describes `QSTA`'s target as an AliasID, and distinguishes reference aliases
(`ALST`) from location aliases (`ALLS`); it does not document how a location
alias would become a compass position. The
[LCTN layout, revision 3507725](https://en.uesp.net/w/index.php?oldid=3507725)
calls `MNAM` the World Location Marker Ref, but does not connect it to
objective targets. Mapping a location alias to `MNAM` is therefore an
inference. The existing `Engine::location_marker` fallback to a static
reference or child's marker is likewise not evidence of quest behavior.

An asset-only scan on 2026-10-20 found no direct location-alias targets in
the following installed Bethesda plugins:

| Plugin | QUST records | QSTA fields | ALLS fields | Records with a QSTA/ALLS ID match |
| --- | ---: | ---: | ---: | ---: |
| Skyrim.esm | 1811 | 1808 | 892 | 0 |
| Update.esm | 84 | 382 | 100 | 0 |
| Dawnguard.esm | 199 | 289 | 160 | 0 |
| HearthFires.esm | 41 | 42 | 66 | 0 |
| Dragonborn.esm | 256 | 264 | 137 | 0 |
| _ResourcePack.esl | 4 | 0 | 0 | 0 |
| ccBGSSSE001-Fish.esm | 33 | 122 | 4 | 0 |
| ccBGSSSE025-AdvDSGS.esm | 17 | 34 | 10 | 0 |
| ccBGSSSE037-Curios.esl | 1 | 0 | 0 | 0 |
| ccQDRSSE001-SurvivalMode.esl | 9 | 0 | 0 | 0 |
| **Total** | **2455** | **2941** | **1369** | **0** |

Method: read plugin record headers (24 bytes), descend through `GRUP`
headers, and inspect only `QUST` payloads. Decompress records with flag
`0x00040000` using their zlib payload after its four-byte uncompressed-size
prefix. Walk subrecords, honoring `XXXX` extended lengths; collect the
first four bytes of every complete `QSTA` as a signed AliasID and of every
complete `ALLS` as an unsigned alias ID, then intersect those ID sets within
each quest record. Counts are physical records and fields per file,
including overrides, not winning records in a merged load order. The four
locally generated test plugins were excluded. No engine executable or
library was inspected, and no asset data is included here.

This absence is evidence only for the inspected records, not proof that
the editor or runtime cannot support such targets, nor a test of inherited
alias definitions across overrides. Until public documentation or a
controlled game/editor asset test establishes the behavior, keep direct
location-alias targets research-gated and do not invent a marker-selection
or condition-subject rule. Reference aliases filled from a location and its
location-ref types are a separate, already implemented mechanism.

## World-map overlays

Local UESP [QUST revision 3507775](https://en.uesp.net/w/index.php?oldid=3507775),
Quest Target, confirms AliasID targets and their following CTDA conditions.
Map and compass now share objective/alias/condition selection. Running, active,
unfinished quests contribute displayed unfinished objectives with passing
conditions. Quests are visited in FormID order for stable map row ordering.

The world map projects exterior targets through the same parent-map transform
as destination markers. Other-space targets use the first load door on the
existing fewest-doors route from the viewed world, favoring an entrance near the
player's projected position (zero when unavailable). Same-position/same-icon
rows are deduplicated and keep the first quest's localized name. The movie's
quest icon 62 and door icon 61 are used. Neither overlay carries a travel FormID;
clicking one never starts travel. These icon, label, deduplication and routing
policies are native choices, not verified Skyrim SE behavior. Rows are rebuilt
on map opening. Direct location-alias targets remain research-gated as
described above.
