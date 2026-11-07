# Papyrus busy-thread budget

Deliberate deviation from Skyrim SE that the user stated on 2026-11-07: "I am
fine deviating from the vanilla usage of fUpdateBudgetMS if need be, this is an
appropriate place for that."

## Original behavior

Skyrim SE's Papyrus VM runs scripts within per-frame time budgets set in the
`[Papyrus]` section of `Skyrim.ini` (`fUpdateBudgetMS`, default 1.2, and
`fExtraTaskletBudgetMS`). How the game shares those budgets between threads,
and how a thread that loops without waiting is scheduled, isn't documented in
the sources checked (`plans/traversal-performance.md`).

## This engine

Each frame, a script thread that woke from a wait, an event or a timer runs up
to 20,000 instructions, as before. A thread whose last run used up its whole
budget is busy (a loop that never waits, such as vanilla
`CWArrowVolleyParentScript.startFiring` outside a siege). Busy threads share
one time slice a frame, 1.2 ms (`fUpdateBudgetMS`'s default value), taking
turns of 500 instructions. The turns start from a different thread each frame,
every frame gives at least one turn, and none gets more than the 20,000
instructions. A busy thread that waits or finishes is no longer busy
(`papyrus::Vm::busy_slice`, `src/app.rs`).

The windowed game uses the shared slice; offscreen runs and tests give every
thread the whole budget unless `VRM_PAPYRUS_BUSY_MS=<ms>` is set (`0` turns
the slice off).
