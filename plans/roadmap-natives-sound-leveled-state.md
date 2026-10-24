# Plan: sound, leveled-list and state/query natives

**Status: done** (2026-10-24). All ten batches are implemented; the list of what is
still undeclared in `src/script/natives.rs` is the systems the plan names as out of
scope. See ROADMAP's Done section.

This plan covers vanilla natives that are missing or stubbed and that need no
new engine system. The exceptions are the music queue (batch 2) and the
reset baseline (batch 8), which each grow a system that already exists. The
list comes from `vrm-tool pex-natives` on the SSE data compared with the arms
of `src/script/natives.rs` (2026-10-24: 686 natives declared, 268 with no arm,
14 stubs). The natives that wait on systems the engine lacks are not in this
plan: effect shaders and visual effects, riding, dragon flight, character
creation, killmoves, beast forms, subgraphs and expressions, spline and forced
movement, player AI control, bleedout and unconscious state, and the crime gaps.

Call counts are `vrm-tool pex-natives` counts. They match calls by name alone,
so names shared between classes share their count: `addform` (245) includes
`FormList.AddForm`, `play` (929) includes `EffectShader` / `VisualEffect.Play`,
and `reset` (81) is quest, cell and reference resets together. Treat them as
upper bounds.

## Ground rules (as in the earlier native batches)

- Public sources only: the Papyrus Index / community CK wiki pages
  (papyrus.bellcube.dev), local UESP (`scripts/uesp.py`), xEdit definitions and
  `refs/CommonLibSSE-NG` headers. Write one authored note per batch under
  `refs/pex-natives/batchNN-*.md` (force-added), citing the revision links and
  separating what the sources say from inference.
- Policies the sources leave open go in `known_ambiguities/remaining-natives.md`,
  or in the existing topic file (`outfits.md`, `formlists.md`, `crime.md`...)
  where one exists. Questions a `vrm-tool test-mod` could settle are marked
  **test-mod** below; collect them into one `autotest` batch for the user to run.
- Any new runtime state gets an optional native-save continuity section
  (`src/save/continuity.rs`, as `FLAD` / `OUTF`). Legacy saves without it
  default to the plugin's state, and each section gets a `check_continuity_payload`
  test and a legacy-load test.
- Each batch is one commit: focused synthetic tests (`esp::writer::RecordBuilder`
  records, as in `src/formlists.rs` / `query_natives_tests.rs`), `cargo fmt`, and
  a fresh `pex-natives` comparison to confirm the arms match the declared names.
- When the plan is done, move the items into ROADMAP's Done section with links
  to this file. Also update the "Natives scripts call that still do nothing"
  bullet.

## Batches

### 1. Sound instances: `Sound.Play`, `StopInstance`, `SetInstanceVolume`, `PlayAndWait` (done)

Done in `src/script_sounds.rs`; notes in `refs/pex-natives/sound-batch01-instances.md`.

Counts: play ≤929 (currently a stub returning 0), stopinstance 63,
setinstancevolume 5, playandwait 0 (stub).

- `Engine::play_sound` should return the `VoiceId` that `Audio::play_pitched`
  already gives back. Add a script instance table to `Engine`:
  `script_sounds: HashMap<i32, Option<VoiceId>>` with a counter starting at 1.
  `Play(akSource)` plays the descriptor at the source's position and returns
  the new instance id. With no audio device (headless tests and runs) it still
  allocates an id mapped to `None`, so scripts' bookkeeping still works. A
  missing descriptor or source returns 0.
- A looping descriptor must loop until `StopInstance`. That is the reason
  `StopInstance` is called so often (ambient loops on quests' activators).
- `StopInstance(id)` → `Audio::stop`, then drop the entry.
  `SetInstanceVolume(id, f)` → `Audio::set_volume(desc volume × category × f)`,
  so keep the descriptor's base gain in the table entry.
- `PlayAndWait`: return `NativeResult::WaitFor` with a key per instance. The
  engine's frame update signals it (`Vm::signal`) once
  `!Audio::is_playing(voice)`. The timeout comes from `clip_duration` when it is
  known, else a generous cap. Without a device, signal at once.
- Prune ended non-looping entries in the frame update.
- Open questions for `remaining-natives.md`:
  - whether ids are reused or monotonic;
  - whether the voice follows a moving source (ours stays at the start position;
    `Audio` has no position setter yet, which would be a small follow-up);
  - whether script loops survive a save. We don't save them, so after a load a
    stored id stops nothing. Inference: SE does not persist sound handles.
    **test-mod**: start a looping `Sound.Play`, save, load, and check whether
    it's still audible and `StopInstance` stops it.

### 2. Music queue: `MusicType.Add`, `MusicType.Remove` (done)

Done in `src/music.rs` (saved as `MUSQ`); notes in `refs/pex-natives/sound-batch02-music-queue.md`.

Counts: add 38, remove ≤84.

- `BSMusicManager` (CommonLibSSE-NG) keeps a `musicQueue` of music types with
  a `current` one. MUSC `PNAM` gives a priority (1 highest, 100 lowest) and
  ducking, and `FNAM` gives flags (Plays One Selection, Abrupt Transition, Cycle
  Tracks, Ducks Current Track); see UESP MUSC, rev 2384538,
  https://en.uesp.net/w/index.php?oldid=2384538.
- Replace `Music::once` in `src/engine.rs` with a queue of
  `(music type, plays_once, finished)`. `update_music` plays the
  highest-priority entry, with the location's music as an implicit entry at its
  own priority. `play_music_once` (discovery and cleared music) becomes a queue
  push with "Plays One Selection" behaviour. `Add` pushes (no duplicates);
  `Remove` takes it out and returns to the next entry. Honour Abrupt Transition
  (stop now) versus waiting for the track to end; fades can wait.
- Save the queue's script-added entries as a new optional `MUSQ` section.
- Open questions: tie-breaking at equal priority (push order versus first
  added), whether `Add` of a queued type moves it, and whether the queue
  survives a save. **test-mod** for the last two.

### 3. Leveled lists: `LeveledItem` / `LeveledActor` / `LeveledSpell` `.AddForm`, `.Revert` (done)

Done in `src/leveled.rs` (additions ride in `Globals::leveled`; saved as `LVAD`); notes in `refs/pex-natives/leveled-batch03-addform.md`.

Counts: addform ≤245, revert ≤8 (shared with `FormList`).

- New `src/leveled.rs`, the same shape as `src/formlists.rs`:
  `added: HashMap<FormId, Vec<(u16 level, FormId, u16 count)>>`.
  `LeveledItem.AddForm(form, level, count)`; actors and spells pass count 1.
  `Revert` drops the list's script additions and leaves the plugin's `LVLO`
  entries. Save them as optional `LVAD`.
- The resolvers in `src/world/actor.rs` (`eligible_at`, `resolve_leveled`,
  `resolve_items`, `resolve_spells`) read records through `&LoadOrder` and only
  get `&Globals`. Bundle both into a `LeveledCtx { globals, added }` and add the
  script entries after the plugin's in `eligible_at`. Thread it through every
  caller, including `src/world/template.rs` and the cell-load spawn paths that
  now pass `&Default::default()`. Otherwise leveled actors placed after an
  `AddForm` would ignore it.
- Cap the total at 255 entries (`TESLeveledList::numEntries` is a `u8`).
- Already-rolled inventories and spawned actors stay as they are; only later
  rolls see the additions.
- Open questions for `known_ambiguities/formlists.md` (or a new `leveled.md`):
  whether `AddForm` deduplicates (we don't, because leveled lists allow
  duplicates), whether the additions persist in SE saves, and whether `Revert`
  also restores entries edited by other means. **test-mod**: add an item to an
  empty list, save and load, then roll it through `AddItem`.

### 4. Reference setters (replacing stubs): `SetOpen`, `SetDestroyed`, `SetNoFavorAllowed`, `SetScale`, `GetHeadingAngle` (done)

Done in `src/ref_state.rs` (saved as `RSCL` / `NOFV`); notes in `refs/pex-natives/ref-batch04-setters.md`. Nothing asks followers for favors yet, so `SetNoFavorAllowed` is only stored.

Counts: setopen 146, setnofavorallowed 60, setdestroyed 39, setscale 25,
getheadingangle 9.

- `SetOpen(abOpen)` → `toggle_door` in `src/world/animated.rs` when
  `open_state` differs from the request. It should not fire activation events
  (check what the sources say about `OnOpen` / `OnClose`). It currently returns
  `true` and does nothing, but Papyrus declares it `None`; fix the return type
  for all three stubs.
- `SetDestroyed(abDestroyed)` → `src/world/destruction.rs`: jump to the final
  destruction stage, or `clear_destruction` when false. Choose whether stage
  side effects (explosions, debris) fire: inference says no.
- `SetNoFavorAllowed` → a per-reference flag in `scripts` that follower command
  targeting (`src/followers.rs`) consults. Save it with the other script ref
  flags.
- `SetScale(f)` → a per-reference scale override, read by the existing
  `GetScale` (batch 6 of the forty) and applied to the render instance and
  physics shape when loaded. Save it as part of the reference continuity data.
- `GetHeadingAngle(akOther)`: the signed angle in degrees (−180..180) from the
  reference's heading to the other's position. This is the documented formula;
  write tests at 0/90/180/−90.

### 5. Reference flags and queries (done)

Done in `src/ref_state.rs`; notes in `refs/pex-natives/ref-batch05-flags-queries.md`.

`IgnoreFriendlyHits` (56) / `IsIgnoringFriendlyHits` (0), `Cell.SetPublic` (13),
`IsFurnitureInUse` (19) / `IsFurnitureMarkerInUse` (0),
`IsInDialogueWithPlayer` (13), `HasNode` (12), `GetMass` (7), `IsDeleted` (1),
`IsActivateChild` (0), `CalculateEncounterLevel` (19).

- `IgnoreFriendlyHits`: a per-ref flag checked where `src/ai/combat.rs` turns a
  friendly hit into aggression; saved.
- `Cell.SetPublic` → the existing `scripts.cell_public` map that `crime.rs` and
  `locks.rs` already use.
- Furniture: use the actor furniture-use state that `scene.rs`, `arrest.rs` and
  `detection.rs` already read. "Marker in use" takes a marker index.
- `IsInDialogueWithPlayer`: the dialogue target that `Actor.GetDialogueTarget`
  already uses.
- `HasNode`: look up the loaded model's node names. Unloaded returns false.
- `GetMass`: the havok body mass that `src/loose.rs` / `src/grab.rs` read.
- `IsDeleted`: the reference's deleted flag, or a deleted created ref.
- `IsActivateChild`: the `XAPR` activate parents of the argument.
- `CalculateEncounterLevel(difficulty)`: the location / encounter-zone level
  formula from UESP's leveling pages, clamped to the zone's min and max. Write
  down the difficulty multipliers in the batch note.

### 6. Actor state (done)

Done in `src/actor_flags.rs` (saved as `ACST`); notes in `refs/pex-natives/actor-batch06-state.md`.

`SetCriticalStage` (45), `Resurrect` (34), `ResetHealthAndLimbs` (8),
`UnequipAll` (7), `UnequipItemSlot` (4), `GetSitState` / `GetSleepState`
(10/10, stubs), `ClearExtraArrows` (10), `SetNotShowOnStealthMeter` (14),
`Set` / `GetVoiceRecoveryTime` (13/0).

- `SetCriticalStage`: store the stage, readable by the existing
  `GetCriticalStage` condition. Of the stage consequences, carry out only those
  that exist now (disintegrate-end removes the body). Ash piles and shaders wait
  for effect-shader rendering; record that in ROADMAP.
- `Resurrect`: the inverse of `kill` in `src/ai/combat.rs`. Clear dead, restore
  health and the other actor values, leave the ragdoll for the behaviour graph,
  and re-enter AI. The inventory is kept. **test-mod**: whether equipped items
  come back worn.
- `UnequipAll` / `UnequipItemSlot` → the unequip helpers in `src/items.rs`.
- Sit and sleep states: compute them from the furniture-use state (the 0–4
  enumerations on the CK wiki pages). Entering and exiting phases may only be
  able to report "sitting / not" until the graph exposes the transition.
- `ClearExtraArrows`: remove stuck arrows from `src/ai/archery.rs` that are
  attached to the actor.
- `SetNotShowOnStealthMeter`: a flag that `detection.rs` skips when computing
  the HUD's detected state.
- Voice recovery: the shout cooldown in `src/shouts.rs`.

### 7. Relationships, factions and followers (done)

Done in `src/actor_flags.rs`, `src/outfits.rs` (base outfits, saved as `BOFT`; favor flag as `FAVR`); notes in `refs/pex-natives/social-batch07-relationships-followers.md`. There is no follower favor mode yet, so `SetDoingFavor` only stores.

`IsCommandedActor` (28), `SetDoingFavor` (41) / `IsDoingFavor` (0),
`HasFamilyRelationship` (1), `HasParentRelationship` (1), `HasAssociation` (6),
`GetFactionReaction` (2), `IsHostileToActor` (3), `ActorBase.SetOutfit` (82
shared with `Actor.SetOutfit`).

- `IsCommandedActor` → `magic.commanded` (summons, reanimated and commanded
  actors).
- Doing favor: a follower command-mode flag in `src/followers.rs`. Setting it
  enters or leaves command mode where that exists, otherwise it is only stored
  and queried.
- Relationship queries → `src/relationships.rs` (`RELA` rank and association
  type; family = parent/child/sibling association types).
- `GetFactionReaction` / `IsHostileToActor` → `src/factions.rs` / combat's
  hostility test.
- `ActorBase.SetOutfit`: change the base's default outfit for every reference
  that shares it and has no reference-level override. This settles the item
  `known_ambiguities/outfits.md` lists as unconfirmed; keep that file current.

### 8. Resets and quest instances (done)

Done in `src/resets.rs` (saved as `RSTC` / `QGLB`); notes in `refs/pex-natives/reset-batch08-resets-instances.md`. `Quest.Reset` leaves the quest's script variables alone (no VM helper rebuilds them yet).

`Quest.Reset`, `ObjectReference.Reset`, `Cell.Reset` (≤81 together),
`Quest.UpdateCurrentInstanceGlobal` (77), `Quest.IsStarting` / `IsStopping` (0).

- This is the largest batch: it needs a reset baseline.
  - `Quest.Reset`: stop the quest, clear stages, objectives and alias fills,
    reinitialise script properties and variables to their defaults, and leave
    it not running (CK wiki). It needs a VM helper to rebuild a script
    instance's variables.
  - `ObjectReference.Reset(akTarget)`: put the reference back at its plugin
    position (or at `akTarget`), enable it, clear its destruction and restore
    its original inventory roll. Define exactly what "reset" restores in
    `remaining-natives.md` before coding.
  - `Cell.Reset`: apply `ObjectReference.Reset` to every reference in the cell
    and mark it for respawn. If the respawn timer system is absent, apply it
    when the cell next loads.
- `UpdateCurrentInstanceGlobal(global)`: radiant quests' text replacement keeps
  a per-quest copy of a global's value. Store that copy on the quest and use it
  in `<Global=...>` text replacement.
- `IsStarting` / `IsStopping`: our transitions are synchronous (batch 2 of the
  forty), so both return false. Document that as a policy, with no fake states.

### 9. Game queries and flags (done)

Done in `src/game_queries.rs` (saved as `GAMF`); notes in `refs/pex-natives/game-batch09-queries-flags.md`.

`Game.FindClosestActor`, `FindClosestReferenceOfType`,
`FindClosestReferenceOfAnyTypeInList`, `FindRandomActor`,
`FindRandomReferenceOfType`, `FindRandomReferenceOfAnyTypeInList` (1–4 each),
`GetSunPositionX/Y/Z` (1–2), `IsPlayerSunGazing` (0), `SetPlayerReportCrime`
(6), `ClearPrison` (2), `SetInChargen` (33, stub), `CalculateFavorCost` (0).

- Finders search loaded references (actors: loaded and not dead unless the
  sources say otherwise) within a radius of a point. "Random" uses the engine
  RNG. Share one spatial iterator with `src/ai/nav.rs`'s closest-reference
  helpers.
- Sun position: take the sun direction that `src/weather.rs` /
  `src/render/sky.rs` already compute, scaled to the documented distance.
  Sun-gazing compares the camera forward with it.
- `SetPlayerReportCrime` → a crime flag that `src/crime.rs` checks before
  reporting. `ClearPrison` → the arrest/jail state in `src/arrest.rs`.
- `SetInChargen(disableSaving, disableWaiting, showControlsDisabledMessage)`:
  store the three flags and gate saving and waiting on them.
- `CalculateFavorCost`: the gold cost formula from the CK wiki, or skip it with
  a note, since nothing calls it.

### 10. Knowledge and the uncalled remainder (done)

Done in `src/knowledge.rs`, `src/script_sounds.rs` (sound categories) and `src/audio/mod.rs`; notes in `refs/pex-natives/knowledge-batch10-remainder.md`. Dialogue, music and weather sounds aren't tagged with categories yet.

`Ingredient.LearnEffect` / `LearnNextEffect` / `LearnAllEffects`,
`Form.PlayerKnows`, `Utility.GameTimeToString`, `Spell.Preload` / `Unload` (5/5),
`SoundCategory.Mute` / `Unmute` / `Pause` / `UnPause` / `SetVolume` /
`SetFrequency`, `Utility.SetINI*`, `Armor` / `Light` / `Actor.GetWarmthRating`.

Except for the spell preloads, nothing in vanilla calls these. Do this batch
last.

- Ingredient learning → `world_state.learned_effects` (`src/magic.rs`).
- `PlayerKnows` → the same store for ingredients, plus word, spell and recipe
  knowledge where it exists.
- `GameTimeToString`: format `game_days()` the way the CK wiki shows.
- `Spell.Preload` / `Unload`: these hint at asset streaming. Arms that only
  return are correct here; note that.
- SoundCategory: needs a category tag on each mixer voice in `src/audio`, a
  per-category runtime multiplier and pause in the mixer, and pitch for
  frequency. The SNCT chain math is in `src/world/sound.rs`.
- INI setters: keep an in-memory map of settings, read wherever a setting is
  consumed. The game writes nothing back.
- Warmth is part of Survival (Creation Club). Return 0 until Survival exists and
  note the dependency.

## Order and rough size

1, 3 and 4 have the highest call counts. 2, 6 and 7 are medium. 5 and 9 are
small. 8 is the largest and needs its baseline policy agreed first. 10 is
optional. Batches 1–7 and 9 are independent of each other; batch 8 benefits
from batch 4's `SetDestroyed` / `SetScale` state.
