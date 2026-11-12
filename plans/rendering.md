# Rendering: shadows, HDR, water, wind and the effect leftovers

**Status: drafted** (2026-11-11). Not started. Covers the rendering items in
ROADMAP Next item 8 that can be built from public sources and game assets:

1. Shadows from point lights and spotlights, which covers interior shadows.
2. Static shadow caching.
3. HDR: eye adaptation, bloom, tone mapping and the image spaces' HDR values.
4. Depth of field.
5. Underwater rendering and water-surface ripples.
6. Wind moving trees (grass and clouds already move), plus grass shadows and
   the grass's vertex lighting flag.
7. The listed particle and controller gaps.

Rules as for all work here (AGENTS.md): public sources and game assets only,
no engine binaries. Choices made without a source go in `known_ambiguities/`.
Open questions go here or in ROADMAP. Never decide a deviation. `cargo fmt`
before each commit. **Each lettered sub-step is one commit**, in this order,
unless a step says otherwise. A follow-up fix that spans steps gets its own
commit, named for what it fixes.

Left out, and why:

- **Volumetric lighting, screen-space reflections, SAO, TAA, lens flares,
  sun glare/damage, procedural lightning.** These are in ROADMAP item 8 but
  weren't among the items picked for this plan. The Ultra INI turns several of
  them on (`bVolumetricLightingEnable`, `bScreenSpaceReflectionEnabled`,
  `bSAOEnable`, `bUseTAA`, `bLensFlare`). Each needs its own research and could
  follow this plan, since step 3's HDR target is what they would draw into.
- **Grass bending round actors.** No public source or vanilla asset says
  vanilla SE does this; it is a well-known feature of mods. See "Decisions
  for the user".
- **Distant LOD fidelity, eye environment-map centres, effect shaders'
  environment maps, auroras, ward collision, EFSH addon models, unscripted
  trigger volumes, skinned effect art.** These stay in ROADMAP item 8.

## Decisions for the user (before or during step 0)

1. **`Skyrim - Shaders.bsa`.** The game ships its compiled HLSL shaders
   (DXBC bytecode) in an asset archive. Disassembling them would answer most
   of this plan's open maths: the tone-map curve, bloom, eye adaptation, light
   attenuation, the water shader and depth of field. AGENTS.md lets us examine
   binary game assets (it names `.swf` bytecode and `.pex` scripts). It also
   forbids examining engine code. Compiled shaders are arguably both. This plan
   does **not** read them unless the user rules that they count as game assets.
   Every step below has a fallback that doesn't need them: public descriptions,
   record data and SE screenshots, with the choices recorded in
   `known_ambiguities/`.
2. **SE reference screenshots** (step 0c). HDR, bloom, depth of field and
   underwater fog have no public formula. The plan proposes a `test-mod
   renderref` that puts the player at fixed spots, times and weathers, for
   screenshots the user takes in SE. Is the user willing to take roughly 20–30
   screenshots once? Without them, step 3's values are tuned by eye and
   recorded as choices.
3. **Grass bending round actors**: drop it, or keep it as an open question?
   Without evidence it can't be implemented as vanilla behaviour, and it can't
   become a deviation unless the user says so.
4. **Quality defaults.** The renderer's sun shadow maps are 1024² with a
   12000-unit reach. The shipped `Ultra.ini` has `iShadowMapResolution=4096`
   and `fShadowDistance=10000`, and `High.ini` lower. Should the defaults
   follow a shipped preset (which one?), or stay as they are with settings to
   raise them?

## Where the renderer stands (2026-11-11)

- **Forward, single pass, lit in gamma space** (`src/render/shaders/object.wgsl`
  says so: "like the original"). The colour target is the swapchain format,
  or the post target when an image space is active (`Renderer::render`,
  `pre_post`). Values are clamped to display range; there is no HDR target.
- **Lights.** `GpuLight` holds only position, radius and colour, in a storage
  buffer (`MAX_LIGHTS` 4096). Each instance gets up to 8 lights
  (`Scene::assign_lights`, `pick_lights`: brightest and closest). The shader
  attenuates by `1 - (d/r)²`. Placed lights (`world/cell.rs`) drop
  off-by-default and negative lights. Their flags (`LightData::flags`) aren't
  used for anything else: no spotlight cone, falloff exponent, flicker or
  pulse, and no shadows.
- **Sun shadows** (`src/render/shadow.rs`): 4 cascades (`SPLITS` 600, 1800,
  5000, 12000), 1024² each, plus a fifth layer for the precipitation occlusion
  map. Far cascades are refreshed every 2nd and 4th frame. Casters are
  gathered per instance every frame (`Renderer::shadow_passes`). The sun only
  casts outdoors (`env.sky`). Interiors have no shadows at all.
- **Post** (`src/render/post.rs`, `post.wgsl`): the IMGS cinematic values,
  tint, fade, blur, double vision and radial/motion blur. The frame's average
  is found with a mip chain. The HDR values (`Imgs::hdr`, HNAM) are parsed but
  unused (`src/imagespace.rs`; `known_ambiguities/image-space.md`, "HDR
  values").
- **Water** (`src/render/water.rs`, `water.wgsl`): flat planes per cell, with
  one noise texture scrolled at made-up rates, Fresnel between the
  shallow/deep and reflection colours, and sun specular. It uses only DNAM's
  sun power, reflectivity, fresnel and colours. Nothing changes underwater.
  Underwater audio exists (`engine.rs`, `underwater_reverb`), and so does
  `IsSwimming`.
- **Wind**: grass sways (`vs_grass`, the weather's wind) and clouds drift by
  QNAM/RNAM (`known_ambiguities/weather.md`). Trees are skinned to their
  branch bones (`render/model.rs`, `is_tree`), and the shader knows their
  vertex alpha holds sway weights (`SF2_TREE_ANIM`), but they don't move.

## Public evidence found while drafting

Write these up as `research/rendering/sources.md` in step 0a.

- **LIGH** (xEdit `wbDefinitionsTES5.pas`; UESP *Skyrim Mod:Mod File
  Format/LIGH*, revision 3507757):
  - DATA flags: 0 Dynamic, 1 Can be Carried, 2 Negative, 3 Flicker, 5 Off By
    Default, 6 Flicker Slow, 7 Pulse, 8 Pulse Slow, 9 Spot Light, 10 Shadow
    Spotlight, 11 Shadow Hemisphere, 12 Shadow Omnidirectional, 13
    Portal-strict.
  - DATA floats: falloff exponent, FOV (0.001–160, default 90), near clip, and
    flicker period, intensity amplitude and movement amplitude.
  - FNAM is the fade value. LNAM is the lens flare.
- **IMGS** (xEdit; UESP revision 3507714):
  - HNAM: eye adapt speed, bloom blur radius, bloom threshold, bloom scale,
    receive bloom threshold, white, sunlight scale, sky scale, eye adapt
    strength. UESP's talk text guesses that fields 5 and 6 are "target
    luminance" values. That is a guess, and xEdit names them differently.
  - DNAM: depth of field strength, distance and range, plus a u16 packing a
    blur radius 0–7 and a "No Sky" flag.
- **IMAD** (UESP revision 3507715): curves for DOF strength, distance and
  range (`WNAM` / `XNAM` / `YNAM`), full-screen motion blur (`NAM4`), and HDR
  multiply/add pairs: eye adapt speed (`\0IAD` / `@IAD`), bloom blur radius,
  bloom threshold, bloom scale, and target luminance minimum and maximum
  (`\x04IAD`, `\x05IAD`...). The modifiers therefore drive a target-luminance
  range, which supports reading HNAM 5 and 6 as adaptation limits.
- **WATR DNAM** (xEdit; UESP revision 3507781):
  - Above-water fog (near, far, amount) and underwater fog (amount, near,
    far).
  - Three noise layers, each with wind direction, wind speed, UV scale and
    amplitude scale, plus noise falloff.
  - Specular (power, radius, brightness, sun sparkle), refraction and
    reflection magnitudes, and depth properties (reflections, refraction,
    normals, specular).
  - A displacement simulator: starting size, force, velocity, falloff,
    dampener. The "Rain Simulator" is marked unused.
  - Flowmap scale (SE), and FNAM flags "Enable Flowmap" / "Blend Normals".
  - INAM is the water's image space. NAM2–4 are the three noise textures.
- **TREE CNAM** (xEdit): trunk and branch flexibility; trunk, front, back and
  side amplitude; front, back and side frequency; leaf flexibility, amplitude
  and frequency.
- **CommonLibSSE-NG** (MIT; names only, no maths):
  - `BSShaderRenderTargets` lists `kWATER_DISPLACEMENT` (+ `_SWAP`),
    `kUNDERWATER_MASK`, `kHDR_BLOOM`, `kHDR_BLURSWAP`, `kSHADOW_MASK`,
    `kSHADOWMAPS`, `kPRECIPITATION_OCCLUSION_MAP` and `kRAW_WATER`.
  - The image-space shaders include `HDRTonemapBlendCinematic(Fade)`,
    `DepthOfField(Fogged / MaskedFogged)`, `ISDepthOfFieldBlurH/V`,
    `ISUnderwaterMask`, `WaterDisplacementRainRipple` and
    `WaterDisplacementWadingRipple`.
  - `ImageSpaceManager` has `playerUnderwater`. `BSShadowLight` exists, and a
    scene keeps `shadowCasterLights`.
  - What this tells us: SE simulates ripples into a displacement texture (rain
    and wading are separate sources), masks the underwater part of the screen,
    tone maps in the same pass as the cinematic values, and runs depth of
    field as separable blurs.
- **Shipped INIs** (game files):
  - `Ultra.ini`: `iShadowMapResolution=4096`, `iNumFocusShadow=4`,
    `fShadowDistance=10000`, `bUse64bitsHDRRenderTarget=1`,
    `bDoDepthOfField=1`.
  - `Skyrim_Default.ini`: `fShadowLODMaxStartFade=1000`,
    `fLightLODStartFade=3500`, `fDynamicDOFFarBlur=0.4`,
    `fDDOFFocusCenterweightExt=2`, `fDOFMaxDepthParticipation=10000`,
    `iRadialBlurLevel=2`.

## Steps

### 0. Research, tools and a reference batch

- **0a.** Write `research/rendering/sources.md` from the evidence above, with
  UESP revision links. Separate source claims from inference.
- **0b.** Add `vrm-tool render-census <data>` to survey what the steps need:
  - LIGH flag counts: shadow spot, hemisphere, omni, spot, flicker, pulse,
    negative, portal-strict.
  - How many shadowed lights are placed per interior cell, and per exterior
    cell, as a distribution.
  - Which IMGS have nonzero HNAM / DNAM, and their ranges (extend `vrm-tool
    image-spaces`).
  - WATR DNAM fields per water (underwater fog, displacement, noise layers),
    and which waters the vanilla cells use.
  - TREE CNAM ranges, and BSTreeNode bone-list sizes over the vanilla tree
    meshes.

  Record the census in the research note. It decides the budgets in steps 1
  and 2.
- **0c.** Only if the user agrees (decision 2): `test-mod renderref`, a
  checkpointed `autotest21::Batch` (`checkpoints: true`) that stops at fixed
  spots and times so the user can take screenshots:
  - Whiterun at noon and at night, clear and stormy.
  - A torch-lit Nordic ruin hall with shadowed lights.
  - Bleak Falls Barrow's entrance (the eye adapting between interior and
    exterior).
  - A lake surface from above and below the waterline.
  - Riverwood's river, for ripples.
  - A forest in wind, for tree sway.

  Steps are driven by `set <prefix>Step to <n>` (vanilla SE has no `cqf`).
  Each stop sets the weather (`Weather.ForceActive`), the time (`GameHour`)
  and the position (`MoveTo` a marker created at fixed coordinates). Our
  engine runs the same plugin, so captures match stop for stop. Give the run
  instructions, including checkpoints and `set <prefix>SkipTest`, with the
  batch.

### 1. Lights: the full LIGH record

Point-light shadows build on lights that are otherwise correct.

- **1a. Light parameters to the GPU.** Widen `GpuLight` with
  spot direction and cosines, falloff exponent and flags, and the shadow slot
  (none yet). Placed lights take the reference's rotation for the spot axis.
  Keep `pick_lights` as it is. No visual change; there's a test that the
  buffer layout matches the WGSL struct.
- **1b. Falloff exponent and spotlights.** Attenuation becomes
  `pow(saturate(1 - d/r), falloff)` or another form chosen from the evidence.
  The current `1 - (d/r)²` is ours, and the exact curve isn't public. Record
  the choice in a new `known_ambiguities/lights.md`. Spotlights (flag 9 or
  10) are masked by FOV with a smooth edge. Test: a unit test of the CPU
  mirror of the curve, and offscreen captures of a census-picked spotlit cell
  before and after.
- **1c. Flicker and pulse.** Intensity, and for flicker the position (movement
  amplitude), vary over the light's period. Use a deterministic noise per
  reference so captures repeat. "Slow" variants take a longer period (the
  scale is a choice: record it). Random Anim Start (header flag 16) sets the
  phase. Moving lights already re-pick through `assign_moving_lights`.
  Flickering lights only change intensity and a small offset, so they reuse
  the per-frame light upload without re-picking.
- **1d. Negative lights.** These are dropped today. Subtract them in the same
  loop, clamping the sum at zero. The census (0b) shows how many vanilla
  references use them. If there are none, skip the step and say so in the
  research note.
- **1e. Portal-strict and the fade value.** FNAM is already multiplied in.
  Portal-strict lights stop at room/portal bounds, which this engine doesn't
  model (they're unscripted trigger volumes, outside this plan). Record that
  in `known_ambiguities/lights.md` and change nothing.

### 2. Shadows from lights (interiors and exteriors)

- **2a. Shadow light selection.** Each frame, choose up to N shadowed lights
  (flags 10/11/12) by screen importance: distance, radius and whether they're
  in view. N comes from the census, starting at 4 as a choice, since SE's
  limit isn't public. `iNumFocusShadow` is the focus shadows' count, not this
  one. Lights that lose their slot fade their shadow over ~0.25 s rather than
  popping. Unit tests cover the selection and the hysteresis.
- **2b. Omnidirectional shadows.** A cube depth array (wgpu
  `TextureViewDimension::CubeArray`, with a fallback to six 2D layers where
  the adapter lacks it). Six faces per light, rendering casters within the
  light's radius only. Linear distance is stored for the comparison. Sample
  with PCF and a normal offset scaled by the texel's world size at that
  distance, as the sun does.
- **2c. Spot and hemisphere shadows.** Spot: one perspective map using the
  light's FOV and near clip. Hemisphere: one 180° paraboloid map, or five cube
  faces skipping the back one. Which projection SE uses isn't public: pick the
  five faces (simpler, reuses 2b) and record the choice.
- **2d. Interior lighting checks.** Interiors' directional light (XCLL) has no
  sun cascades today. Whether it casts in SE is unknown. Leave it unshadowed
  and add it to `known_ambiguities/lights.md` as an open question for 0c's
  dungeon stop. Turn on `tsh`/`VRM_NO_SHADOWS` for light shadows too, and add
  `VRM_NO_LIGHT_SHADOWS` to compare.
- **2e. Actors and first person.** Light shadows include actors and their
  equipment, as the sun cascades do. The first-person body casts into light
  maps as well: a held torch shouldn't shadow its own arm, so a light carried
  by the camera's actor skips that actor's meshes. That's a choice; record it.

Verification: offscreen captures of the census's busiest shadowed interior,
plus frame times from `Renderer::bench` with lights' shadows on and off. Pass
condition: the busiest vanilla room stays within 2 ms extra in the bench at
1080p. If it doesn't, step 3 comes before tuning.

### 3. Static shadow caching

- **3a. Per-cell caster batches.** When a cell is uploaded, build its static
  casters (instances without skinning, animation, or material/visibility
  controllers) into one instance buffer per (model part, cell). Store the
  cell's bounds. `shadow_passes` culls whole batches against each cascade
  (`Cascade::sees`) and only walks dynamic casters per instance. The
  `MIN_CASTER` small-caster cut moves into batch building, as one batch per
  cascade class. Test: the same depth texels as the per-instance path for a
  fixture cell (readback comparison, which `render/readback.rs` exists for).
- **3b. Cached light shadow maps.** A shadowed light whose map holds only
  static casters keeps it until something dynamic enters its radius, it
  moves, or its cell loads or unloads. Draw static casters into a cached map,
  then copy it into the working map and draw dynamic casters on top each frame
  they're present. Track the dirty state per light, and test it.
- **3c. Far cascades from batches.** Cascades 3 and 4 already refresh every
  2nd and 4th frame. With 3a's batches they also skip cells entirely outside
  the slice. Measure the effect on the traversal route
  (`plans/traversal-performance.md`'s capture recipe) and record the numbers
  in that plan.

### 4. HDR

The largest change. It goes in stages so the frame looks the same until the
step meant to change it.

- **4a. An HDR scene target.** The scene renders into an `Rgba16Float` target
  every frame (not only when post is active), and the post pass always runs
  last. Lighting stays in gamma space, and nothing changes the values yet. The
  "tone map" is a clamp, so captures must match the current output
  bit-for-bit, or within 1/255 where float precision differs. Add a golden
  capture test over 3–4 fixed scenes (offscreen `render_to_image`; the
  fixture traps in the engine-fixture-tests memory apply), so later steps show
  as deliberate diffs.
- **4b. Luminance and eye adaptation.** Reduce the frame's log-luminance (the
  mip chain already exists; switch it to log-average) and keep an adapted
  luminance that follows it each frame:
  `adapted += (target - adapted) * (1 - exp(-dt * speed))`.
  - Speed is HNAM's eye adapt speed, times or plus the IMAD curves (`\0IAD` /
    `@IAD`).
  - The target is clamped by the target luminance minimum and maximum. HNAM 5
    and 6 are read as those limits until better evidence: record this.
  - Eye adapt strength blends between no adaptation and full.
  - Adaptation resets on cell change and on load, so that a loading screen
    doesn't carry a dark adaptation outside. Whether SE resets is open: check
    against 0c's Bleak Falls stop.
- **4c. Tone mapping and White.** Exposure = key / adapted, then a curve with
  HNAM's White as the white point. Reinhard-extended is the candidate:
  `x (1 + x/W²) / (1 + x)`. The curve isn't public: record it. This is the
  step where the look changes. Retune against 0c's screenshots if they exist.
  Then the cinematic contrast pivot moves from "the frame's average" (our
  workaround, `image-space.md`) to mid grey on the tone-mapped frame, which
  is what the CommonLibSSE shader name `HDRTonemapBlendCinematic` suggests.
  Update `known_ambiguities/image-space.md`'s "How the effects are drawn" to
  match.
- **4d. Sunlight and sky scale.** Read them as multipliers on the sun's
  directional colour and on the sky's (sky dome and clouds) output before tone
  mapping. Sky scale is -0.15..1, and 0.05 at clear noon, so a plain
  multiplier would black out the sky. The candidate reading is
  `1 + sky_scale`, and the census (0b) may suggest more. This is the
  least-evidenced part of the plan: keep it behind `VRM_HDR_SCALES=1` until
  0c's screenshots, or the shader decision, settle it. Record it either way.
- **4e. Bloom.** Bright pass at the bloom threshold (and receive bloom
  threshold per the reading in 4b, recorded). Downsample, blur by the bloom
  blur radius, and add the result at bloom scale before tone mapping. Hook up
  the IMAD bloom curves. Effect shaders' glow maps feed bloom through their
  emissive output; there's no separate bloom layer.
- **4f. HDR and the rest.** The sky, auroras (`weather.md` notes they're far
  too bright: re-check after 4c), precipitation, particles and the HUD/menus
  composite after tone mapping, as today (`swf_ui` draws over). Loading
  screens and menus with 3D (the race menu, the skills sky) render through the
  same chain. Check each against its current golden capture.

### 5. Depth of field

- **5a. IMGS/IMAD depth of field.** Parse DNAM (strength, distance, range and
  the radius/No Sky u16, decoded with the xEdit enum table) and the IMAD
  curves (`WNAM` / `XNAM` / `YNAM`). Compute circle of confusion from scene
  depth (the soft-depth copy exists) as `strength * smoothstep` over
  [distance, distance + range]. Blur in two separable passes at half
  resolution with radius 0–7 steps, then composite before tone mapping. "No
  Sky" leaves sky-depth pixels sharp. The units of distance and range (world
  units) and the step size of a radius are choices: record them.
- **5b. Dynamic depth of field** (the journal's DDOF option; ROADMAP item 6
  waits on this). Focus from a centre-weighted depth sample
  (`fDDOFFocusCenterweightExt`), far blur at `fDynamicDOFFarBlur`, and depths
  beyond `fDOFMaxDepthParticipation` ignored. Only on when the option is set.
  Wire the System page's DDOF switch to it, and remove the ROADMAP note.

### 6. Water: the surface from its record, underwater, ripples

- **6a. The surface from DNAM.** Three noise layers (NAM2–4) with their wind
  direction and speed, UV scale and amplitude. Noise falloff with distance.
  Specular power, radius and brightness; sun sparkle; reflection magnitude.
  Above-water fog near, far and amount, applied on the water instead of the
  cell fog. These replace the made-up scroll rates. Water is still drawn as a
  plane: no reflections pass (SSR is out of scope), with the reflection colour
  as now. Flowmaps (FNAM bit 3) only if 0b finds vanilla waters using them.
- **6b. Being underwater.** The camera is underwater when it's below the
  surface of a water plane whose bounds contain it. Use the same query as
  swimming. Then:
  - Fog switches to the water's underwater fog (amount, near, far) in the
    deep colour.
  - The surface draws from below: flip the Fresnel, with no sky reflection.
  - The water's INAM image space applies as the base (`ImageSpace` gets a
    water override, as the interior's XCIM is). Underwater audio already
    switches.
  - The underwater mask: when the near plane straddles the surface, split the
    screen per pixel by projecting the water plane, as `ISUnderwaterMask`
    suggests SE does.
- **6c. Ripples.** A displacement simulation on a camera-centred grid
  texture: two `Rgba16Float` targets ping-ponged (`kWATER_DISPLACEMENT` /
  `_SWAP`), at a fixed step (60 Hz), using a damped wave equation with DNAM's
  displacement simulator values (starting size, force, velocity, falloff,
  dampener). How the five values map onto the equation isn't public: record
  the mapping.
  - Sources: actors wading or swimming (capsule positions at the water plane,
    as `WaterDisplacementWadingRipple` suggests); rain (random drops at the
    precipitation's density while rain falls over that water,
    `...RainRipple`); projectiles and falling objects crossing the surface.
  - The surface normal takes the displacement's gradient.
  - The grid scrolls with the camera, keeping the simulation in world space.
  - Test: a unit test of the CPU reference step (energy decays at the
    dampener's rate), and an offscreen capture of an NPC walking through
    Riverwood's river.

ROADMAP item 5's "diving controls after underwater rendering" becomes
unblocked by 6b. Note that in ROADMAP rather than doing it here.

### 7. Wind, trees and grass

- **7a. Trees sway.** Keep BSTreeNode's two bone lists (nif.xml's `Bones 1`
  and `Bones`; `crates/nif/src/blocks.rs` reads and discards them today; which
  is trunk and which branch is to be settled from the vanilla meshes in 0b).
  In the skinned path, drive those bones with the weather's wind and the TREE's
  CNAM: trunk flexibility and amplitude, front/back/side amplitude and
  frequency, branch flexibility. Leaves sway per vertex by vertex alpha (the
  `SF2_TREE_ANIM` weights) with leaf flexibility, amplitude and frequency. All
  of this is done on the GPU, with no CPU pose per tree. How the CNAM values
  combine isn't public (SpeedTree's runtime isn't either): record the
  formulas. LOD trees (BTT/LST billboards) don't sway. Test: deterministic
  pose for fixed time and wind (CPU mirror), and an offscreen capture
  sequence.
- **7b. Grass receives sun shadows.** `vs_grass`'s fragment path calls
  `sun_shadow`. Grass is not a caster; whether vanilla grass casts is open:
  record it, and check at 0c's Whiterun stop.
- **7c. Grass's vertex lighting flag** (GRAS DATA). Unset: lighting
  per-fragment with the terrain normal. Set: per-vertex with the blade's own
  normal, which is the closest reading of the field's name. Record it in
  `known_ambiguities/grass.md`.
- **7d. A grass scatter cache.** `bAllowLoadGrass=0` in the shipped INIs, so
  SE builds grass at runtime too. A cache is a performance choice, not
  behaviour. Cache each cell's scatter result (keyed by load order hash and
  cell) in memory across unloads, with an optional disk cache under the user
  cache directory. Measure on the traversal route before and after. Drop the
  step if the scatter is already under 1 ms per cell.
- **Clouds** already drift (QNAM/RNAM). Tying their drift to the weather's
  wind direction has no evidence. Leave it, and remove "clouds" from
  ROADMAP's "wind moving trees, grass and clouds".

### 8. Particle and controller leftovers

Each of these is one commit with its own tests, choices recorded in
`known_ambiguities/particles.md` or `node-controllers.md`. They're
independent: take them in this order, which is roughly by vanilla usage. Have
`vrm-tool` count each feature's use first (extend the census) and skip any
with no vanilla use, saying so in the note.

- **8a.** Transform keys' quadratic and TBC interpolation (rotation,
  translation, scale). `known_ambiguities/quadratic-value-interpolation.md`
  and `tbc-value-interpolation.md` already hold the reading for float values:
  reuse it.
- **8b.** Controller flags: playback backwards, forced update, scaled time.
- **8c.** `NiAlphaProperty` test functions other than the lower bound (less,
  equal, greater-or-equal..., from the flags' bits 10–12).
- **8d.** Particles' decay and turbulence modifiers, drag's exact formula,
  and the rotation axis.
- **8e.** Subtexture loops; bombs (`NiPSysBombModifier`); inherited velocity.
- **8f.** Addon nodes' caps, flags and keyframes.
- **8g.** Strips (`BSStripParticleSystem` / `BSStripPSysData`).
- **8h.** Animated colliders, and skinned/deformed and animated mesh
  emission.
- **8i.** Particle LOD (`BSPSysLODModifier`).

## Ordering and dependencies

```
0a/0b ─┬─ 1 ── 2 ── 3
       ├─ 4a ── 4b ── 4c ── 4d, 4e ── 4f ── 5
       ├─ 6a ── 6b ── 6c        (6b uses 4a's target for the mask)
       ├─ 7                     (independent)
       └─ 8                     (independent)
0c (user run) ── calibrates 1b, 2d, 4b–4e, 5a, 6b, 7a, 7b
```

Run 0c early so its results come back while 1–3 are built. Steps 4c and 4d
are the ones to hold if neither the screenshots nor the shader decision has
arrived.

## Verification throughout

- Offscreen captures (`render_to_image`), with golden images from 4a on. Keep
  them under ignored `tests/assets/` and never commit them. They contain game
  textures.
- `Renderer::bench` frame times before and after steps 2, 3, 4 and 6. Record
  them in this plan's steps as they land.
- GPU-ignored tests run here with `--include-ignored` (engine-fixture-tests).
- When a step finishes: move its ROADMAP item 8 clause to Done with a pointer
  here, and add its open questions to the relevant `known_ambiguities/` file.
