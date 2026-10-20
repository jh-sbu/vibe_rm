# Saves

Implemented in `src/save/` (`file.rs` the container, `state.rs` what it holds,
`codec.rs` the encoding), with `Game.RequestSave` / `RequestAutoSave` in
`src/script/natives.rs` and the console's `save`, `load`, `saves`.

This is the engine's *own* format (`.vrs`), chosen rather than the original
game's `.ess`: the original format stores the original engine's internal
structures (Papyrus VM stacks, process lists), which this engine doesn't have.
Reading `.ess` saves (the `ess` crate parses them read-only, for test-mod
results) stays a separate, later job. For orientation only, UESP's
[Skyrim Mod:Save File Format](https://en.uesp.net/w/index.php?oldid=3067914)
(revision 3067914) lists the original's sections.

## Design

- A magic, a format version, then tagged sections, each with a length and
  CRC-32. Damaged, truncated or foreign files are refused, and a save is
  fully read before any of the game is touched. Unknown sections are skipped,
  so sections can be added without a version bump.
- Written to a temporary file and renamed, so a crash keeps the old save.
- Saves live in `$VRM_SAVES`, else `$XDG_DATA_HOME/vibe_rm/saves`, else
  `~/.local/share/vibe_rm/saves`.
- Form ids are saved as loaded (global ids, with the load-order index in the
  top byte), so a save loads only with the same plugins in the same order;
  `HEAD` lists them and another list is refused. (The original game maps ids
  through a form id array; that is the likely route to loading across load
  order changes.)
- Loading unloads the cells, replaces the saved state, then enters the saved
  location with the player on their feet where they stood. The old game's
  pending stage and event queues are dropped.

## What is saved

The date and clock; globals; quests (stage, objectives, log, alias fills, the
Story Manager event that started them); statistics; actor value changes made
by scripts and effects; every inventory with what is equipped, owners, souls,
poisons and weapon charges; what scripts did to references (taken from,
disabled, locked, owners, factions, blocked activation); the world state
kept across cell loads (dead actors and their bodies, hurt actors, doors left
open, moved references, charges of dropped weapons, broken joints, learned
ingredient effects, explicit alerts, destructible-object health and stage by
reference); references made while playing; the
player's health, stamina, skills, perks, spells, words of power and shouts,
favorites, hotkeys; dialogue lines heard; bounties; merchants' restock times;
the Story Manager's quest reset times, per-node do-all-before-repeating rounds,
social deadlines, last observed location (including an observed absence of a
location), discovered finder/body pairs and body-check deadline; changed
relationship ranks by actor pair. `STOR` and `RELA` are optional sections:
older native saves load with empty story memory and no relationship overrides,
replacing any memory in the destination engine. Authored story node caches
rebuild from plugins; relationship overrides restore without change-rank events.
The event currently being synchronously dispatched is transient and not saved.
Destructible state is stored in optional `DSTR` as sorted reference, f64 health
and i32 stage entries; older native saves without it load with no destruction
state. Loading replaces existing state without replaying destruction events.
This engine-owned representation does not establish how Skyrim's `.ess` saves
store destructible references.

The scripting side (`vm.rs`, sections `VM` and `VMAR`): the scripts attached
to each object with their variables and properties (`Vm::attach` keeps an
instance that exists, so cells loading after a load don't reset them),
`OnInit` done, the threads suspended (their calls by defining class, state
and function, the instruction they stopped at, locals and return slots), the
`OnUpdate` timers, and the sleep, inventory filter and animation event
registrations. Arrays are shared by identity in the VM, so they are saved
once and referred to by index.

## Not saved yet (see `plans/ROADMAP.md`)

- Threads waiting for something the engine tracks by a key (a message box's
  button, an animation event) are kept as timed waits that finish when their
  timeout runs out, with the default result: nothing saved would signal them.
  A thread whose function no longer resolves (other scripts) is dropped with
  a warning. Scripts on active magic effects are dropped with the effects.
  Registrations other than these (key, LOS gain, tracked hits...) aren't
  built in the engine yet.
- Active magic effects (timed effects, summons, wards), the leveled spell
  rolls, scenes playing, AI and
  package progress, schedules, objects' behaviour graphs, weather and
  detection state, and the player's spells held ready (the equipped spells
  are saved with the inventory's hands).

## Unconfirmed choices

- **`RequestSave` / `RequestAutoSave`** write `quicksave` and `autosave`. The
  original names its saves with a counter; the Creation Kit wiki isn't
  reachable here (`public-sources-access`). The save is made on the next
  update, after the script returns, not inside the native call. A save is
  refused while the player is dead ("You cannot save right now.",
  `sCantSaveNow`); the original's other refusals (combat, menus, scripted
  forbidding) aren't built. "Autosaving..." (`sAutoSaving`) is shown for the
  autosave, which is our guess for when the game prints it.
- Console `save` with no name writes `console`.
- Save names: letters, digits, spaces, `-` and `_`, up to 64.

- **Story memory persistence:** the local UESP dump's
  [Story Event Manager save-format page](https://en.uesp.net/w/index.php?oldid=2309086)
  (revision 2309086) calls the original payload unknown; it does not establish
  which cooldown, rotation, discovery, or social scheduling fields Skyrim SE
  preserves. Our native sections retain all existing runtime memory so loading
  does not reset those behaviors. This is a continuity choice for this engine,
  not a claim that Skyrim SE persists the same fields. No executable data was
  examined. With old saves lacking `STOR`, the first update establishes the
  location baseline and body discoveries can happen again; that lost memory
  cannot be recovered from those saves.
