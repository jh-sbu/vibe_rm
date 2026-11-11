# `SetDontMove`

- `Actor.SetDontMove(true)` holds an actor where it stands
  (`world_state.dont_move`): its package steps don't move it, and a walk
  its package starts is halted without turning it towards where it would
  have gone. Its animations' root motion (a sandbox idle) doesn't move it
  either; it can still fall. It still turns for other reasons (facing the
  player while barking, combat), idles, fights in place and can be moved
  by scripts (`MoveTo`, `SetPosition`). `SetDontMove(false)` lets it go.
  Not saved.
- Research: the Creation Kit wiki's page is not reachable (Cloudflare);
  UESP has nothing on it, the CK's Papyrus sources aren't installed, and
  CommonLibSSE-NG names no such flag (only a `Restrained` actor state).
- Skyrim SE (`test-mod autotest24`, `research/autotest24.md`, ViewCone): an
  NPC held by `SetDontMove` in the middle of the floor read 300 units off
  at every spot it was read from, as placed, for over a minute. Before
  this, ours ignored the call and the same NPC sandboxed: its idles' root
  motion moved it 5 to 15 units.
- Unconfirmed: whether a held actor turns, sits, uses furniture or gives
  ground in combat in SE; whether the flag lasts in a save.
- `SetRestrained` stays a no-op (the plans' test mods use it to hold
  combat targets; see `plans/ROADMAP.md`).
