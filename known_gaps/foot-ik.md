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
- Not used: foot unlock, align world-from-model (horses 0.15: pitching the
  body with the ground), hip orientation and max knee angle difference (0
  everywhere), and the legs' `ungroundedEvent`s.
