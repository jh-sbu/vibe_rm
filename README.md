# vibe_rm

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
| Weather / climate: sky gradient, clouds, sun, fog, directional ambient, time of day | done |
| Water, collision/physics, player controller, load doors and animated doors, cell streaming | done |
| Actors: NPC assembly, GPU skinning, Havok animation playback | done (idle/walk only) |
| AI: navmeshes, pathfinding, packages (sandbox/travel/sit/sleep), daily schedules across cells, furniture and crafting stations picked through the IDLE tree, idle markers, seated and standing eating / drinking, sitting variants, anim objects in hand; behaviour-graph event lookup (`PlayIdle`, `SendAnimationEvent`) | in progress |
| Papyrus VM, conditions, quests, dialogue, HUD, console, audio, music | in progress |
| Combat, magic, inventory, saves | planned |

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
`use <actor> <furniture>`, `sae <actor> <event>`, `pi <actor> <idle>`; `"@<frame> <command>"`
runs it at that frame of `--wait`), `--watch-angle`
orbits the `--watch` camera, `VRM_SEED=<n>` fixes the engine's random seed, and
`VRM_AI_NO_SNAP=1` makes actors walk into furniture on cell load instead of starting
out in it.

The data directory is found from `--data`, `$SKYRIM_DATA`, or the default Steam
locations on Linux.

Controls: click to capture the mouse, WASD to move, Space/Ctrl for up/down,
Shift to go faster, T to fast-forward time, Esc to release the mouse or quit.
The developer console has a subset of Skyrim's commands (`help` lists them; `tai`
toggles actor AI).

## Layout

- `crates/bsa`: BSA archive reader
- `crates/esp`: plugin reader, load order and record index
- `crates/nif`: NIF model reader
- `crates/vfs`: virtual file system (loose files over archives)
- `crates/tools`: `vrm-tool` CLI for inspecting and verifying data
  (`bsa-list`, `bsa-verify`, `esp-info`, `esp-dump`, `nif-verify`, `nif-dump`,
  `navm-verify`, ...)
- `src/`: the engine (renderer, world, app)

## Verifying format support

```sh
cargo run --release -p vrm-tools -- nif-verify "<Data>/Skyrim - Meshes0.bsa" "<Data>/Skyrim - Meshes1.bsa"
```

This parses every mesh and reports any block whose parse didn't consume exactly its declared size.

## License

vibe_rm is licensed under the [MIT License](LICENSE), except as noted below.

`src/condition/functions.rs` contains a condition function table derived from
[xEdit](https://github.com/TES5Edit/TES5Edit)'s record definitions and is
licensed under the [Mozilla Public License 2.0](https://mozilla.org/MPL/2.0/).
