# Roadmap

The goal is parity with the Skyrim SE runtime for content built with the Creation Kit:
the original data files should play as designed. Work is ordered by how much of the
game each piece unlocks.

## Done
- BSA, plugins (load order, ESL, overrides, strings), NIF (render + collision blocks), DDS
- Interior/exterior cells, landscape, water, weather/climate/sky, time of day
- Havok collision shapes -> rapier3d; first-person character controller
- Exterior cell streaming, load doors

## Next
1. **Actors**
   - Skin blocks (NiSkinInstance, BSDismemberSkinInstance, NiSkinData, NiSkinPartition)
   - GPU skinning; skeleton.nif; bind pose
   - NPC assembly: RACE skeleton/skin, OTFT/ARMO/ARMA equipment, FaceGen head + tint
   - Leveled lists (LVLN/LVLI), templates (TPLT/ACFG)
2. **Animation**
   - NiControllerSequence / interpolators for animated statics (water wheels, flags)
   - Havok packfile reader (hkx 2010, 64-bit), hkaSplineCompressedAnimation decoding
   - Behavior graph subset: idle / walk / run / turn selection
3. **Scripting**: Papyrus `.pex` loader + VM, native function library, events
   (OnInit, OnActivate, OnTriggerEnter, ...), properties filled from VMAD
4. **Game logic**: conditions (CTDA), quests and stages, dialogue (DIAL/INFO),
   AI packages, navmesh pathfinding, combat, magic, inventory, leveling
5. **UI**: HUD (compass, bars), activation prompt, dialogue, inventory, console
6. **Audio**: SNDR/SOUN, music types, xWMA/FUZ decoding, voice + lip sync
7. **Rendering**: shadows, HDR/image spaces, particles, distant LOD (BTR/BTO/trees),
   grass, decals, environment maps
8. **Saves**: an engine-native save format (reading .ess later)
9. **Performance**: async loading, GPU-driven culling
