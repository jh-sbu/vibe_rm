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
- AI furniture: chairs, benches, stools, beds, bedrolls and wall-lean markers from NIF
  furniture markers; ownership; reservations; sit / sleep packages and sandbox
  "Allow ..." inputs; enter / exit clips placed by root motion from the behaviour
  projects' animation data
- Behaviour graphs: hkbBehaviorGraph generator trees (state machines, nested state
  targets, clip triggers, references between graphs) to resolve animation events to
  clip sequences and their exits; IDLE tree by keyword for crafting stations and
  special furniture; idle markers; Papyrus `PlayIdle` / `SendAnimationEvent`
- Patrol (linked-ref routes with idle markers on the way, repeat / start at nearest) and
  follow / escort packages; SitTarget
- Actor avoidance: walkers keep clear of other actors and the player, sidestep or queue
  when blocked, and stop short of a destination someone is standing on

## Next
1. **AI depth**
   - Anim objects (mugs, bowls, hammers) for table / counter / work idles; chair idle
     variants; child furniture clips; IDLE conditions (currently ignored); chairs and
     beds still pick clips by name rather than through the graphs
   - Remaining procedures: flee, force greet, guard, use weapon / magic, dialogue packages;
     escort waits for its target
   - Doors opening (needs NiControllerSequence); off-screen travel between worldspaces
   - Leveled lists (LVLN/LVLI) beyond the first entry, templates (TPLT/ACFG)
2. **Animation**: NiControllerSequence for animated statics (water wheels, flags);
   root motion for locomotion (walk speeds and turns from the motion data)
3. **Game logic**: combat, magic, inventory, leveling, crime
4. **UI**: inventory, map, bars
5. **Audio**: lip sync
6. **Rendering**: shadows, HDR/image spaces, particles, distant LOD (BTR/BTO/trees),
   grass, decals, environment maps
7. **Saves**: an engine-native save format (reading .ess later)
8. **Performance**: async loading, GPU-driven culling
