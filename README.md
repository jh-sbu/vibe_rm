# VibeRM

A from-scratch, open source game engine written in Rust that runs games built
for the Skyrim Special Edition era of Bethesda's Creation Engine. It reads the original
game data formats directly: BSA archives, ESM/ESP/ESL plugins, NIF models, DDS
textures, and so on.

Like [OpenMW](https://openmw.org) for Morrowind, **this repository contains no
game assets**. To play, you must own and install Skyrim Special Edition.
The engine reads your installed `Data` directory at runtime. Never commit game
files to this repository (`.gitignore` blocks the common asset types).

## Status

Early but already visual:

| Area | State |
| --- | --- |
| BSA archives (v103/104/105, zlib + LZ4) | done |
| Plugins: load order, ESM/ESP/ESL FormID resolution, overrides, localized strings | done |
| NIF (SSE BS v100 / LE v83): nodes, BSTriShape family, NiTriShape/Strips, shader props | done for rendering; all base-game meshes parse with exact block sizes |
| DDS: BC1–BC7, uncompressed, cubemaps | done |
| Interior cells: references, lighting templates, point lights | done |
| Exterior cells: objects, landscape heightmap with multi-layer splatting | done |
| Weather / climate: sky gradient, clouds, sun, fog, directional ambient, time of day, transitions, rain and snow, wind, lightning, sky statics, auroras | done |
| Effects: particle systems (fire, smoke, embers), billboards, addon nodes (candle flames, torches), animated shader properties (scrolling, pulsing), environment maps, grass, projected decals, impacts (blood sprays, sparks and their effect models' meshes and keyframes, and blood on the ground and on bodies from hits), spell art/graphs/lights, EFSH membranes and surface particles | in progress |
| Water, collision/physics, player controller, load doors and animated doors, cell streaming | done |
| Sun shadows (cascaded shadow maps) | done |
| Actors: NPC assembly, GPU skinning, Havok animation playback | done (idle/walk only) |
| AI: navmeshes, pathfinding, packages (sandbox/travel/sit/sleep), daily schedules across cells, furniture and crafting stations picked through the IDLE tree, idle markers, seated and standing eating / drinking, sitting variants, anim objects in hand; humanoids and creatures animated by running their behaviour graphs (walking, running, sneaking and turning at their movement types' speeds; `PlayIdle`, `SendAnimationEvent`) | in progress |
| Papyrus VM, conditions, quests, dialogue, console, audio, music | in progress |
| The game's own menus (Interface `.swf` through the vendored Ruffle): the HUD, message boxes, the inventory, containers, bartering, gifts, magic, favorites, the tween menu, the map, sleeping and waiting, training, the journal | in progress |
| Inventories (NPC items, outfits, containers), sheathed weapons and shields | in progress |
| Combat (melee between NPCs, creatures and the player), ragdoll deaths; skills, leveling and perk trees | in progress |
| Magic: effects on actors, abilities, potions, enchantments, effect scripts; the player casting spells and powers by hand (magicka, projectiles, areas); shouts (words of power, dragon souls, the voice's rest) | in progress |
| Saves | native `.vrs` saves implemented; original `.ess` import remains planned |

## Building and running

```sh
cargo build --release
# Interior cell by editor id (or 8-digit hex FormID)
./target/release/vibe_rm --cell WhiterunBanneredMare
# Exterior cell by editor id, at sunset
./target/release/vibe_rm --cell Riverwood --hour 19
# Exterior by worldspace + grid
./target/release/vibe_rm --world Tamriel --grid 4,-12
# Render one frame offscreen to a PNG (useful for testing; needs no display)
./target/release/vibe_rm --cell Riverwood --screenshot out.png
# Let the world run 20 s first, saving a frame every 0.5 s with the camera on an actor
./target/release/vibe_rm --cell WhiterunBanneredMare --hour 20 --wait 1200 --burst 30 \
    --watch 0001A675 --screenshot shots/mare.png
```

Testing aids: `--console "<command>"` runs console commands after loading (e.g.
`use <actor> <furniture>`, `travel <actor> <ref> [run|jog|fastwalk] [sneak]`, `sae <actor> <event>`, `pi <actor> <idle>`, `gstate <actor>` (`00000014` for the player), `cgf Actor.GetCombatState @<actor>` to call a Papyrus native, `cqf <quest> <function>` to call a function of a quest's scripts, `loose` for the loose objects near the player, `pwalk <frames> [forward|back|left|right] [run|sprint]` to walk the player (`tcl` first under `--wait`), `pjump` to jump, `useitem <item>` to use one of the player's items as the inventory would, `[ref.]equipitem <item> [left|right]` to equip one in a hand, `set gamedayspassed to <days>` to move the clock on, `pdraw [off]` and `ppress` / `prelease [left|both]` (the attack buttons) for the player's weapons, `pcast <right|left|power> [off]` for the spell in a hand (`[ref.]equipspell <spell> <left|right|voice>` readies one), `tpcam <degrees>` to turn the third-person view about the player (180 sees them from the front), `impact <IPCT>` for an impact on what the camera faces; `"@<frame> <command>"`
runs it at that frame of `--wait`), `--menu-key <key>` presses a key in the menus after loading (`click:x,y` clicks there, `equip:right` / `equip:left` are the attack buttons, `pause` lets a third of a second pass), `--watch-angle`
orbits the `--watch` camera, `--burst-from <frame>` starts `--burst` at that frame, `--player-at-camera` puts the player there (NPCs look at it),
the `tcam x y z yaw pitch` console command holds the `--wait` camera elsewhere, `VRM_SEED=<n>` fixes the engine's random seed, `VRM_AUDIO=1` gives offscreen runs audio
(sounds are tracked and logged even without an output device), and
`VRM_AI_NO_SNAP=1` makes actors walk into furniture on cell load instead of starting
out in it.

The HUD and message boxes are the game's own (`Interface/hudmenu.swf`,
`messagebox.swf`; a box takes the mouse and the keys: W/A/S/D or the arrows,
E or Enter); `--egui-hud` draws egui's instead (also used when the Interface
files can't be read).

`VRM_NO_SPELL_FX=1` disables casting/hit art, spell projectile animation and
lights, effect shaders and enchantment visuals. Their fidelity choices and
original-game comparisons are recorded in `known_ambiguities/spell-visuals.md`
and `known_ambiguities/effect-shaders.md`. The `spell-visuals` test-mod generator
provides the manual SE probe (`plans/spell-visuals.md`), and
`scripts/spell-visuals-captures.sh` renders vibe_rm's captures of the same scenes;
generated plugins, scripts and captures belong outside this repository.

The data directory is found from `--data`, `$SKYRIM_DATA`, or the default Steam
locations on Linux.

Mods: the base masters and Creation Club content (`Skyrim.ccc`) always load.
`--plugins <plugins.txt>` adds that file's enabled (`*`) entries, and
`--plugin MyMod.esp` (repeatable) adds a plugin from the data directory after those.
A plugin's own `MyMod.bsa` / `MyMod - Textures.bsa` and loose files in `Data` are
picked up too.

Controls: click to capture the mouse, WASD to move, Space/Ctrl for up/down,
Shift to go faster, T to fast-forward time, E to activate (talk, open, take, search),
Tab for the menus, M for the map (WASD or drag to pan, wheel or Q/E to zoom, Page Up/Down to tilt), R to draw or sheathe your weapon, F to switch between first and third person (the wheel
zooms the third-person camera), Esc to release the mouse or quit.
The developer console has a subset of Skyrim's commands (`help` lists them; `tai`
toggles actor AI).

## Layout

- `crates/bsa`: BSA archive reader
- `crates/esp`: plugin reader, load order and record index
- `crates/nif`: NIF model reader
- `crates/vfs`: virtual file system (loose files over archives)
- `crates/ess`: read-only original save game reader, as far as test mods need;
  native `.vrs` saving and loading lives in `src/save`
- `crates/ui-swf`: the game's Interface menus (Scaleform `.swf`) through Ruffle
- `crates/tools`: `vrm-tool` CLI for inspecting and verifying data
  (`spell-fx <Data> [effect-or-spell]`, `bsa-list`, `bsa-verify`, `esp-info`, `esp-dump`, `nif-verify`, `nif-dump`, `nif-materials`, `fsts`,
  `navm-verify`, `hkb-tree`, `hkb-run` (a project directory or a creature's project file), `hkb-vars`, `pack-speeds`, `force-greets`, `alias-packages`, `alias-fills`, `ref-types`, `scenes`, `triggers`, `story`, `faction-owners`, `crime-factions`, `save-info`, ...), and `test-mod` / `test-results` (below)
- `src/`: the engine (renderer, world, app)
- `vendor/ruffle`: a subset of [Ruffle](https://ruffle.rs) with Scaleform
  changes (`VENDOR.md`)
- `spike/swf`: the spike that worked out running the menus (a harness and its
  notes)

## Local UESP reference

The manually downloaded UESP wiki dump can be indexed for offline research:

```sh
python3 scripts/uesp.py import refs/uespwiki-2026-10-11-current.xml
python3 scripts/uesp.py search "armor rating"
python3 scripts/uesp.py read "Skyrim:Armor" --sections
```

The SQLite index retains every page, with searches scoped to Skyrim-related
namespaces by default. Queries return bounded snippets or numbered page sections
with source revision links. See [the reference tool documentation](scripts/README.md)
for filters, reading sections, and refreshing the index. The dump and database
remain in ignored `refs/`.

## Verifying format support

```sh
cargo run --release -p vrm-tools -- nif-verify "<Data>/Skyrim - Meshes0.bsa" "<Data>/Skyrim - Meshes1.bsa"
```

This parses every mesh and reports any block whose parse didn't consume exactly its declared size.

## Test mods

What public sources and the game's data leave open (`known_ambiguities/`)
can sometimes be settled by a small plugin run in Skyrim SE itself:

```sh
cargo run --release -p vrm-tools -- test-mod venv "<Data>" VenvTest.esp
```

Each test mod (`crates/tools/src/test_mods/`, written with `esp::writer`)
documents its setup, the outcome each reading predicts and, once tested, the
result. Its vanilla forms are looked up by editor id in your Skyrim.esm; the
plugins aren't committed.

A test mod whose script runs its tests on its own (`test-mod autotest`,
`test-mod autotest2`; `plans/autotest-survey.md` sorts the open ambiguities
by what such a test would take) keeps each report in a script variable and
has the game save once done;
`test-results` reads the reports back from the newest save holding any:

```sh
cargo run --release -p vrm-tools -- test-results            # the game's Saves directory
cargo run --release -p vrm-tools -- test-results <save.ess | dir> [--all]
```

The Saves directory is `$SKYRIM_SAVES` or Proton's under the default Steam
locations.

## License

VibeRM is licensed under the [MIT License](LICENSE), except as noted below.

`src/condition/functions.rs` contains a condition function table derived from
[xEdit](https://github.com/TES5Edit/TES5Edit)'s record definitions and is
licensed under the [Mozilla Public License 2.0](https://mozilla.org/MPL/2.0/).

Audio preferences are stored in `$XDG_CONFIG_HOME/vibe_rm/audio.ini` (or
`~/.config/vibe_rm/audio.ini`), using `[AudioMenu]` master and decimal
`uIDn`/`fValn` pairs. `audio` lists settings; `audio music 0.4`,
`audio voice 0.8`, `audio effects 1`, `audio footsteps 0.6` and
`audio master 0.7` save changes. Category editor IDs or form IDs also work.
`VRM_AUDIO_SETTINGS` overrides the path; `VRM_AUDIO_MASTER`,
`VRM_AUDIO_MUSIC`, `VRM_AUDIO_VOICE`, `VRM_AUDIO_EFFECTS`,
`VRM_AUDIO_FOOTSTEPS`, or `VRM_AUDIO_<8-digit-form-ID>` override startup
values for offscreen runs. Overrides remain session-only when preferences are
saved; an explicit console/API change saves the underlying preference even if
an environment override still determines the effective value. SkyrimPrefs.ini
is never imported automatically; its AudioMenu section can be copied to this
engine's file explicitly.
