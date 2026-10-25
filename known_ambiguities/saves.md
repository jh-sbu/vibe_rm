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
timeline, or already-raised events.

The weather (optional `WTHR`): the current and outgoing weathers, how far the
transition has come, whether an override holds, the weather it replaced, and
the game hours until the next roll. Older native saves load with no weather,
so the place picks one as it does for a new game. The regional list isn't
saved: it is taken again from the place the save loads into.

Signal waits whose host state isn't saved load as timed waits that finish
when their timeout runs out, with the default result. A thread whose function
no longer resolves (other scripts) is dropped with a warning. Scripts on
finished effects (kept briefly for `OnEffectFinish`) are dropped. Ward-hit
outcome and remaining `IsWardState` time are in optional `WSTA`
(`known_ambiguities/wards.md`).

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
  player is dead ("You cannot save right now.", `sCantSaveNow`).
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
  approximation.

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

- `PKCT` retains caught-pickpocket victims by identity; optional `PKTM` adds
  the game hour each caught the player and whether it has unloaded since (the
  two-day retry gate: `known_ambiguities/crime.md`). `PKTM` must cover exactly
  `PKCT`'s victims. A save with `PKCT` and no `PKTM` treats its victims as
  caught at the loaded hour, not yet away; with neither, nobody remembers.
  The original save encoding is unknown.

- `ASLT` stores both existing assault-memory sets. It preserves the current
  classification inputs without generating assault/murder events on load.
  The original SE clearing lifetime is unresolved; no inference about `.ess`
  structures is made.

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

- `SREG` stores remaining native stamina regeneration delay, clamped to zero
  after expiry. Legacy loads have no delay. Original SE save timing and the
  exact within-frame expiry behavior remain unverified; existing update cadence
  is preserved. This does not save NPC stamina or combat execution.

- `MREG` stores remaining magicka regeneration delay, with expired timers saved
  as zero. Legacy loads have no delay. Loads discard cast/release input state
  rather than firing an old spell; behavior graphs and channels are not resumed.
  UESP [Magicka, revision 2947763](https://en.uesp.net/w/index.php?oldid=2947763)
  describes regeneration and casting suppression but not SE save semantics.

- `JMEM` retains the native miscellaneous-objective visibility toggle and
  sequence allocators. Legacy loads show miscellaneous objectives and recover
  allocator minima from saved quest entries/alias fills. Capture uses those same
  minima, keeping native save/load/save stable even after legacy imports. This fixes native
  ordering continuity; exact SE journal persistence and ordering remain
  unverified (`known_ambiguities/journal.md`).

- `MUSE` retains existing spell/school/shout usage metrics and tied incumbents.
  Missing sections clear all three histories. This preserves VibeRM's existing
  counting and tie policy, whose SE parity remains unverified (journal.md).
  Raw nonnull form identities survive independently of whether a spell/shout
  is currently known; schools are restricted to the five existing magic skills.

- `CAMR` retains perspective and third-person zoom within the current runtime
  bounds. Legacy saves choose first person and the native default distance.
  Combat interpolation starts fresh; cinematic, test and transient collision
  camera offsets are not saved. SE camera persistence and restoration ordering
  remain unverified; this is a native user-view continuity policy.

## Further native continuity policies (2026-10)

Roadmap batch: `plans/roadmap-durable-fifteen-items.md`. Public references read
through the local UESP index:
[Crime, revision 3639929](https://en.uesp.net/w/index.php?oldid=3639929)
describes warnings, arrest resistance, yielding, jail sentences and escape's
restored bounty. [REFR, revision 2354868](https://en.uesp.net/w/index.php?oldid=2354868)
documents placed-reference data. Neither establishes save encoding or precise
save/load lifetime for these states. The following are native continuity choices,
not assertions about `.ess` or SE lifecycle behavior. No executable/library data
was examined; no game assets were included.

Crime progression and deadlines replace the prior session's memory. Pending
arrest conversations do not resume dialogue; guards can approach again. Legacy
files clear these categories. Package-owned home maps are retained as native
memory, subject to normal package refresh; exact SE package refresh ordering is
unverified. Delayed activations retain absolute native real-time deadlines and
queue order; loading does not send the events early. Random streams resume their
saved sequence, with fixed nonzero legacy seeds. Critical-hit memory retains the
last-hit condition. Sneak stance and attack recovery resume; held input and
animation phases do not. Map camera preference resumes independently of an open
menu. Image modifiers retain order, clocks, strength and cross-fades; script screen
fades retain their clocks and color. Sleep overlays and camera shakes are transient.
SE behavior for each of these transitions remains unverified by a controlled test.

Cell attachment uses the restored native random seed for reconstruction, then
reinstates both saved streams so reconstruction does not consume subsequent
rolls. This does not retain already-chosen AI decisions or procedural progress.
The map preference uses the existing native world-independent `map_start`
coordinates; it does not restore an open menu or establish cross-worldspace map
camera semantics. Console camera limits are normalized on capture as on opening;
nonfinite pending preferences are discarded. Script-fade snapshots exclude
camera shake offsets; modifier/fade rendering is recomputed on the next update.

## Active magic effects (`AEFF`)

Optional `AEFF` keeps every unfinished active effect: its id, target, caster,
item and effect index, magnitude, duration, elapsed time, whether it is doing
its work, what it holds on its actor values, its landing point, its summon and
its quiet / dual / channelled / scripted flags, with the next effect id.
Telekinesis is left out because the object it holds isn't saved; the grab ends
on load. The scripts of already-finished effects are not kept. Optional
`TAPR` keeps value changes still tapering off (the effect as it landed, the
rate it started at and the seconds since), so they run out after loading;
legacy saves have none. Whether SE saves tapers at all is unknown.

Evidence: CommonLibSSE-NG's public `RE/A/ActiveEffect.h` declares `SaveGame`,
`LoadGame`, `FinishLoadGame` and `Revert` on `ActiveEffect`, so SE does save
active effects with their `elapsedSeconds`, `duration` and `magnitude`. Which
other fields it keeps, and what `FinishLoadGame` redoes (visuals, sounds,
scripts, value changes), are not public. The local UESP
[Save File Format, revision 3067914](https://en.uesp.net/w/index.php?oldid=3067914)
lists the global `Effects` section, whose
[Effects page, revision 2309066](https://en.uesp.net/w/index.php?oldid=2309066)
holds image-space modifiers only, and the
[ACHR change form, revision 3187377](https://en.uesp.net/w/index.php?oldid=3187377)
doesn't document where actors' effects go.

Native policies (not SE claims):

- **No restart.** `AVAL` already holds what the effects hold, so restored effects
  are not started again: no `OnEffectStart`, `OnMagicEffectApply`, hit, combat or
  value change happens on load. The old session's effects are discarded without
  giving their holds back into the loaded values.
- **Summons** keep their created actor (saved in `WRLD`), and who commands it is
  rebuilt from the summon effects. A summon left unloaded still ends the effect
  on the next update, as at runtime.
- **NPC effects** restore before the save's cells load, and end normally on the
  next update if their actor didn't load.
- **Legacy saves** without `AEFF` have no effects. Their `AVAL` still holds what
  the effects held, so those changes stay on, and abilities are applied over
  them again. Those saves can't say what was held, so nothing is subtracted.
- **Scripted effects** keep their scripts, variables and waiting threads in
  `VM` (object kind 2, already in the layout; the layout is unchanged). Only
  effects `AEFF` keeps are written; finished effects' lingering scripts are
  dropped, as before. A save whose `VM` names an effect `AEFF` doesn't keep
  is refused before anything changes. Legacy saves had no effect objects.

## Behaviour graph open states (`GOPN`)

Objects' behaviour graphs themselves (states, clocks, variables) are not
saved; loading clears the parked graphs as before. Optional `GOPN` keeps the
open state each graph object reached through its `Open` / `Close` /
`Opened` / `Closed` events and raised `Opening` / `Closing` (what
`GetOpenState` reads). After a load, a graph built afresh is sent one event
to get back there: `Opened` or `Closed` for the end states, `Open` or `Close`
for a movement under way, which then plays again from the graph's start.
Legacy saves have no open states, so graph objects start as their graphs
start.

Native policy, not an SE claim. UESP's
[Save File Format, revision 3067914](https://en.uesp.net/w/index.php?oldid=3067914)
lists an "Anim Objects" global section (1002) with no documented contents.
Open: whether the original saves graph states and resumes them exactly,
whether a graph that doesn't know `Opened` / `Closed` (only `Open` /
`Close`) ends up in the right pose, which this replay can't do, and graph
objects with other states (levers, rockfalls, traps), which this doesn't
cover.

## Faction reactions (`FRCT`)

Optional `FRCT` keeps every faction reaction scripts changed (faction,
towards, modifier, group combat reaction 0-3), sorted by pair; loading
replaces this session's changes, and legacy saves have none, so factions
react as their plugins say. The original saves a faction's changed
reactions as change form data (CommonLibSSE `TESFaction::ChangeFlags`
`kFactionReactions`); its layout is unknown. Native policy, not an SE claim.
Choices: `known_ambiguities/factions.md`.

## Faction player flags (`FPLR`)

Optional `FPLR` keeps the player expelled / enemy flags (`DATA` bits 2 and 3)
of each faction a script changed, sorted by faction. Loading replaces this
session's flags; legacy saves have none, so each faction's `DATA` holds.
The original keeps changed faction flags as change form data
(`kFactionFlags`, layout unknown). Native policy, not an SE claim.

## Actors attacked on sight (`AOSG`)

Optional `AOSG` lists the actors (the player included) that
`SetAttackActorOnSight` marked, sorted. Loading replaces this session's
marks, and legacy saves have none. Native policy, not an SE claim.

## Actor bases' flags (`BFLG`)

Optional `BFLG` lists (base, `ACBS` flag, set) for each essential, protected
or invulnerable flag scripts changed, sorted. Loading replaces this
session's changes; legacy saves have none, so bases are as their records
say. The original keeps changed base data as change form data (layout not
researched). Native policy, not an SE claim.

## Ghosts (`GHST`)

Optional `GHST` lists the references `SetGhost` changed with their state,
sorted. Loading replaces this session's; legacy saves have none, so each
actor is as its base says. The original keeps `ExtraGhost` on the reference
(change form layout not researched). Native policy, not an SE claim.

## Form list additions (`FLAD`)

Optional `FLAD` lists, by form list (sorted), the forms scripts added in the
order added. Loading replaces this session's; legacy saves have none, so
lists are as their plugins say. The original saves them as the list's
`kAddedForm` change data (layout not researched). Native policy, not an SE
claim.

## Outfits (`OUTF`)

Optional `OUTF` lists (actor, outfit) for each reference `SetOutfit` gave a
default outfit, sorted. The armor itself is in the saved inventories.
Loading replaces this session's; legacy saves have none, so actors wear
their NPC's `DOFT`. Native policy, not an SE claim.

## Actors with their AI off (`AIOF`)

Optional `AIOF` lists the actors `EnableAI(false)` turned off, sorted.
Loading replaces this session's; legacy saves have none, so every actor's AI
runs. Native policy, not an SE claim.
