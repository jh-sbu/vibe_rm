# Saves: what is left, sorted

**Status: drafted** (2026-11-15). Not started. Follows `plans/saves-unblocked.md`
(done 2026-11-12) and covers ROADMAP item 9's remaining list. Each numbered
step is one commit, in order. Each step that adds or changes a save section
updates `known_ambiguities/saves.md` and the section list in
`src/save/state.rs`'s module documentation. Every new section is optional, as
the others are: a save without it loads as before.

## What was checked (2026-11-15)

ROADMAP item 9 listed these as open. Each was checked against the code and the
available evidence:

| Item | Verdict | Why |
| --- | --- | --- |
| Load orders that changed since the save | **Unblocked** | The user observed SE's three cases (`research/ui/main-menu.md` item 4; `plans/main-menu.md`, "Remaining"). The user decided the prompts' wording (`deviations/main-menu.md`). UESP *Save File Format* ([rev 3067914](https://en.uesp.net/w/index.php?oldid=3067914), "RefID", "Plugin Info", "Light Plugin Info") shows that SE stores form ids against the save's own plugin lists, so it remaps by name. `sSaveGameContentIsMissing` ([GMST defaults, rev 3098585](https://en.uesp.net/w/index.php?oldid=3098585)) gives the general missing-content text. What this needs is engineering, below. |
| Stale session state after a load | **Unblocked (found while checking)** | `Barks` (`greeted`, `chatter_at`, `quiet_until`, `HelloContacts`) and `ScriptSounds` are neither saved nor reset by `Snapshot::apply`. Loading in a running game keeps the old session's hello timers. Loading in a new process forgets them, so every NPC in range may greet at once. |
| Sound instance ids | **Unblocked (part)** | `ScriptSounds::next` starts at 1 in a new process, but VM variables loaded from the save can hold earlier ids. A stored id can then match a new sound, and `StopInstance` stops the wrong sound. Saving the counter fixes that. Whether a script's loop plays on after a load is still a probe question (`known_ambiguities/remaining-natives.md`, "Sound instances"). |
| NPCs' greetings | **Unblocked** | Native continuity policy, like `FGHT` and `AEXC`: the hello and chatter timers above, plus force greeters' `greeting` / `greet_wait` / `greet_check` (`ActorExec`, `src/ai/greet.rs`). No SE claim. |
| A goal set by hand (console) | **Unblocked, small** | `ActorExec` keeps it (`src/ai/exec.rs:878`), but `AEXC` leaves it out. |
| Actors' behaviour graphs, NPC casts and swings in progress, procedures' loaded-only state | **Blocked on an SE observation; the probe is unblocked** | On load, every execution goes through `Engine::away` (`restore_executions`). So the save's own cells attach their actors as a cell reload does: `Stage::starts_fresh` follows autotest16's reload readings, where a seated Sit-package actor stands by its seat and a seated sandboxer picks anew. Nothing says what SE does with actors loaded *at save time*. autotest14 reloaded a save only for actors away from the player. Restoring graphs mid-furniture needs `seat`, `sub`, `moving`, `sneaking`, `drawn`, `torch` and more from `ActorRuntime` restored to match. That isn't worth building until SE shows what it keeps. Step 9 builds the probe. |
| Script sounds playing on after a load | **Blocked on the same probe** | `known_ambiguities/remaining-natives.md` already asks for the test. It goes into step 9. |
| SE's refusals to save | **Blocked on observation** | No public source covers this, and a script can't attempt a save and read whether it was refused. Step 9's run instructions add a short manual checklist for the user. |
| Collecting unused runtime forms | **Left out** | SE's collection timing is unknown. A form a Papyrus `Int` holds (`GetFormID`) can't be traced, so dropping forms could break scripts. It stays deferred, as `known_ambiguities/saves.md` has it. |
| Reading SE's `.ess` saves | **Left out** | A separate project, and not needed for parity of play. |

## Part A: session state across loads

1. **Reset what a load replaces.** Audit `Engine`'s fields for state that a
   save neither holds nor resets, and that is tied to the game being
   played, not to the process (`Barks`, `ScriptSounds::instances`, others
   the audit finds). Reset each in `Snapshot::apply` before the snapshot is
   applied. Test: one save loaded into a fresh engine and into an engine
   that has been running another game reaches the same state, across the
   reset fields.
2. **Sound instance ids** (`SNDI`). Save `ScriptSounds::next`; loading
   takes the larger of it and the running counter. Legacy saves start at 1
   (the current behaviour). Test: a saved id can't stop a sound started
   after the load. The loop itself still doesn't play on (step 9).
3. **Greetings** (`GRTG`). The player hello times (`greeted`, less those
   already free to greet again), the chatter times and `quiet_until`.
   They're stored against `scripts.real_time`, which `TIME` already saves.
   Legacy saves keep step 1's reset. Native policy, recorded in
   `known_ambiguities/saves.md`. Left out when implementing: force
   greeters' `greeting` / `greet_wait` / `greet_check` and the NPC-to-NPC
   hello contacts. Unloading a cell already drops them (`take_back`
   doesn't carry them), and a load attaches actors as a cell load does.
4. **A goal set by hand** in `AEXC`, as a new version of the section.
   Older versions load with none, as now.

## Part B: loading a save into a changed load order

The behaviour, from the user's SE observations, with the prompt wording
the user decided:

- A save made without a plugin that is now loaded: loads with no warning.
- Plugins in a different order: no observation. They load remapped, as
  the first case implies; recorded in `known_ambiguities/saves.md`.
- A save that needs a plugin that's in Data but not loaded: a prompt in
  plugin terms (`deviations/main-menu.md`).
- A save that needs a plugin not in Data: "continue loading without it?",
  in plugin terms. If continued, the missing plugins' forms are gone.

5. **The save's plugin slots** (`PLGS`, `src/save/plugin_map.rs`). Store each
   plugin's slot as saved: full index, or light index for `FE` slots.
   `HEAD` keeps names only. For saves without `PLGS`, a plugin's lightness
   is taken from the plugin if it's loaded now, else from an `.esl`
   extension. That fallback is a choice, recorded in
   `known_ambiguities/saves.md`. Build a `FormMap` from the saved slots to
   the current load order: names matched case-insensitively; `FF` runtime
   ids unchanged; ids of plugins not loaded map to `Gone`. Classify a save as
   same, superset, reordered or needs-missing (a plugin in Data but not
   loaded counts as missing; see "Decided" below). Unit tests for each
   class, and for light and full slots swapping order.
6. **Remapped reading.** `Reader::with_map` maps every `form()` /
   `opt_form()`. The section readers' ordering checks (about 60:
   "unsorted", "not ordered") go through one `Reader` helper.
   - Same-order saves: the helper does the checks, as now. No behaviour
     change for them.
   - Remapped saves: the file is first read once unmapped, with every
     check, so damage is still caught. The second, mapped read skips the
     ordering checks, and vectors that later code expects sorted are sorted
     after it (the audit in this step lists them).
   - The other places form ids hide: `ObjectId::Form` and `ObjectId::Alias`'s
     quest in `VM` (read as `u32`); the runtime `ALCH` / `ENCH` records in
     `FORM`, whose form-id fields are known from their builders (`EFID`;
     `YNAM`, `ZNAM`, `KWDA` and `ENIT`'s form fields on `ALCH`; `ENIT`'s
     base-enchantment and worn-restriction fields on `ENCH`), checked
     against the UESP record pages; `r.u32()` sites and opaque byte blobs
     (graph snapshots, procedure tree keys: editor ids, no form ids).
     Each is checked and listed.
   - Not remappable: form ids scripts stored as plain `Int`s
     (`GetFormID`). SE can't remap those either; recorded.
   Test: a save made with plugins A, B loads with B, A into the same
   state, and loads unchanged with a plugin added after them. Both with
   test plugins built in the test, not game files.
7. **Missing plugins' forms.** A pruning pass between the mapped read and
   `apply`'s validation, so `apply`'s validations don't refuse the whole
   save over a form that is gone:
   - Entries keyed by a gone form are dropped.
   - Optional fields naming one become none.
   - Papyrus values naming one become `None`; script objects on gone forms
     are dropped, with their threads (as for "a function that no longer
     resolves").
   - Inventories lose the items.
   - Created references whose base is gone are dropped.
   - A save standing in a gone cell or worldspace is refused, with a
     message naming the plugin.
   Each rule goes in `known_ambiguities/saves.md` as a choice: the
   observation shows only that SE asks and then loads. The pass runs
   section by section, through the same lists step 6 audited. Test: a
   save whose quests, items, references, scripts and executions use a
   test plugin loads without it, with those entries gone and everything
   else as saved.
8. **The menus.** The journal's Load page and the main menu's Load and
   Continue (`save_list_in`, `metadata.rs`'s `plugins_hash`) classify
   saves by step 5. Same, superset and reordered saves load as usual, no
   longer marked obsolete. A needs-missing save (an inactive plugin counts as missing) asks,
   worded as in `deviations/main-menu.md`, with the plugins listed, and
   loads without them on Yes (see "Decided" below). Tests on the
   listing and on the prompts' text and outcomes. Update
   `plans/main-menu.md`'s "Remaining" and ROADMAP item 9.

## Part C: the SE probe

9. **`test-mod saveload`** (`crates/tools/src/test_mods/saveload.rs`). Uses
   autotest14's reload pattern: a box asks the user to load the save the
   game makes when it closes; a gap of over 15 s means the save was loaded.
   Uses checkpoints. Loaded NPCs near the player, each in one state when
   the save is made:
   - seated on a Sit package
   - asleep in a bed
   - a sandboxer mid-idle
   - sneaking (`StartSneaking`)
   - fighting a held target
   - mid UseMagic practice
   - one walking a Travel package halfway

   A looping `Sound.Play` is started by the script, its id stored. After
   the load, each NPC is read at once, at 2 s and at 10 s: `GetSitState`,
   `GetSleepState`, `IsSneaking`, `IsWeaponDrawn`, `GetCombatState`,
   position and heading against the save, and the graph variables a vanilla
   graph exposes (`GetAnimationVariable*`). Then `StopInstance` is called on
   the stored loop id. Every native used is checked against
   `vrm-tool pex-natives`; the ones above are all in vanilla scripts. The
   page asks the user to note whether the loop was audible after the load,
   and whether it stopped. Run instructions add an optional manual
   checklist: try to save in combat, in the air, in dialogue, during a
   scene and while sitting, and note which give "You cannot save right
   now". Build only; the user runs it. Its results decide a follow-up plan
   for actors' graphs, casts, swings and script loops after a load.
10. **The roadmap.** Move what's done to ROADMAP Done; rewrite item 9's list
    with what the probe leaves.

## Decided: saves that need an inactive plugin

The user chose (2026-11-15) to treat a plugin that is in Data but not
loaded as missing: the save gets the missing-plugins prompt and, if
continued, loads without it. SE offers to activate the plugin instead.
That would mean rebuilding the load order, VFS and engine at runtime,
which only `app.rs`'s startup does. Recorded in
`deviations/main-menu.md`.
