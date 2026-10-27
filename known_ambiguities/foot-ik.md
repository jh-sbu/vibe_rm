# Foot IK

Implemented in `src/world/footik.rs` (placement) and
`crates/havok/src/behavior` (`hkbFootIkDriverInfo`, `hkbFootIkControlsModifier`).
Sources: the vanilla behaviour data (`vrm-tool hkb-obj`, `hkb-vars`, `hkb-run`)
and the member names it carries. What each gain does inside Havok isn't public.

## Settled

- **The modifier switches placement on.** Feet are placed only while a graph
  runs an enabled `hkbFootIkControlsModifier`. Humanoids' is enabled by
  `bHumanoidFootIKEnable` (default 0, which no graph writes), so the engine must
  set it. The graph reports `bHumanoidFootIKDisable` through the `isActive` of
  killmove, paired and swimming behaviours. Horses' `bHorseFootIKEnable`
  defaults to 1. Fourteen creature projects run one; `hkb-run <project>` lists
  their gains.
- **Layout**: after `hkbModifier` (0x50), twelve `hkbFootIkGains` floats
  (on/off, ground ascending / descending, foot planted / raised / unlock,
  world-from-model feedback, error up/down bias, align world-from-model, hip
  orientation, max knee angle difference, ankle orientation), then the legs
  array (0x80), the error out translation (0x90) and the align-with-ground
  rotation (0xA0). Humanoids bind the feedback gain to
  `m_worldFromModelFeedbackGain` (0.35); giants bind every gain to an
  `m_<gain>` variable, whose defaults differ from the stored values.

## Open (choices made here)

- **Gains are per 30 Hz frame**, like the rest of the engine's easing.
- **On/off** fades the whole placement (offsets, body drop, ankle tilt). When
  the modifier stops, it fades out at 0.2, the gain every vanilla graph uses.
- **Ground ascending / descending** ease each leg's offset up or down (1 for
  most, so offsets follow the ground at once; giants 0.5).
- **Feedback** eases the body's drop towards the lowest foot. Most quadrupeds
  author 0, so their bodies stay where the controller puts them and only the
  legs bend. **Error up/down bias** is read as the share of the error taken
  by dropping. The body rises by `1 - bias` of the lowest offset where every
  foot is higher (horses 0.6, falmer 0.5).
- **Foot planted / raised** scale each leg's lift, blended by how planted the
  animated ankle is (1 for most; giants' raised feet 0.5).
- **Ankle orientation** eases the planted foot's tilt with the ground.
- Foot unlock and align world-from-model are implemented below. Hip
  orientation is zero in vanilla; knee angle difference remains unsupported.
  The survey below found no authored ungrounded events.
- **Actors stand on the collision surface** within a step of the navmesh's
  height. The step is the character's foot IK ray reach (humanoids 40 up and
  32 down, giants 100 / 128; 40 / 64 without foot IK data). The game's
  character controller does this with its capsule, whose step height isn't
  public. Pathing still runs at the navmesh's height.

## Follow-up survey (2026-10-27)

`vrm-tool animation-survey <Data>` examined 49 project files, referencing 26
controls modifiers and 80 control legs (shared graphs counted for each project).
Every controls modifier authors unlock 1. Horse graphs author align 0.15;
others author zero. Hip orientation is zero, including all bound defaults.
Knee difference is nonzero on falmer/riekling (0.4), giant/lurker/vampire
brute (0.2); its cross-leg interpretation remains future work. This corrects
the earlier "0 everywhere" note. Humanoid controls have zero legs; creature
controls have two or four. All 80 ungrounded event IDs are -1: unused in
vanilla, so step 5d adds no event-delivery implementation.

The survey reads the controls-leg array at +0x80, 0x30 bytes per leg, with
an event ID at +0x10. Horse asset words show four repeated blocks, each with
-1 at that offset. The public Skyrim Behavior Editor's
[hkbfootikcontrolsmodifier.h](https://github.com/BrannigansLaw/Skyrim-Behavior-Editor-/blob/master/src/hkxclasses/behavior/modifiers/hkbfootikcontrolsmodifier.h)
names the vector, event ID/payload, vertical error and two flags in that order.
Its MIT license (Wayne K, 2016) was checked against this project's MIT license
before consulting it. Only field vocabulary/layout evidence is used; no
implementation code was copied. No engine binaries were examined.

## Planted-foot locking

Chosen implementation: acquire a world-space ankle target when the existing
planted/raised height blend reaches 0.8; ease lock weight towards that blend
with footUnlockGain at 30 Hz. The pin includes the placed ground offset.
Moving or turning the actor transforms the pin back into its current model
space before the two-bone solve, keeping a planted foot fixed in the world.
Lifting releases it, and missing ground, disabled controls, or a discontinuity
larger than twice hip-to-ankle reach clears it. These thresholds and the gain's
formula are implementation choices, not documented Havok behavior. The survey
and public member names establish the gain's existence, not its algorithm.
A synthetic chain verifies a foot stays put after moving the body, then lifts.

## Align world-from-model

Chosen policy: fit a support plane to the ankles' ground hits, falling back
to averaged hit normals for a degenerate footprint. Ease the body's pitch/roll
towards that plane using alignWorldFromModelGain, and conservatively cap its
angle at the smallest leg max-ankle angle. Placement then solves the legs from
the tilted body, preserving planted pins. Humanoids author zero, so stay upright;
horses author 0.15. The support-plane fit and use of ankle limits as body limits
are implementation choices; original body limits and cross-leg knee constraints
need further evidence. Synthetic step/slope tests cover the plane fit, gain
convergence and unchanged zero-gain bodies.

Before/after offscreen capture bursts were inspected at
`/tmp/footik-turn-{before,after}_*.png` (`--burst-from 10`, ten-frame spacing),
using a Dragonsreach NPC starting and stopping combat to turn/draw. No gross
pose regression was visible; the comparison does not measure original SE parity.
The synthetic world-pin test is the quantitative check for reduced sliding.
