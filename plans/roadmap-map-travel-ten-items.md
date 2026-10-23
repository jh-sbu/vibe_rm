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

## 5 completed

Added the validated map-travel relocation entry point through both normal
loaders. Five fast-travel tests passed on host Vulkan. The new integration
checks refusal without movement, exterior grid/feet/heading, then an interior
linked arrival with its own feet/heading. UI and elapsed time follow next.

## 6 completed

Added a documented native straight-line walking-time approximation using
fFastTravelSpeedMult and TimeScale, with bounded subsystem catch-up. Seven
fast-travel tests passed, including distance/speed inputs, malformed bounds,
midnight rollover, away wounds, unchanged Papyrus clocks, no wait/sleep stats
and transactional date-overflow refusal. Cross-space time and exact SE timing
remain explicitly unresolved.

## 7 completed

Map rows carry stable destination FormIDs and validate movie indices. Clicks
preflight and open a shared Yes/No box; cancellation preserves the map and
acceptance rechecks live state. MBXW kind 2 preserves pending confirmation.
Eight fast-travel regressions and the invalid/fractional/overlay callback-index
test passed; the original map SWF was not exercised.

## 8 completed

Shared party hooks carry loaded living teammates actively following the player
across map travel and load doors, deduplicated with summons. TEAM saves teammate
and canDoFavor flags, replacing stale state and clearing legacy membership.
The production party integration passed: selection, durable destination move,
flag restoration and legacy clearing. Codec malformed-ID/duplicate/trailing-byte
checks passed. Synthetic pinned goals stand in for unavailable PACK assets;
unloaded followers and visual reattachment remain documented limits.

## 9 completed

Child destination markers, the player and the map camera owner share a bounded
parent-map resolver with composed ONAM transforms. Stable destination IDs remain
physical references. Two regressions passed: nested/identity transforms, malformed
and cyclic data, foreign maps, child-row permissions and disabled markers.
Projection semantics and missing-field policy remain documented in map-view.md.

## 10 completed

World-map overlays share the compass's active quest/objective/alias/condition
selection, project exterior targets onto inherited maps and route other-space
targets to an entrance. Overlays keep quest labels but carry no travel identity.
The synthetic integration covers a child-world target, interior door routing,
false conditions, duplicate targets, completed objectives, inactive/failed quests,
disabled targets and shared compass behavior. All 13 map-filtered tests passed.
Final review also removed the save-refusal string from the dead-player travel
message, leaving a travel-specific localized lookup and fallback.

## Final validation and limits

- Workspace tests: 541 passed, 161 ignored, no failures.
- Full save suite including synthetic Engine integrations: 62 passed.
- Travel-filtered suite including the new Engine integrations: 12 passed.
- Map-filtered suite including parent projection and quest overlays: 13 passed.
- Changed Rust files pass rustfmt; the complete batch passes git diff --check.

Host Vulkan was required for Engine tests because the sandbox adapter's texture
binding limit cannot create the existing terrain pipeline. Tests generated all
records; no game assets, engine executable/library data, leaked documents or SDK
material were read or committed. Public sources are linked in known_ambiguities,
with local UESP permanent revision links. Exact travel time and cross-space time,
unloaded followers, mounted/Survival restrictions, visual SWF/actor attachment,
autosaves, presentation and map quest routing details remain explicitly open.
All ten items are independently committed, with this file recording the scope
and validation of each.
