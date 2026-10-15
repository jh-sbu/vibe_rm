# Soul trap

Implemented in `src/soul_trap.rs`. Sources: UESP's Skyrim:Soul Trap and
Skyrim:Soul Gem (sizes by level, black souls for people, the smallest
gem that can hold it), the game's settings (`i<Size>SoulActorLevel` 4, 16,
28, 38; `sSoulCaptured`, `sSoulGemTooSmall`), the soul gem records (`SOUL`,
`SLCP`, `NAM0` the filled form, record flag 0x20000 on the black ones),
`magicSoulTrapFXScript` (calls `Caster.TrapSoul(Victim)` as the effect
finishes on a body, then plays the visuals if it returned true).

## Choices made without a source

- **Black souls**: actors with `ActorTypeNPC` (theirs or their race's);
  `NoSoulTrap` ones give none. Dremora and other `ActorTypeNPC` creatures
  count as people.
- **White souls in black gems**: allowed, but only when no other empty gem
  holds the soul.
- **Filling**: a gem filled to its size becomes its linked filled form
  (`NAM0`: the common ones, the black one); a smaller soul, or a gem with no
  linked form (petty, Azura's Star), keeps the soul with the empty gem
  (the game's per-item soul). Partly filled gems aren't refilled.
- **"No soul gem large enough."** shows when the caster has empty gems but
  none takes the soul; with no empty gem at all, nothing shows. A soul not
  trapped can still be taken by a later trap on the same body.

## Inventory transfers (2026-10-15)

`Inventory::remove_with_souls` now returns the souls removed along with the
item counts. `Engine::remove_stack` restores those souls in the receiving
inventory before sending its add-item event. Storage, gifts, barter,
confiscation, pickpocketing and script transfers use this path. Removal
without a destination discards the removed souls. Fully filled linked forms
already travel through their base form; this change covers the side list.

This is a compatibility choice, not a claim that every Skyrim transfer
preserves souls. The public [YASTM author's explanation](https://www.nexusmods.com/skyrimspecialedition/articles/3574)
describes separate soul data and bugs involving dropping and merging gems.
[Contemporary player reports](https://gaming.stackexchange.com/questions/92152/can-i-empty-a-soul-gem-in-skyrim)
also describe losing souls in containers, with unofficial patches changing
the behavior. These do not establish exact Skyrim SE rules by operation,
gem size, patch version, or reusable gem. A controlled game test is needed.
No engine binary or SDK source mirror was examined.

- Dropping still removes the side-list soul and creates an empty-base
  world reference. Public reports support emptying partially filled gems,
  but the correct rules for fully filled gems without linked forms (notably
  reusable stars) remain unresolved. Do not mark dropping as implemented.
- Ownership and souls are separate aggregate lists, so an owner-filtered
  removal cannot identify which particular owner's gems contain souls.
  The existing rule takes empty gems first across the form's entire count,
  then soul entries in insertion order; this is an approximation for mixed
  owners or multiple soul sizes. Menus do not yet select a soul-size stack.
- Synthetic tests check removal ordering, partial and oversized removals,
  multiple soul sizes, an owned-gem round trip, no-op removals and destructive
  removal. They validate metadata conservation, not original-game behavior.
  `cargo test -p vibe_rm --bin vibe_rm --offline` passed all 60 tests;
  formatting of changed Rust files and `git diff --check` also passed.
