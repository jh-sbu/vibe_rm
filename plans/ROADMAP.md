# Roadmap

The goal is parity with the Skyrim SE runtime for content built with the Creation Kit:
the original data files should play as designed. Work is ordered by how much of the
game each piece unlocks.

## Done
- BSA, plugins (load order, ESL, overrides, strings), NIF (render + collision blocks), DDS
- Interior/exterior cells, landscape, water, weather/climate/sky, time of day
- Havok collision shapes -> rapier3d; first-person character controller
- Exterior cell streaming, load doors
- Actors: skinning, GPU skinning, NPC assembly (race, outfit, FaceGen); Havok clip playback
- Papyrus VM (PEX, natives, events, VMAD), conditions (CTDA), quest stages, dialogue
- egui HUD and console; audio (WAV/xWMA/FUZ), music, ambient sounds, voice
- AI: navmeshes (NVNM), A* + funnel pathfinding across cells, package selection
  (schedule + conditions), sandbox / travel behaviour, idle/walk cross-fade,
  persistent NPC whereabouts with leaving / arriving through load doors

## Next
1. **AI depth**
   - Furniture and idle markers (sit, sleep, eat, work idles), behaviour-graph subset
   - Remaining procedures: follow, escort, patrol (linked refs), flee, dialogue packages
   - Actor-actor avoidance; doors opening; off-screen travel between worldspaces
   - Leveled lists (LVLN/LVLI) beyond the first entry, templates (TPLT/ACFG)
2. **Animation**: NiControllerSequence for animated statics (water wheels, flags);
   root motion from behaviour data
3. **Game logic**: combat, magic, inventory, leveling, crime
4. **UI**: inventory, map, bars
5. **Audio**: lip sync
6. **Rendering**: shadows, HDR/image spaces, particles, distant LOD (BTR/BTO/trees),
   grass, decals, environment maps
7. **Saves**: an engine-native save format (reading .ess later)
8. **Performance**: async loading, GPU-driven culling
