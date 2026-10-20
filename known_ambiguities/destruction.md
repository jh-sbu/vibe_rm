# Destructible objects

Destructible definitions and the initial per-reference stage query are
implemented in `crates/esp/src/destructible.rs`,
`src/world/destruction.rs`, `src/script/natives.rs` and
`src/condition/mod.rs`. Health, damage, stage changes, visuals, events,
clearing and saves remain unimplemented. Sources: UESP's DEST field page
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
- **When a stage applies.** A prior candidate was the last stage whose health
  percentage is at or above the object's current health percentage. That
  would make a 100% stage (15 records, e.g. `TG02BeeHive`) active from the
  start and the web's 99% stage active after its first point of damage. This
  candidate conflicts with the public query note above that a completely
  undamaged object appears to be stage `-1`; the stage-selection rule and
  whether entering a stage runs its explosion/debris remain unresolved.
- **Cap damage.** Choice: one hit can't carry the object past the stage it
  enters. Its health stops at the next stage's threshold, so the web
  always takes two blows. The alternative (health stops at the capping
  stage's own threshold) gives the same count for the web but differs for
  stages further apart.
- **How much a weapon hit does.** Choice: the blow's damage as it would be
  to an actor before armor and blocking. Objects have no armor, and the
  game's formula for objects isn't public.
- **Which spells damage objects, and by how much.** UESP's "burn through"
  shows fire works. Whether frost, shock and other hostile damage do too
  is unconfirmed. Choice: any hostile effect that damages health deals its
  magnitude to an object it lands on or whose area reaches it. Explosions
  deal their `EXPL` damage within their radius.
- **Ignore external damage** (650 stages, mostly mine ores and the civil
  war resource objects). Choice: weapons, spells and explosions do nothing.
  Only `DamageObject` and the stage's self damage per second apply.
- **To settle in game** (`vrm-tool test-mod`): how many hits of what size
  the web takes with cap damage; whether frost or shock spells damage it;
  whether stage 1's model changes in the Bleak Falls web; and
  `GetCurrentDestructionStage` after each hit.
