# Plan: spell visuals

**Status: implemented** (2026-10-26); original-game visual comparisons remain
unverified, with decisions recorded in `known_ambiguities/spell-visuals.md` and
`known_ambiguities/effect-shaders.md`.

The implementation reads all visual records and attachment tags, exposes the
survey command, shares independently clocked effect instances with impacts,
binds manager-owned node/material/particle channels, and runs authored HKX
sequence leaves through the existing graph runtime. Actor art, casting and
projectile lights, visual streams, hit lifetimes, membrane overlays, surface
particles, drawn weapon enchantments, dual visual overrides and scripted
Play/Stop use that state. Active effects reconstruct derived visuals after load.

Implementation choices relative to the proposal:
- Keep all moving magic lights in `SpellFx.lights`, merged alongside existing
  held torch lights, without changing the torch's actor attachment format.
- EFSH retains the entire DATA payload and uses bounded typed accessors, including
  optional older tails. Membranes reuse existing rigid/skinned draw machinery
  with private overlay materials and shared geometry instead of a separate pass.
- Retain CPU vertex/index copies and skin only sampled vertices for EFSH surface
  emission. General NiPSys skinned/deformed mesh emitters remain a Rendering item.
- Sequence-generator blend modes, exact shader equations, held flame/beam and
  travelling cone visuals, lifetime policy and script visual persistence are documented choices, not
  assertions of SE parity. The manual `test-mod spell-visuals` probe generates
  its own PEX and references a vanilla shader without copying assets.

Validation: synthetic records/attachment strings, sequence visibility and
emission switching, standalone impact posing, graph transition weights,
envelopes, birth ramps, sampled skin/morph transforms and native Play/Stop;
installed CastingBasic/BeginLoopEnd/IdleOnLoad/Autoplay graph fixtures; headless engine
fixtures with `--include-ignored`. Offscreen `--screenshot --burst` captures of
Flames hand art/lights and a script-played membrane were read back and inspected.
Captures and asset inspection output stay in `/tmp`, outside the repository.
Final validation: 974 tests passed across the engine, NIF, Havok and tool
packages with `--include-ignored --test-threads=1` (the serial option avoids an
existing AI fixture temp-directory collision). `pex-natives` still reports 929
name-matched Play calls and 1898 Stop calls for each visual class, as expected:
implementing natives changes no game scripts. The survey distinguishes model
paths, readable models, tagged models and individual NamedNode tags, rather than
assuming the proposal's snapshot counts are current load-order totals.

The full SE screenshot comparison set still requires observations in SE; it is
not represented as completed validation here.

This plan covers the "effect visuals" clause of ROADMAP Next item 5 (magic):
spell projectiles' models running their graphs, particles and shader
controllers, casting and hit art, and lights. It also takes in the visual
halves of entries listed elsewhere in Next where they share the machinery:
`EffectShader` / `VisualEffect` `Play` / `Stop` (item 5's natives list),
enchantment visuals, and dual-cast art scale. Magic sounds, explosions
(`EXPL`), wards' collision geometry, procedural lightning beams and skinned
effect meshes are outside this plan. Each is noted where a step leaves room
for it.

## Pending SE validation

These comparisons are unfinished work, not evidence of SE parity. The researched
implementation decisions remain in `known_ambiguities/spell-visuals.md` and
`known_ambiguities/effect-shaders.md`.

- Casting selection/idle: compare Flames, Firebolt, Healing and a mixed-effect
  spell in each hand while idle, charging and releasing; repeat with a staff to
  check hand variables and event forwarding.
- Attachment: compare untagged hit art on a human and creature, and both staff
  hands. Check RFCT transform precedence with facing, camera attachment and
  inherited rotation enabled together.
- Visibility: compare an invisible actor's cloak and hand glow, and a quietly
  applied ability (including No Hit Effect).
- Lifetime: compare overlapping cloaks, dispel during begin, instantaneous hits
  and unload/reload.
- Streams: compare left/right Flames, dual Flames, Frostbite, a shout's cone,
  Ice Storm and a lightning bolt at fixed distances, both into open air and
  at a wall 200 and 500 units away (the spray's length blend). Note where the
  spray starts and ends against where the scorch decal lands, and whether a
  cone visibly travels out to its range. Cast input identifies independent
  left/right emitters and selects the corresponding magic node; missing nodes,
  dual casts and non-hand inputs retain the supplied cast origin.
- Flame look: offscreen captures (2026-10-26) of Flames sprayed at the rock
  `--yaw 140` faces showed the fire all but invisible and a white-hot ball
  where the spray struck. Asset inspection and captures with parts switched
  off found two causes:
  - Dim fire: palette-coloured particles were tinted by the vertex colour
    that had already picked their palette row (Flames' jet 0.067,
    MPSFlamesFire 0.192). Fixed; evidence in `known_ambiguities/particles.md`
    (Palette colour and vertex colour). The hand and addon fire now draw
    orange.
  - The ball: each 0.25 s concentration tick lands a full impact
    (`MAGFlames01ImpactSet` -> `FireballImpact01.nif` -> addon 9,
    MPSFireBoltImpact01, an additive fireball explosion) at the same spot,
    each with its own copy of the addon's particle systems, so four or more
    overlap. With impacts switched off the ball was gone; neither the
    projectile light, the spark-trail addon (5) nor billboards made it.
    Open: whether SE lands an impact on every tick, and how its shared
    master particle systems bound overlapping impacts (UESP's ADDN page has
    the master particle cap always 0 in vanilla files, and addon 9 sets the
    0x0002 "Always Loaded" flag). Watch Flames on a wall in SE.
  - Still open: the spray's own jet (`pFlameSpellBase04`) wasn't seen
    between the hand and the rock even with both fixed. It may be hidden
    end-on behind the player; otherwise suspect the blended lifespan and
    speed controllers (mSprayShort 0.033 s at 1500, mSprayLong 0.267 s at
    2550) or its `BSPSysLODModifier` (begin 0.033, end 0.233). Capture it
    from the side, then compare against SE.
- Enchantments/dual scale: compare a sword drawn/sheathed and full/empty, and a
  dual projectile beside a single cast.
- Membranes: compare timed captures of Oakflesh's actual ShieldSpellFXS, Soul
  Trap, fire burns and an RFCT holes dissolve; check close silhouette/occlusion
  views for extrusion and depth placement.
- Shape selection: compare head, body, clothing, armour, shield and weapons.
- Stacking: apply two noncommuting colored shaders in opposite orders.
- Particles: compare differently sized actors, a changing pose and animated
  frames. Before implementing addon models, inspect an EFSH using DEBR to
  establish spawn placement/timing.
- Breath origins: dragon fire/frost, centurion steam and Fire Breath cast from
  the caster's eye height (non-hand sources keep the cast origin); which node
  their models attach to (a mouth or head node) is open.
- Dual-cast streams: DUAL projectile replacements and scale aren't applied to
  held flame/beam models or cone waves yet.

### Pending automated coverage

The new lifecycle tests use synthetic records and geometry-free instances; they
are not end-to-end gameplay or SE parity checks. GPU-dependent cases are ignored
by default and require an explicit headless-adapter run.

- Actual collider-driven projectile impact remains untested: the projectile test
  forces lifetime expiry to exercise retirement, pose and light cleanup instead.
  Add a collision fixture that verifies impact delivery and visual retirement.
- Actual summon spawning/despawning remains untested: the hit-art test sets
  `ActiveEffect.summoned` metadata and unloads the target, but does not create or
  remove a summoned actor. Add a summon lifecycle integration test.
- The manual SE shader save/load probe below is still pending; its predicted
  outcomes are not observations and automated tests cannot replace it.

### Manual shader save/load probe

Build the tool before generating the plugin; keep outputs under ignored
`tests/assets/` (the generated PEX is written into its `Scripts/` directory):

```sh
cargo build -p vrm-tools
target/debug/vrm-tool test-mod spell-visuals "<Skyrim SE>/Data" tests/assets/VrmSpellVisuals.esp
```

Do not install the plugin or run Skyrim here. The user loads a disposable SE save
with the plugin, notes the Oakflesh sheen in third person, then reloads the new
numbered save requested by the quest without calling Play again. The persisted
`Ran` guard prevents replay. After observation, `cqf VrmSpellVisualsQuest StopVisual`
stops the shader. Screenshots stay outside the repository. The user can place
resulting saves in `tests/results/` for `vrm-tool test-results tests/results`, but
shader visibility still requires human observation; no Papyrus query supplies it.

| Checkpoint | Predicted outcome / interpretation (not an SE result) |
| --- | --- |
| First update, before save | Oakflesh sheen appears; Play runs once with indefinite duration. |
| Reload numbered save, shader persists in SE | Sheen remains without replay; contradicts our transient-script-visual choice. |
| Reload numbered save, shader is transient in SE | Sheen is absent without replay; supports our current choice. |
| After `StopVisual` | Any remaining sheen fades/stops; no new Play is issued. |

A missing initial sheen invalidates the persistence comparison. Record the actual
observation before changing the saving decision.

## Baseline before implementation

- **Particles, controllers, addons, billboards** are done (ROADMAP Done:
  particle systems, addon nodes, billboard nodes, shader property
  controllers, soft effects).
- **Impact effects** (`src/impacts.rs`): `start_effect` loads
  `<model>#effect`, makes one `Instance` per animated part plus the root,
  each with fresh `ParticleState`s on the effect's clock, and `Effect::pose`
  drives parts from their own (standalone) controllers. World-space particles
  are carried. `update_impact_effects` / `draw_impact_effects` step and draw
  them. Only **standalone** controllers run: "Manager-controlled ones
  (sequences') don't run" (`known_ambiguities/impacts.md`).
- **Named NIF sequences** play on placed objects (`AnimatedObject::play`,
  `src/world/animated.rs`; `PlayGamebryoAnimation`, doors), driving node
  transforms, visibility and material channels
  (`known_ambiguities/sequence-material-controllers.md`). Particle
  controllers owned by a sequence (timing flag 0x20) are rejected by
  `render/particles.rs` (test case "sequence owned").
- **Object behaviour graphs** run hkx clips on a model's nodes
  (`start_object_graph`). The havok crate has no variant for
  `BGSGamebryoSequenceGenerator`: it parses as `Generator::Other`. The
  skills menu hand-codes its stars' graph states instead (`src/stats_view.rs`).
- **Spell projectiles** fly by their `PROJ` (`spawn_spell_projectile`,
  `src/ai/archery.rs:443`). Their model is drawn as a static `GpuModel`,
  with no particles, controllers, graph or light.
- **`MagicEffect`** (`src/magic.rs:140`) reads `DATA` but none of its visual
  fields. Nothing reads `ARTO` (only accepted as a placeable base,
  `src/world/records.rs:362`), `EFSH`, `RFCT` or `DUAL`.
- **Moving lights**: one `HeldLight` per `ActorInstance` (a torch), placed by
  the model's `AttachLight` node, merged each frame by
  `update_held_lights` (`src/ai/equipment.rs:364`) through
  `assign_moving_lights`.
- **Actor attachments**: `ActorInstance.attachments` / `equipment` hold
  rigid models on bones, drawn with the actor. They have no particles or
  clock of their own.
- **Graph events for casting**: `player_graph_event` (`src/casting.rs`) and
  `ActorRuntime::graph_event` (`src/ai/mod.rs:1040`) send `MRh_/MLh_/DualMagic_`
  `Spell...Start`, `..._SpellReady_Event`, `..._SpellRelease_Event`,
  `..._Equipped_Event` to the actor's graph.

## What the data says

Sources: xEdit `wbDefinitionsTES5.pas` (dev-4.1.6, raw GitHub), UESP
[ARTO](https://en.uesp.net/w/index.php?oldid=3089063) and
[EFSH](https://en.uesp.net/w/index.php?oldid=3089075) (thin; xEdit is
fuller), `refs/CommonLibSSE-NG` (`ModelReferenceEffect.h`,
`ShaderReferenceEffect.h`, `ActorMagicCaster.h`, `RefAttachTechniqueInput.h`,
`BGSDualCastData.h`), and surveys of the vanilla data made for this plan with
`vrm-tool` (`esp-list`, `nif-dump`, `nif-raw`, `nif-tree`, `hkb-obj`) and a
throwaway script over the masters. CommonLibSSE gives names and layouts
only, not behaviour.

**`MGEF` `DATA` visual fields** (offsets from xEdit; ours already read 0x48,
0x64, 0x88 at these offsets):

| Offset | Field | Vanilla MGEFs using it (of 1534) |
|---|---|---|
| 0x18 | Casting Light (`LIGH`) | 522 |
| 0x20 | Hit Shader (`EFSH`) | 737 |
| 0x24 | Enchant Shader (`EFSH`) | 190 |
| 0x5C | Casting Art (`ARTO`) | 682 |
| 0x60 | Hit Effect Art (`ARTO`) | 345 |
| 0x6C / 0x70 | Dual Casting Art (`DUAL`) / scale | 11 |
| 0x74 | Enchant Art (`ARTO`) | 21 |
| 0x78 / 0x7C | Hit / Enchant Visuals (`RFCT`) | 0 |
| flag bit 12 | FX Persist | 874 |
| flag bit 27 | No Hit Effect | — |

**`ARTO`**: `MODL` and `DNAM` art type (0 casting, 1 hit effect,
2 enchantment). 320 records, 309 models.

**How art attaches** (from the models, not code): art models carry
`NiStringsExtraData` tags.
- `NamedNode&<node>` on a node: that subtree attaches to the actor's node
  of that name. 244 of 309 models. Most common: `MagicEffectsNode` (62),
  `NPC Head [Head]` (23), `NPC Spine2 [Spn2]` (16), hands, calves, and
  creature nodes.
- `MultiTechnique` on the root (31 models): several `NamedNode` children,
  each to its own node (`FireStormCastBodyFX`: `MagicEffectsNode`, head,
  left hand, both upper arms).
- `HavokGeometry` (3: wards, `DLC1ShadowShieldFX`): collision, out of scope.
- No tag: in `Skyrim.esm`, all 419 MGEF casting-art uses and 36 of 224
  hit-art uses. Casting art with no tag goes on the casting hand's
  `NPC L/R MagicNode [LMag]/[RMag]` (in the skeleton; `ActorMagicCaster`
  keeps a `magicNode`). Where untagged hit art goes is open.
- `RefAttachTechniqueInput` has `actor`, `actorRace`, `nodeName`: the
  technique is resolved against the actor's own skeleton.

**Effect graphs**: art and projectile models carry
`BSBehaviorGraphExtraData`. Graphs used by art (count of the 309 models):
`GenericBehaviors\Autoplay` 53, `Magic\BeginLoopEnd` 43,
`Magic\IdleOnLoad` 35, `Magic\CastingBasic` 34, `Magic\CastingRitualBody`
24, `Magic\CastingWithIntro` 16, and a long tail. Their leaves are
`BGSGamebryoSequenceGenerator` (sequence name at +0x48, a 1.0 float at
+0x54), each playing the NIF's `NiControllerSequence` of that name
(`mIdle`, `mReady`, `mCharge`, `mCast`, `mIdleStaff`, `mIntro`; `mBegin`,
`mLoop`, `mEnd`). The graphs contain no clips.
- `CastingFXBehavior01` (CastingBasic) listens for the **actor's** casting
  events: `MRh_/MLh_SpellAimedStart`, `..._SpellSelfStart`,
  `..._Spell{Aimed,Self}ConcentrationStart`, `DualMagic_...`,
  `..._SpellReady_Event`, `..._SpellRelease_Event`, `..._Equipped_Event`,
  with variables `bRightHand`, `bLeftHand`, `iHandType`.
- `BeginLoopEnd`: `StopEffect`, `EndAnim`, `SoundPlay`, `RestartEffect`.
- `IdleOnLoad`: `EndAnim`, `PlaceActor`, `StopEffect`, `SoundPlay`,
  `Imod`, `Rimod`.

**Sequence-owned controllers are the content.** In
`Fireball01HandEffects.nif` the emitters', gravity's and shaders'
controllers have timing flags 0x68 / 0x6C (manager-controlled), so today's
effect path would run none of them.

**Projectiles**: `FireballProjectile02.nif` runs `BeginLoopEnd` with
addon nodes; `LightningBolt04.nif` runs `IdleOnLoad` with
`BSProceduralLightningController` and an `AttachLight` node 4000 units out.
`PROJ DATA` +16 is its Light (`LIGH`).

**`EFSH`** (280 records): textures `ICON` fill, `ICO2` particle,
`NAM7` holes, `NAM8` membrane palette, `NAM9` particle palette. The big
`DATA` struct has two halves (field list in xEdit):
- Membrane: blend modes and z-test; fill colour keys; alpha fade in / full /
  fade out; persistent ratio; pulse; UV scroll and scale; edge fall-off,
  colour and alpha envelope; holes start/end; flags such as Affect Skin
  Only, Edge Effect Inverse, Grayscale Color/Alpha, No Weapons.
- Particles: birth ramp up / full / ramp down, ratio, persistent count;
  lifetime ±; speed and acceleration along the normal; initial velocity and
  acceleration; two scale keys; three colour keys; rotation; animated
  frames (`Texture Count U/V`).

`ShaderReferenceEffect` holds the textures, `addonModels`, `particleShaders`,
`alphaTimer`, `effectShaderAge`.

**Skinned art**: 38 of 309 models are skinned. They are mostly creature
attachments (spider and Benthic Lurker hands, draugr eyes, the Dragonborn
and Miraak tentacles, weapon streaks). Only `MAGDragonMouthFireSkin` is a
spell's. The player's spell art is rigid.

## Ground rules

- Public sources only (clean-room memory): xEdit, UESP, CommonLibSSE names
  and layouts, the game data itself, and in-game observation. CommonLibSSE
  says nothing about timing or policy, and the plan must not read it as if
  it did.
- Open choices go in `known_ambiguities/spell-visuals.md` (new), and
  `known_ambiguities/effect-shaders.md` for step 8. What a test mod can
  observe (Papyrus-visible state) goes to the next autotest. Purely visual
  choices are settled by side-by-side screenshots the user takes in SSE;
  list each such comparison in the ambiguity entry.
- One step, one commit (bigger steps may be a few). `cargo fmt` before
  each. Unit tests on synthetic NIFs and records. Engine tests per the
  fixture-test memory (`--include-ignored`). An offscreen
  `--screenshot --burst` run of the visible result, read back before
  calling a step done.
- `VRM_NO_SPELL_FX=1` turns off everything this plan adds (as
  `VRM_NO_IMPACTS` does for impacts).
- Visuals are derived state: rebuilt from active effects, casting state and
  projectiles. They are not saved, except what step 10 says about scripted
  shaders.

## Steps

### 1. Records and a survey tool

- `MagicEffect` gains `casting_light`, `hit_shader`, `enchant_shader`,
  `casting_art`, `hit_art`, `dual_cast: (FormId, f32)`, `enchant_art`,
  `hit_visuals`, `enchant_visuals`. Add flag constants `FX_PERSIST` (bit 12)
  and `NO_HIT_EFFECT` (bit 27).
- New `src/spell_fx/records.rs`: `ArtObject { model, kind }`,
  `EffectShader` (all of `DATA`, with an older short `DATA` tolerated:
  `SetOptionalFrom(0)`), `VisualEffect { art, shader, flags }` (`RFCT`),
  `DualCastData` (`DUAL`).
- `nif`: parse `NiStringsExtraData` (it is `Unknown` today). Add
  `nif::attach_tags(&Nif) -> AttachTags` returning, per node,
  `NamedNode&` targets, `MultiTechnique` and `HavokGeometry`.
- `vrm-tool spell-fx <data> [mgef|spell]`: one effect's visuals (art models,
  their tags, graph, sequences, shader textures, lights), or totals over all
  MGEFs. This reproduces this plan's tables and is the tool later steps
  check against.
- Tests: synthetic MGEF/ARTO/EFSH/RFCT/DUAL records; a strings-extra-data
  block round trip.

### 2. Sequence-driven effect instances

The foundation: an effect model whose named `NiControllerSequence` can be
played, switched and blended, driving everything the sequence owns.

- Pull `Effect` out of `src/impacts.rs` into `src/spell_fx/effect.rs` as a
  shared `EffectInstance`: root + parts, own clock, `pose`, `step`, `draw`.
  Impacts keep their current behaviour (standalone controllers, text-key
  `start`), and their tests must pass unchanged.
- Load the model's sequences (`ModelAnim::sequences` already holds node and
  material channels for objects; extend the `#effect` conversion to keep
  them).
- **Particle controllers in sequences**: accept manager-controlled particle
  controllers when the playing sequence's `ControlledBlock` names their
  modifier (`NiPSysEmitterCtlr`, `...SpeedCtlr`, `...LifeSpanCtlr`,
  `...InitialRadiusCtlr`, `GravityStrengthCtlr`, the rest already
  supported standalone). The sequence's interpolator replaces the
  controller's `NiBlend*Interpolator`. A sequence that doesn't own a
  controller leaves it at rest (emitters off).
- `EffectInstance::play(name)` restarts the sequence's clock. Transitions
  are immediate at first (step 3 adds the graph's blend durations).
- Tests: a fixture NIF with two sequences driving an emitter's birth rate
  and a node's visibility. Switching sequences stops and starts emission.
  An impact fixture still poses exactly as before.

### 3. Effect behaviour graphs

- havok: a `Generator::Sequence { name, sequence, … }` variant for
  `BGSGamebryoSequenceGenerator`. Its +0x50 byte (`eBlendModeFunction`) and
  +0x54 float (`fPercent`) weigh it in a non-parametric blender
  (`known_ambiguities/spell-visuals.md`).
- `GraphAnim` (or a slim sibling, if `GraphAnim` assumes a rig) reports its
  active sequence leaves with weights each step, instead of a pose. State
  machines, transitions and wildcards run as they already do.
- `EffectInstance` with a graph: the graph picks the sequence;
  `send_event` and `set_variable` are forwarded. The project is loaded
  through `self.graphs` (the projects use `SingleBoneSkeleton.hkx`).
- Tests: run `CastingBasic`, `BeginLoopEnd`, `IdleOnLoad` and `Autoplay`
  from the game data in GPU-ignored tests. Check the event → sequence
  sequences: `MRh_SpellAimedStart` → `mCharge`, `MRh_SpellReady_Event` →
  `mReady`, `MRh_SpellRelease_Event` → `mCast`, `MRh_Equipped_Event` →
  `mIdle`, and `StopEffect` → `mEnd`.
- Later, the skills menu's stars could run their graphs this way instead of
  the hand-coded states. Note it in ROADMAP; not this plan.

### 4. Art attached to actors

- `ArtEffect`: an `EffectInstance` plus a target actor, an attach map
  (instance part → skeleton bone index, from step 1's tags resolved against
  the actor's skeleton) and a default node. Each frame each attached
  subtree takes `actor.transform * pose[bone]`. Particles in world-space
  systems are carried as for impacts.
- Resolution: a `NamedNode&X` node goes to bone `X`. Under `MultiTechnique`,
  each tagged child goes to its own bone and untagged children to the
  default node. No tag sends the root to the default node. A missing bone
  (a creature without `MagicEffectsNode`) falls back to the default node,
  then to the actor's root, and is logged once per model.
- `SpellFx` state on `Engine`: art effects by `(actor, key)` so the same art
  isn't stacked twice. Drop them when the actor unloads or is disabled.
  Hide them while the actor's 3D is hidden (invisibility: decide in the
  ambiguity file whether the art is hidden too).
- Drawn among `scene.dynamic` like impact effects. Particle batches go
  through the same path.
- Tests: tag resolution on synthetic skeletons; an art effect follows a
  moving bone; dropping on unload.

### 5. Casting art and casting light

- When a hand starts a cast (player: `player_hand_start` /
  `player_dual_start`; NPCs: `ai/casters.rs`), attach the spell's first
  effect's casting art. Whether the first effect, or each effect with art,
  shows is open: check spells with mixed effects in `spell-fx`. Default
  node: that hand's `NPC L/R MagicNode`. Set `bLeftHand` / `bRightHand` /
  `iHandType` and send the hand's start event.
- Forward the hand's casting events (`Spell…Start`, `SpellReady_Event`,
  `SpellRelease_Event`, `Equipped_Event`) to the art's graph, where they are
  sent to the actor. Whether the game forwards sent events, events the
  actor's graph raises, or both is open. Start with the sent ones, which is
  where the listened names match.
- Detach on the hand's `Equipped_Event` after release once the graph is idle
  (`mIdle`), when the spell is unequipped, or when the hand is sheathed.
  Hand art while a spell is only equipped and drawn but not cast (the idle
  glow in the hands) is the `mIdle` state. Show it whenever magic is drawn,
  and treat that as an ambiguity entry to compare with SSE.
- Dual casting: one art per hand, or the `DUAL` record's scale when it
  applies (step 9).
- Casting light: `LIGH` at the hand's magic node, while the hand is drawn
  with that spell. `ActorInstance.held_light` becomes `held_lights: Vec<…>`,
  `update_held_lights` gathers them all, and the torch keeps working.
- Staffs: the enchantment's effect's casting art at the staff. Its default
  node is unknown: survey which tags staff art carries first (`AnimObjectR`
  and `Weapon` both appear as `NamedNode` targets). This can split into its
  own commit.
- Screenshot check: the player drawing Flames, Firebolt and Healing in both
  hands (`pdraw`, then `pcast`, per the remote-testing memory), and an NPC
  mage in combat.

### 6. Spell projectiles

- `spawn_spell_projectile` makes an `EffectInstance` of the `PROJ` model
  (graph from step 3), oriented along its velocity each frame. The graph
  starts in its start state (`BeginLoopEnd`: `mBegin` → `mLoop`).
- On impact: send `StopEffect` and leave the instance where it struck until
  `mEnd` ends (or the projectile's `Fade Duration`, `DATA` +48), with
  emitters stopped. Particles live out their lives. Then drop it. The
  impact itself is already handled.
- Projectile light: `PROJ DATA` +16 `LIGH` at the model's `AttachLight`
  node (or its origin), moving with it. It joins the moving lights.
- Flames (type 0x08): one model held at the hand while casts feed it, its
  `fFlameProjectileLength` the hit distance over the range. (The first
  implementation relaunched flying copies at `Relaunch Interval` for
  `Lifetime`; the lifetime is 0 on nearly every vanilla record, and the
  models are stationary emitters.) Cones (0x10): one model per cast, sent
  out at `Speed` as far as the range. The gameplay hit tests stay as they are.
- Beams (0x04, lightning): place the model from the hand to the hit point
  (`BeamEnd` node at the far end), running its graph. The bolt geometry is
  `BSProceduralLightningController`, a ROADMAP Rendering item: until it
  lands, the beam shows its glow, light and particles only. Say so in the
  ambiguity file.
- Tests: projectile instance pose follows velocity; `StopEffect` at
  impact; lights added and removed.

### 7. Hit effect art

- When an active effect starts on a target (`ActiveEffect` gains nothing
  saved; `SpellFx` keys by effect id), attach its `hit_art` unless the
  effect has `NO_HIT_EFFECT`, or it was applied quietly
  (`apply_item_quietly`) — check that against SSE.
- Lifetime: with `FX_PERSIST`, for the effect's duration (send `StopEffect`
  when it ends and let `mEnd` play out). Without it, play once: drop when
  the graph's sequence has ended or after the longest sequence's length.
  Instantaneous effects (duration 0) play once either way.
- One instance per `(target, art)`: a second effect with the same art
  refreshes rather than stacks. Constant effects (abilities, worn
  enchantments) persist while active and conditions hold (`active`).
- Self-delivered effects use the same path (cloaks' `FXFireCloak01` on
  `MagicEffectsNode`). Ritual "cast body" art is hit art on the caster.
- Untagged hit art (36 uses in `Skyrim.esm`): default to `MagicEffectsNode`,
  else the root. This is an ambiguity, to compare one such effect's screenshot
  with SSE.
- Tests: persistence vs one-shot, refresh, end on dispel, summons and
  unload.

### 8. Effect shaders: membrane

The largest step. Hit shaders cover 737 MGEFs: flesh spells' sheen, fire's
burn glow, soul trap, paralysis, invisibility's shimmer and ethereal forms.

- `src/render/membrane.rs`: a pass that draws an actor's meshes again
  (same skinning, slight inflation along the normal; amount open) with the
  EFSH membrane material. Fill texture with UV scroll and scale; colour
  from the three fill keys over the effect's age × `Color Scale`; alpha
  envelope (fade in, full, fade out, persistent ratio, pulse amplitude and
  frequency); edge term from the view angle with `Edge Effect - Fall Off`,
  edge colour and its own envelope (`Edge Effect Inverse` flips it);
  greyscale-to-palette lookup through `NAM8` when the Grayscale flags are
  set; `NAM7` holes dissolving between start and end values over their
  times; blend source / destination / op and z-test from `DATA`.
- `Affect Skin Only`: only skin shapes (body, hands, head); otherwise armour
  and clothing too. `No Weapons`: leave out weapon attachments.
- Enchant shaders are the same pass on an enchanted weapon's model, equipped
  in hand (step 9).
- `ShaderReferenceEffect` is stackable (`GetStackable`): several shaders
  on one actor all draw. Their order and the cap are open.
- Lifetime: as hit art (step 7), with the envelope's fade-out after the
  effect ends.
- Tests: envelope sampling at fixed times against hand-computed values;
  record → material. Screenshot: Oakflesh, Soul Trap and a Flames burn on an
  NPC, each beside an SSE screenshot the user takes. Record both in
  `known_ambiguities/effect-shaders.md`.

### 9. Effect shaders: particles; enchantments; dual cast scale

- EFSH particle shader (unless `No Particle Shader`): particles born on the
  actor's surface with the birth ramps, lifetime ±, speed and acceleration
  along the normal, initial velocity and acceleration, scale and colour
  keys, rotation, and animated frames over `Texture Count U × V`, drawn with
  `ICO2` and palette `NAM9`. Emission positions need skinned vertices on the
  CPU. Particles' "skinned/deformed mesh emission" is a ROADMAP Rendering
  item, so either build that here (keep CPU copies of skin vertices and
  weights, skin a sampled subset per frame) or emit along bone segments as a
  stopgap. Recommend the former: it also closes the Rendering item for
  NiPSys mesh emitters.
- Addon models (`DEBR`, with fade and scale times): leave out and list in the
  ambiguity file.
- Enchantments: equipped enchanted weapons show their enchantment's effect's
  `enchant_shader` (membrane on the weapon model) and `enchant_art`
  (attached to the weapon, `NamedNode&Weapon` seen in two models). Whether
  it shows only while drawn, and how it dims as charge runs out, are open.
  Armour enchantments show nothing in vanilla, as far as the data shows
  (190 enchant shaders are mostly weapon effects; check with `spell-fx`).
- `DUAL`: dual-cast projectiles, hit art and explosions scale by the
  MGEF's dual-cast scale where `Inherit Scale` says so; its own projectile,
  shader, hit art and impact set replace the MGEF's when present.
- Tests: particle shader emission counts over the ramps; enchanted sword
  shows its shader only when drawn.

### 10. Scripts' visuals

- `EffectShader.Play(ref, duration)` / `Stop(ref)` and
  `VisualEffect.Play(ref, duration, facing)` / `Stop(ref)`: shader and art
  effects keyed by `(ref, form)`, a duration of -1 lasting until stopped.
  `RFCT` flags: Rotate to Face Target (the `facing` ref), Attach to Camera,
  Inherit Rotation.
- `ObjectReference` targets that aren't actors: membrane on their rigid
  model, art at the root (or a `NamedNode` the model has).
- These are the first natives of ROADMAP item 5's "still do nothing" list.
  Check `vrm-tool pex-natives` counts before and after.
- Saves: whether script-played shaders survive a save and load is
  testable: an autotest that plays a shader with no duration, saves, and has
  the user reload and report whether it still shows (manual observation).
- Tests: VM natives with synthetic records; a scripted shader stopped by
  `Stop`.

### 11. Roadmap and docs

- Move what's done to ROADMAP Done. Keep Next's remaining items (lightning
  beams, skinned art, wards' geometry, explosions, magic sounds, EFSH addon
  models) in their places.
- `known_ambiguities/spell-visuals.md` and `effect-shaders.md` list each
  choice made above with its evidence and remaining uncertainty. Pending
  comparisons and probe instructions belong in this plan, not those files.

## Order and dependencies

```
1 records ─┬─ 2 sequences ── 3 graphs ─┬─ 4 attach ─┬─ 5 casting art + light
           │                           │            └─ 7 hit art
           │                           └─ 6 projectiles (+ light)
           └─ 8 membrane ── 9 particles / enchant / dual ── 10 natives
```

Steps 1–5 give the casting hands. That is the biggest visible gain for the
least new rendering: no new pipeline, just sequences, graphs and
attachments. Steps 6–7 reuse all of that. Step 8 is the first new
render pass. Steps 8–9 can run in parallel with 5–7 once step 1 is in.

## Open research questions

1. Untagged hit art's attach node (36 uses).
2. Which of a multi-effect spell's casting arts show.
3. Actor graph events forwarded to art graphs: sent, raised, or both.
4. Hand art while magic is drawn but idle.
5. Hit art and shaders on quietly applied effects, and on invisible
   actors.
6. One-shot hit art lifetime without `FX_PERSIST`.
7. Membrane inflation, stacking order and cap.
8. Flame and cone projectiles' visuals vs their instant hit test: held and
   travelling models chosen (`known_ambiguities/spell-visuals.md`); the
   exact `fFlameProjectileLength` measure is unverified.
9. Enchantment visuals while sheathed and as charge falls.
10. `BGSGamebryoSequenceGenerator`'s byte at +0x50 and float at +0x54:
    blend mode and percent, applied as blender child weights
    (`known_ambiguities/spell-visuals.md`).
11. Script-played shaders across save and load.
