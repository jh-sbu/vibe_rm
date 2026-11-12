# Explosions (`EXPL`)

What the public sources and the game's records say about explosions, for
`plans/explosions.md`. Choices made where they say nothing are in
`known_ambiguities/explosions.md`.

## Sources

- **xEdit** `wbDefinitionsTES5.pas` (refs/xedit, `RegisterRecordDef(EXPL`):
  `EDID`, `VMAD`, `OBND`, `FULL`, model, `EITM` (enchantment), `MNAM`
  (image space modifier), `DATA`: light (`LIGH`), sound 1, sound 2
  (`SNDR`), impact data set (`IPDS`), placed object (any form), spawn
  projectile (`PROJ`), force, damage, radius, IS radius, vertical offset
  mult (floats), flags (u32), sound level (u32). Flags: bit 1 "Always Uses
  World Orientation", 2 "Knock Down - Always", 3 "Knock Down - By
  Formula", 4 "Ignore LOS Check", 5 "Push Explosion Source Ref Only", 6
  "Ignore Image Space Swap", 7 "Chain", 8 "No Controller Vibration"; bit 0
  unnamed. The struct is optional from the vertical offset mult on, with
  the comment "Contradicted by FireStormExplosion02".
  Who names explosions: `MGEF` `DATA` (after the projectile, 0x4C), `PROJ`
  `DATA` 0x24 (with "Explosion - Alt. Trigger - Proximity" 0x1C and
  "- Timer" 0x20, flags 0x2 "Explosion" and 0x4 "Alt. Trigger"), `DSTD`
  (destruction stages, 0x08), `DUAL` `DATA` 0x04, `BPTD` (explodable and
  severable limbs).
- **UESP** *Skyrim Mod:Mod File Format/EXPL* (revision 2366823,
  <https://en.uesp.net/w/index.php?oldid=2366823>): the same subrecords;
  `DATA` is "52 bytes, exceptions: FXEmptyObject.nif, 48 w/ EITM, 44 w/o,
  spider/dragon projectiles 40 w/ 0s OBND".
- **CommonLibSSE-NG** (MIT; refs/CommonLibSSE-NG): `BGSExplosionData` names
  the same fields in the same order (`impactPlacedObject`,
  `imageSpaceRadius`, `verticalOffsetMult`, `eSoundLevel`) and the same
  flags. `Explosion` is a `TESObjectREFR` (explosions in the world are
  references) with an actor owner, an `ActorCause`, a non-actor magic
  caster (for its enchantment), a weapon source, a radius, an image-space
  radius, a damage, start / end / hit key times, two sound handles and a
  light, and the flags "Ignore Image Space Swap" and "In Water". No
  behaviour.
- **UESP gameplay pages**:
  - *Skyrim:Elemental Burst* (revision 3496897): "the explosion uses line of
    sight (LoS) rules to calculate which targets in the blast radius are
    hit. Targets can be shaded from the explosion by even small terrain or
    other enemies, resulting in no damage"; its explosion's enchantment
    (`ccBGSSSE014_EnchShockDamageExplosion01`) deals its shock damage to
    those in the blast. Also: "For *Deep Freeze Paralyze*, *Disintegrate*,
    and *Perk Impact Stagger* to take effect, the spell projectile must hit
    the target, not just the explosion alone."
  - *Skyrim:Traps* (Flammable Gas, Oil Slicks): a gas trap's explosion
    "will also alert nearby enemies"; a follower hurt by it may turn
    hostile.
  - *Skyrim:Fireball*: "A fiery explosion for 40 points of damage in a 15
    foot radius" (the spell's area; its explosion's radius is 320 units).
  - *Skyrim:Chain Lightning*: "Lightning bolt that does 40 points of shock
    damage ... then leaps to a new target."
- **Game settings** (Skyrim.esm): `fExplosionForceMultLinear` 1,
  `fExplosionForceMultAngular` 1, `fExplosionSourceRefMult` 3,
  `fExplosionSplashRadius` 250, `fExplosionWaterRadiusRatio` 0.1,
  `sExplosionSplashParticles` "Effects\FXEmptyObject.nif",
  `fExplosionImageSpaceSwapPower` 0.75, `fKnockdownCurrentHealthThreshold`
  50, `fCombatThreatProximityExplosionAvoidTime` 0. Their use is not
  documented.
- **Earlier engine observations**: `test-mod autotest4` saw a spell whose
  projectile explodes send two alike `OnHit`s 0.1 s apart, both with the
  spell as source and the projectile as projectile (`src/magic.rs`,
  `send_magic_hit`).

## The records (`vrm-tool explosion-census`, Skyrim.esm + DLC + CC masters)

280 `EXPL` records. 273 have the 52-byte `DATA`. The rest:

- 48 bytes (`TrapPoisonGasExplosion01`): read as the 52-byte layout its
  vertical offset mult would be `0x141` and its flags 2. Read without the
  vertical offset mult, its flags are `0x141` and its sound level 2, which
  matches every other `TrapPoisonGasExplosion0N` (flags `0x141`, level 2).
  So the 48-byte layout lacks the vertical offset mult.
- 44 and 40 bytes (`FireStormExplosion02`, `fakeForceBall1024`,
  `fakeForceBallTiny`, `MAGSpiderSpitExplosion`, `FXDragonFireExplosion`,
  `MQ104TestDragonDeathExplosion`): their first words (`0x00C80000`,
  `0x00800000`) aren't form ids, and flag-like values (`0x43`, `0x141`,
  `0x145`) sit at 0x14. An older layout whose order isn't known.

Fields in use: 86 have an enchantment (all with nonzero damage too, by
coincidence of the census: traps, spiders, bolts); placed objects are
hazards (`TrapOilHazard01`, `FireBarrierHazardDropCatapult`), activators
(markers scripts react to), NPCs (the exploding spiders' eggs and ash
guardians) and one explosion (`HavokRocksExplosion`); one spawns a
projectile (`ChainLightningMassExplosion` → `ShockBoltAimChain`, flags Chain).
Flags of the 273 full records: bit 0 (unnamed) on all of them, bit 6
"Ignore Image Space Swap" on 257, bit 4 "Ignore LOS Check" on 84, bit 8 "No
Controller Vibration" on 69, bit 1 "Always Uses World Orientation" on 55,
bit 5 "Push Explosion Source Ref Only" on 8, "Knock Down - Always" (bit 2)
on 7, "By Formula" (bit 3) on 7 (`ExplosionFireBoltExpert01`,
`AlduinFirestormImpactExplosion`, `fakeForceBall1024ByFormula`...), "Chain"
(bit 7) on 2. Commonest values: `0x41` 93, `0x51` 45, `0x141` 40, `0x43` 31.

Who uses them: 161 magic effects (self-area spells such as Fire Storm and
Battle Cry, shouts, the illusion mass spells), 96 projectiles (expert bolts,
runes, ballista and exploding crossbow bolts, the spiders' spit, the
catapult), 46 destruction stages (gas traps, urns, webs), one `DUAL`, and
441 script properties: scripts `PlaceAtMe` the explosion
(`magicPlaceExplosionScript`, `TrapOilPool.OnDestructionStageChanged`,
`TrapPoisonGas`, `defaultGhostScript` and the dragons' scripts).

Examples:

| Explosion | Model | Force | Damage | Radius | Flags | Enchantment |
| --- | --- | --- | --- | --- | --- | --- |
| `MG08FireBallExp01` | `FireBallExp01.nif` | 0 | 0 | 320 | 0x41 | |
| `TrapExplosiveGasExplosion01` | `FXGasTrapBlast.nif` | 240 | 5 | 500 | 0x41 | `TrapExplosiveGasEnch01` |
| `TrapBallistaBoltExplosion06` | `BowTrapBoltExplosion.nif` | 20 | 250 | 320 | 0x11 | |
| `CWCatapultExp` | `CWCatapultExplosion.nif` | 240 | 30 | 320 | 0x41 | (places `FireBarrierHazardDropCatapult`) |
| `TrapPoisonGasExplosion06` | `FXEmptyObject.nif` | 0 | 0 | 50 | 0x141 | `TrapPoisonGasEnch06` |
| `ExplosionDwarvenSpider` | `ExplosionShock01.nif` | 128 | 1 | 300 | 0x43 | `EnchDwarvenSpiderShockDamage` |
| `ChainLightningExplosion` | `ImpactShock02Explo.nif` | 150 | 0 | 10 | 0x81 | (Chain Lightning's `MGEF`) |

Scripts treat an explosion's `OnHit` source as a weapon-like form:
`TrapExplosiveGas.onHit` ignores a repeated projectile, then explodes for a
weapon in its gas-weapon list, the torch, a projectile in
`flExplodesGasTrapOnHit` or a source with the flame keyword.

## What nobody says

How damage falls off with distance; whether armor, resistances or
difficulty reduce it; what "By Formula" computes; what force means in
impulse units and how the vertical offset mult shapes it; what the hit key
time is (the model's text keys?) and when the blast lands relative to the
model; what "Chain" and the spawn projectile do with targets; when the IS
radius applies the image space and how strongly; what "Ignore Image Space
Swap" swaps; what bit 0 means (set on almost all). These are the choices in
`known_ambiguities/explosions.md`, and the questions for `test-mod
explosions`.
