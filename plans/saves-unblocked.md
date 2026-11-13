# Saves: the unblocked items

**Status: done** (2026-11-12; steps 1–6). Covers what ROADMAP item 9 (Saves) leaves
open that can be built now. Each numbered step is one commit, in order;
each updates `known_ambiguities/saves.md` (and the section list in
`src/save/state.rs`'s module documentation) with what it adds.

Every section added here is optional, like the others since `HEAD`: a save
without it loads as before. As elsewhere in `known_ambiguities/saves.md`,
these are this engine's own layouts and policies, not claims about how
Skyrim SE's `.ess` stores the same state.

## Already done (the roadmap was behind)

- Scene state: `SCNE`, `SCLF`, `SCAB`, `SDLG`, `SLRP`, `SRSQ`, `SFGT`.
- LOS and tracked-stats registrations: built and saved (1947d9e).
- The player's spells held ready: discarded on load by policy (`MREG`: a
  load doesn't fire an old spell, and the cast button can't be held across
  a load, so a restored charge would only be cancelled).

## Steps

1. **Behaviour graph snapshots** (`crates/havok`). `Instance::snapshot` and
   `Instance::restore`: the running tree (state machines' states, clocks,
   cross-fades and pending transitions, clips' times, blends, selectors,
   modifiers' timers and counters), the variables, queued and raised
   events, the random state. Restore checks every node and modifier
   against the project (graph and generator indices, the generator's kind,
   state ids, child counts) and refuses anything that doesn't fit, leaving
   the instance as it was. A pending transition is saved by what picks it
   out (event, target state, nested target, priority, flags) and found
   again in the machine's transitions on restore.
2. **Objects' behaviour graphs in saves** (`GRPH`). For every graph object,
   loaded or parked with an unloaded cell: its reference, its project's
   file and the graph snapshot (with the script-pinned variables). On load
   they are held in the world state like parked graphs; an object whose
   cell attaches builds its graph and restores the snapshot in place of
   `GOPN`'s replayed event. A snapshot that no longer fits (other graph
   files) falls back to `GOPN`. `PlayAnimationAndWait` senders resume from
   where they were, so the event an `ANMW` wait needs is raised when the
   restored clip gets to it.
3. **Fights and searches** (`FGHT`). Loaded actors' fights (target,
   fleeing, the fight's confidence adjustment and threat clock, flee
   distance, how long the target has gone unseen and where it was last
   seen, the ally killed) and searches (target, where, alert or lost, how
   long unseen, whether it saw the target flee). Applied once the save's
   cells have attached their actors: the fight or search is set again
   without barks or `OnCombatStateChanged` (the reported state is set to
   match), weapons drawn. Paths, swings, blocks, casts and other moment to
   moment state start afresh, as they do after a stagger. Fights of actors
   whose cells didn't load are dropped, as on unloading.
4. **Scripted moves and the rest of the signal waits** (`PTHW`, `KOFF`).
   `PathToReference` waits (actor, target, the waiting thread) and
   `KeepOffsetFromActor` settings. Once actors attach, each path is
   started again toward its target and its thread waits on the new path;
   a path that can't start gives the thread False, as at runtime.
   `GetOpenState` waits wake at once on load (the value is the one taken
   when it was asked). `Sound.PlayAndWait` keeps the timed fallback, which
   ends when the sound would have (the sound itself doesn't play on).
5. **Live player conversations** (`CONV`). The speaker, the topic and INFO
   being said, the response being said and those still to come, whether
   it is ending, the said-once set. Applied once the speaker has attached:
   the conversation opens again on that INFO without running its begin
   fragment; the response being said starts over (as `SDLG` does for
   scene lines), then the rest, then the options as usual. A conversation
   whose speaker didn't load, whose records no longer match, or that was
   an arrest is dropped as before.
6. **The roadmap.** Move what's done to ROADMAP Done and rewrite item 9's
   remaining list.

## Left out

- **Load orders that changed since the save.** Form ids are written as
  loaded throughout, and many sections check that their keys are sorted,
  so remapping ids as they're read would break those checks; `FORM`'s
  records also carry form ids inside their bytes. It needs a remapping pass
  over the read snapshot (or sorting after remapping) and a policy for
  missing plugins: separate work.
- **The original's refusals to save** (combat, menus, scripted): no public
  source found says when Skyrim SE refuses (`sCantSaveNow` alone).
- **Actors' behaviour graphs**, NPC casts in progress and procedures'
  loaded-only state: an actor attaches standing where it was, so its graph
  starts from its equipment and stance; a cast or swing in progress starts
  over. Restoring an actor's graph mid-furniture or mid-attack without the
  physics and seat state that go with it would be inconsistent.
- Script sounds playing on after a load, and the original `.ess`.
