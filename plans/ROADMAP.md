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
- Behaviour graph runtime (`havok::behavior::runtime`): each humanoid runs its character
  project's graphs: state machines with event, wildcard, promoted global wildcard and
  condition transitions (expression conditions), cross-fades, clips (crop, start time,
  backwards, clip-local triggers), parametric / cyclic / synchronised blends with per-bone
  weights, additive (offset) clips, selectors following variables, references, control
  modifiers (expressions, every-N-events, timers, event-driven, on-deactivate, is-active).
  The AI sends events (`moveStart`, furniture enter / exit, `IdleStop`...) and sets
  variables (`Speed`, `isInFurniture`, `IsNPC`, `weapAdj`...), follows the graph's root
  motion in furniture and takes anim objects and `IdleFurnitureExit` from its events;
  `PlayIdle` succeeds only when the graph takes the event. The static event -> clip
  resolver remains for planning (enter start pose, durations)
- Creature behaviour graphs: every actor runs the project its race names (project file ->
  character -> root graph, `hkbProjectStringData` / `hkbCharacterStringData`): chickens,
  dogs, wolves, horses, cows, deer... idle in their layered idles and walk through their
  locomotion states; root motion by clip generator name from each project's animation
  data; AI walk speed measured from the graph's own walk
- Locomotion through the graph: `BSSpeedSamplerModifier` (`SpeedSampled` drives the speed
  blends), `hkbDampingModifier`, synchronised blends timed by their clips' playback speed
  and nested blends; state machine start modes (sync variables, so sneaking / sprinting
  survive re-entry; random start states). Movement types (`MOVT`) found through the
  graph's `iState_<name>` variables give walk / run speeds and turn rates. Packages'
  preferred speed (walk, jog, run, fast walk) and "Always Sneak" (`PKDT`): NPCs run and
  sneak through their graphs (`SneakStart` / `SneakStop`, `iState`). `TurnDelta` from
  the AI's turning: lean blends while walking, turn-in-place loops (`turnLeft` /
  `turnRight` / `turnStop`) when standing
- Creature idles: creatures standing about pick from the `ActionIdle` action tree's branches
  for their own graphs (IDLE `DNAM`): howling, foraging, lying down, looking about, feeding;
  held and ended (`IdleStop`) like humanoid idles. Humanoids standing about pick their
  idle styles from the same tree (`AnimationDrivenIdleRoot`: hands on hips, hands down,
  motion-driven). The event -> clip resolver and furniture
  planning run on each actor's own project
- Head tracking: humanoids' `BSLookAtModifier` (spine, neck, head and eye bones, each
  within its limit, eased by the graph's gains, off past the overall limit) turns NPCs
  towards the player close by and while talking (`bHeadTracking`, `LookAtOutOfRange`)
- Foot placement: the character's `hkbFootIkDriverInfo` (humanoids, horses): rays through
  the static world under each ankle, the body dropped to the lowest foot's ground, two-bone
  IK raising the others within their knee limits, planted feet tilted with the slope
- Talking gestures: each line of dialogue picks from the `ActionTalking` tree (hands on hips,
  hand gestures, angry / happy / expressive idles) by the line's emotion (`TRDT`) and the
  speaker's graph variables (`GetGraphVariableInt` / `Float` now read the actor's graph);
  `IsTalking`, `GetDialogueEmotion(Value)`, `GetMovementSpeed`
- Reversed clips (negative clip generator speed, root motion seen from the clip's end);
  loops that lead on by their own trigger (sitting variants play once and return through
  their reversed enter clip); `PlayIdle` on a seated actor plays in the seat
- Transition timing: trigger intervals (events outside them are ignored), initiate
  intervals (a transition triggered early waits for its window: woodcutters finish the
  swing, talking gestures and standing idles leave at their authored moment), both
  bounded by events or seconds in the state; uninterruptible transitions hold the
  machine until their blend ends. The AI's furniture exits wait for a graph that is
  waiting; looping exit clips that leave by their own `IdleFurnitureExit` count as
  one-shot
- Eating and drinking: occasional for sandboxing actors (every idle for Eat packages);
  standing meals from `EatingRoot` / `DrinkingRoot`, put away through `AnimObjectIdleStop`
- Leveled lists (LVLN / LVLI): each reference picks among the entries eligible at the
  player's level (all levels up to it, or the highest reached), deterministically by
  reference; chance of none; "use all" item lists. Templated NPCs take their name from
  the template (Base Data)
- Inventories: NPCs' items (`CNTO`, leveled item counts, "each item in count"), outfits
  and containers' contents, kept by reference; `GetItemCount`, `GetEquipped`,
  `GetEquippedItemType`, Papyrus `GetItemCount` / `AddItem` / `RemoveItem` /
  `RemoveAllItems` / `IsEquipped`. NPCs wield their best weapon (melee unless their
  combat style favours ranged; skill-weighted damage) and a shield; weapons hang
  sheathed from the bone their model names (`Prn`: hip, back), bows posed from their
  own string bones, shields on the forearm; the graph's `iLeftHandType` /
  `iRightHandType` follow. Torches after dark: actors outdoors between dusk and dawn
  light one they carry (in place of the shield, put away to use furniture), the graph
  raises it and its flame lights what is around (moving lights)
- Player items: activating an item picks it up (`XCNT` counts; gone from the world for
  good), containers open a take / store window beside the inventory (Tab); console
  `[ref.]additem`, `removeitem`, `showinventory`, `openactorcontainer`, `activate`.
  Books open to be read (their markup reduced to paragraphs and pages, illuminated
  letters kept), taken unless they can't be; books in the inventory are read by clicking
- Clip annotations: the triggers each project's animation data lists for its clip
  generators (`animationdata/<project>.txt`: hit frames, weapon draw / sheathe, footsteps,
  `SoundPlay.<sound>`) play as clip triggers; `SoundPlay` / `NPCSoundPlay` sounds play at
  the actor. Weapons drawn and sheathed through the graph (`WeapEquip` / `Unequip`): the
  weapon goes to the hand at `weaponDraw` (bows to the left), its scabbard stays; console
  `<ref>.drawweapon` / `sheatheweapon`, Papyrus `DrawWeapon` / `SheatheWeapon` /
  `IsWeaponDrawn`
- Barks: NPCs greet the player as they pass (`HELO` topics, once in a while each) and
  chatter now and then (`IDLE` topics, their own chances deciding), voiced where they
  stand with a subtitle, gesturing and looking at the player; one speaker at a time.
  `GetSitting` / `GetSleeping` / `IsMoving` answer from what actors are doing outside
  idle picking; console `bark <ref> <subtype>`; vrm-tool `dial-subtypes`
- Combat (first pass): health from race and NPC stats; hostility from faction relations
  (`XNAM` combat reactions) and aggression (`AIDT`), enemies noticed within sight
  range; fighters draw their weapons (creatures take their combat stance), run in
  along the navmesh, face and attack with their race's attack data (`ATKD` / `ATKE`,
  by chance; situational ones left out), the hit landing at the graph's `HitFrame`
  within reach and strike angle for weapon (or unarmed) damage times the attack's
  multiplier; targets flinch (`recoilStart`) or stagger and fight back; death drops
  the ragdoll. The player has health (HUD bars) and swings at what they look at
  (click). Console `startcombat <a> <b|player>`
- Bleedout: essential actors (and protected ones, except to the player) brought to zero
  health bleed out through the graph (`bleedOutStart` / `bleedOutStop`), dropped as
  targets, and get up after a while with a quarter of their health; the player can
  finish off a protected one that's down. Seated actors drop out of the furniture
  in front of it. Papyrus `Kill()`
  bleeds them out (`KillEssential()` doesn't), `IsBleedingOut`; console `damage <ref> <n>`
- Armor ratings: worn armor and shields rate their base times the wearer's light / heavy
  armor skill (`ceil(base x (1 + k x skill / 100))`, k 1.5 for NPCs, 0.4 for the player,
  from `fArmorRatingMax` / `fArmorRatingPCMax`); blows lose 0.12% per point plus 3% per
  piece, at most 80% (`fArmorScalingFactor`, `fArmorBaseFactor`, `fMaxArmorRating`).
  Console `player.equipitem` / `unequipitem <armor>`; `cstats` shows armor
- Blocking: fighters raise their guard (`blockStart`, retried while the graph is busy)
  against a swing started at them, or now and then while waiting to strike, as
  defensive as their combat style (`CSGD`; a quarter as often without a shield,
  `fCombatBlockChanceWeaponMult`). Blows from ahead lose 45% + 0.2% per shield rating
  point (shield) or 30% + 0.2% per point of the attacker's weapon damage (weapon), x
  (1 + 1.5 x block skill / 100), x 0.66 for power attacks, at most 85%, after armor;
  `blockHitStart` instead of a flinch; power attacks break the guard. The player
  blocks with the right mouse button. Console `guard <ref> <secs>`, `pblock`
- Power attacks: whether to power attack first (combat style offensiveness, `CSME`
  power-attack-blocking multiplier against a raised guard), then which attack by
  chance; longer recovery after one. Stagger-capable hits stagger at
  `iStaggerAttackChance`. Attacks the graph refuses (mid-flinch, or a stagger cut the
  draw short) are retried and the weapon drawn again
- Stamina: from race and NPC stats, back at the race's rate (percent of the most a
  second; x0.35 in combat, `fCombatStaminaRegenRateMult`) a moment after it was spent.
  Power attacks cost `fStaminaAttackWeaponBase` + weapon weight x
  `fStaminaAttackWeaponMult`, times the attack's own multiplier (`ATKD`); NPCs short
  of it attack normally. Blocked blows cost `fStaminaBlockBase` + 0.25 x the damage
  stopped; the player's sprinting drains it (`fSprintStaminaDrainMult`, armor weight)
  and stops when it runs out. The player power attacks by holding the attack button
  (a basic swing without the stamina); HUD bar. Papyrus `GetActorValue` /
  `DamageActorValue` read and spend live health / stamina. Console `stamina`,
  `pattack`; vrm-tool `gmst <pattern>`
- Bashing: the race's bash attacks (`bashStart`, `bashPowerStart`) with a shield or a
  melee weapon out, within `fCombatBashReach`, for `fStaminaBashBase` /
  `fStaminaPowerBashBase` stamina. A bash isn't blocked: it breaks the guard,
  staggers and cuts short the target's swing; it strikes for the shield's rating
  (or weapon's damage) x a share growing with block skill (`fShieldBashMin` / `Max`,
  `fWeaponBashMax`) x the attack's multiplier. Fighters bash a raised guard and,
  from their own guard, the swings coming at them, as the combat style's bash
  multipliers (`CSME`) have them. The player bashes by attacking with the guard up.
  Console `pbash`; `cstats` shows the bash multipliers
- Bows: archers (NPCs wielding a bow) close in until within range with a clear line,
  then stand, draw through the graph (`bowAttackStart`), hold to aim and loose
  (`attackRelease`); the arrow flies at the clip's `arrowRelease`, aimed over its drop
  at the target's body within `fBowNPCSpreadAngle`. Arrows are their ammo's projectile
  (`PROJ` speed, gravity, flight model without its tracer), swept through the physics
  world and the player's capsule: hits are blows of bow + arrow damage (shields block
  them), a third end up in the target's inventory (`iArrowInventoryChance`), misses
  stick where they land a minute. NPCs shoot their best arrows and never run out.
  The player wields a bow (`player.equipitem`), draws holding the attack button and
  looses on release (weaker and slower before full draw, nothing before the nock),
  using up arrows. Console `pshoot [secs]`, `tcam`
- Death: ragdolls from the skeleton's rigid bodies and constraints (capsules, cone /
  twist / plane and hinge limits as rapier joints); bodies can be searched;
  `GetDead`, `IsDead`, `Kill()`, `OnDying` / `OnDeath`; console `kill`
- Synchronised blends of single-play clips (directional attacks) end instead of
  cycling, and their heaviest child raises the clip triggers
- Character property bindings: bone switches bound to the character's bone weight
  properties (`LeftArm`, `ShieldOnly`...: `hkbCharacterData` values) layer as authored
- Patrol (linked-ref routes with idle markers on the way, repeat / start at nearest) and
  follow / escort packages; SitTarget
- Actor avoidance: walkers keep clear of other actors and the player, sidestep or queue
  when blocked, and stop short of a destination someone is standing on
- Anim objects: `AnimObjDraw` events (state enter events and clip triggers with ANIO
  payloads) put brooms, tankards, hammers, lutes, books... in actors' hands, hung from the
  bone named by the model's `Prn`; put away when the actor leaves the idle / furniture
- Sun shadows outdoors: four cascades fitted to the view (texel-snapped, far ones
  refreshed every 2nd / 4th frame), cast by objects (alpha-tested ones through their
  cutouts), actors, their weapons and the landscape; 3x3 PCF with normal offset.
  Console `tsh`, `VRM_NO_SHADOWS`
- Footstep sounds: the graph's footstep events (`FootLeft` / `FootRight`, creatures'
  `FootFront` / `FootBack`...) through the footstep set of what the actor wears on its
  feet (ARMA `SNDD`, else its skin's) for its gait (walk, run, sneak), the impact
  data set's sound for the ground below: the collision's Havok material per triangle
  (material types matched by the CRC-32 of their name, parents for missing pairs)
  or the landscape texture showing most there (LTEX material). The player steps by
  stride. Console `probe` shows materials; vrm-tool `nif-materials`
- Keyframe animation: NiControllerManager / NiControllerSequence / NiTransformInterpolator
  / NiTransformData; animated nodes drawn as separate parts; doors open and close (player
  activation, NPCs walking through) with their leaves' collision; statics loop "Idle"

## Next
1. **Animation: behaviour graphs at runtime** (humanoids run their graphs; see Done)
   - Pose modifiers: twist, keyframe bones; foot IK gains from `hkbFootIkControlsModifier`,
     locking planted feet; creatures' look-at modifiers (unbound:
     the game picks and aims them itself); NPCs looking at each other in conversation
   - Chooser start states, state machine `currentStateId` outputs, selectors' own blends
   - `Direction` (strafing), sprinting; character properties other than bone weights;
     `hkbRotateCharacterModifier`; the other action trees (`ActionTurnLeft`...) as the
     way events are chosen
   - Carry furniture (`CarryFurnitureScript`: wood piles): carrying the load away
     (`OffsetCarryLogStart`) and putting it down by inventory (`GetItemCount`); for now
     the graph is reset (`IdleForceDefaultState`) after the pick-up
   - Delayed state changes (`FLAG_DELAY_STATE_CHANGE`), blending effects' event /
     self-transition modes (`vrm-tool hkb-flags` counts and lists them)
   - Engine variables and events still missing: combat, first person;
     INFO speaker / listener idles; listeners' reactions
   - Footsteps: water (wading, swimming), jump / land events, the player's sneaking
   - Later / separate: NiTransformController (non-sequence), texture / material controllers
2. **AI depth**
   - Remaining procedures: flee, force greet, guard, use weapon / magic, dialogue packages;
     escort waits for its target
   - Off-screen travel between worldspaces; locked doors and keys
   - Templates (TPLT) beyond traits, inventory and name: stats, factions, spells, AI
     data, keywords, scripts; leveled list counts ("each item in count")
3. **Game logic**: combat (crossbows and bolts, arrows in hand while drawing and
   stuck in actors, archers keeping their distance, sneak shots, power bashes for the player (perk), armor perks, tempering
   and enchantments, crime and assault, killmoves, the player's own weapon and
   animations), magic, inventory (player's, equipping by hand, armor from
   inventory, ammo / quivers; torches in dark interiors, burning out), leveling, crime
4. **UI**: inventory (categories, equipping, item details), map, bars
5. **Audio**: lip sync
6. **Rendering**: point light and interior shadows, static shadow caching (per-cell
   caster batches), HDR/image spaces, particles, distant LOD (BTR/BTO/trees),
   grass, decals, environment maps
7. **Saves**: an engine-native save format (reading .ess later)
8. **Performance**: async loading, GPU-driven culling
