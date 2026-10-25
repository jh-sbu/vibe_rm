# Reference flags and queries

Public pages consulted 2026-10-24 (Papyrus Index / community CK wiki text,
summarized), https://papyrus.bellcube.dev/skyrimse/script/ `objectreference/function/`
`ignorefriendlyhits`, `isignoringfriendlyhits` (the reference doesn't turn hostile
after several friendly hits), `isfurnitureinuse`, `isfurnituremarkerinuse` (marker
index from 0; optional "ignore reserved"; no marker or remote activation reads
false), `isindialoguewithplayer`, `hasnode`, `getmass` (0 unless Havok holds the
object), `isdeleted`, `isactivatechild(akChild)` (is the argument an activate child
of this reference), `calculateencounterlevel(aiDifficulty = 4)` (0 easy, 1 medium,
2 hard, 3 very hard, 4 none; from the player's level, the area's level and the
difficulty); `cell/function/setpublic`. Local UESP:
[Skyrim:Dungeons](https://en.uesp.net/w/index.php?oldid=3575803) (below a minimum
zone level, enemies use the minimum) and
[ECZN](https://en.uesp.net/w/index.php?oldid=3056899) (`DATA`: min level, flags
0x01 Match PC Below Minimum Level, max level).

Implementation (`src/ref_state.rs`; inference where marked):
- `IgnoreFriendlyHits`: a blow from the player to a teammate of the player's, or
  from anyone whose factions are allies (`faction_relation` ≥ 2), does its damage
  but doesn't make the target fight or count as an assault (inference: the engine
  has no count of friendly hits; this is the "several" never reached). Saved as
  `IFHT`.
- `Cell.SetPublic` writes `scripts.cell_public`, which crime and locks read.
- Furniture: a marker is in use when reserved (`FurnitureWorld::users`); with
  "ignore reserved" only when the user is already getting in, using or leaving.
  Markers number from 0 in model order. The player's own furniture use is not
  tracked, so a player at a chair reads as free.
- `IsInDialogueWithPlayer`: the current conversation's NPC.
- `HasNode`: false unless the 3D is loaded; actors by skeleton node, objects by
  the base model's node names (case-insensitive, cached per model).
- `GetMass`: the summed Havok mass of a loose object; 0 for everything else
  (ragdolling actors are not reported).
- `IsDeleted`: record flag 0x20, or an id in the created range nothing holds now.
- `IsActivateChild`: `XAPR` of the argument names this reference.
- `CalculateEncounterLevel`: player level held to the encounter zone's min
  (unless Match PC Below Minimum Level) and max, times `fLeveledActorMult{Easy,
  Medium,Hard,VeryHard}` (the GMST when a plugin has it, else 0.5, 0.75, 1.0,
  1.5: unconfirmed, see `known_ambiguities/remaining-natives.md`), rounded, at
  least 1; difficulty 4 (none) is the zone-held level.

Tests: synthetic records for each query, pure tests of the zone clamp, a combat
test of the ignored blow, and `IFHT` payload and legacy-load tests.
