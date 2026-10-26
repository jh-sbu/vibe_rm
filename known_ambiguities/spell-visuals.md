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
  effects; that does not identify their arbitration. Compare Flames, Firebolt,
  Healing and a mixed-effect spell in each hand while idle, charging and releasing.
- **Events and hand variables.** Forward events sent by the actor's casting code;
  do not relay every event the actor graph raises. The authored CastingBasic
  graph accepts the matching MRh/MLh names. `bLeftHand`/`bRightHand` identify the
  art's hand; `iHandType` is 0 for a spell, 1 for a staff. The variable's full
  value domain and engine forwarding boundary are not established by public
  declarations. Compare the same cast with both hands and a staff. NIF `end`
  text keys, distinct from `EndAnim`, advance the inspected magic graphs.
- **Attachment fallback.** Untagged hit art uses `MagicEffectsNode`, then the actor
  root; casting art uses the hand's magic node. Missing named nodes use the
  default, then root, and are logged once per model/target-name pair. MultiTechnique
  tags establish independent subtrees, not a default for untagged hit art.
  Staffs default to Weapon/SHIELD; their explicit tags still take precedence.
  Compare an untagged hit effect on a human and creature, and both staff hands.
- **Quiet/invisible targets.** Quiet applications and No Hit Effect suppress both
  hit art and hit shaders. Other art stays visible with invisibility; disabled
  and unloaded targets drop their instances. The player's shadow-only body in
  first person does not suppress hand art. The local invisibility references
  describe interruption, not these renderer policies. Compare an invisible
  actor's cloak, hand glow, and a quietly applied ability in SE.
- **Refresh and ending.** One hit-art instance per target/art, retained by all
  active owning effect ids. Nonpersistent art ends at graph completion or the
  longest authored sequence (a one-second fallback for models without sequences).
  Persistent art stops when its last active owner ends. Ended instances keep
  particles until they expire; meshes have a three-second stopping fallback unless
  PROJ supplies its fade duration or an authored mEnd duration. Retrying StopEffect while stopping lets a graph finish its begin
  state before hearing the stop in its loop. Public records do not establish
  caps or early-stop policy. Compare overlapping cloaks, a dispel during begin,
  instantaneous hits and unload/reload.
- **Stream reconstruction.** Flame/cone visuals are short-lived models launched at
  PROJ relaunch interval, speed and lifetime, refreshed by concentration delivery.
  The existing immediate gameplay hit test is unchanged. Their source uses the
  right magic node when available; precise per-hand stream arbitration remains
  unconfirmed. Beams position BeamEnd where available and run authored graphs;
  procedural lightning geometry remains outside this plan. Compare left/right
  Flames, dual Flames, a cone and a lightning bolt against SE at fixed distances.
- **Dual scale and enchantments.** DUAL's optional projectile/shader/hit-art
  replacements are visual only, and its inherit bits select MGEF scale for art
  and projectile visuals. Drawn equipped weapons show enchantment art and shader;
  sheathing stops them, with no charge-dependent dimming. The records do not
  specify the charge/visibility curve. Compare the same sword drawn/sheathed,
  full/empty, and a dual projectile beside a single cast. Explosion scaling is
  part of the excluded explosion implementation.
- **Sequence generator raw values.** The HKX assets identify sequence name at
  +0x48; +0x50 and +0x54 are preserved as raw fields. Examined graphs commonly
  contain 0 and 1.0. No accessible public documentation establishes their full
  semantics, so they do not impose a playback policy here.
- **RFCT transforms.** Facing overrides inherited rotation; camera attachment
  replaces the target origin, and named-node attachment is resolved in the
  target model. Exact precedence and camera-space orientation need comparison
  with an RFCT using all three flags.
- **Saving.** Reconstruct active-effect/casting visuals after load. Script-played
  visuals are transient. Public record documentation and the local index do not
  answer whether indefinite script shaders survive SE saves. `vrm-tool test-mod
  spell-visuals <Data> /tmp/SpellVisuals.esp` builds a once-only Oakflesh shader
  probe: load a disposable SE save with the plugin, note its appearance, reload
  the numbered save it requests, and compare without calling Play again. Its
  persisted Ran guard prevents replay; `cqf VrmSpellVisualsQuest StopVisual`
  stops it afterward. This requires a human visual observation, not an invented
  Papyrus query. Engine and SE screenshots must remain outside the repository.
