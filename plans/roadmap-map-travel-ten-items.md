# Ten map and travel items

Use only public references and permitted assets, never executable data. Tests
use generated records. Each numbered implementation is committed separately.

1. Game.EnableFastTravel / IsFastTravelEnabled and scripted movement reset.
2. Durable global and per-marker travel permissions, including explicit denial.
3. Eligibility checks and localized refusal reasons.
4. Linked arrival reference resolution and destination validation.
5. Relocation through the existing exterior/interior loaders.
6. Distance-dependent game time and subsystem catch-up.
7. Marker selection, confirmation and revalidation from the world map.
8. Following teammates across travel and load doors.
9. Child-worldspace markers on the parent's map.
10. Active quest targets on the world map.

Progress and validation are recorded below as each item completes.

## 1 completed

Implemented global script control, default-true enable and player MoveTo /
SetPosition reset. The production native-dispatch regression passed with host
Vulkan. The sandbox adapter cannot create the existing terrain pipeline.

## 2 completed

Explicit marker denial overrides authored travel flags and discovery without
recounting discovery. Optional MAPS replaces global gate and marker memory on
load; legacy files reset runtime memory. Four travel-filtered tests passed,
including production native/save and marker permission checks and malformed
MAPS payloads. Deterministic encoding and old-save behavior are covered.

## 3 completed

Added live departure restrictions, world DATA flag handling and localized
refusals with fallbacks. Reused waiting's detrimental-health predicate.
Three fast-travel tests passed on host Vulkan, covering live gate, location,
world flag, encumbrance, airborne and death transitions. Hostile radius and
unsupported mounted/Survival behavior are documented as open limits.

## 4 completed

Map destinations resolve unkeyed linked arrival references and their current
transform, with a self-marker fallback. Reject unavailable markers and broken,
disabled, deleted, nonfinite or unplaced landing points. Four fast-travel tests
passed on host Vulkan, including generated world/cell/marker records, linked
heading, keyworded-link isolation, denial and malformed destination cases.
