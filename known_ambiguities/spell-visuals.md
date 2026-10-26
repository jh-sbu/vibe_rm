# Spell visual attachment and lifecycle decisions

Evidence checked on 2026-10-26: the local UESP record documentation
([ARTO revision 3089063](https://en.uesp.net/w/index.php?oldid=3089063),
[MGEF revision 3507756](https://en.uesp.net/w/index.php?oldid=3507756)),
public xEdit dev-4.1.6 definitions (MPL-2.0, consulted only for serialized
field facts), MIT CommonLibSSE-NG declarations at b93280e832f263dbef44e44cbe2936622a02f91a
(names/layouts only), the MIT Skyrim Behavior Editor at
7b7167cb18e28f435a64069acbf61eca49a07be3 (HKX field names and enum labels
only), and installed NIF, ESM and HKX assets. References and their
licenses are retained under ignored `refs/spell-visuals/`. No engine executable,
DLL, disassembly, leaked source or executable-memory data was examined. No game
asset, extracted asset dump, screenshot or game-derived fixture is committed.

These sources specify fields, tags and graph transitions; they do not establish
Skyrim SE's attachment/lifetime policy. The following are implementation choices,
not claims about verified original-engine behavior.

- **Casting selection and idle.** Use the first listed effect's art and light,
  rather than stacking every effect's casting art. Draw it while magic is drawn,
  even before casting. The spell survey exposes spells with several candidate
  effects; that does not identify their arbitration.
- **Events and hand variables.** Forward events sent by the actor's casting code;
  do not relay every event the actor graph raises. The authored CastingBasic
  graph accepts the matching MRh/MLh names. `bLeftHand`/`bRightHand` identify the
  art's hand; `iHandType` is 0 for a spell, 1 for a staff. The variable's full
  value domain and engine forwarding boundary are not established by public
  declarations. NIF `end`
  text keys, distinct from `EndAnim`, advance the inspected magic graphs.
- **Attachment fallback.** Untagged hit art uses `MagicEffectsNode`, then the actor
  root; casting art uses the hand's magic node. Missing named nodes use the
  default, then root, and are logged once per model/target-name pair. MultiTechnique
  tags establish independent subtrees, not a default for untagged hit art.
  Staffs default to Weapon/SHIELD; their explicit tags still take precedence.
- **Quiet/invisible targets.** Quiet applications and No Hit Effect suppress both
  hit art and hit shaders. Other art stays visible with invisibility; disabled
  and unloaded targets drop their instances. The player's shadow-only body in
  first person does not suppress hand art. The local invisibility references
  describe interruption, not these renderer policies.
- **Refresh and ending.** One hit-art instance per target/art, retained by all
  active owning effect ids. A new landing replaces an existing nonpersistent
  one-shot or stopping instance rather than stacking it; revisiting the same
  effect id does not restart it. Nonpersistent art ends at graph completion or
  the longest authored sequence duration, adjusted by absolute playback frequency,
  with no minimum duration. Sequence-less models use the longest positive finite
  standalone node, material or particle controller span, likewise frequency-adjusted;
  static art or art
  without a usable span uses a one-second fallback. The same lifetime determines
  completion when an indefinitely timed graphless effect stops; explicit impact
  durations remain unchanged. ModelAnim retains the model start offset but no end
  time, so controller spans supply this fallback rather than a model stop marker.
  The references above do not establish SE's sequence-less lifetime; these spans
  and the static fallback are choices to keep art visible beyond its first update.
  Completed/replaced one-shots retain live particles but no mesh fade tail.
  Persistent art stops when its last active owner ends, with graph completion or
  a three-second stopping cap. Other stopped effects retain their mesh fade
  contract, including PROJ fade duration or an authored mEnd duration. Retrying
  StopEffect while stopping lets a graph finish its begin state before hearing
  the stop in its loop. Observed effect ids survive target unload: persistent
  visuals may resume on reload, but one-shots do not replay. Public records and
  the inspected asset transitions do not establish refresh, caps or early-stop
  policy; these remain implementation choices, not verified SE behavior.
- **Flame, beam and cone visuals.** One flame or beam model is held per
  caster, projectile and hand at the hand's magic node (else the cast origin),
  aimed at what the cast reached and refreshed by each delivery; it stops 0.3 s
  after the last one and fades in place for the PROJ fade duration. The vanilla
  flame models are stationary emitters whose particles carry the reach
  (FireSprayFX01 1500 units/s, dragon fire 2400-3300, centurion steam 1800),
  nearly every flame record's lifetime is 0, and CommonLibSSE's FlameProjectile
  only declares an expiration timer and cone angle, so neither relaunching
  copies nor holding one model is documented; holding one matches the assets.
  `fFlameProjectileLength` is the hit distance over the PROJ range, 1 when
  nothing is hit. Its 0-1 domain follows from the graphs (a blend weight in
  SprayRange; SprayRangePlus's selector index picks its max-range sequence only
  at 1), but the engine's exact measure (from the eye or the hand, the ray used)
  is unverified. Cones send one model per cast at the PROJ speed as far as its
  range, then stop and fade where they are; ConeProjectile's declared origin,
  cone angle tangent and growing collision sphere fit a projectile travelling
  outward, but the visual itself is unverified. The immediate gameplay hit tests
  are unchanged. Beams anchor BeamEnd at the reach and run authored graphs;
  procedural lightning geometry remains outside this plan.
- **Dual scale and enchantments.** DUAL's optional projectile/shader/hit-art
  replacements are visual only, and its inherit bits select MGEF scale for art
  and projectile visuals. Drawn equipped weapons show enchantment art and shader;
  sheathing stops them, with no charge-dependent dimming. The records do not
  specify the charge/visibility curve. Explosion scaling is
  part of the excluded explosion implementation.
- **Sequence generator blend modes.** The HKX assets identify sequence name at
  +0x48; the behaviour editor names +0x50 `eBlendModeFunction` (BMF_NONE,
  BMF_PERCENT, BMF_ONE_MINUS_PERCENT) and +0x54 `fPercent`, which the vanilla
  graphs bind by that member name. Every bound use pairs a percent child with a
  one-minus-percent child under a non-parametric blender, both bound to one
  variable (SprayRange, SprayRangePlus, BlendBetweenStatesVariable, the ward
  graphs, the vampire drain graph, moths and fireflies). The factor scales the
  child's weight in such a blender before normalisation; parametric blends and
  sequences outside a blender ignore it. Whether Gamebryo applies it as a
  sequence weight in the same way is not documented.
- **RFCT transforms.** Facing overrides inherited rotation; camera attachment
  replaces the target origin, and named-node attachment is resolved in the
  target model. Exact precedence and camera-space orientation remain unverified.
- **Saving.** Reconstruct active-effect/casting visuals after load. Script-played
  visuals are transient. Public record documentation and the local index do not
  answer whether indefinite script shaders survive SE saves.

Pending SE comparisons and the manual save/load probe instructions live in
`plans/spell-visuals.md`, under "Pending SE validation".
