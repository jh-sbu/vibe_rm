# Crafting choices

## Runtime form identity and lifetime (steps 2a–2c)

Research: UESP ALCH/ENCH format pages (permanent links in
`research/crafting/records.md`) describe effects but not deletion timing.
Public gameplay evidence is stacking of identical player-created potions.
Choice: intern generated forms by non-name fields and sorted effect pairs;
record IDs and FULL/EDID do not distinguish them. IDs share the created-reference
allocator and are never reused during a game.

Collect unreachable forms when serializing a snapshot and compact the live
runtime plugin only after the save has been written successfully. Collection
conservatively treats any matching four-byte handle in saved state as a root,
excluding FORM itself and thumbnails, and follows matching handles in retained
records. This covers VM arrays, story memory and future optional sections as
well as inventories without maintaining a second, easily incomplete root list.
Accidental byte matches can retain an otherwise unused form, but cannot delete
one still mentioned by a save. This is a deliberate conservative lifetime choice;
the original's collection timing and root algorithm are not established.

## Quality and health (step 3a)

Research: Smithing revision 3612986, lines 264–273 gives exact rating bonus
(3.6 * level - 1.6), halved outside chest armor, with rounding at display time.
This conflicts with some rounded entries in the earlier summary table. Use the
explicit formula for calculations, ceil armor display, round weapon display.
The local UESP index has no ExtraHealth article; GetItemHealthPercent appears
only in function lists. No permitted source established its encoding.

Provisional choice: store health as 1 + quality level / 6, the documented value
multiplier; untouched copies return 1. This is an internal encoding, not a claim
about Skyrim's ExtraHealth bytes. The manual plugin must check GetItemHealthPercent
before calling this native compatible for tempered copies. Quality ratings and
value multipliers do not depend on the unresolved external encoding.

Unspecified removals use unmodified copies before modified ones within each
ownership stack. Explicit row selection will select the copy state. Metadata
keeps its ownership; it must not migrate between stolen and owned stacks.
