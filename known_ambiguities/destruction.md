# Destructible objects

Destructible definitions, queries, scripted damage and stage events are
implemented in `crates/esp/src/destructible.rs`, `src/world/destruction.rs`,
`src/script/natives.rs` and `src/condition/mod.rs`. `ClearDestruction` resets
runtime state, and the engine's own save format persists it. This is not Skyrim `.ess` save support. Sources: UESP's DEST field page
([permanent rev 2334527](https://en.uesp.net/w/index.php?oldid=2334527)),
[xEdit's TES5 definitions](https://github.com/TES5Edit/TES5Edit/blob/dev-4.1.6/Core/wbDefinitionsTES5.pas)
(`wbDEST`, `EXPL`), CommonLibSSE's `BGSDestructibleObjectForm.h`, and the
vanilla data (Skyrim.esm, Update.esm and the three DLC; 675 records with
`DEST`, nearly all ACTI).

## Settled

- **Layout.** `DEST`: health (uint32), stage count, VATS targetable, two
  unknown bytes. Each stage: `DSTD` (health %, index, model damage stage
  0-8, flags, self damage per second, explosion `EXPL`, debris `DEBR`,
  debris count), then optionally a replacement model (`DMDL`, `DMDT`,
  `DMDS`), then `DSTF`. Stage flags: 0x01 cap damage, 0x02 disable, 0x04
  destroy, 0x08 ignore external damage. CommonLibSSE also names 0x10
  "becomes dynamic". No vanilla stage sets it.
- **Implemented parser subset.** `DEST` health, VATS flag, all fixed `DSTD`
  fields, referenced explosion/debris forms, and a stage's optional `DMDL`
  path are retained. `DMDT`/`DMDS` texture replacement data is skipped. The
  fixed field sizes and meanings follow UESP; CommonLibSSE's public header
  corroborates the in-memory stage concepts but does not describe plugin byte
  ordering. Invented records exercise stage boundaries and malformed lengths;
  no game assets were added.
- **Initial per-reference query.** A placed reference with no recorded
  runtime stage reads as `-1`, independently of other references to the same
  base. This follows the community [Skyrim Papyrus index's
  `GetCurrentDestructionStage` page](https://papyrus.bellcube.dev/skyrimse/script/objectreference/function/getcurrentdestructionstage/),
  which says a completely undamaged object appears to have stage `-1`; the
  [public vanilla `ObjectReference.psc`](https://github.com/ianpatt/skse64/blob/master/scripts/vanilla/ObjectReference.psc)
  declares the native and describes it as the object's current stage. CTDA
  function 470 is [UESP's `GetDestructionStage`](https://en.uesp.net/w/index.php?oldid=3566882).
  The initial `-1` policy is public-source-supported, not verified in Skyrim
  SE here. Stage threshold behavior, including authored 100% stages, remains
  unresolved and is not used to compute the initial stage.
- **Scripted `DamageObject` subset.** The native returns no value. For a
  reference listed in any currently instantiated `CellRuntime.refs`, a
  positive finite amount subtracts from per-reference health initialized to
  `DEST.health`; health is bounded to `[0, DEST.health]`. Invalid amounts,
  missing DEST/zero max health, and unloaded references are no-ops. This
  loaded boundary is the runtime cell reference list, not a separate check of
  3D attachment. This follows the public Papyrus [DamageObject
  page](https://papyrus.bellcube.dev/skyrimse/script/objectreference/function/damageobject/)
  only for the stated unloaded-object caveat; its note is observational and
  we have not verified parity in game. The existing stage query reports `-1`
  until a threshold is crossed. On a hit, crossed stages are considered in
  authored array order; query state stores the last crossed `DSTD.index`, or
  the first crossed `CAP_DAMAGE` stage's index when a cap applies. The first
  crossed cap stage limits health to its own threshold and prevents that hit
  from recording a later stage. Intermediate crossed stages currently have
  no event or other side effect. These threshold and cap rules are explicit
  implementation choices, not confirmed Skyrim behavior.
- **Stage-number ambiguity.** The same Bellcube [GetCurrentDestructionStage
  page](https://papyrus.bellcube.dev/skyrimse/script/objectreference/function/getcurrentdestructionstage/)
  documents untouched `-1`, but does not specify whether later script-visible
  values correspond to `DSTD.index` or the separate model damage stage. A
  public [destructible-object author
  tutorial](https://skyrimmw.weebly.com/skyrim-modding/destructible-object-creation-breakable-stuff-skyrim-modding-tutorial)
  calls the model damage stage what scripts reference. Runtime uses
  `DSTD.index` per the current query policy; synthetic tests deliberately use
  unequal values so this assumption stays visible. Mapping is unresolved.
  Public vanilla [`ObjectReference.psc`](https://github.com/ianpatt/skse64/blob/master/scripts/vanilla/ObjectReference.psc)
  declares the native but does not settle the mapping.
- **`OnDestructionStageChanged` dispatch.** When a `DamageObject` call changes
  stored stage, one event with old and new stage integers is queued after the
  new state is stored, through the normal reference/effect/running-alias
  dispatcher. Health-only hits and `ClearDestruction` queue none; clear does
  not erase previously queued events. The public Bellcube
  [event page](https://papyrus.bellcube.dev/skyrimse/script/objectreference/event/ondestructionstagechanged/)
  describes worsening-stage notifications and old/current arguments. Runtime
  sends one old/final notification per call even when thresholds are skipped;
  whether Skyrim sends one event per skipped stage is unknown. The unresolved
  `DSTD.index` versus model-damage-stage mapping also applies to these values.
- **`ClearDestruction` state reset.** The Papyrus native clears this
  reference's stored health/stage entry regardless of load state; repeated
  calls are no-ops and do not queue `OnDestructionStageChanged` or other
  events. Subsequent supported damage initializes state again at `DEST.health`
  and stage `-1`. Bellcube's [ClearDestruction
  page](https://papyrus.bellcube.dev/skyrimse/script/objectreference/function/cleardestruction/)
  says it clears destruction effects and must not fire the stage-changed
  event, while its caveat reports effects do not seem to be reapplied by
  later damage. Our reset is only the in-memory health/stage state: visual,
  collision, flags and destruction effects are not represented or restored,
  so effect fidelity and the later-damage behavior remain unresolved.
- **Engine save persistence.** The engine-owned snapshot format has an optional
  `DSTR` section containing sorted `(reference, health:f64, stage:i32)` tuples,
  an independently designed engine-save schema. Missing `DSTR` in older
  snapshots means an empty map; applying a snapshot replaces existing
  destruction state and does not replay stage events. The reader rejects
  null/duplicate references, non-finite or out-of-range health, stages outside
  `-1..=255`, truncated entries and trailing bytes. This only covers this
  engine's save format; Skyrim `.ess` destruction-state semantics are not
  implemented or inferred.
- **Stage order.** In every vanilla record, `index` equals the stage's
  position and the health percentages descend. The last stage is usually
  at 0%.
- **The Bleak Falls web** (`FXspiderWebKitDestructible`, ref `000AA83F`):
  health 10. Stage 0 is at 99%, damage stage 1, cap damage, explosion
  `FXSpiderwebStripExplosion`, no replacement model. Stage 1 is at 0%,
  damage stage 2, the same explosion, and replacement
  `FXspiderWebKitDoorSpecialDest.nif` (strands on constrained rigid
  bodies). The web's other bases (E3Demo, MS13, Actor, DLC2 NoObs) have the
  same stages.
- **Webs don't thrash when hit.** Their graph (`WebBehavior.hkx`) plays
  `ThrashShort` / `ThrashShort2` on `IdleWebThrashShort(2)`, `ThrashOnce`
  and `Thrash2Once`. Only `SpiderWebAnimationsTrigger`'s
  `SpiderWebAnimationsSCRIPT` sends these, for a web with a trapped actor,
  while the player is in its trigger. No graph event is named for hits.
- **Weapons and fire damage webs.** UESP: "Some webs block your path; cut
  through them" (*The Golden Claw*,
  [rev 3587159](https://en.uesp.net/w/index.php?oldid=3587159)); "hack or
  burn through the thick webbing" (*White Ridge Barrow*,
  [rev 3564654](https://en.uesp.net/w/index.php?oldid=3564654)).

## Unconfirmed: choices to make

- **What "model damage stage" selects.** Seven vanilla models have
  `BSDamageStage` nodes named for stages: `DamageStage0` .. `DamageStage3`
  (`StockadeBarricade01.nif`, whose bases' stages use damage stages 1-4,
  the last with a replacement model), and ranges such as `DamageStage0-1`
  (windmills, grain mill) and `DamageStage2-3` (bee hive). Choice: show the
  child whose name covers the current damage stage and hide the others.
  The nodes' own range fields aren't parsed (`NodeKind::Range` has none),
  so we go by the names; whether the game uses the names or the fields is
  unconfirmed.
- **A stage with neither a replacement model nor matching nodes** (175 of
  817 stages, including the web's first): the model in use stays. Four
  records (`ResourceObjectFarm`, `ResourceObjectFarmWR`,
  `ResourceObjectMine`, `CWFortNorPortcullis`) follow a stage that has a
  replacement with one that has none. Choice: the last replacement stays,
  as nothing says to go back to the base model.
- **Damage threshold and cap parity.** Runtime currently records the last
  crossed stage after damage and caps a hit at the first crossed cap stage's
  own threshold; it does not run intermediate stage effects. This preserves
  the documented untouched `-1`, but exact handling of 100% stages, overkill,
  and later stages after a cap has not been checked in Skyrim SE.
- **How much a weapon hit does.** Choice: the blow's damage as it would be
  to an actor before armor and blocking; arrows deal their projectile
  damage. Power attacks add nothing beyond the power attack's own blow
  damage, and bashes deal their bash damage. Objects have no armor, and the
  game's formula for objects isn't public (UESP's DEST and DSTD pages and
  the function index say nothing about it).
- **Which spells damage objects, and by how much.** UESP's "burn through"
  shows fire works. Whether frost, shock and other hostile damage do too
  is unconfirmed. Choice: any hostile effect that damages health deals its
  magnitude to an object it lands on or whose area reaches it. Its magnitude
  is taken as authored, without resistances or dual-cast power, and a
  stage's Ignore External Damage flag blocks it as it blocks blows. Touch
  spells land on the object they touch.
- **Ignore external damage** (650 stages, mostly mine ores and the civil
  war resource objects; DSTD flag 0x08). Choice: while the reference is in
  a stage with the flag set, weapon, arrow and spell hits do nothing; the
  flag is read from the current stage, not the one a hit would enter.
  `DamageObject` and the stage's self damage per second still reach these. Source: UESP DEST Field,
  https://en.uesp.net/w/index.php?oldid=2334527 (flag name only).
- **To settle in game** (`vrm-tool test-mod`): how many hits of what size
  the web takes with cap damage; whether frost or shock spells damage it;
  whether stage 1's model changes in the Bleak Falls web; and
  `GetCurrentDestructionStage` after each hit.
