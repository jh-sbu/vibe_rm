# Favorite Weapon statistic plan

## Source and scope

The local UESP index entry for [Skyrim:Statistics, permanent revision 3569833](https://en.uesp.net/w/index.php?oldid=3569833), lines 158–159, defines Favorite Weapon as the weapon most often used and limits it to melee weapons or bows. This describes the displayed statistic, but does not define how an attack is weighted. The implementation policy uses one count per accepted player melee attack and one per actual player bow release. Under that policy, a melee miss counts; target hits and repeated graph notifications do not add extra use counts.

The current code already models player weapon attacks and bow release in `src/ai/combat.rs`, `src/player_body.rs`, and `src/ai/archery.rs`. Favorite Weapon will consume accepted action events at those control points and reuse the existing incumbent-on-ties `Usage<T>` policy from `src/stats.rs`. It will store the weapon base form and show its ordinary localized display name. It will not alter manual favorites or magic-use tracking.

## Decisions

- Count a player melee weapon attack once after all control, death, block, recovery, and attack-graph acceptance gates have accepted it. A dual-wield or other attack that uses both weapons contributes one use to each weapon. Follow the attack's ignore-weapon flags. Count attempts that miss; never count per target or `HitFrame`.
- Count a bow once when the player actually looses a projectile after the minimum hold, ammunition, and release gates pass. A canceled draw, failed release, spell projectile, crossbow/staff, or NPC action contributes nothing. Projectile hits never count.
- Exclude unarmed attacks, bashes, staves, invalid or missing weapon forms, and actions not initiated by the player. Validate forms as `WEAP`, then classify bows by the existing bow record logic and other eligible weapons as melee.
- Keep counts per weapon base form. Ties retain the incumbent, including across saves; no favorite exists before the first use. This is an implementation policy because UESP does not document tie behavior.
- Persist only the weapon usage counts and incumbent in a small optional native-save section. Old saves without that section start with empty weapon usage; applying one replaces existing weapon usage. Validate duplicate, null, zero-count, invalid-form, and inconsistent-winner entries. Write entries in deterministic form order. Do not add magic usage persistence.
- `Favorite Weapon` is a named stat and `QueryStat("Favorite Weapon")` returns zero, consistent with other name-valued stats.

## Work and validation

1. Add `WeaponUsage` and name resolution to `src/stats.rs`, with focused usage and stat-query tests.
2. Record accepted player melee attacks and actual player bow releases at existing action gates without a second graph/no-graph path or per-hit counting.
3. Add the optional weapon usage native-save section and round-trip, old-save, replacement, malformed-data, and deterministic-order coverage.
4. Add synthetic engine integration coverage for accepted and rejected melee/bow action paths; GPU-dependent integration cases remain ignored by default following the repository convention.
5. Run formatting on owned Rust files and compile/pure save checks. GPU integration execution requires the parent environment's external GPU setup.

Validation completed: the two focused pure tests passed, both `stats::action_tests` passed in the external GPU environment (including the new weapon action case), all 24 native save tests passed, the default engine suite passed 294 tests with 67 ignored, and the `vrm-esp` suite passed 7 tests. The ignored action tests require a GPU adapter and synthetic records.

## Unresolved behavior

UESP does not specify whether “most often used” counts attack starts, successful hits, draw releases, individual hands, power attacks, or charge strength; it also does not state tie handling or when the favorite becomes visible. The chosen action-use metric and policies above are explicit inferences. Controlled gameplay comparison in Skyrim SE is still needed to establish these behaviors; available game assets alone do not establish the usage event. See `known_ambiguities/journal.md` for the ongoing record.
