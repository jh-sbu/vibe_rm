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
