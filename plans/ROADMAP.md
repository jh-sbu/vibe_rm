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
- Fleeing by threat ratio: each fighter's combat strength (damage a second x
  health over what its armor lets through) refreshed every
  `fCombatStrengthUpdateTime`; its side's strength over the other side's (sides
  linked through who fights whom, the player among them) checked every
  `fCombatThreatRatioUpdateTime` against its confidence (`fConfidence<Level>`) plus a
  modifier rolled per fight (`fCombatConfidenceModifierMin` / `Max`); only the hurt
  (and cowards) flee. Fleeing actors say a `FLEE` line, run to the navmesh places
  furthest from the threat out to `fCombatFleeDistance*`, wait there and leave the
  fight after `fFleeIsSafeTimer`; hurt or approached they check again, and turn back
  when the ratio recovers; cornered, they fight. Cowards don't draw. NPCs' health
  comes back out of combat. `GetThreatRatio`, `IsFleeing`; `cstats` shows strength
  and confidence. Open questions: `known_gaps/combat-fleeing.md`
- Assistance: actors not fighting join a fight they see where an ally (same or
  allied faction) or, for those who help friends too, a friend is fighting someone
  they aren't friendly with, as their AI data's assistance has it; the player
  fights whoever fights them, so attacking one of the townsfolk sets their
  neighbours on the player. Negative faction ranks aren't membership. `cstats`
  shows assistance. Open questions: `known_gaps/combat-assistance.md`
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
  then stand (backing off between shots from a target close in, as readily as
  their combat style's fallback multiplier, `CSCR`), draw through the graph
  (`bowAttackStart`), hold to aim and loose
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
- Combat state for scripts: fighting, or searching for a target it lost
  (`GetCombatState`; the player is in combat while anyone fights them). Papyrus
  `StartCombat`, `StopCombat` (searchers give up too), `IsInCombat`,
  `GetCombatState`, `GetCombatTarget`, and the events `OnCombatStateChanged`
  (on each change of state), `OnHit` (aggressor, weapon, the arrow's projectile,
  power attack, bash, blocked) and `OnEnterBleedout`. Conditions `IsInCombat`,
  `IsWeaponOut`, `IsBleedingOut`; Papyrus `IsInFaction`, `GetFactionRank`,
  `IsGuard`. Console `cgf <Class.Func> [@self] [args]` calls a native; vrm-tool
  `script-users` finds what attaches a script. Open questions:
  `known_gaps/combat-state.md`
- Player controls from scripts: `DisablePlayerControls` / `EnablePlayerControls`
  (Papyrus's defaults for missing arguments) stop the player moving, fighting
  (attacks, bashes, blocking, shooting), looking, sneaking, opening the inventory
  and activating; the `Is...ControlsEnabled` queries. Console `epc`. Open
  questions: `known_gaps/player-controls.md`
- Loose objects: references whose model has one simulated rigid body
  (`MO_SYS_DYNAMIC`: clutter, food, weapons, baskets) are rapier bodies with the
  NIF's mass, friction and restitution (meshes as their convex hull), lying still
  where placed until disturbed; models of several bodies are joined by their
  constraints (hand carts roll on hinged wheels, signs swing on chains, trip
  wires and bone alarms hang from rope links, beehives from a spring that
  breaks). Every NIF constraint type the vanilla meshes use is read, motors
  included (none are switched on). vrm-tool `model-users`. The player pushes them aside, actors walking into
  them wake them, arrows knock them away (and send `OnHit` to whatever they
  strike); where they come to rest is kept across loads. Papyrus
  `ApplyHavokImpulse`, `SetMotionType` (the weapon racks' scripts hold their
  weapons keyframed), `DropObject`. Objects made in the world (`PlaceAtMe`,
  dropped items) are drawn, fall, have their base's scripts and can be picked up;
  the inventory drops items on right click. Console `loose`, `[ref.]drop`, `pwalk`.
  Open questions: `known_gaps/loose-objects.md`
- Synchronised blends of single-play clips (directional attacks) end instead of
  cycling, and their heaviest child raises the clip triggers
- Character property bindings: bone switches bound to the character's bone weight
  properties (`LeftArm`, `ShieldOnly`...: `hkbCharacterData` values) layer as authored
- Patrol (linked-ref routes with idle markers on the way, repeat / start at nearest),
  follow and escort packages (leading the target to the destination, waiting while it
  lags more than "Distance to Wait for Follower(s)" behind, turning in place to face
  it, running to catch up with one more than "Run If Behind Distance" ahead);
  followers in range face their target; SitTarget. Open questions:
  `known_gaps/escort.md`
- Quest alias packages (`ALPC`): actors filling a running quest's reference
  aliases run those aliases' packages ahead of their own, higher priority quests
  first; package conditions run in the owner quest (`QNAM`) and alias locations /
  targets resolve through its fills; whereabouts follow them (Lucan and Camilla's
  MS13 opening, Balgruuf holding court until MQ103 moves on). vrm-tool
  `alias-packages`. Open questions: `known_gaps/alias-packages.md`
- Quest alias fills: find matching reference (persistent or loaded references by
  the alias's conditions; closest, reuse, reserved, dead / disabled flags),
  location alias references by location ref type (`XLRT`, the locations' static
  reference lists), other quests' aliases, and location aliases (specific, a
  reference alias's location by keyword, by conditions), filled in record order;
  quests whose required aliases can't be filled don't start. Near alias fills
  (the nearest matching reference about another alias's: roadside encounters'
  travel markers by their trigger). Conditions whose parameters are aliases
  ("use aliases"). Editor and current
  locations for conditions (`GetIsEditorLocAlias`, `GetInCurrentLocAlias`,
  `HasRefType`, `HasSameEditorLocAsRefAlias`, `LocAliasIsLocation`...), packages
  at location aliases, Papyrus `LocationAlias`. vrm-tool `alias-fills`,
  `ref-types`. Open questions: `known_gaps/alias-fills.md`
- Created references: aliases that create theirs (`ALCO`, at or in another
  alias's reference) and Papyrus `PlaceAtMe` / `PlaceActorAtMe` make references
  (`FF000800` up): items in containers go to the inventory, actors appear where
  they were made and live among the persistent actors (packages, whereabouts,
  combat). Console `placeatme`
- Force greet packages (ForceGreet / ForceGreetFromSitting templates): the greeter
  waits at its wait location (standing, sandboxing or in its seat) until the player
  is in the trigger location (and seen, if asked), walks up to within the force
  greet distance at the package's speed and opens a conversation with the package's
  topic (or the first `HELO` line); seated ones speak from the seat. Again 10 s
  after the conversation if the package still applies. `GetTalkedToPC`. vrm-tool
  `force-greets`. Open questions: `known_gaps/force-greet.md`
- NPC templates (`TPLT`): each "Use ..." flag takes that part (traits, stats,
  factions, AI data and combat style, AI packages, base data, inventory, script,
  attack data and attack race, keywords) from the template, through nested templates
  and leveled NPC lists picked per reference as its looks are, so a leveled draugr's
  health, factions and packages are the ones of the draugr it looks like. Actors'
  `HasKeyword` includes their race's; `GetInFaction` / `GetFactionRank` and door
  ownership follow templates. Console `templates <ref>`; vrm-tool `npc-templates`.
  Open questions: `known_gaps/npc-templates.md`
- Actor values: one store per actor that conditions (`GetActorValue`,
  `GetBaseActorValue`, `GetPermanentActorValue`, `GetActorValuePercent`), Papyrus
  (`Get` / `Set` / `Force` / `Mod` / `Damage` / `RestoreActorValue`,
  `GetActorValuePercentage`) and the console share: bases from the records (AI
  data, skills, health / magicka / stamina offsets, the race's starting values,
  rates, carry weight), a permanent modifier and damage over them. Live health and
  stamina are folded in (damaging health to nothing kills); changes to health,
  stamina, aggression, confidence, assistance and the armor and block skills
  reach combat. `Variable01`-`10` and `WaitingForPlayer` now gate packages and
  lines as scripts set them. Console `[ref.]getav`, `setav`, `modav`, `forceav`,
  `damageav`, `restoreav`; vrm-tool `av-conditions`. Open questions:
  `known_gaps/actor-values.md`
- Package procedure trees and default package lists: templates that pick one of
  several branches (`Stacked` roots: `DefaultMasterPackageTemplate`) run as one
  package per branch, the branch's conditions after the package's own, its main
  procedure's inputs taken by position (sandbox allowances, patrol radius /
  repeat, follow radii); tree conditions on package inputs (run-on package data,
  inputs as parameters), linked references, `IsActor`, `HasLinkedRef`,
  `IsLinkedTo`. NPCs' default package lists (`DPLT`, through templates) follow
  their own packages, so draugr, guards, predators and the 2600 NPCs on
  `DefaultMasterPackageList` patrol from their linked markers, follow linked
  actors or sandbox where they stood instead of standing still. vrm-tool
  `pack-tree`, `pack-lists`. Open questions: `known_gaps/package-trees.md`
- Package procedures: packages with their own procedure tree (draugr ambushes
  in sarcophagi, thrones and alcoves: creatures use furniture through their own
  graphs' `ActionActivate` branch, starting in it and getting out when the
  ambush ends), procedures by tree for unknown templates (HoldPosition,
  GuardPost), and UseWeapon practice: archers take a bow of the package's
  weapon type, draw and loose at the targets in barrages, melee fighters swing.
  Open questions: `known_gaps/package-trees.md`
- Enable state: references follow their enable parent (`XESP`, or its opposite)
  unless scripts set them; enabling or disabling a parent shows or hides its
  children (cells, sounds, furniture, actors). Actors flagged "Starts Dead" are
  placed as corpses (C04's Silver Hand bodies, treasure-room skeletons)
- World state across cell loads (in memory, `src/world_state.rs`; what saves
  will write out): references disabled when their cell loads are built hidden
  (collision, lights, load doors, furniture, looping sounds follow), so enabling
  them or their enable parent in place shows them, actors included; dead actors
  stay dead, their bodies where and as they lay (`IsDead` / `GetDead` unloaded
  too); actors that leave hurt come back as hurt, less what they'd have healed;
  doors the player opened stay open; references scripts move (`MoveTo`,
  `SetPosition`, `SetAngle`, `MoveToMyEditorLocation`, `GetAngleX/Y/Z`) move
  with their collision and lights, between cells too, and stay moved. Console
  `[ref.]enable` / `disable`, `[ref.]moveto`. Open questions:
  `known_gaps/world-state.md`
- Scenes (`SCEN`): started by Papyrus (`Start`, `ForceStart`, `Stop`,
  `IsPlaying`, `IsActionComplete`) or with their quest, phases played in order
  (skipped when their start conditions fail, ended by completion conditions or
  their actions): dialogue actions say their topic's line where the speaker
  stands (voiced, subtitled, gesturing, looping ones again and again), head
  tracking and facing whom the action names; package actions run their
  packages ahead of all others until done (arrived, seated, force greet held);
  timers. Phase and scene fragments, actors' death / combat / dialogue pause
  and end flags, "no player activation", "stop quest on end".
  `IsSceneActionComplete`, `IsScenePlaying`, `IsInScene`,
  `IsScenePackageRunning`; `GetDistance` between interiors / worldspaces is
  far. Console `startscene`, `stopscene`, `scenes`, `startquest`, `stopquest`,
  `setstage`; vrm-tool `scenes`. Open questions: `known_gaps/scenes.md`
- Story Manager: events run down the tree of event, branch and quest nodes
  (`SMEN` / `SMBN` / `SMQN`) under their type, stacked or random, node
  conditions on the event's data, quests started with reset hours, max running,
  num to run, do all before repeating and shared events. Events: NPCs striking
  up conversations (`ADIA`: every town's conversation quests and their scenes),
  the player changing location (`CLOC`), script events
  (`Keyword.SendStoryEvent`), kills (`KILL`). "From event" aliases,
  `GetEventData` and run-on-event-data conditions. Quests started from Papyrus
  get their scripts once the running scripts yield (they were lost before).
  More events: the player taking items (`AIPL`: picked up, from containers and
  bodies, stolen), NPCs greeting the player (`AHEL`), finding bodies (`DEAD`),
  assaults (`ASSU`, crime when the victim keeps the law), relationship rank
  changes (`CHRR`). Crime events: crime gold added (`ADCR`), the player
  giving themselves up to the guard arresting them (`ARRT`: Thieves Guild
  `TG00`'s arrest monitor), jailed (`JAIL`: `JailQuest`, the Windhelm jail
  fight), escaping (`ESJA`: "Retrieve your possessions", the achievement)
  and serving the sentence (`STIJ`), paying (`PFIN`). Quests started by an
  event get its Papyrus `OnStory...` event with the data. Quests' own event
  conditions. Relationships (`RELA`):
  `GetRelationshipRank` and highest / lowest, Papyrus get / set. Conditions
  `GetIsCurrentPackage`, `IsInList`, `DoesNotExist`, location keyword data.
  Console `storyevent` (with `v1=`, `f1=`...), `setrelationshiprank`;
  vrm-tool `story` (`CONDS=1`, `OWN=1`). Open questions:
  `known_gaps/story-manager.md`
- Ownership and crime: taking what a person owns is stealing; what a faction
  owns only past its favor cap (the lowest of its living members' who are
  friends or better with the player, `iFavor*Value`). The player's thefts,
  assaults and murders seen by actors with a crime faction (`CRIF`) add crime
  gold there, violent or not, from its crime values (`CRVA`, or the defaults:
  murder 1000, assault 40, half a stolen item's value), unless it ignores the
  crime against non-members. Kill events say whether a kill was a (reported)
  murder and the rank between them. Papyrus `Faction.GetCrimeGold`,
  `ModCrimeGold`, `SetCrimeGold(Violent)`, `PlayerPayCrimeGold`, infamy,
  `Actor.Get` / `SetCrimeFaction`; conditions `GetCrimeGold(Violent /
  Nonviolent)`, `GetIsCrimeFaction`, `CanPayCrimeGold`,
  `GetInSharedCrimeFaction`. Stolen goods stay marked as their owners' (red in
  the menus, the red "Steal" / "Steal from" prompts) and are confiscated into
  the faction's stolen goods chest on paying; `GetStolenItemValue(NoCrime)`.
  Picking a lock someone else owns is a crime when seen. Trespassing: in an
  off limits cell, or a home while its owner's package locks the doors (not
  public areas), whoever sees the player warns them with the Trespass topic
  (`GetTrespassWarningLevel`, `IsTrespassing`), warns again, then calls the
  guards and reports it (`fAITrespassWarningTimer`); off limits at once.
  Pickpocketing while sneaking: the victim's pockets (not what it wears)
  with each item's chance from the `fPickPocket*` settings, taken stolen;
  caught, a crime the victim reports and won't let the player try again.
  Console `pickpocket`; vrm-tool `trespass-cells`, `ctda-uses`,
  `topic-lines`. Arrests: guards (`IsGuard`: `IsGuardFaction`, the class
  flag) near a reported crime run after the player calling on them to halt
  and stop them with the arrest dialogue (blocking branches, `GetAlarmed`):
  pay (on the spot, or let out at the jail), go to jail, or resist; walking
  off is resisting. Factions' Arrest / Attack on Sight flags make guards
  attack at a bounty. Jail: the sentence by bounty, belongings and stolen
  goods taken, the jail outfit, served by sleeping in the cell bed.
  `SendPlayerToJail`, `SetPlayerResistingArrest`, `GetDaysInJail`,
  `GetArrestingActor`; console `alarm`, `jail`, `servetime`; vrm-tool
  `quest-topics`. Witnesses: killing the last one of a crime before the
  guards hear of it takes its bounty back ("Last witness killed."); one of
  them says the crime's line (`STEA` / `ASSA` / `MURD`, or the non-combat
  `STFN` / `ASNC` / `MUNC`), and wronged witnesses who don't report crimes
  attack if aggressive. Escaping jail (unlocking the cell, or leaving):
  the bounty stands again with the escape gold, the guards come.
  Console `crime`, `player.setcrimegold`,
  `player.paycrimegold`, `crimefaction`; vrm-tool `faction-owners`,
  `crime-factions`. Open questions: `known_gaps/crime.md`
- Trigger volumes: scripted box and sphere primitives (`XPRM`) send
  `OnTriggerEnter` / `OnTriggerLeave` as the player and actors step in and out
  (set-stage, start-scene, music and comment triggers...);
  `GetTriggerObjectCount`. Papyrus variables and locals start at their type's
  default (an `Int` at 0, not None). vrm-tool `triggers`. Open questions:
  `known_gaps/triggers.md`
- Detection: one detection value per observer and target from the CK wiki's
  formula and the game settings (`fSneakBaseValue`, `fSneakMaxDistance` and
  its exterior multiplier, distance attenuation; sound from movement, worn
  armor weight and running, muffled out of sight; sight within a view cone
  with a clear line, by the light level where the target stands: ambient,
  unshaded sun and point lights; the observer's Sneak against the sneaking
  target's). Sleepers see nothing. The player's stealth points drain and refill
  with the most any enemy detects them by; enemies attack once they are gone
  or past `iCombatStealthPointDetectionThreshold`. Combat noticing enemies,
  joining fights, crime witnesses and force greets' "player must be detected"
  use it. Conditions `GetDetected`, `IsSneaking`, `GetLightLevel`; Papyrus
  `IsDetectedBy`, `IsSneaking`, `GetLightLevel`; the HUD's sneak eye. Console
  `detect`. Detection states: enemies alert to someone they detect but don't
  find yet search where they were; fighters whose target goes undetected lose
  it and search where they last saw it (fights no longer end by distance);
  searchers attack on finding the target and give up once its stealth points
  are full, with the detection topics (`NormalToAlert`, `AlertIdle`,
  `CombatToLost`...). Actors have stealth points too. Voice types through
  templates (leveled actors' barks and `GetIsVoiceType`). Open questions:
  `known_gaps/detection.md`
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
  or the landscape texture showing most there (LTEX material), water for feet in
  shallow water (none deeper: swimming). The player steps by stride, sneaks (Ctrl:
  slower by the sneaking movement type, the view lowered, sneak footsteps; sprinting
  stands them up) and sounds their jumps and landings (`JumpUp` / `JumpDown`).
  Console `probe` shows materials and water; `psneak`, `pjump`; vrm-tool `nif-materials`
- Locks: doors and containers with a lock (`XLOC`) start locked; the player opens
  them with the key (a load door pair shares its lock; houses always let the
  player out) or pick them: a lock and pick view (mouse / A, D move the pick,
  Space turns the lock), the sweet spot and partial zones sized by lock level and
  skill (`fLockpickSkillSweetSpotMult`, `fPartialPick*`), picks wearing out under
  strain and breaking. Sleep packages with
  "Lock Doors?" lock the sleeper's home doors, packages unlocking doors at start /
  on change (`PKDT`) open them, so shops close at night. Papyrus `Lock`,
  `IsLocked`, `GetLockLevel`, `SetLockLevel`; conditions `GetLocked`,
  `GetLockLevel`; console `lock` / `unlock`, `picklock`; vrm-tool `locks`. NPCs
  open locked animated doors only with the key or owning them (cages and cells
  stay shut). Open questions:
  `known_gaps/locks.md`
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
   - Footsteps: swimming (and swimming itself), splashes
   - Later / separate: NiTransformController (non-sequence), texture / material controllers
2. **Story Manager** (core and fifteen events done; see Done)
   - Events waiting on their systems: crafting, level / skill increases, item
     removal (dropping now exists; the event's data needs a source), spell cast, shouts, bribe /
     intimidate / flatter; hellos between NPCs and creatures'
   - Story Manager state and relationship ranks in saves
3. **Detection (stealth)** (see Done); what's left waits on other systems or
   sources:
   - Perks (Stealth, Muffled Movement, Silence, Quiet Casting, Shadow
     Warrior) with the perk system; muffle, invisibility and blindness, and
     spells and shouts as action sounds, with magic (the formula's slots for
     them are in `crate::detection`)
   - Weapon and impact sounds (action sounds, detection events): need the
     `iSoundLevel*` values, which no public source gives; weapons are silent
     until then
   - Finding bodies by detection
4. **AI depth**
   - Alias fills: created objects cleaned up
   - Remaining procedures: guard (restricted areas), use magic, dialogue,
     activate / carry, flee, orbit packages; ambush triggers (sleepers'
     reduced detection: see Detection);
     escorts' follower min / max distances and several followers, riding
   - Off-screen travel between worldspaces; paths round locked animated doors,
     lockpicking perks and skill gain
   - Templates: spells (with magic)
   - Package trees: branches of `Sequence` / `Simultaneous` roots, `GetNumericPackageData`,
     guard / wait / find / acquire procedures
   - Vehicles: carts pulled by horses (`TetherToHorse`; the carriage held to the
     horse by physics constraints), passengers riding seated in them (`SetVehicle`)
   - Dragons: flying, circling, landing and perching; breath and shout attacks
     (with magic)
5. **Game logic**: combat (crossbows and bolts, arrows in hand while drawing and
   stuck in actors, sneak shots, power bashes for the player (perk), armor perks, tempering
   and enchantments, killmoves and the decapitations they end in, the player's own weapon and
   animations), magic (with detection's muffle, invisibility, blindness and
   spell / shout sounds), shouts, perks (the sneak tree's detection perks among them;
   see Detection), inventory (player's, equipping by hand, armor from
   inventory, ammo / quivers; torches in dark interiors, burning out), leveling, crime
   (escape routes, yielding, skill loss in jail; witnesses running to the
   guards; fences; see Done)
   - The player as an actor: a body (race, sex, outfit) and a third-person
     camera, not only the first-person controller
   - Player control from scripts: `SetPlayerAIDriven`, `SetHudCartMode`;
     disabled fighting putting the weapon away, the POV type argument
   - Sneak attacks (`OnHit`'s sneak flag is always false)
   - Physics: making fixed objects dynamic (`SetMotionType` on a static
     model), simulating constraint motors (none in the vanilla meshes), the
     player grabbing objects, traps (their scripts hold them keyframed until
     triggered), arrows knocking down beehives, melee blows and spells pushing them
   - Scenes: the player's own lines (skipped for now; see `known_gaps/scenes.md`)
6. **UI**: inventory (categories, equipping, item details), map, bars, quest journal
   and objectives, help messages (`ShowAsHelpMessage`); support for the game's .swf assets still undecided
   - Character creation: the race menu (`ShowRaceMenu`, `SetInChargen`) and the
     player's face built live from FaceGen data (sliders, head parts, tints);
     NPCs only use pre-baked heads
7. **Audio**: lip sync
8. **Rendering**: point light and interior shadows, static shadow caching (per-cell
   caster batches), HDR/image spaces, image space modifiers (fades and blur:
   `FadeOutGame`, `PlayerImodAnimation`), camera shake (`ShakeCamera`), particles
   (fire, smoke, explosions), distant LOD (BTR/BTO/trees),
   grass, decals, environment maps
9. **Saves**: an engine-native save format (reading .ess later), writing out
   the scripts' state, inventories and `WorldState`; `RequestSave` /
   `RequestAutoSave`
10. **Performance**: async loading, GPU-driven culling
11. **New game and the opening** (an eventual goal; it needs most of the items above)
   - A new game separate from the developer launch, which drops the player at
     Tamriel (4, -12) outside Riverwood with only start-game-enabled quests running
   - How the game starts `MQ101` (not start-game-enabled) and where it places
     the player (`PlayerRef` has no placement in the data): needs a public
     source; record it in `known_gaps/`
   - Alternate start mods as the nearer target: their own start-game-enabled
     quest, a `MoveTo` and message boxes, all of which run already
   - The real opening in steps: new game and placement; the cart ride with a
     default race (vehicles, player control); the execution and Alduin
     (killmoves, dragons, particles, image spaces); Helgen Keep (the player's
     combat, magic, leveling, help messages, journal); character creation last
