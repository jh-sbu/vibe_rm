# Plan: scrolls hold the player in place while cast

**Status: implemented and verified** (2026-11-02); manual SE edge-behavior
observation remains open. ROADMAP Next item 5, "Magic, NPCs and
items": "scrolls holding the player in place while cast (UESP
Skyrim:Scrolls)". Each numbered step is one commit; run `cargo fmt` before
each.

## The evidence

- UESP [Skyrim:Scrolls](https://en.uesp.net/w/index.php?oldid=3543120)
  (revision 3543120, the page `known_ambiguities/scrolls.md` already
  cites), "Using scrolls": hold the attack button through the casting
  animation, which "may take several seconds" for higher-level scrolls.
  "Unlike with casting learned spells, the player cannot walk or run during
  the casting." Releasing the button lets the spell go, and an interrupted
  cast doesn't use the scroll.
- The humanoid graph (`vrm-tool hkb-vars <Data>
  meshes/actors/character/defaultmale.hkx`) has no scroll-specific variable
  or event. `hkb-events` finds no "scroll" either. The hold isn't something
  the graph does, so the engine has to do it.

## What exists (2026-10-30)

- `src/casting.rs`: a scroll in hand is cast as a charged spell with the
  scroll's own charge time (`HandCast`, `player_cast_press` /
  `player_cast_release`), costs no magicka, and is used up when it fires
  (`use_up_scroll`). One let go before it is charged isn't used.
- `Engine::update` (`src/engine.rs`) builds the player's `MoveInput`. It
  zeroes the input when `disabled_controls.movement` is set, adjusts it for
  encumbrance and sprinting, then runs `update_casting` and moves the
  player.

## Design

- **When held.** From the press that starts a scroll's cast until its
  release has fired (`Firing.fired`) or the cast ends some other way: let
  go early, the scroll left the hand, the hands were put away. Both hands
  count, including a scroll needing both hands.
- **What is held.** Walking, running and sprinting: `forward` and `right`
  go to zero, and `sprint` goes off. The source only names walking and
  running, so the rest are choices to record in
  `known_ambiguities/scrolls.md`:
  - turning and looking stay free (a held mouse aim);
  - jumping is refused, because the press already holds a hand;
  - sneaking can still be toggled, but doesn't move;
  - an airborne or swimming player isn't stopped mid-air or mid-stroke
    beyond the input being zeroed.
- **Concentration scrolls,** if the data has any (step 0), are held for as
  long as they flow, by the same rule.

## Steps

0. **Survey (done).** `vrm-tool magic <Data>`: list the scrolls' cast types and
   charge times (`SCRL` `SPIT`), so the plan knows whether concentration
   scrolls exist and how long the longest hold is. Note the result in
   `known_ambiguities/scrolls.md`. Commit only if the tool needs a change to
   print it. Results: `research/magic/scroll-casting-hold.md`; two
   concentration records, maximum stored charge time three seconds. The survey
   mode is `vrm-tool magic <Data> --scrolls`.
1. **The hold.** Add `Engine::player_scroll_held() -> bool` in
   `src/casting.rs`: true while either hand's `HandCast` or `Firing` (not
   yet fired) names a `SCRL`. In `Engine::update`, after
   `update_casting`, zero `forward` / `right`, clear `sprint` and `jump`
   while it is true. Engine tests in `src/casting_scroll_tests.rs`:
   - press with a scroll, try to walk for half a second: the player doesn't
     move;
   - release after charging: once fired, walking works again;
   - release early: free at once, and the scroll is still carried;
   - an ordinary spell in the same hand doesn't hold.
2. **Docs.** Update `known_ambiguities/scrolls.md` with the hold and its
   choices (turning, jumping, sneaking, mid-air). Remove the clause from the
   ROADMAP entry.

## Verification

- `cargo test` for step 1.
- Offscreen run: put a scroll in the right hand (`player.additem`, then
  `useitem` after initialization), cast with a held right button (`--console` with
  `pcast`, as earlier casting captures did) while `test_walk` drives
  forward input. Read the player position before, during and after the
  cast from the log.

## Open questions

- Whether turning, jumping and sneaking are blocked too. A manual SE
  observation would settle it: hold a Fireball scroll and try each. Add it
  to the next manual batch rather than an autotest, because Papyrus can't
  press buttons.
- Interruption by blows and shouts is a separate ROADMAP clause ("charges
  interrupted by blows or shouts") and stays out of this plan.

- Whether auto-calculated charge time differs from stored SPIT charge time for
  scrolls whose SPIT time is zero (the format reference describes calculation
  from effects). The survey reports stored times; changing charge-time
  calculation is separate from the movement hold.

## Completed verification (2026-11-02)

- Six scroll engine tests pass with host Vulkan GPU access, including both
  hands, two-handed equipment, early cancellation, unequipping, concentration,
  jump suppression, waiting for SpellFire, and ordinary spells remaining mobile.
- Engine regression suite: 604 passed, 398 ignored, zero failures. GPU access
  is needed even for some non-ignored engine tests; the sandbox GL fallback
  cannot construct the terrain renderer (17 sampled textures, limit 16).
- `cargo fmt --check` and `git diff --check` pass.
- Offscreen QASmoke check with vanilla Scroll of Firebolt (00096598): add two,
  `@10 useitem 00096598`, `@20 pdraw`, `@150 pcast right`, then
  `@151 pwalk 140`. Position at frames 151 and 210 was identical:
  (-363.3, 2035.2, 7042.0). After `@240 pcast right off`, the log confirms
  firing and position at frame 290 is (-856.0, 1950.3, 7042.0).
  Run with `tcl`, `--cell QASmoke --wait 300`, and host Vulkan access.
  Logs/capture were written under `/tmp/vrm-scroll-*`.
- Startup equipment must be scheduled after initialization for this check.
  The console's existing `player.equipitem` path consumes magic items, including
  scrolls; `useitem` exercises inventory equipping correctly. Repairing that
  console command remains separate open work.
