# Fifteen more PEX natives

One commit per item. Native declarations were confirmed by reading installed PEX
assets with `vrm-tool pex-natives` / `pex-dump`. No assets were copied into the
repository, and no game engine binaries were inspected.
Research used the local UESP index first, then public community documentation.
The files under `refs/pex-natives/` are authored research summaries with links,
not game scripts or copied game content.

| Item | Native | Behavior / reference note |
| --- | --- | --- |
| 1 | `Math.Acos` | Arccosine in degrees; `acos.md` |
| 2 | `Math.Asin` | Arcsine in degrees; `asin.md` |
| 3 | `Math.Atan` | Arctangent in degrees; `atan.md` |
| 4 | `Math.Tan` | Tangent with degree input; `tan.md` |
| 5 | `Math.DegreesToRadians` | Signed angle conversion; `degreestoradians.md` |
| 6 | `Math.RadiansToDegrees` | Signed angle conversion; `radianstodegrees.md` |
| 7 | `ActorBase.IsUnique` | Base ACBS bit 0x20 instead of constant true; `unique.md` |
| 8 | `ActorBase.GetClass` | CNAM from the Use Stats template source; `class.md` |
| 9 | `Cell.IsInterior` | Target CELL DATA bit 0, independent of the player; `interior.md` |
| 10 | `ObjectReference.GetEditorLocation` | Original placement location or None; `editor.md` |
| 11 | `ObjectReference.GetCurrentLocation` | Runtime cell, schedule, moved/created place, editor fallback; `current.md` |
| 12 | `Location.GetRefTypeAliveCount` | Enabled living editor references, deduplicated; `alive.md` |
| 13 | `Location.GetRefTypeDeadCount` | Enabled dead editor references, deduplicated; `dead.md` |
| 14 | `ObjectReference.HasRefType` | Direct XLRT and location-indexed types; `reftype.md` |
| 15 | `Location.IsLoaded` | Loaded cells and their parent locations; `loaded.md` |

Math implementations share a renderer-independent dispatcher; existing math
natives retain their calculations. Population queries share the condition
counter, so duplicate and disabled-reference corrections apply there too.
Current-location lookup also recognizes explicit moves and created world places,
while the editor lookup retains the original placement.

Unresolved behavior is recorded in
[`math-natives.md`](../known_ambiguities/math-natives.md) and
[`query-natives.md`](../known_ambiguities/query-natives.md), including exceptional
math inputs, leveled templates, navmesh-dependent location reporting, descendant
population counting and parent location loading. These are explicit choices,
not claims of verification in Skyrim SE.

## Validation

Six math tests and nine synthetic-plugin native integration tests were added.
Focused checks passed before committing each implementation. GPU-dependent
fixtures require host graphics access: the sandbox's fallback adapter exceeds
the renderer's sampled-texture limit, while the host adapter runs them.

- `cargo test --workspace`: all pass; engine 423 passed, 236 ignored.
- `cargo test --bin vibe_rm -- --include-ignored --test-threads=1`: 659 passed.
- `rustfmt --check` on changed Rust implementation and test files: passed.
- `git diff --check`: passed.
- Audited the batch's tracked files: source, authored reference notes, ambiguity
  notes and this summary only; no game assets or unrelated changes.
