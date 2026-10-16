# UseWeapon aiming and trigger controls

Implemented in `src/ai/package.rs` and `src/ai/practice.rs`, with reference
resolution and position snapshots in `src/ai/mod.rs`.

## Evidence

Only game **assets** and public material were consulted. No engine executable
or other engine binary data was examined, and no game assets are included here.

- `Skyrim.esm`'s `UseWeapon`, `UseWeaponAlreadyHeld` and
  `UseWeaponMultiTarget` package templates, inspected with the project's
  `vrm-tool pack-tree` command. Their input names identify Aim Only and Pause
  between Barrages; the latter feeds the procedure's volley flag.
- The same assets' procedure-tree conditions compare each target reference
  to its matching Trigger Ref with its Trigger Radius. Radii are Int inputs.
  MultiTarget has three independently conditioned UseWeapon procedures.
  Its outer OR condition's second comparison runs on Target 01, whereas
  the corresponding procedure's own comparison runs on Target 02.
- First-hand Creation Kit experiments by UNI:
  [UseWeapon procedure](https://skyrimeasy.blog-rpg.com/ckworks/procedure03)
  and [UseWeapon templates](https://skyrimeasy.blog-rpg.com/ck/packagetemplate08).
  The author reports that Aim Only holds a bow drawn, trigger distances compare
  the target to its trigger reference, and barrage pauses are optional.
  Revisited 2026-10-16: both pages identify Always Power Attack; the
  procedure experiments report that enabling it forces power attacks, while
  disabling it permits random power attacks.

## Current behavior

- Trigger references use the package's existing reference, linked-reference,
  Self and quest-alias resolution. An authored trigger that cannot resolve, or
  whose position is unavailable, does not allow an attack.
- Only eligible targets can start a barrage. Reference origins, including their
  height, determine distance; the bounds-centre aim offset does not. Self uses
  the actor's current position rather than the position snapshot.
- Aim Only holds a bow drawn without release and suppresses melee swings.
  Disabling barrage pauses removes their extra delay while keeping the recovery
  between individual attacks.
- Leaving a trigger range before release cancels the draw with `attackStop`.
  A rejected stop is retried before drawing again or sheathing. A released
  shot finishes normally. A new eligible barrage can start after cancellation.

## Always Power Attack

The named Bool input is carried from the package into practice. When enabled,
melee practice selects a right-hand, non-bash power attack from the actor's
existing attack data that fits its equipped hands, preferring the stationary
`attackPowerStartInPlace` when available. Bare fists and creature attacks use
that same authored-data selection. Aim Only still suppresses swings, and bows
retain their draw/release behavior. Each accepted swing consumes one attack in
the current barrage; graph rejection or no suitable authored attack consumes
none. These rules have synthetic regression coverage without game assets.

The public experiments establish the flag's intent, but not its exact attack
selection or exhaustion rules. Preferring stationary then first authored
right-hand power attack is a local policy, not verified Skyrim selection.
Actors without a fitting power attack currently keep the barrage pending.
Practice still uses its existing swing recovery times, consumes no stamina and
causes no melee damage; exhaustion, power-attack-specific recovery, and creature
behavior need original-game observation. False or missing flags preserve the
existing basic-only melee path; the reported random power attacks are still
unsupported. Bows do not power attack in this implementation; any special
original-game handling of this flag for ranged weapons remains unverified.
Finite completion, resumption and time limits remain outside this change.

## Unresolved behavior and limitations

- Public descriptions and assets establish the distance gate, but not its exact
  boundary comparison, update cadence, or cancellation/resumption ordering.
  This implementation uses a three-dimensional, inclusive origin-distance
  check each AI step. Whether the original allows a pending release when its
  trigger ceases to pass, or resumes the remaining barrage after re-entry,
  requires observation in the original game.
- MultiTarget currently chooses an eligible target per barrage. The original
  Random branch's selection lifetime, finite-burst boundaries, and the effect
  of the differing outer OR comparison remain unresolved. The three individual
  procedure gates are used here; the outer comparison is not reproduced.
- Cross-cell eligibility follows the existing reference-position lookup; it
  does not yet reject unrelated interiors whose local coordinates happen to
  overlap. Missing template inputs still use the existing parser's fallback
  policy: an absent trigger imposes no gate, and a missing declared radius
  becomes zero. Exact inherited defaults need further investigation.
- Never End, finite barrage completion, the simultaneous Wait's attack time
  limit, random power attacks when the flag is false, Hold when Blocked,
  head tracking, Always Hit,
  damage and combat-start flags, and acquisition of weapons from the world
  remain unsupported. Finite completion needs package/procedure lifecycle
  handling. Practice attacks retain their existing harmless behavior.
