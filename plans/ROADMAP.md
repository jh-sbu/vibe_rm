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
  clip sequences and their exits, start states bound to graph variables (`i1stPerson`...);
  idle markers; Papyrus `PlayIdle` / `SendAnimationEvent`
- IDLE tree: walked in authored sibling order with conditions (furniture anim type, entry
  side, sitting state, child, furniture keywords...) to pick every furniture marker's enter
  / exit events from `ActivateRootChar` (chairs, stools, tables, beds, bedrolls, leans,
  crafting stations, child chairs) and seated idles from `NonCombatIdles` (eating and
  drinking with bread / tankards, table drinking, sitting variants)
- Reversed clips (negative clip generator speed, root motion seen from the clip's end);
  loops that lead on by their own trigger (sitting variants play once and return through
  their reversed enter clip); `PlayIdle` on a seated actor plays in the seat
- Eating and drinking: occasional for sandboxing actors (every idle for Eat packages);
  standing meals from `EatingRoot` / `DrinkingRoot`, put away through `AnimObjectIdleStop`
- Patrol (linked-ref routes with idle markers on the way, repeat / start at nearest) and
  follow / escort packages; SitTarget
- Actor avoidance: walkers keep clear of other actors and the player, sidestep or queue
  when blocked, and stop short of a destination someone is standing on
- Anim objects: `AnimObjDraw` events (state enter events and clip triggers with ANIO
  payloads) put brooms, tankards, hammers, lutes, books... in actors' hands, hung from the
  bone named by the model's `Prn`; put away when the actor leaves the idle / furniture
- Keyframe animation: NiControllerManager / NiControllerSequence / NiTransformInterpolator
  / NiTransformData; animated nodes drawn as separate parts; doors open and close (player
  activation, NPCs walking through) with their leaves' collision; statics loop "Idle"

## Next
1. **Animation: behaviour graphs at runtime.** Each actor runs its behaviour graph the
   way the game does, instead of the engine resolving events to fixed enter / loop / exit
   clip lists and driving them from AI code. This keeps the engine generic over animation
   content (anything authored as behaviour graphs + animation data, including FNIS /
   Nemesis output, plays through `SendAnimationEvent` / IDLE records).
   - Graph instance per actor: variables (with defaults), event queue, active state per
     state machine, transitions (event / wildcard / nested, transition blend times),
     clip generators (modes, speed incl. reverse, crop, triggers), blenders weighted by
     bound variables, selectors, behaviour references, modifier generators (pass-through
     first)
   - Events back to the engine from triggers and state enter / exit events
     (`AnimObjDraw`, `SoundPlay`, `HeadTrackingOn`...), root motion from the active clips
   - Engine drives it like the game: events (`IdleStop`, `IdleChairExitStart`,
     `moveStart`...) and variables (`Speed`, `Direction`, `iSyncIdleLocomotion`...)
   - Replace the event -> clip-list lookup, the furniture / sub-idle clip sequencing and
     the walk / idle clips chosen by file name
   - Then: creature behaviour projects; turning / run through the graph
   - Later / separate: NiTransformController (non-sequence), texture / material controllers
2. **AI depth**
   - Remaining procedures: flee, force greet, guard, use weapon / magic, dialogue packages;
     escort waits for its target
   - Off-screen travel between worldspaces; locked doors and keys
   - Leveled lists (LVLN/LVLI) beyond the first entry, templates (TPLT/ACFG)
3. **Game logic**: combat, magic, inventory, leveling, crime
4. **UI**: inventory, map, bars
5. **Audio**: lip sync
6. **Rendering**: shadows, HDR/image spaces, particles, distant LOD (BTR/BTO/trees),
   grass, decals, environment maps
7. **Saves**: an engine-native save format (reading .ess later)
8. **Performance**: async loading, GPU-driven culling
