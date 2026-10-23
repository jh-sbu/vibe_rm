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
open by the player and by actors (`ADOR`), moved references, charges of dropped weapons, broken joints, learned
ingredient effects, explicit alerts, destructible-object health and stage by
reference); references made while playing; the
player's health, stamina, skills, perks, spells, words of power and shouts,
favorites, hotkeys; dialogue lines heard; bounties; merchants' restock times;
the Story Manager's quest reset times, per-node do-all-before-repeating rounds,
social deadlines, last observed location (including an observed absence of a
location), discovered finder/body pairs and body-check deadline; changed
relationship ranks by actor pair. `STOR` and `RELA` are optional sections:
older native saves load with empty story memory, no relationship overrides and
no actor-opened doors, replacing any memory in the destination engine. Authored story node caches
rebuild from plugins; relationship overrides restore without change-rank events.
The event currently being synchronously dispatched is transient and not saved.
Destructible state is stored in optional `DSTR` as sorted reference, f64 health
and i32 stage entries; older native saves without it load with no destruction
state. Loading replaces existing state without replaying destruction events.
Favorite Weapon counts and its current winner are stored in optional `WUSE`;
older native saves without it load empty weapon usage, replacing any usage in
the destination engine. Counts are sorted by weapon form and validated against
the loaded plugin records before applying the save. This persists the engine's
chosen attack/release metric, whose parity with Skyrim SE is unconfirmed.
This engine-owned representation does not establish how Skyrim's `.ess` saves
store destructible references. The origin of each created reference made in
the world is stored in optional `ORIG` as its creation position and rotation;
the existing `WRLD` layout remains unchanged. Older native saves retain the
created reference's other data but have no recoverable starting transform, so
`GetStartingPos` / `GetStartingAngle` fail for those references.
Player and actor stealth-point reservoirs, with their remaining regeneration
delay, are stored in optional `STPT`. Loading replaces the entire reservoir map;
an older save without the section clears it. Loading also drops cached
observer-to-player detection values and schedules an immediate fresh observation.
This persists the engine's reservoir values only: searches, combat targets,
detection state transitions and AI execution remain outside save state.

Greater Powers' last-use game days are stored by spell form in optional `PWCD`.
Loading replaces the cooldown map; legacy native saves without `PWCD` clear it.
The section retains the existing engine policy of a full 24 game hours between
uses. UESP documents a once-per-game-day limit ([Skyrim:Powers, revision
3595900](https://en.uesp.net/w/index.php?oldid=3595900)); it does not establish
how the original `.ess` payload stores cooldown state or whether Skyrim resets
at midnight. Cooldowns remain attached to loaded Greater Power records even if
the player no longer knows or equips them.

Bodies whose souls were successfully captured are stored by non-null reference
form in optional `SOUL`. Loading replaces the spent-body set; legacy saves clear
it. Entries are retained even when the actor is no longer loaded or present in
the current world state, so its identity remains spent. Failed captures are not
added. This preserves the engine's current one-capture-per-body rule; original
`.ess` persistence and whether resurrection resets a body remain unverified.

The scripting side (`vm.rs`, sections `VM` and `VMAR`): the scripts attached
to each object with their variables and properties (`Vm::attach` keeps an
instance that exists, so cells loading after a load don't reset them),
`OnInit` done, the threads suspended (their calls by defining class, state
and function, the instruction they stopped at, locals and return slots), the
`OnUpdate` timers, and the sleep, inventory filter and animation event
registrations. Arrays are shared by identity in the VM, so they are saved
once and referred to by index.

Message boxes and `Message.Show` signal waits use optional `MBXW`, leaving the
`VM` / `VMAR` layouts unchanged. The queue retains titles, text, the currently
filtered button indices and labels, native poison actions, and the bounded
message-wait key allocator. A wait link names the exact filtered thread order
written to `VM`; loading replaces any stale live queue and restores only boxes
whose waiting thread imported. A legacy save without `MBXW` has no saved boxes,
and signal waits use the existing timed fallback. Other signal sources remain
outside this section until their host state is saved.

`PlayAnimationAndWait` event waits use optional `ANMW`, linking saved VM threads
to the sender reference and case-insensitive event name. Loading recomputes the
runtime signal key and replaces stale wait descriptors; legacy saves without
`ANMW` keep the existing timed fallback. This resumes a wait for a subsequent
matching event, but does not save or restore the sender's behavior graph pose,
timeline, or already-raised events. Persisting those graph states remains
separate work.

The weather (optional `WTHR`): the current and outgoing weathers, how far the
transition has come, whether an override holds, the weather it replaced, and
the game hours until the next roll. Older native saves load with no weather,
so the place picks one as it does for a new game. The regional list isn't
saved: it is taken again from the place the save loads into.

## Not saved yet (see `plans/ROADMAP.md`)

- Other signal waits are kept as timed waits that finish when their timeout
  runs out, with the default result: their host state is not saved yet.
  A thread whose function no longer resolves (other scripts) is dropped with
  a warning. Scripts on active magic effects are dropped with the effects.
  Registrations other than these (key, LOS gain, tracked hits...) aren't
  built in the engine yet.
- Active magic effects (timed effects, summons and ward spells), live player
  conversations, actor
  procedure/animation progress, AI package progress
  and schedules, objects' behaviour
  graphs, and search/combat detection state and AI execution (stealth reservoirs
  alone are saved),
  and the player's spells held ready (the equipped spells
  are saved with the inventory's hands).
- Ward-hit outcome and remaining `IsWardState` time are preserved separately
  in optional `WSTA`; this does not resume an active ward spell or its graph
  animation. The one-second hold is a native runtime policy, not a Skyrim SE
  save-format claim (`known_ambiguities/wards.md`).

## Unconfirmed choices

- **`RequestSave` / `RequestAutoSave`** write `quicksave` and `autosave`. The
  local UESP [GMST defaults page](https://en.uesp.net/w/index.php?oldid=3098585)
  (revision 3098585) gives `sMenuDisplayQuicksaveName` the display text
  "Quicksave". Bethesda's [Kinect support article](https://help.bethesda.net/app/answers/detail/a_id/16611/~/why-am-i-unable-to-create-more-than-one-quick-save-for-the-elder-scrolls-v:)
  says that interface has one quicksave slot. Neither source establishes the
  exact filename/counter behavior of `Game.RequestSave` in Skyrim SE. This
  engine therefore uses an explicit **native history policy**: each
  flushed `RequestSave` writes the next unused `quicksave N.vrs`, starting at 1,
  preserving earlier numbered files. It finds the next number from existing
  files, so gaps are retained and failed writes do not consume a number.
  `RequestAutoSave` continues to replace `autosave.vrs`; manual save names keep
  their existing replace-in-place behavior. Repeated requests of the same type
  in one pending update remain deduplicated. This is an engine policy, not a
  claim of Skyrim SE parity. The save is made on the next update, after the
  script returns, not inside the native call. A save is refused while the
  player is dead ("You cannot save right now.", `sCantSaveNow`); the original's
  other refusals (combat, menus, scripted forbidding) aren't built.
  "Autosaving..." (`sAutoSaving`) is shown for the autosave, which is our guess
  for when the game prints it.
- **Weather on load.** The local UESP
  [Save File Format / Weather page](https://en.uesp.net/w/index.php?oldid=2309091)
  (revision 2309091) lists the original's weather section: climate, weather,
  the previous weather (only during a transition), a regional weather, the
  current time, the weather's beginning time, the transition fraction, and
  some unnamed fields. It doesn't say whether the game re-rolls a loaded
  place's regional weather, nor what the unnamed fields are. Here the saved
  weather holds on load, whatever the place's region: the place takes its
  region without a change, and a region change or the saved roll timer
  brings a new weather as usual. Whether Skyrim SE does the same is
  unverified.
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

Optional `MAPS` retains map discovery and explicit travel grants/denials plus
the global script travel gate. Legacy files reset this runtime memory; see
`known_ambiguities/fast-travel.md`.

Optional `TEAM` retains teammate and canDoFavor flags by replacement. Legacy
files clear membership; see `known_ambiguities/followers.md`.

## Additional native continuity adjuncts

These optional sections retain existing VibeRM state; they do not establish
Skyrim SE's save representation, exact lifecycle or reset rules. Reading an
adjunct rejects duplicate sections, trailing data, invalid values and malformed
collection keys before live state changes. Loading replaces destination memory;
legacy saves clear the corresponding memory unless a different default is stated.
No game assets or engine binaries were inspected for these additions.

- `HRVT` retains harvested-reference identities and their game-hour timestamps.
  Existing regrowth checks run against the restored clock; load does not grant
  produce or replay sounds. UESP [Respawning, revision 3558013](https://en.uesp.net/w/index.php?oldid=3558013)
  describes reset schedules, including the default 240 hours, but does not settle
  individual plant save payloads or validate our harvest-time-based regrowth
  approximation (cell re-entry/reset behavior is separate work).

- `CLRD` retains cleared location identities without calling `SetCleared` during
  load. Original save encoding and reset lifetime remain unverified. The existing
  clear/statistic policy is unchanged; see `known_ambiguities/cleared.md`.

- `LKWD` retains `Location.SetKeywordData` values by location/keyword pair,
  including zero and negative values. Local UESP title searches for
  SetKeywordData and SetCleared found no API pages. Persistence/lifetime in SE
  remains unverified; this adjunct preserves the existing native API state.
  Form identities are retained without requiring a currently loaded location.

- `CRFC` retains script crime-faction assignments and explicit `None` overrides.
  Loading does not report crimes or change bounty. SE reset/persistence rules
  are unverified; see the public API research in `known_ambiguities/crime.md`.

- `PKCT` retains the existing caught-pickpocket gate by victim identity.
  UESP [Pickpocket, revision 3581319](https://en.uesp.net/w/index.php?oldid=3581319)
  discusses detection and retry restrictions; the precise SE reset lifetime and
  save encoding remain unverified. This does not add a new retry timer.

- `ASLT` stores both existing assault-memory sets. It preserves the current
  classification inputs without generating assault/murder events on load.
  Combat resumption and the original SE clearing lifetime remain separate,
  unresolved work; no inference about `.ess` structures is made.

- `UWIT` preserves existing pending reports, ordered witness lists and amounts.
  UESP [Crime, revision 3639929](https://en.uesp.net/w/index.php?oldid=3639929)
  describes witnesses and bounty consequences. The current native policy treats
  an unloaded witness as having reached guards; this adjunct does not resolve
  that approximation or SE save timing. Reports are not advanced during apply.

- `LCRM` preserves the existing last-crime input for later arrests without
  replaying an arrest or report. The native enum uses the already documented
  public crime types; SE save persistence and competing-report priority remain
  unverified. See `known_ambiguities/crime.md`.

- `CTRL` retains script-disabled controls and cart HUD mode. Both POV lock owners
  are saved independently, so enabling one does not release the other. Legacy
  loads enable controls and clear cart mode. Exact SE persistence/reset behavior
  is unverified; existing public control research is in player-controls.md.

- `VREC` retains shout cooldown seconds and the meter's original duration;
  old key holds and flashes are discarded. Legacy saves restore a ready voice.
  UESP [Dragon Shouts, revision 3279131](https://en.uesp.net/w/index.php?oldid=3279131)
  documents cooldown and its compass meter, not native save/reset semantics.
  Real-world time spent outside the runtime does not consume this countdown.
