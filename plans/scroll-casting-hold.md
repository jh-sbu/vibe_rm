# Plan: scroll casting controls and ritual movement

**Status: corrected and verified from user SE observations** (2026-11-02). The original blanket movement lock was based on an
overgeneral reading of UESP and is superseded by these observations.

## Evidence and behavior

See `research/magic/scroll-casting-hold.md` for the user's numbered SE tests,
permanent UESP links, and asset checks. No engine binaries or game-memory
inspection were used or are needed. The user does not request finer timing
measurements.

- Ordinary Fireball scrolls allow walking, aiming and sneak toggling during
  casting; sprint and jump are disabled. Casts cannot start while jumping or
  swimming, but work while wading.
- Blizzard ritual casting holds movement from cast start. Sheathing is refused.
  Early release cancels, retains the scroll, and restores movement after a brief
  unwind; switching hand contents also cancels without consuming the scroll.
- Detect Life is a release cast lasting about 15 seconds, despite SPIT type 2.
  Its four effects are fire-and-forget with 15-second durations.
- Fireball's effect stores 0.5s casting time despite SPIT storing zero;
  Blizzard's SPIT and longest effect both store 3s. These agree with the user's
  qualitative timing observations without more precise instrumentation.

## Implementation steps

1. Record observations and enrich `vrm-tool magic` with effect casting times
   and equip-slot output. **Done.**
2. Correct movement restrictions: hold translation only for both-hand ritual
   scrolls, suppress jump/sprint for all scrolls, gate airborne/swimming starts,
   refuse ritual sheathing, and enter/exit the asset's ritual animation state.
   Use the release clip exit for recovery and a documented 0.3s cancellation
   fallback. Resolve default scroll charge times from effects and the Detect
   Life casting-type contradiction. Update synthetic engine tests. **Done.**
3. Update ambiguity notes and ROADMAP, and verify ordinary and ritual scrolls
   offscreen. Run `cargo fmt` before each commit. **Done.**

## Verification

- Synthetic engine tests: ordinary movement allowed in either hand; sprint/jump
  suppressed without changing sneak; ritual translation held during charging
  and release, cancellation recovery, sheathing refused, replacement preserves
  the scroll; grounded/swimming start gates; default effect charge time;
  concentration-tagged FF scroll waits for release and is consumed once.
- Engine regression suite, with host Vulkan GPU access. Sandbox GL fallback
  cannot construct the terrain renderer (17 sampled textures, limit 16).
- Offscreen QASmoke: add Fireball/Blizzard, equip via `@10 useitem`, draw via
  `@20 pdraw`, start casting after initialization and apply `pwalk`. Compare
  positions while casting and after cancellation/firing. Startup equipment can
  otherwise be replaced. Use `tcl` for deterministic developer movement.

## Remaining work and limits

- Both-hand equip slots identify rituals by inference, not observation of all
  scrolls. Exact cancellation recovery is intentionally approximate; see
  `known_ambiguities/scrolls.md` for choices and their evidence.
- Telekinesis's concentration scroll, mixed effect casting types, and conflicting
  authored versus effect charge times haven't been observed.
- Learned-spell/other-hand restrictions while airborne or swimming, and charges
  interrupted by blows or shouts, remain separate work.
- The existing console `player.equipitem` path consumes magic items, including
  scrolls. Use `useitem` for this verification; command repair is separate work.

## Superseded initial verification

The previous six scroll tests and 604 engine regression tests passed, and an
old offscreen Firebolt check confirmed the implemented blanket movement hold.
Those verified implementation consistency, not fidelity to SE: the user's
observations contradict that hold. Revised verification is recorded below.

## Revised verification (2026-11-02)

- Nine scroll engine tests pass; engine regression suite: 604 passed, 401
  ignored, zero failures. Tests use synthetic records and host Vulkan access.
- Fireball offscreen: after `@150 pcast right`, `@151 pwalk 140`, position
  changes from (-363.3, 2035.2, 7042.0) at frame 151 to
  (-944.7, 1935.0, 7042.0) at frame 210 while still charging.
- Blizzard offscreen, after the startup Survival dialog auto-dismisses: cast at
  450, walk at 451, positions unchanged at 451 and 470. Sheathing at 471 is
  refused. Early release at 480 cancels; movement has resumed at 520. A second
  cast at 550 holds position through frame 700; release at 740 logs one cast,
  and movement resumes after the release clip, confirmed at 810.
- Captures and diagnostic logs are outside the repository at
  `/tmp/vrm-scroll-ordinary.*` and `/tmp/vrm-scroll-ritual.*`.
- Formatting and diff whitespace checks pass.
