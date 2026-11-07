# Background texture loading

Part of [traversal performance](traversal-performance.md). Texture loading for
new models blocks the main thread. `load_textures` reads and parses in parallel
(rayon) but waits for all of them, then uploads each on the main thread.
Measured on the traversal route:

- A first-time exterior cell (frame 709): 220 of 272 ms was `assets.model_textures`
  (145 ms read/parse, 81 ms upload).
- A world encounter spawning actors (frame 442): 42.5 of 74 ms was 29 textures
  (26.7 ms read/parse, 15.8 ms upload).

## Decided behavior (user, 2026-11-07)

A mesh whose textures are still loading is not drawn. When they are all in,
it fades in quickly with the existing screen-door dither instead of popping.
Screenshot runs, tests and loading screens wait for every texture first, so
what they capture is unchanged.

## Design (implemented 2026-11-07)

1. **Background jobs.** The model loader stops blocking for textures. Each new
   path is marked pending in `TextureCache` and handed to a job on a dedicated
   pool (a quarter of the cores, at least two) that reads and parses it. `Vfs`
   is already shared as `Arc`. Results come back over a channel. Uploading from
   the workers too, or running them on the global rayon pool, kept the main
   thread waiting (on the device, or on model parsing queued behind the jobs).
2. **Per-frame uploads.** At the start of each render, the main thread uploads
   parsed textures for at most 4 ms. Missing files resolve to `None`, as now.
   `TextureCache::get` waits for a pending path and uploads it at once, so the
   synchronous loaders (terrain, LOD, decals, weather, spell effects) never
   keep a placeholder for a texture a model requested first.
3. **Materials wait.** A model material (`shared_material`) made while textures
   it names are pending binds fallbacks and keeps its `MaterialDesc`. When they
   have all resolved, a complete material is built once and kept inside it;
   drawing, controllers and membrane updates go through `GpuMaterial::current`.
   Texture presence affects only the bind group and uniform flags, never the
   pipeline key.
4. **Hide, then fade.** A waiting material's uniform has `decal.w` 1 (unused
   before), and `fs_main` dithers that share of its fragments away with the same
   screen-door test as distance fading, so every draw path hides it. Once ready
   it falls to 0 over `TEXTURE_FADE_SECONDS` (0.25 s), its uniform written every
   frame meanwhile. Shadow casters skip materials more than half hidden.
5. **Waiting for everything.** `Renderer::finish_textures()` blocks until nothing
   is pending and shows waiting materials unfaded. A loading screen calls it after
   its load. Background loading is only switched on in the windowed game, after
   the start is loaded (`VRM_SYNC_TEXTURES=1` turns it off; offscreen runs opt
   in with `VRM_BACKGROUND_TEXTURES=1`), so screenshots and tests are unchanged.

Other texture users (terrain, LOD, decals, weather, spell effects) keep loading
synchronously for now; only model materials are deferred.

## Actors' models (2026-11-07)

With textures in the background, a spawn's main cost was parsing and uploading
its models (5–16 ms a spawning frame). In the windowed game `spawn_actors` now
requests missing models from the same asset pool instead of loading them.
An actor whose models aren't all in spawns at once (AI, physics, behavior
graph) but is drawn with nothing, so it's never half dressed. Each frame,
`dress_waiting_actors` installs parsed models and then dresses ready actors
with `redress`, each for up to 3 ms (at least one of each). Their materials
then fade in as above. A loading screen finishes every model and dresses
every actor before it lifts. Decided with the user (2026-11-07): an actor may
be invisible for the few frames its models take.

An ignored test enters an interior with background loading on, checks that the
waiting actors draw nothing, then that, once dressed, every actor has the same
meshes as in a synchronous load.

Same traversal route: the first-time cells frame fell from ~103 to 75 ms
(`cell.actors` ~28 → 6 ms), with dressing at most ~5 ms a frame over the next
few. World encounter starts' script stage fell from 35 to 23 ms and from 55 to
38 ms in comparable runs (which encounter fires is random).

## Results

Same traversal route as [traversal performance](traversal-performance.md): the
first-time cells hitch (frame 709) fell from 243 ms to 105 ms of frame work
(`sim.streaming` 233 → 76 ms). An offscreen burst of a spawned
`ImpPillar01ice` showed it hidden, dithered in over ~15 frames, then solid.

## Open questions

- Fade duration: 0.25 s, not yet judged by eye in the windowed game.
- Whether placed objects' model parsing should move off the main thread too
  (`objects.models` ~17 ms in that frame).
- Whether the 4 ms upload budget needs tuning on other hardware.
