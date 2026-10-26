# Spell visual attachment and lifecycle decisions

Evidence checked on 2026-10-26: the local UESP record documentation
([ARTO revision 3089063](https://en.uesp.net/w/index.php?oldid=3089063),
[MGEF revision 3507756](https://en.uesp.net/w/index.php?oldid=3507756)),
public xEdit dev-4.1.6 definitions (MPL-2.0, consulted only for serialized
field facts), MIT CommonLibSSE-NG declarations at b93280e832f263dbef44e44cbe2936622a02f91a
(names/layouts only), and installed NIF, ESM and HKX assets. References and their
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
- **Stream reconstruction.** Flame/cone visuals are short-lived models launched at
  PROJ relaunch interval, speed and lifetime, refreshed by concentration delivery.
  The existing immediate gameplay hit test is unchanged. The references above
  do not establish how SE reconstructs continuous stream geometry; short-lived
  model emission is an implementation approximation, not verified SE behavior.

  Beams position BeamEnd where available and run authored graphs;
  procedural lightning geometry remains outside this plan.
- **Dual scale and enchantments.** DUAL's optional projectile/shader/hit-art
  replacements are visual only, and its inherit bits select MGEF scale for art
  and projectile visuals. Drawn equipped weapons show enchantment art and shader;
  sheathing stops them, with no charge-dependent dimming. The records do not
  specify the charge/visibility curve. Explosion scaling is
  part of the excluded explosion implementation.
- **Sequence generator raw values.** The HKX assets identify sequence name at
  +0x48; +0x50 and +0x54 are preserved as raw fields. Examined graphs commonly
  contain 0 and 1.0. No accessible public documentation establishes their full
  semantics, so they do not impose a playback policy here.
- **RFCT transforms.** Facing overrides inherited rotation; camera attachment
  replaces the target origin, and named-node attachment is resolved in the
  target model. Exact precedence and camera-space orientation remain unverified.
- **Saving.** Reconstruct active-effect/casting visuals after load. Script-played
  visuals are transient. Public record documentation and the local index do not
  answer whether indefinite script shaders survive SE saves.

Pending SE comparisons and the manual save/load probe instructions live in
`plans/spell-visuals.md`, under "Pending SE validation".
