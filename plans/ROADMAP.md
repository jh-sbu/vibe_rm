# Roadmap

The goal is parity with the Skyrim SE runtime for content built with the Creation Kit:
the original data files should play as designed. Work is ordered by how much of the
game each piece unlocks.

## Done
- `IsRunning` follows the player's run toggle and NPCs' running gait;
  `GetUnconscious` reads bleeding out. Choices:
  `known_ambiguities/status-conditions.md`
- Combat group conditions: `GetCombatGroupMemberCount`, `GetGroupTargetCount`,
  `IsLastHostileActor`, `GetCombatTargetHasKeyword`, `ShouldAttackKill`,
  `GetShouldHelp`, `IsProtected` and `GetActorAggroRadiusViolated`.
  `GetShouldAttack`, `GetGroupMemberCount`, `IsIgnoringCombat` and
  `GetActorsInHigh` stay unsupported. Choices:
  `known_ambiguities/combat-conditions.md`
- Record-data conditions: `GetScale`, `GetIsCreature`, `GetFactionRelation`,
  `GetFactionRankDifference`, `GetWeaponAnimType`, `IsCarryable`,
  `GetTargetHeight`, `GetActivatorHeight`, `GetClothingValue`, `GetDisease`,
  `IsActorUsingATorch`, `GetInCellParam` and `GetCannibal`. Oblivion leftovers
  with no Skyrim meaning stay unsupported. Choices:
  `known_ambiguities/status-conditions.md`
- Player, clock and package-input conditions: `IsPlayerInRegion`, `GetInZone`,
  `MenuMode`, `GetPlayerControlsDisabled`, `IsTimePassing`,
  `GetRealHoursPassed`, `GetSecondsPassed` and `IsNullPackageData`.
  `GetCurrentAIPackage`, `GetCurrentAIProcedure` and `IsWaiting` stay
  unsupported (no source for their values). Choices:
  `known_ambiguities/status-conditions.md`
- Location population conditions: `GetRefTypeAliveCount`, `GetRefTypeDeadCount`
  and their location-alias forms count a location's references of a ref type,
  and `GetKeywordDataForAlias` reads an aliased location's keyword data.
  Choices: `known_ambiguities/status-conditions.md`
- `GetFactionCombatReaction` reads a faction's group combat reaction towards
  another. Choices: `known_ambiguities/status-conditions.md`
- A null reference in `GetHeadingAngle`, `GetRelativeAngle` and
  `GetLineOfSight` names the subject's combat target. This is inferred from
  vanilla combat idles' use: `known_ambiguities/condition-geometry.md`
- Location conditions: the reference forms of `HasSameEditorLocAsRef` and
  `IsInSameCurrentLocAsRef`, `GetInCurrentLocFormList`, `IsLocationLoaded`
  and `IsLocAliasLoaded`. Choices: `known_ambiguities/status-conditions.md`
- `IsPCSleeping` and `IsPC1stPerson` read the sleep menu's hours and the
  camera. Choices: `known_ambiguities/status-conditions.md`
- `GetXPForNextLevel` gives the player's next level-up cost.
  Choices: `known_ambiguities/status-conditions.md`
- Ownership conditions: `IsOwner`, `IsCellOwner` and `IsInMyOwnedCell` read
  owners the way theft and trespass do (scripts' owners included). Which
  party the Skyrim forms ask about: `known_ambiguities/status-conditions.md`
- `HasTwoHandedWeaponEquipped` and `HasBoundWeaponEquipped` read the weapons
  in the actor's hands. Choices: `known_ambiguities/status-conditions.md`
- `GetKeywordItemCount` totals the carried items with a keyword.
  Choices: `known_ambiguities/status-conditions.md`
- `GetIsPlayableRace` reads the subject's race's Playable flag.
  Choices: `known_ambiguities/status-conditions.md`
- Platform conditions: `IsWin32` holds, `IsXBox`, `IsPS3` and `IsGoreDisabled`
  fail. Choices: `known_ambiguities/status-conditions.md`
- Successfully soul-trapped body identities survive native saves through
  optional `SOUL`; failed captures remain retryable, and legacy saves clear the
  spent set. Original `.ess` and resurrection behavior remain unverified:
  `known_ambiguities/soul-trap.md` and `known_ambiguities/saves.md`.
- Greater Power cooldowns survive native saves through optional `PWCD`;
  cooldowns restore by replacement, including clearing on legacy saves.
  The existing 24-game-hour policy and the original `.ess` uncertainty are
  documented in `known_ambiguities/casting.md` and `known_ambiguities/saves.md`.
- `PlayAnimationAndWait` threads survive native saves through optional `ANMW`;
  restored waits remap sender/event identities to current runtime keys and
  resume on later matching events. Graph poses and timelines remain unsaved:
  `known_ambiguities/saves.md`.
- `Message.Show` boxes and suspended scripts survive native saves through
  optional `MBXW`, which leaves `VM` and `VMAR` layouts unchanged. Restored
  filtered button indices wake the matching saved thread; other signal sources
  keep the timed fallback. Legacy behavior and key limits:
  `known_ambiguities/saves.md`.
- Created-reference starting transforms: `GetStartingPos` and
  `GetStartingAngle` retain a world reference's creation position and rotation
  through movement and native saves. The optional `ORIG` section leaves `WRLD`
  unchanged; older saves and inventory-created references have unknown origins.
  Skyrim SE semantics for created references remain unverified:
  `known_ambiguities/condition-geometry.md`.
- Timer-only scene phases retain their phase and elapsed timer time in optional
  `SCNE`; loading replaces old runs, and legacy saves clear them. `SCLF` keeps
  lifecycle state for all scenes and `SCAB` keeps explicit action actor bindings
  and reconstructible package ownership. Dialogue continuation, actor procedure
  progress and original `.ess` compatibility remain open:
  `known_ambiguities/scenes.md` and `known_ambiguities/saves.md`.
- Scene lifecycle state survives native saves in optional `SCLF`: begun,
  stopping, paused and action waiting/running/done/stopped state are restored,
  while running timer actions keep elapsed time even in mixed scenes. `SCAB`
  retains bound actors, including empty bindings, and rebuilds owned packages
  from plugin definitions before cell actors attach. Dialogue and actor
  procedure progress remain separate work: `known_ambiguities/scenes.md`.
- Package placement headings: scheduled Sandbox actors take a reference's
  heading at placement; Travel actors turn toward their marker after unloaded
  travel. Moved/dead actors and unrelated runtime steering retain priority.
  The Travel threshold, linked/alias resolution and timing policies are
  documented in `known_ambiguities/unloaded-placement.md`.
- Sleep fades: accepted sleep fades the world to black before advancing hours,
  holds through sleeping, and fades back after completion or interruption.
  Ordinary waiting and canceling the hour chooser do not fade. Chosen timing,
  script-fade composition, external-close and load policies:
  `known_ambiguities/sleep-wait.md`.
- Detection stealth points and regeneration delays persist through optional
  `STPT`; loading replaces the reservoir map and refreshes derived detection
  observations. Search/combat state and AI progression remain separate work:
  `known_ambiguities/detection.md` and `known_ambiguities/saves.md`.
- Papyrus `Actor.GetKiller` returns the recorded killer for a dead actor, or
  `None` for a living actor or a death without known attribution. Death records
  remain durable through `DETH`; attribution limits and unsupported death-cause
  and killing-limb queries: `known_ambiguities/death.md`.
- Actor comparison and status conditions: `SameFaction`, `SameRace`, `SameSex` and
  their player forms, `GetPCInFaction`, `GetIsClass` / `GetPCIsClass`, `GetGold`,
  `GetStaminaPercentage`, `GetHealthPercentage`, `IsOverEncumbered` (the player),
  `GetPCMiscStat`. Open questions: `known_ambiguities/comparison-conditions.md`
- Reference-state conditions: `GetDestroyed` (a stage with the destroy flag reached),
  `GetIsObjectType` (the base's record type, by xEdit's form-type numbers),
  `IsWeaponInList` (the weapon in hand), `GetDefaultOpen` (the placed `ONAM`) and
  `IsUnlockedDoor` (a door not locked now). Open questions:
  `known_ambiguities/reference-conditions.md`
- Combat-state conditions: `GetCombatState`, `IsCombatTarget` (the target the
  actor fights or searches for, as `GetCombatTarget`), `IsPowerAttacking` (the
  swing being made), `IsShieldOut` (shield held with weapons drawn), `IsTorchOut`
  (torch in the left hand). Open questions: `known_ambiguities/combat-conditions.md`
- Script-requested native quicksaves are append-only numbered `.vrs` files;
  manual names and the replaceable autosave slot keep their existing policies.
  This is a native history choice, not claimed Skyrim SE parity:
  `known_ambiguities/saves.md`
- Quest-alias factions (`ALFC`) contribute temporary actor memberships while
  filled in running quests: owned-door access, faction conditions and player
  ownership checks see them; clear/refill, quest stop and saves retain the
  right lifetime. Rank/conflict choices: `known_ambiguities/locks.md`
- Quest-alias inventory (`CNTO`) grants direct items to actors, the player, or
  containers on successful new fills. Grants stay with the reference through
  alias clears, quest stop/restart and native saves; signed-count overflow is
  bounded. Form-list and leveled-list entries remain unsupported; source
  conflict and same-reference policy: `known_ambiguities/locks.md`
- Scripted `ObjectReference.Lock` honors its "as owner" argument: door
  unlocking makes the adjoining interior public, locking makes it private;
  trespass checks and native saves retain that access state. Adjoining-cell
  choices and remaining lock behavior: `known_ambiguities/locks.md`
- Lockpicking with Wax Key evaluates `Mod Lockpicking Key Reward Chance` on
  the lock after a successful pick and awards its valid `KEYM` if the player
  does not already carry it; its chance and first-pick limits:
  `known_ambiguities/perks.md`
- The egui inventory fallback offers a quantity chooser for stacks over five
  when moving, pickpocketing/placing, or dropping; it retains the selected
  source row and rechecks it on confirmation. The SWF menu already handles
  quantities: `known_ambiguities/inventory-cards.md`
- The journal's Favorite Weapon follows accepted player melee attacks and
  actual bow releases, retaining counts and its incumbent through native
  saves. The usage metric and tie policy remain unverified in Skyrim SE:
  `known_ambiguities/journal.md`
- BSA, plugins (load order, ESL, overrides, strings), NIF (render + collision blocks), DDS
- Interior/exterior cells, landscape, water, weather/climate/sky, time of day
- Havok collision shapes -> rapier3d; first-person character controller
- Exterior cell streaming, load doors
- Actors: skinning, GPU skinning, NPC assembly (race, outfit, FaceGen); Havok clip playback.
  NPCs' skin tone (`QNAM`) colours their bodies to match their FaceGen heads
  (`known_ambiguities/skin-tone.md`), and their weight (`NAM7`) blends what
  they wear between its `_0` and `_1` models, meeting the heads' necks
  (`known_ambiguities/body-weight.md`).
  NPCs wear what their inventory has equipped: dressed again when it
  changes (scripts' `EquipItem`, the console's `equipitem`, a body stripped)
  and as they were when their cell loads again; an NPC's inventory is its
  items and outfit before it ever loads
- Papyrus VM (PEX, natives, events, VMAD), conditions (CTDA), quest stages, dialogue
- The game's HUD (`hudmenu.swf` through the vendored Ruffle, `crates/ui-swf`):
  health, magicka and stamina meters, compass heading and the map markers
  near the player on it (discovered, and undiscovered ones hollow; map
  markers come onto the map and are discovered as the player nears them,
  with the banner, the discovery music and Locations Discovered; `AddToMap`,
  `EnableFastTravel`, `IsMapMarkerVisible`; `known_ambiguities/map-markers.md`),
  the map (`map.swf`: M or the tween menu's Map; the worldspace's `MNAM` map data
  and the world map's camera over its distant LOD, panned, zoomed and tilted,
  with the markers on it, the player's and hovering names them;
  `known_ambiguities/map-view.md`),
  the tracked quests' targets (`QSTA`; the load door the way to one starts
  at when it is elsewhere) and the enemies fighting the player
  (`known_ambiguities/quest-targets.md`),
  location, the
  crosshair's activate text, subtitles, notifications, help messages, quest
  banners, the sneak eye, enemy health with the name between brackets
  parted to fit it, the activate key's art; message boxes
  (`messagebox.swf`, answered with the mouse or keys); the inventory
  (`inventorymenu.swf`: categories, the item card with armor rating and damage
  and their change if worn, effects, the bottom bar's stats; equipping, using,
  reading, dropping, favorites; scroll spell cards and localized soul-gem
  cards, refreshed when a gem is filled, the best-in-class mark (`known_ambiguities/inventory-cards.md`);
  ingredient cards' learned effects (eating one teaches its first, or as many as
  Experimenter's entry point reveals; the rest read "$UNKNOWN");
  weapon cards' poison icon, refreshed when poison is applied or exhausted
  (`known_ambiguities/poisons.md`);
  `showinventory open`); containers, bodies,
  teammates' inventories and pockets (`containermenu.swf`: taking, storing,
  giving, Take All, the pickpocket chance) and bartering (`bartermenu.swf`
  from `Actor.ShowBarterMenu`: merchant chests, buy / sell lists, stolen
  goods, prices from Speech, the Haggling / Allure perks and Fortify Barter, the raw deal
  warning, Speech XP, merchant chests restocked every `iDaysToRespawnVendor`
  days, with the Investor / Master Trader gold their leveled lists' globals
  (`LVLG`) give) (choices: `known_ambiguities/swf-hud.md`,
  `known_ambiguities/barter.md`); the journal (`quest_journal.swf`: quests with
  their latest log entry and objectives, the miscellaneous objectives,
  completed quests, Toggle Active (`Quest.SetActive` / `IsActive`), the
  Stats page (`Game.IncrementStat` / `QueryStat`; kills, crimes, bounties,
  jail, locks and pockets picked, theft, gold, chests, books, skills, food
  and potions, barters, quest completions, spell / school / shout usage
  favorites), the
  bottom bar's date and level; `known_ambiguities/journal.md`); gifts
  (`giftmenu.swf` from `Actor.ShowGiftMenu`: the gift filter, giving and
  taking, favor points for the script; `known_ambiguities/gifts.md`); the magic
  menu (`magicmenu.swf`: spells by school, powers, active effects, the cost,
  level and school meter, readying spells in a hand (`Actor.EquipSpell` /
  `GetEquippedSpell`), favorites; `known_ambiguities/magic-menu.md`); the favorites
  menu (`favoritesmenu.swf` on Q: favorite items, spells and powers, equipping
  them, hotkeys 1 to 8 set there and used in the world (top-row and numpad
  bindings from the control map);
  `known_ambiguities/favorites.md`); the tween menu (`tweenmenu.swf` on Tab: Skills,
  Magic, Items, the date and level; the item and magic menus come back to
  it; `known_ambiguities/tween-menu.md`); sleeping and waiting (`sleepwaitmenu.swf`
  from T or a bed: the game's reasons not to, the hours passing with the
  clock, weather, magic effects and the player's health, magicka and
  stamina, the other actors healed and put where their schedules have
  them, enemies coming upon the player ending it;
  `RegisterForSleep` / `OnSleepStart` / `OnSleepStop`, so the
  game's Rested; `SetActorOwner` / `SetFactionOwner`, so rented rooms;
  `known_ambiguities/sleep-wait.md`); training (`trainingmenu.swf`: the card
  filled and the meter filling with each session, E to train, Tab to
  leave); the dialogue menu (`dialoguemenu.swf`: the speaker's name, their
  lines, the topics sliding in, topics already heard greyed, the clicked
  topic kept above while the answer is said, a click or E skipping a line,
  Tab or Esc leaving; `known_ambiguities/dialogue-menu.md`);
  egui console and debug text (`--egui-hud` for egui's HUD); audio
  (WAV/xWMA/FUZ), music, ambient sounds, voice
- The menus' cursor (`cursormenu.swf`): follows the pointer through viewport
  scaling and resizing, drawn over the UI; the system cursor remains available
  for the console and missing cursor assets. Scaling and visibility choices:
  `known_ambiguities/cursor-menu.md`
- AI: navmeshes (NVNM), A* + funnel pathfinding across cells, package selection
  (schedule + conditions), sandbox / travel behaviour, idle/walk cross-fade,
  persistent NPC whereabouts with leaving / arriving through load doors;
  multi-location sandboxes moving between their stops (the tree's repeating
  branch; `known_ambiguities/package-trees.md`); actors appear where their
  package took them while unloaded, as `test-mod autotest6` and `autotest7`
  found (Travel 40 units short on the straight line, or 10 inside a wider
  radius; Sandbox on its centre), fallen onto the floor beneath rather than
  scattered across floors
  (`known_ambiguities/unloaded-placement.md`)
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
  State machines publish their `currentStateId` bindings on entry and transition
  (blocking, one-handed weapon and equipped-item state variables); timing choices:
  `known_ambiguities/state-machine-outputs.md`.
  Authored state / transition change events retain their payload and graph-local
  mapping, raised when a transition commits; additional notification boundaries
  remain unresolved (`known_ambiguities/state-machine-events.md`).
  Blending effects' self-transition modes (a transition into the state already
  entered continues, restarts or blends) and event modes (`ProcessAll` lets the
  state being left hear events while it fades) are honoured. Argonian and Khajiit
  NPCs' tails sway: a worn model whose `BSBehaviorGraphExtraData` controls the base
  skeleton (the tail's `TailProject`) runs its graph beside the actor's, hearing its
  events and sharing its variables by name, posing the `TailBone`s
  (`known_ambiguities/object-graphs.md`). The player's third-person body runs
  its worn tail graph too; NPC redressing adds, removes or replaces wearer
  graphs while preserving an unchanged selection's running state.
  Player equipment rebuilds retain compatible body graphs and drawn/ready
  weapon state, including draws and sheathes in progress; rigid equipment
  uses the retained attachment state (`known_ambiguities/player-body.md`).
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
- Single-bone additive LINEAR `hkbTwistModifier` poses: bound angle and enable,
  event activation, model and local axes, Havok-to-NIF bone mapping. Angle units,
  chains, other modes and blending limits: `known_ambiguities/twist-modifier.md`
- `hkbRotateCharacterModifier` data and active inputs: authored enable, rate,
  multiplier and aligned axis retained; scalar variable / character-property
  bindings and event gates exposed by the runtime; running modifiers' yaw
  (rate x multiplier x dt about the vertical axis) is added to the frame's root-motion
  yaw. Open: `known_ambiguities/rotate-character.md`
- Read-only scalar character properties on behaviour generators: clip start,
  playback speed and user-controlled time, selector index, blend parameter and
  state-machine start ID use the runtime's existing input paths, with graph
  variables taking precedence. Remaining inputs and lifecycle questions:
  `known_ambiguities/character-properties.md`
- Timer modifiers resolve `alarmTimeSeconds` graph-variable and scalar
  character-property inputs at the existing elapsed-time comparison, retaining
  one alarm per activation. Sampling, invalid-input and lifecycle policies:
  `known_ambiguities/timer-properties.md`
- Blender-child `boneWeights` character properties resolve per character
  (arms, upper body and mounted-combat masks), preserving existing parent-mask
  composition and authored fallbacks. Remaining limits:
  `known_ambiguities/character-properties.md`
- Blender-child scalar `weight` character properties feed ordinary, additive
  and parametric blends, including synchronized timing, with graph-variable
  precedence and authored fallbacks. Evidence and remaining limits:
  `known_ambiguities/character-properties.md`
- Transition-effect `duration` character properties resolve per character at
  transition commit, with graph-variable precedence and authored fallbacks.
  Evidence and lifecycle limits: `known_ambiguities/character-properties.md`
- Transition effects resolve whole-member `selfTransitionMode` graph-variable
  and scalar character-property inputs at commit, preserving existing
  continue/reset/blend behavior. Binding and lifecycle limits:
  `known_ambiguities/transition-self-mode-properties.md`
- Transition effects resolve whole-member `eventMode` graph-variable and
  scalar character-property inputs at commit, preserving existing event
  routing. Ignore-to routing and lifecycle limits:
  `known_ambiguities/transition-event-mode-properties.md`
- Transition effects select the authored or whole-member graph-variable / scalar
  character-property `blendCurve` at commit. Linear blending and the existing
  smoothstep curve are supported; the two asymmetric enum modes retain the
  prior smoothstep approximation pending formula evidence:
  `known_ambiguities/character-properties.md`
- Transition effects resolve authored, whole-member graph-variable and scalar
  character-property `toGeneratorStartTimeFraction` inputs at commit. A positive
  fraction seeks a destination clip through transparent wrappers with cropped
  and reverse playback; zero preserves the clip's existing `startTime`. Compound
  generator and exact Havok timing policies remain open:
  `known_ambiguities/character-properties.md`
- Transition effects resolve authored and whole-member `endMode` inputs at
  commit. Direct single-play source clips and transparent wrappers support
  remaining-time and duration-cap policies using crop, direction and speed;
  compound, looping and zero-speed source timing remain open:
  `known_ambiguities/character-properties.md`
- Cyclic blend wrappers forward `fBlendParameter` character properties to their
  blender children, including cyclic weights and synchronized clip timing.
  Existing variable precedence and authored fallbacks remain; wrapper lifecycle
  limits: `known_ambiguities/character-properties.md`
- Cyclic blenders resolve `minCyclicBlendParameter` / `maxCyclicBlendParameter`
  scalar character properties and live graph variables, sharing range inputs
  between pose weights and synchronized timing for direct and wrapped blenders.
  Invalid-input and lifecycle limits: `known_ambiguities/character-properties.md`
- Clip generators resolve scalar character properties and graph variables for
  start/end cropping at activation. Forward/reverse playback, user-controlled
  samples and synchronized timing share the resolved range; lifecycle and
  invalid-input limits: `known_ambiguities/clip-crop-properties.md`
- Clip generators resolve scalar character-property and graph-variable `mode`
  inputs at activation across single-play, looping, user-controlled and
  ping-pong paths, including synchronized loop detection. Invalid-input and
  lifecycle limits: `known_ambiguities/clip-mode-properties.md`
- Clip generators resolve authored, graph-variable and character-property
  `enforcedDuration` inputs through playback advancement, synchronized cycle
  timing and transition source-end estimates. Positive duration retimes the
  activated cropped length while preserving reverse direction; update sampling,
  ping-pong and invalid-input policies: `known_ambiguities/clip-enforced-duration.md`
- State machines resolve scalar character-property and graph-variable
  `startStateMode` / `syncVariableIndex` inputs at activation, preserving the
  existing default/sync/random paths and graph-local sync state. Lifecycle,
  invalid-input and chooser limits: `known_ambiguities/state-machine-start-properties.md`
- Blenders resolve whole-member scalar character-property and graph-variable
  `flags` inputs consistently across pose weights, synchronized timing and
  trigger masters. Integer, bit-binding and lifecycle limits:
  `known_ambiguities/blender-flags-properties.md`
- Synchronized blenders retain the signed `indexOfSyncMasterChild` input and
  use authored, variable or property selection for trigger routing in direct
  and nested blends. Automatic fallback and runtime-parity limits:
  `known_ambiguities/blender-sync-master.md`
- Integer generator inputs exclude unsupported bit selectors before evaluating
  complete enums and indices, preserving authored fallbacks and supported
  whole-member bindings (`known_ambiguities/integer-generator-bit-bindings.md`).
- Foot placement: the character's `hkbFootIkDriverInfo` (humanoids, horses, most creatures): rays through
  the static world under each ankle, the body dropped to the lowest foot's ground, two-bone
  IK raising the others within their knee limits, planted feet tilted with the slope;
  only while the graph runs its `hkbFootIkControlsModifier` (humanoids' switched on by
  the engine through `bHumanoidFootIKEnable`, off in furniture and while the graph sets
  `bHumanoidFootIKDisable`), eased by its gains: on / off, ground ascending /
  descending, body feedback and up / down bias, planted / raised, ankle orientation
  (open questions: `known_ambiguities/foot-ik.md`)
- Talking gestures: each line of dialogue picks from the `ActionTalking` tree (hands on hips,
  hand gestures, angry / happy / expressive idles) by the line's emotion (`TRDT`) and the
  speaker's graph variables (`GetGraphVariableInt` / `Float` read the reference's graph,
  an object's or an actor's); `IsTalking`, `GetDialogueEmotion(Value)`, `GetMovementSpeed`
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
- NPC spell-list templates: spells, abilities and perks come from the source
  selected by Use Spell List, through nested and leveled NPC templates; empty
  lists stay empty. Racial spells follow the traits source independently.
  Leveled spell lists (`LVSP`: bandit and ghost mages, dragon priests,
  draugr overlords, battlemages, vampires) are rolled for the actor's level,
  each actor keeping its pick.
  Evidence and remaining limits: `known_ambiguities/npc-templates.md`
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
- Hands: one record per actor of what each hand and the voice hold (weapons,
  spells, a shield, a torch; a power or shout), placed by equip slots (`ETYP`
  -> `EQUP` and its parents: either hand, both, the left, the voice). One-handed
  weapons go in either hand, two of the same one in each; a two-handed weapon
  or both-hands spell fills both; spells share the hands with weapons. The
  item, magic and favorites menus' attack buttons equip in their hand (the
  vendored Ruffle's Scaleform `Mouse.getTopMostEntity`), with left / right /
  both equip icons; a left-hand weapon drawn in the left hand, the graph's
  hand types from both hands; console `equipitem <item> [left|right]`,
  Papyrus `GetEquippedWeapon(abLeftHand)` / `GetEquippedShield`,
  `GetEquippedItemType` 9 for spells. Open questions:
  `known_ambiguities/hands.md`
- Fighting with each hand: the left attack button swings a weapon or fist
  there (else raises the guard), both buttons strike together (dual-wield
  attacks; the fists' combo), held a power attack (directional left-hand
  ones too); the race's attack for the hands, each `HitFrame` landing its
  hand (`Left` payloads the left) with that hand's weapon, damage, reach,
  perks, skill and enchantment; power attacks' further blows costing
  stamina again; no blocking with a weapon in each hand. Hand-to-hand: empty
  hands punch with the race's bare-handed attacks (player and NPCs). The
  graphs' attack speeds (`weaponSpeedMult`, `leftWeaponSpeedMult`) from the
  weapons and actor values. NPCs whose combat style allows it dual wield a
  second one-handed weapon. Condition `IsAttackType` (the attack's type
  keyword: Dual Savagery, Critical Charge...). Console `ppress` /
  `prelease [left|both]`, `pattack [power] [left|both]`
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
  `fCombatStrengthUpdateTime`; contributions to other fighters' group sums
  refreshed separately every `fCombatGroupCombatStrengthUpdateTime`, including
  the player's (each actor counts its own personal strength in its check);
  its side's strength over the other side's (sides
  linked through who fights whom, the player among them) checked every
  `fCombatThreatRatioUpdateTime` against its confidence (`fConfidence<Level>`) plus a
  modifier rolled per fight (`fCombatConfidenceModifierMin` / `Max`); only the hurt
  (and cowards) flee. Fleeing actors say a `FLEE` line, run to the navmesh places
  furthest from the threat out to `fCombatFleeDistance*`, wait there and leave the
  fight after `fFleeIsSafeTimer`; hurt or approached they check again, and turn back
  when the ratio recovers; cornered, they fight. Cowards don't draw. NPCs' health
  comes back out of combat. Flee dialogue retries on its own timer using
  `fCombatDialogueFleeMinElapsedTime` / `MaxElapsedTime`, deferring while
  another line or conversation owns the channel; scheduling choices remain
  documented in `known_ambiguities/combat-fleeing.md`.
  `GetThreatRatio`, `IsFleeing`; `cstats` shows strength
  and confidence. Open questions: `known_ambiguities/combat-fleeing.md`
- Assistance: actors not fighting join a fight they see where an ally (same or
  allied faction) or, for those who help friends too, a friend is fighting someone
  they aren't friendly with, as their AI data's assistance has it; the player
  fights whoever fights them, so attacking one of the townsfolk sets their
  neighbours on the player. Negative faction ranks aren't membership. A
  script's faction changes count in fights at once (`AddToFaction` of
  `CurrentFollowerFaction`, an ally of the player's faction in the data, makes a
  hired follower help the player as its assistance has it), and across cell
  loads. `cstats` shows assistance. Player teammates (`SetPlayerTeammate`,
  `IsPlayerTeammate`, `GetPlayerTeammate(Count)`) join the player's fights
  whatever their factions, wear armor they are given, and the player answers
  for their blows and their summons' as for their own (an assault on someone
  who keeps the law; UESP Crime). Open questions: `known_ambiguities/combat-assistance.md`, `known_ambiguities/followers.md`
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
  With the Power Bash idle's authored conditions satisfied, holding attack
  makes a power bash; a tap makes an ordinary bash. Pending bashes cancel when
  their guard, equipment or eligibility changes. Timing and fallback policies:
  `known_ambiguities/power-bash.md`
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
- Bow ammunition selection rejects bolts, malformed records and exhausted
  equipped stacks on every path; equipped compatible arrows take precedence,
  with the existing NPC iron-arrow fallback. Sources and remaining auto-equip
  questions: `known_ambiguities/archery-ammunition.md`
- Death: ragdolls from the skeleton's rigid bodies and constraints (capsules, cone /
  twist / plane and hinge limits as rapier joints); bodies can be searched;
  `GetDead`, `IsDead`, `Kill()`, `OnDying` / `OnDeath`; console `kill`;
  `GetTimeDead`, `IsKiller`, `IsKillerObject` over the killer and time of deaths in play
  (`WorldState::deaths`, saved in `DETH`)
- Combat state for scripts: fighting, or searching for a target it lost
  (`GetCombatState`; the player is in combat while anyone fights them). Papyrus
  `StartCombat`, `StopCombat` (searchers give up too), `IsInCombat`,
  `GetCombatState`, `GetCombatTarget`, and the events `OnCombatStateChanged`
  (on each change of state), `OnHit` (aggressor, weapon, the arrow's projectile,
  power attack, sneak attack, bash, blocked) and `OnEnterBleedout`. Conditions `IsInCombat`,
  `IsWeaponOut`, `IsBleedingOut`; Papyrus `IsInFaction`, `GetFactionRank`,
  `IsGuard`. Console `cgf <Class.Func> [@self] [args]` calls a native; vrm-tool
  `script-users` finds what attaches a script. Open questions:
  `known_ambiguities/combat-state.md`
- Sneak attacks: a blow or arrow from a sneaking attacker that the struck actor
  doesn't detect does the weapon type's multiple of its damage before armor
  (`fCombatSneak1HSwordMult`... 3x one-handed, 2x two-handed and unarmed; 2x
  bows), with "Sneak attack for 3.0X damage!" for the player and `OnHit`'s
  sneak flag set; the player's "Sneak Attacks" and, with a one-handed weapon on
  a target facing away, "Backstabs". A survivor that wasn't fighting its
  attacker searches for it rather than fighting at once. Open questions:
  `known_ambiguities/sneak-attacks.md`
- Critical strikes: the perks' "Calculate My Critical Hit Chance" (Bladesman,
  Deep Wounds, Critical Shot 10 / 15 / 20 %, the Critical Charges 100 % for a
  sprinting power attack) and "Mod Enemy Critical Hit Chance" give the chance;
  a critical blow adds the weapon's `CRDT` damage through "Calculate My
  Critical Hit Damage" (x1.25, x1.5, x2) past armor, counts the player's
  "Critical Strikes" and sets `GetLastHitCritical`; `IsSprinting` is the
  player's. Open questions: `known_ambiguities/critical-hits.md`
- Skill use perks: "Mod Skill Use" scales a skill use's XP for the skills
  its `EPMagic_IsAdvanceSkill` conditions name (`known_ambiguities/skills.md`)
- Active player magic effects supply their "Perk to Apply" (`MGEF DATA`
  0x88) to perk checks and entry points, including skill-use bonuses, without
  changing permanent ownership. Source overlap, temporary ability / quest
  sections and lifecycle limits: `known_ambiguities/effect-perks.md`
- Scrolls (`known_ambiguities/scrolls.md`): used from the inventory or
  favorites into the hand asked for (or away again), cast like a spell with
  the scroll's charge time, free of magicka and skill, and used up by the
  cast (one cut short isn't); `GetEquippedSpell` leaves them out.
- Saves (`src/save/`, `known_ambiguities/saves.md`): the engine's own format
  (sections with CRCs, written through a temporary file) holding the clock,
  globals, quests, statistics, actor value changes, inventories with their
  equipment, script changes to references, the world state kept across cell
  loads, references made while playing, and the player's health, skills,
  perks, spells, shouts, favorites, dialogue heard, bounties and restocks;
  loading refuses damaged files and other plugin lists, then enters the
  saved location. The Papyrus VM too: attached scripts with their variables
  and properties, suspended threads (kept at the instruction they waited at),
  `OnInit` done, update timers, and sleep, inventory filter and animation
  event registrations. The weather (current and outgoing, the transition,
  override and roll timer; optional `WTHR`). `Game.RequestSave` /
  `RequestAutoSave` (written on the next update), console `save`, `load`,
  `saves`.
- Saves retain Story Manager cooldowns, do-all-before-repeating rounds,
  social deadlines, location and body-discovery memory, and changed
  relationship ranks. Optional `STOR` / `RELA` sections preserve older native
  saves; loading replaces runtime memory without dispatching rank-change
  events. Original-game persistence equivalence remains unverified:
  `known_ambiguities/saves.md`
- Skills and leveling: the player's skill uses earn XP by each skill's `AVSK`
  (use mult and offset), skills go up at `improve mult x level ^ fSkillUseCurve
  + offset` ("One-handed Increased to 21"), each level reached giving character
  XP toward level ups ("Level up available."), taken in a skills menu (K) or
  the console: +10 health, magicka or stamina (stamina +5 carry weight), a perk
  point, all three restored. Uses: landed blows and shots (one-handed,
  two-handed, archery by base damage), sneak attacks, blocking, the armor worn
  when struck, picking locks and breaking picks, pickpocketing by value,
  sneaking hidden; skill books teach once. Trainers (`Game.ShowTrainingMenu`
  from their dialogue, `trainingmenu.swf`): the skill their class teaches, a
  level a session up to its most (Common 50, Expert 75, Master 90), five
  sessions a player level, priced as UESP gives from `fTrainingMultCost`
  and `iTrainingExpertCost` / `iTrainingMasterCost`, the gold to the
  trainer, Training Sessions counted. The player's level drives leveled
  lists; NPCs' levels from `ACBS` (PC level mult). Story Manager `SKIL` and
  `LEVL` events. Papyrus `Game.AdvanceSkill`, `IncrementSkill`,
  `IncrementSkillBy`, `GetGameSettingFloat` / `Int`, `Actor.GetLevel`;
  condition `GetLevel`. Console `skills`, `player.advskill`, `player.incpcs`,
  `player.advlevel`, `levelup`, `getlevel`, `<ref>.showtrainingmenu`,
  `<ref>.train`. Legendary skills: at base 100 the skills menu offers a
  confirmed reset to 15, returning the owned perk ranks in that skill's tree;
  repeated resets counted, skill XP cleared, character progress kept.
  Console `legendary <skill>`. Open questions:
  `known_ambiguities/skills.md`
- Perks: `PERK` records (requirements, rank chains, quest and entry point
  sections with their tabs' conditions and functions); actors' perks from
  their records (`PRKR`, through templates) plus those added and removed;
  entry points run where the engine reaches them: attack, power attack and
  bash damage, sneak attack multipliers (Backstab 6x, Assassin's Blade 15x,
  Deadly Aim 3x), armor penetration, incoming damage, blocking, each armor
  piece's rating, power attack stamina, detection (Stealth, and observers'
  light and movement), the lockpick sweet spot and unbreakable picks,
  pickpocket chance. Perk trees from the skills' `AVIF` nodes in the skills
  menu, bought with perk points when the rank's conditions pass. Conditions
  `HasPerk`, `IsWeaponSkillType`, `WornHasKeyword`,
  `WornApparelHasKeywordCount`, `IsBlocking`; Papyrus `Actor.AddPerk`,
  `RemovePerk`, `HasPerk`, `Game.AddPerkPoints` (and SKSE's perk point
  functions). Console `perks`, `addperk`, `removeperk`, `hasperk`, `perkep`,
  `perkpoints`, `skills open <skill>`; vrm-tool `perks`. Open questions:
  `known_ambiguities/perks.md`
- Magic, first part (effects on actors): magic effects and the items carrying
  them (spells, enchantments, potions, ingredients, scrolls) read; active
  effects on actors for their duration: value modifiers held (abilities,
  Recover: fortify, resist, weakness), per second (damage and restoration
  over time) or at once (Firebolt, healing potions), detrimental ones taking
  away; peak value modifiers not stacking; dual value modifiers, absorb, cure
  disease / poison / paralysis, invisibility (unseen in detection), stagger;
  resistances and magic resistance; temporary modifiers in the actor value
  store. Abilities and diseases from records, races, perks and `AddSpell`,
  their conditions looked at again every second; worn things' constant
  enchantments, weapon enchantments on each blow. Effect scripts on
  `ActiveMagicEffect` (`OnEffectStart` / `OnEffectFinish`, the target's
  events); hostile spells landing send `OnHit` and start a fight. Muffle
  scales movement noise; magic armor adds to the armor rating. Potions,
  food and ingredients used from the inventory; spell tomes teaching their
  spells. Conditions `HasMagicEffect`,
  `HasMagicEffectKeyword`, `HasSpell`, `IsSpellTarget`,
  `EPMagic_SpellHasKeyword` / `SpellHasSkill`; the hands' casting
  (`HasEquippedSpell`, `GetCurrentCastingType` / `GetCurrentDeliveryType`,
  `IsCurrentSpell`, `SpellHasKeyword`: `known_ambiguities/casting.md`); Papyrus `AddSpell`,
  `RemoveSpell`, `HasSpell`, `DispelSpell`, `DispelAllSpells`,
  `HasMagicEffect(WithKeyword)`, `DoCombatSpellApply`, `EquipItem` /
  `UnequipItem`, `Spell.Cast` / `RemoteCast`, `ActiveMagicEffect`'s getters,
  `Dispel` and updates. Console `effects`, `addspell`, `removespell`, `cast`,
  `dispelallspells`; vrm-tool `magic`. Native saves retain per-actor leveled
  spell-list rolls, including empty results, in optional `RLSP`; older saves
  clear the roll cache. Optional `WSTA` preserves ward-hit `IsWardState`
  outcomes and their remaining native hold time, but not active ward effects.
  Open questions: `known_ambiguities/magic.md`,
  `known_ambiguities/npc-templates.md`, `known_ambiguities/wards.md`
- Timed invisibility ends on resolved combat melee/bashes (including misses),
  arrow release, actual spell delivery and player activation, including scripts'
  `Activate(player)`. Charging and invisibility renewal preserve it; companion
  effects remain. Timing, constant sources and other interaction limits:
  `known_ambiguities/invisibility.md`
- Magic, second part (casting by hand): the player casts the spell in either
  hand with that hand's attack button (fire-and-forget spells charging for
  their charge time and cast when it comes up, concentration spells going on
  while it is held), paying magicka (by the second for concentration), the
  meter blinking and "You don't have enough Magicka" without it; powers on
  Z, greater powers once a day. The player's body casts through its graph
  (`magicbehavior.hkx`): each hand charges, holds and releases the spell's
  kind of cast (aimed, self, concentration, ward, telekinesis), the spell
  leaving at the release clip's `SpellFire`. With the school's dual casting
  perk, the same spell in both hands pressed together is dual cast
  (`DualMagic_...` on the left hand's graph): 2.8 times the cost, its
  effects' magnitude or duration 2.2 times; casters with both hands free and
  the perk dual cast too, and Impact staggers on a dual cast
  (`EffectWasDualCast`). Console `equipspell`. Spell costs include Fortify school effects
  (`AlterationMod` .. `RestorationMod`) through the game's `PerkSkillBoosts`
  entries, shared by casting and the magic menu, stacking to free casting
  at 100%; `SpellHasCastingPerk` lets the Novice .. Master perks halve
  the cost of their spell tiers (`known_ambiguities/magic-menu.md`).
  Deliveries: self, contact, target actor,
  aimed (missiles flying as their `PROJ` does, flames and beams reaching at
  once) and target location; areas striking the actors around where a
  spell lands; impacts (scorch decals) from the effect's impact data set.
  Destruction and the other schools gain XP from casting (hostile spells as
  they land). Magicka comes back at the race's rate times `MagickaRateMult`,
  slower in combat, after a moment. The Story Manager's player spell cast
  event carries the current cast location into from-event aliases and
  `OnStoryCastMagic` (`known_ambiguities/story-cast-events.md`).
  Console `pcast`. NPCs whose combat style favours magic fight with their
  strongest offensive spell while their magicka lasts (keeping their
  distance, throwing or channelling it at their target), then by hand;
  their magicka comes back. Humanoid casters cast with a hand in their
  graph, as the player does (the left beside a one-handed weapon). Open questions: `known_ambiguities/casting.md`
- NPC combat self-healing: injured fighters choose an affordable known
  self-delivered Health restoration spell, check its effects' conditions on
  themselves and pay magicka through the existing casting pipeline. Healing
  bypasses enemy range/sight and cancels a pending bow draw. Start/stop
  thresholds and scoring remain provisional: `known_ambiguities/npc-healing.md`
- Slow Time (archetype 37, the shout's three words): while an effect lasts
  the world (actors, projectiles, physics, particles, triggers, hazards,
  animated objects) runs at its magnitude's share of real time (0.3, 0.2,
  0.1), the slowest winning; the player, magic timers and scripts stay in
  real time: `known_ambiguities/magic.md`
- Telekinesis: the player's held Telekinesis spell grabs what the crosshair is
  on up to the settings' distance (`fMagicTelekinesisBaseDistance`...) and
  carries it as Activate's hold does, dropping it when the spell is let go;
  holding something already shows `sMagicTelekinesisNoRecast`:
  `known_ambiguities/telekinesis.md`
- Drinking and eating: a potion's or food's own consume sound (`ENIT`) plays
  where the actor stands: `known_ambiguities/magic.md`
- Illusion's hold on fighting: calm, fear (and turn undead), frenzy and rally
  effects hold the target's aggression or confidence at their own value
  (calm -1, fear 0, frenzy 3, rally foolhardy) for as long as they last,
  reaching only targets up to their magnitude's level (not rally's); the
  data's own conditions keep them off undead, daedra and automatons without
  Master of the Mind (`IsUndead`, `IsCommandedActor`). The calmed stop
  fighting and look for no fights until attacked; the frightened flee at
  once and come back when it ends; the frenzied fall on whoever is nearest,
  packmates too (casting it is still an assault), and when it ends they and
  those they fought give up the fights neither would have picked; the
  rallied stop fleeing. Open questions: `known_ambiguities/magic.md`
- Summons: Conjuration's summon effects bring their creature where the
  spell lands, commanded by the caster for the effect's duration (one at a
  time, two with Twin Souls: Mod Commanded Actor Limit); it follows its
  commander from cell to cell and the player through load doors (moved
  beside it when left far behind),
  fights whoever fights its commander, picks no fights of its own, and
  goes when the effect ends or it dies (`IsCommandedActor`). Weaponless
  casters whose style scores magic and melee the same (atronachs) fight
  with their spells; spells and arrows strike actors made while playing
  (they flew through them). Open questions: `known_ambiguities/magic.md`
- Bound weapons: the effect's weapon in hand while it lasts (Bound Bow's
  script giving its arrows), gone at the end or when put away. Weapon
  `Can't Drop` flags prevent dropping; non-playable weapons stay out of
  inventory, container, barter and gift lists and their transfers, so bound
  weapons remain with the caster (`known_ambiguities/inventory-restrictions.md`)
- Shouts: words of power learned and unlocked (`Game.TeachWord`,
  `UnlockWord`, `IsWordUnlocked`; a dragon soul in the magic menu), the
  shouts they give, `AddShout` / `RemoveShout` / `EquipShout` /
  `UnequipShout` / `GetEquippedShout`, conditions `HasShout`,
  `GetEquippedShout`, `IsCasting`. Holding Z speaks one to three words
  (`fShoutTime1` / `fShoutTime2`) and casts that word's spell (cones:
  Unrelenting Force staggers everyone before the player); the voice rests
  for the word's recovery times `ShoutRecoveryMult`, the HUD's shout meter
  filling and flashing when Z comes too soon. The magic menu's Shouts (the
  words, those unlocked, the recharge, the dragon souls) and the favorites
  menu's; the shout stats. Console `teachword`, `unlockword`, `addshout`,
  `equipshout`, `pcast voice`. Disarm (effect archetype 9, the shout and the
  draugr's): the target's drawn weapon drops from its right hand, a
  dual-wielder's left goes to the inventory, bound weapons stay, up to the
  magnitude's level; the player's "Weapons Disarmed" statistic. Open
  questions: `known_ambiguities/shouts.md`, `known_ambiguities/disarm.md`
- Harvesting: plants (`FLOR`, `TREE` with produce) found by their
  non-collidable shapes, giving their produce with their own verb and sound,
  someone else's stolen; harvested models, growing back
  (`known_ambiguities/harvest.md`)
- Player controls from scripts: `DisablePlayerControls` / `EnablePlayerControls`
  (Papyrus's defaults for missing arguments) stop the player moving, fighting
  (attacks, bashes, blocking, shooting), looking, sneaking, opening the inventory
  and activating; disabling fighting sheathes a drawn weapon. The POV type
  argument keeps Script and Werewolf camera locks separately; the
  `Is...ControlsEnabled` queries. `SetHudCartMode` selects the cart HUD's
  visibility group in both SWF and fallback HUDs; disabling movement temporarily
  overrides it, restoring cart mode when movement is enabled. Console `epc`. Open
  questions: `known_ambiguities/player-controls.md`
- The player's body: their NPC record's race, sex and skin, what they have
  equipped (rebuilt when it changes) and a head of the race's default head parts
  and their own (`HEAD`, `PNAM`, extra parts); it runs the race's behaviour
  graph from the player's movement (walking, running, sneaking, turning,
  strafing and walking backwards by `Direction`, sprinting, jumping, falling
  and landing), drawing and sheathing (R), swings and power attacks by
  movement landing at the graph's `HitFrame`, bow draws with arrows loosed at
  the graph's `arrowRelease`, blocking and bashes. A
  third-person camera (F; the wheel zooms 155 to 600 units) over the right
  shoulder, centred and farther back with the weapon drawn (the `[Camera]`
  defaults), stopping short of walls; aim rays from the eye through the
  crosshair,
  `Game.ForceFirstPerson` / `ForceThirdPerson`; in first person the body only
  casts its shadow. Open questions: `known_ambiguities/player-body.md`
- Loose objects: references whose model has one simulated rigid body
  (`MO_SYS_DYNAMIC`: clutter, food, weapons, baskets) are rapier bodies with the
  NIF's mass, friction and restitution (meshes as their convex hull), lying still
  where placed until disturbed; models of several bodies are joined by their
  constraints (hand carts roll on hinged wheels, signs swing on chains, trip
  wires and bone alarms hang from rope links, beehives from a spring that
  breaks). Every NIF constraint type the vanilla meshes use is read, motors
  included (none are switched on). vrm-tool `model-users`. The player pushes them aside, actors walking into
  them wake them, arrows and the player's blows knock them away (and send `OnHit` to
  whatever they strike, corpses too); where they come to rest is kept across loads. Papyrus
  `ApplyHavokImpulse`, `SetMotionType` (the weapon racks' scripts hold their
  weapons keyframed), `DropObject`. Objects made in the world (`PlaceAtMe`,
  dropped items) are drawn, fall, have their base's scripts and can be picked up;
  the inventory drops items on right click. Grabbing: holding Activate on a
  loose object or a body lying as a ragdoll picks up the body under the
  crosshair and carries it ahead of the view (lifting up to
  `fZKeyMaxForceWeightHigh`, dragging what is heavier; corpses are dragged
  by a limb, and lie where they are left); letting go drops it moving as it
  was, and it slips away when stuck; a tap still takes or searches.
  `OnGrab` / `OnRelease`, `Game.GetPlayerGrabbedRef`, condition
  `IsPlayerGrabbedRef`. Console `loose`, `[ref.]drop`, `grab`, `pwalk`.
  Open questions: `known_ambiguities/loose-objects.md`
- Synchronised blends of single-play clips (directional attacks) end instead of
  cycling, and their heaviest child raises the clip triggers
- Character property bindings: bone switches bound to the character's bone weight
  properties (`LeftArm`, `ShieldOnly`...: `hkbCharacterData` values) layer as authored;
  read-only scalar properties drive supported modifier inputs and pose gates
  (`IsMale`, `IsThirdPerson`), with character-specific runtime caches.
  Limits: `known_ambiguities/character-properties.md`
- Patrol (linked-ref routes with idle markers on the way, repeat / start at nearest),
  follow and escort packages (leading the target to the destination, waiting while it
  lags more than "Distance to Wait for Follower(s)" behind, turning in place to face
  it, running to catch up with one more than "Run If Behind Distance" ahead);
  followers in range face their target; SitTarget. Open questions:
  `known_ambiguities/escort.md`
- Quest alias packages (`ALPC`): actors filling a running quest's reference
  aliases run those aliases' packages ahead of their own, higher priority quests
  first; package conditions run in the owner quest (`QNAM`) and alias locations /
  targets resolve through its fills; whereabouts follow them (Lucan and Camilla's
  MS13 opening, Balgruuf holding court until MQ103 moves on). vrm-tool
  `alias-packages`. Open questions: `known_ambiguities/alias-packages.md`
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
  `ref-types`. Open questions: `known_ambiguities/alias-fills.md`
- Created references: aliases that create theirs (`ALCO`, at or in another
  alias's reference) and Papyrus `PlaceAtMe` / `PlaceActorAtMe` make references
  (`FF000800` up): items in containers go to the inventory, actors appear where
  they were made and live among the persistent actors (packages, whereabouts,
  combat). Console `placeatme`. A quest stopping or starting over clears away
  what its aliases created: items from the containers, objects lying about
  (what the player holds, and actors, stay)
- Force greet packages (ForceGreet / ForceGreetFromSitting templates): the greeter
  waits at its wait location (standing, sandboxing or in its seat) until the player
  is in the trigger location (and seen, if asked), walks up to within the force
  greet distance at the package's speed and opens a conversation with the package's
  topic (or the first `HELO` line); seated ones speak from the seat. Again 10 s
  after the conversation if the package still applies. `GetTalkedToPC`. vrm-tool
  `force-greets`. Open questions: `known_ambiguities/force-greet.md`
- NPC templates (`TPLT`): each "Use ..." flag takes that part (traits, stats,
  factions, AI data and combat style, AI packages, base data, inventory, script,
  attack data and attack race, keywords) from the template, through nested templates
  and leveled NPC lists picked per reference as its looks are, so a leveled draugr's
  health, factions and packages are the ones of the draugr it looks like. Actors'
  `HasKeyword` includes their race's; `GetInFaction` / `GetFactionRank` and door
  ownership follow templates. Leveled spell lists (`LVSP`) in spell lists are
  expanded, rolled once for each actor when its spells are first asked for.
  Console `templates <ref>`; vrm-tool `npc-templates`. Open questions: `known_ambiguities/npc-templates.md`
- Cloaks: the wearer's cloak gives its spell once a second to those within
  its magnitude in feet whom it is hostile to (`IsHostileToActor`); one
  cloak at a time (`known_ambiguities/magic.md`)
- Soul trap: `Actor.TrapSoul` (the soul trap effect's script) and archetype
  23 effects put a dead actor's soul (by level; people's black) into the
  smallest of the caster's empty gems that holds it ("Soul captured!",
  Souls Trapped); inventory transfers (including storage, gifts, barter and
  script removal to another container) carry the per-item souls as well;
  `src/soul_trap.rs`, `known_ambiguities/soul-trap.md`
- Poisons: used from the inventory, asked about (`sPoisonConfirmMessage`,
  a bow's next shot) and put on the weapon held (staffs, a poisoned weapon
  and empty hands refuse); a dose per blow, Concentrated Poison's two
  (Mod Poison Dose Count), a bow's dose leaving with the arrow; Poisons
  Used (`src/poisons.rs`, `known_ambiguities/poisons.md`)
- Placed hazards (`PHZD`: campfires' `FireLgPlacedHazard`, fire plates,
  the Throat of the World's blizzard) give their spell to whoever is within
  reach every target interval (`src/hazards.rs`, vrm-tool `hazards`,
  `known_ambiguities/triggers.md`)
- Wards (Accumulate Magnitude on `WardPower`): a ward held builds at its
  magnitude a second to `fMagicWardPowerMaxBase` times it (whole at once
  dual cast), and after hits builds again; spells, scrolls, staffs and
  shouts from in front are absorbed, wearing it down, or break it (the
  ward gone, its caster staggered: `WardBreak`), doing nothing to the warded either way;
  gear wards (Spellbreaker) absorb without wearing; `OnWardHit` (First
  Lessons' Tolfdir); Ward Absorb's magicka. A concentration spell's
  effects without a duration (the ward's armor too) last while it is held.
  NPCs who know a ward (Vigilants, warlocks, Delphine...) raise it in the
  hand their weapon leaves free while a hostile spell is being cast at
  them, facing its caster, and let it go a moment after; after a ward ends
  or breaks they wait before another. Ward hits pick their reaction from
  the `ActionWardHit` idles through `IsWardState` (`WardAbsorbed`,
  `WardBroken`, draugr, dragon priests' and Hermaeus Mora's daedra's own)
  (`src/wards.rs`, `known_ambiguities/wards.md`)
- Staffs: a staff in hand casts its enchantment as a spell
  through the body's graph (the staff release clips), paid from its charge
  (`EAMT`) at the enchantment's cost less for skill in its school, "Your
  item has insufficient charge" when it runs out; no skill gained;
  `RightItemCharge` / `LeftItemCharge`. NPCs whose style weighs staffs
  (`CSGD` 0x20) take up their staff and fight with it (before their
  spells where the style weighs staffs over magic) until it runs dry.
  Enchanted weapons' blows pay their enchantment from the charge the same
  way (none needed without `EAMT`: bound weapons, the Ebony Blade), the
  blow landing without it once dry; spent charge goes with the weapon from
  inventory to inventory and through dropping and pickup, keeping distinct
  depleted copies of the same weapon. Copy selection remains limited
  (`known_ambiguities/staffs.md`). Charge in the inventory (T: the card's list of
  souls; `src/recharge.rs`): each soul's size in charge, gems used up but
  Azura's Star and the Black Star, Enchanting XP (`src/staffs.rs`,
  `known_ambiguities/staffs.md`)
- Ethereal (Become Ethereal): no blows, bashes or harmful magic land on or
  from the ethereal, no stagger, no falling damage (nor for the paralyzed);
  the player attacking, casting or activating ends it (`src/ethereal.rs`,
  `known_ambiguities/magic.md`)
- Carry weight: what an actor carries weighs its items, worn armor through
  "Mod Armor Weight" (Conditioning, Unhindered); the player over their
  `CarryWeight` walks, told `sOverEncumbered` (`src/encumbrance.rs`,
  `known_ambiguities/encumbrance.md`); worn armor slowing the player and
  loaded NPCs (package gaits, sneaking and combat chasing / fleeing), using
  the shared approximate GMST / perk policy (`fMoveEncumEffect*`, `fMoveWeightMax`)
- Followers' carry limit: a teammate given a stack past its `CarryWeight` refuses
  it whole with `sTeammateOverencumbered` (menus, gifts, fallback UI all go through
  `transfer_item`; `known_ambiguities/followers.md`)
- Falling damage: the player's falls past `fJumpFallHeightMin` hurt by
  UESP's formula (the `fJumpFall*` settings), the "Mod Falling Damage"
  perk entry point (Cushioned) applied; deep water spares them
  (`src/falling.rs`, `known_ambiguities/falling.md`)
- Actor values: one store per actor that conditions (`GetActorValue`,
  `GetBaseActorValue`, `GetPermanentActorValue`, `GetActorValuePercent`), Papyrus
  (`Get` / `Set` / `Force` / `Mod` / `Damage` / `RestoreActorValue`,
  `GetActorValuePercentage`) and the console share: bases from the records (AI
  data, skills, rates, carry weight; health / magicka / stamina from the
  race and the offsets, auto-calculated stats from the class at the NPC's
  level, "PC level mult" ones too: `esp::auto_calc`), a permanent modifier
  and damage over them. Live health and
  stamina are folded in (damaging health to nothing kills; damage done
  while unloaded is taken as the actor loads); changes to health,
  stamina, aggression, confidence, assistance and the armor and block skills
  reach combat. `Variable01`-`10` and `WaitingForPlayer` now gate packages and
  lines as scripts set them. Console `[ref.]getav`, `setav`, `modav`, `forceav`,
  `damageav`, `restoreav`; vrm-tool `av-conditions`. Open questions:
  `known_ambiguities/actor-values.md`
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
  `pack-tree`, `pack-lists`. Open questions: `known_ambiguities/package-trees.md`
- `GetNumericPackageData` conditions read authored Bool, Int and Float inputs
  by index from the candidate package, including expanded template branches
  during loaded and persistent actor package selection. Missing or unsupported
  values fail the condition. Remaining input and context limits:
  `known_ambiguities/package-trees.md`
- `GetWithinPackageLocation` conditions check the subject against a package
  `Location` input placed for the package's actor (force greet, travel-to-radius,
  civil war hold-position and Dawnguard disguised vampire template branches,
  which always failed before). Package start positions and unloaded actors:
  `known_ambiguities/package-trees.md`
- Package procedures: packages with their own procedure tree (draugr ambushes
  in sarcophagi, thrones and alcoves: creatures use furniture through their own
  graphs' `ActionActivate` branch, starting in it and getting out when the
  ambush ends), procedures by tree for unknown templates (HoldPosition,
  GuardPost), and UseWeapon practice: archers take a bow of the package's
  weapon type, draw and loose at the targets in barrages, melee fighters swing.
  Each target's trigger reference / radius gates its attacks; Aim Only holds
  a bow drawn without release, and pauses between barrages are optional.
  Always Power Attack selects a fitting authored melee power attack; rejected
  or unavailable attacks preserve the barrage count; unforced swings are
  sometimes power attacks. Finite barrage counts (Never End off) and the
  attack time limit complete the package, which is then skipped while it
  still applies.
  UseMagic practice (the College's casting at its targets, Colette's spells on
  herself, Neloth's): at the package's place the actor draws its hands and casts
  the package's spell, holding a concentration spell for the cast time, with
  the cooldown between casts and a number of casts before the package is
  done; "hold when blocked" waits while something is in the way.
  Activate packages (the sawmill workers' lever and log pile): the actor goes to
  its target, uses it if it is furniture, and the package is done after "Number
  to Activate" uses; its place is its target's.
  Open questions: `known_ambiguities/package-trees.md`,
  `known_ambiguities/use-weapon.md`, `known_ambiguities/use-magic.md`,
  `known_ambiguities/activate.md`
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
  doors the player opened stay open, and doors actors left open at unloading
  (`ADOR` in saves) stay open until they close; references scripts move (`MoveTo`,
  `SetPosition`, `SetAngle`, `MoveToMyEditorLocation`, `GetAngleX/Y/Z`) move
  with their collision and lights, between cells too, and stay moved. Console
  `[ref.]enable` / `disable`, `[ref.]moveto`. Open questions:
  `known_ambiguities/world-state.md`
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
  `setstage`; vrm-tool `scenes`. Native saves preserve running scene lifecycle,
  action bindings, selected dialogue responses and already chosen looping
  dialogue waits, pending restarts and scene force-greet completion markers in
  optional adjunct sections; live player conversations, audio playback and
  package procedure progress are not resumed. Scene snapshot references are
  checked against loaded plugin definitions before applying; invalid data leaves
  the live game intact. Open questions:
  `known_ambiguities/scenes.md`
- Story Manager (also pick lock `LOCK` and trespass `TRES` events, no vanilla node
  listens): events run down the tree of event, branch and quest nodes
  (`SMEN` / `SMBN` / `SMQN`) under their type, stacked or random, node
  conditions on the event's data, quests started with reset hours, max running,
  num to run, do all before repeating and shared events, "warn if no child
  quest started" (a logged warning). Events: NPCs striking
  up conversations (`ADIA`: every town's conversation quests and their scenes),
  the player changing location (`CLOC`), script events
  (`Keyword.SendStoryEvent`), kills (`KILL`). "From event" aliases,
  `GetEventData` and run-on-event-data conditions. Quests started from Papyrus
  get their scripts once the running scripts yield (they were lost before).
  More events: the player taking items (`AIPL`: picked up, from containers and
  bodies, stolen), NPCs greeting the player (`AHEL`), finding bodies (`DEAD`),
  assaults (`ASSU`, crime when the victim keeps the law; blows to the player
  too, never a crime; NPC kills have crime status 0), relationship rank
  changes (`CHRR`), the player dropping items, giving them and putting them in
  containers (`REMP`: the `WIRemoveItem` quests; selling sends nothing,
  `known_ambiguities/story-manager.md`). Crime events: crime gold added (`ADCR`), the player
  giving themselves up to the guard arresting them (`ARRT`: Thieves Guild
  `TG00`'s arrest monitor), jailed (`JAIL`: `JailQuest`, the Windhelm jail
  fight), escaping (`ESJA`: "Retrieve your possessions", the achievement)
  and serving the sentence (`STIJ`), paying (`PFIN`). Quests started by an
  event get its Papyrus `OnStory...` event with the data. Quests' own event
  conditions. Relationships (`RELA`):
  `GetRelationshipRank` and highest / lowest, Papyrus get / set. Conditions
  `HasAssociationType` / `HasAssociationTypeAny` and `HasFamilyRelationship` /
  `HasFamilyRelationshipAny` query direct authored links and their association's
  family flag, `HasParentRelationship` the parent end of a Parent / Child link;
  secret metadata retained, behavior still pending
  (`known_ambiguities/relationships.md`). Conditions
  `GetIsCurrentPackage`, `IsInList`, `DoesNotExist`, location keyword data.
  Cleared locations: by their bosses dying or `SetCleared`, with the banner,
  the cleared music and Dungeons Cleared; `IsCleared`, `GetLocationCleared`,
  `GetLocationAliasCleared` (`known_ambiguities/cleared.md`).
  Console `storyevent` (with `v1=`, `f1=`...), `setrelationshiprank`;
  vrm-tool `story` (`CONDS=1`, `OWN=1`). Open questions:
  `known_ambiguities/story-manager.md`
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
  Nonviolent)`, `GetInfamy(Violent / NonViolent)`, `GetIsCrimeFaction`, `CanPayCrimeGold`,
  `GetInSharedCrimeFaction`. Stolen goods stay marked as their owners' (red in
  the menus, the red "Steal" / "Steal from" prompts) and are confiscated into
  the faction's stolen goods chest on paying; `GetStolenItemValue(NoCrime)`.
  Dropping stolen goods keeps their owner on the reference (one per owner of
  a mixed stack), so picking them up stays theft; what the player dropped
  unowned is theirs, even in an owner's house. Picking a lock someone else owns is a crime when seen. An owned reference's
  required faction rank (`XRNK`, on it or its cell) counts: a member below it
  is as good as outside for stealing, locks, beds and cells. Scripts put actors in
  and out of factions (`AddToFaction`, `RemoveFromFaction`,
  `RemoveFromAllFactions`, `SetFactionRank`, `ModFactionRank`; conditions and
  `GetFactionRank` follow), and the members that let the player take a
  faction's things (the favor cap) follow too. Trespassing: in an
  off limits cell, or a home while its owner's package locks the doors (not
  public areas), whoever sees the player warns them with the Trespass topic
  (`GetTrespassWarningLevel`, `IsTrespassing`), warns again, then calls the
  guards and reports it (`fAITrespassWarningTimer`); off limits at once.
  Pickpocketing while sneaking: the victim's pockets (what it wears or
  wields with Misdirection or Perfect Touch; a poison placed with Poisoned
  takes effect, unnoticed) with each item's chance from the `fPickPocket*` settings, taken stolen,
  or the player's own items placed in them with the same chance (the
  victim wearing or wielding them when better, as teammates given them do);
  caught, a crime the victim reports and won't let the player try again.
  Console `pickpocket`; vrm-tool `trespass-cells`, `ctda-uses`,
  `topic-lines`. Arrests: guards (`IsGuard`: `IsGuardFaction`, the class
  flag) near a reported crime run after the player calling on them to halt
  and stop them with the arrest dialogue (blocking branches, `GetAlarmed`):
  pay (on the spot, or let out at the jail), go to jail, or resist; walking
  off is resisting; sheathing afterwards is yielding (the guards stop and
  arrest again, once a fight). Factions' Arrest / Attack on Sight flags make guards
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
  `crime-factions`. Open questions: `known_ambiguities/crime.md`
- Trigger volumes: scripted box and sphere primitives (`XPRM`), and scripted
  references whose model has phantoms (`bhkSimpleShapePhantom`: pressure
  plates, oil pools, trip wires), send
  `OnTriggerEnter` / `OnTriggerLeave` as the player and actors step in and out
  (set-stage, start-scene, music and comment triggers...), and `OnTrigger`
  about twice a second for each one inside (poison gas and lava place
  their explosions on who stays); `GetTriggerObjectCount`. Activate parents (`XAPR`): activating a reference
  (the player, `Activate`) activates the references naming it as parent after
  their delays, unless its activation is blocked, so pressure plates set off
  trap linkers and linkers their rockfalls; "Parent Activate Only" references
  can't be activated by the player. Console `sv` (a reference's script
  variables); vrm-tool `pex-calls`.
- Trap hits: loose objects whose scripts handle trap events send
  `OnTrapHitStart` / `OnTrapHit` / `OnTrapHitStop` while touching living
  actors or the player; `ProcessTrapHit` deals their damage and stagger.
  Open questions: `known_ambiguities/traps.md` Papyrus variables and locals start at their type's
  default (an `Int` at 0, not None). vrm-tool `triggers`. Open questions:
  `known_ambiguities/triggers.md`
- Water types: loaded cell surfaces retain their resolved `XCWT` / worldspace
  `NAM2` water type; `IsInDangerousWater` tests the `WATR` `FNAM` damage flag
  for actors touching it, including wading. Console `[ref.]isindangerouswater`;
  contact threshold and unloaded actors: `known_ambiguities/water.md`
- Detection: one detection value per observer and target from the CK wiki's
  formula and the game settings (`fSneakBaseValue`, `fSneakMaxDistance` and
  its exterior multiplier, distance attenuation; sound from movement, worn
  armor weight and running, muffled out of sight; sight within a view cone
  with a clear line, by the light level where the target stands: ambient,
  unshaded sun and point lights. The observer's Blindness actor value reduces
  only vision while the target sneaks and the observer is out of combat; the
  observer's Sneak contributes against a sneaking target. Sleepers see nothing.
  The player's stealth points drain and refill
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
  `known_ambiguities/detection.md`
- Papyrus `SetAlert` / `IsAlerted` and condition `GetIsAlerted` share an explicit
  actor flag retained across cell unloading in memory. Automatic detection-state
  coupling and weapon-posture effects remain unresolved:
  `known_ambiguities/detection.md`
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
  on change (`PKDT`) open them, so shops close at night (unloaded actors' as
  their packages start: the schedule is looked at each half hour of game
  time). Papyrus `Lock`,
  `IsLocked`, `GetLockLevel`, `SetLockLevel`; conditions `GetLocked`,
  `GetLockLevel`; console `lock` / `unlock`, `picklock`; vrm-tool `locks`. NPCs
  open locked animated doors only with the key or owning them (cages and cells
  stay shut). Open questions:
  `known_ambiguities/locks.md`
- Keyframe animation: NiControllerManager / NiControllerSequence / NiTransformInterpolator
  / NiTransformData; animated nodes drawn as separate parts; doors open and close (player
  activation, NPCs walking through) with their leaves' collision; statics loop "Idle"
- Loaded placed objects' standalone `NiTransformController` / `NiVisController`
  chains on nodes and rigid `BSTriShape` / `NiTriShape` / `NiTriStrips`
  geometry (also in effects): nested poses and visibility, independent controller clocks and
  block-identity targets, scaled bounds and hidden parts omitted from shadows
  and particles. Limits and timing choices: `known_ambiguities/node-controllers.md`
- Standalone reverse controller cycles respect nonzero start times, frequency
  and phase across node, material and particle animation; remaining timing
  questions: `known_ambiguities/node-controllers.md`
- Particle systems: `NiParticleSystem` with its data, emitters (box, cylinder, sphere),
  birth rate and visibility controllers, gravity, drag, rotation, scale, simple colour,
  subtexture flipping and spawning on death, run on the CPU near the camera and drawn as
  camera-facing (or motion-aligned) quads with their effect materials: fires burn and
  smoke. Drag respects its object's axis, position, scale, range and falloff,
  preserving perpendicular velocity. Greyscale palettes are looked up clamped.
  Rigid mesh emitters sample vertices, face/edge centres and surfaces from
  referenced meshes, with normal, random or axis velocity; weighting and
  deformation limits remain documented below.
  Collider managers resolve planar and spherical chains at object rest
  transforms, with swept point contacts, bounce, collision spawning/death and
  bounded repeated impacts; contact and response policies remain documented below.
  Speed-to-aspect (aspect flag bit 8) interpolates between authored ratios
  using current model-space speed, composed with rotation and velocity orientation;
  interpolation, sizing and speed-space policies remain documented below.
  Standalone initial rotation angle, speed and variance controllers sample
  birth values on their own clocks, with modifier activation; axes, sequence
  ownership and interpolation limits remain documented below.
  Open questions:
  `known_ambiguities/particles.md`
- Addon nodes: a model's `BSValueNode`s take the addon node (ADDN) whose index is
  their value, its model (the `MPS` master particle systems) placed at the node and
  merged into the host's: candle and chandelier flames, torches' fire and embers,
  forges, chimney smoke and street mist (242 vanilla models). `VRM_NO_ADDONS=1` turns
  them off; vrm-tool `addons` lists them and their users
- Billboard nodes (`NiBillboardNode`): their subtrees turned to the camera in the vertex
  shader about the node's origin (facing the view direction or the camera, or turning about
  the up axis only, by mode): fires' glows and flame cards
- Shader property controllers (`BSEffectShaderProperty*` / `BSLightingShaderProperty*`
  float and colour controllers with `NiFloatData` / `NiPosData` keys): UV scrolling and
  scaling, emissive colour and multiple, alpha and falloff animate on every material
  that has them (clouds, auroras, waterfalls, magic), by the controllers' frequency,
  phase and loop / reverse / clamp cycles
- Standalone alpha-test controllers (`BSNiAlphaPropertyTestRefController`):
  alpha-property chains merged with shader animation, keyed and constant
  references on the existing material clocks, shared object / shadow cutouts.
  Open questions: `known_ambiguities/alpha-test-controllers.md`
- Named NIF sequences on placed rigid objects bind shader float / colour and
  alpha-test interpolators by exact controller reference, including material-only
  animated parts. Each reference has its own material uniforms and sequence
  clock, with terminal samples retained. Reverse cycles retain both playback
  legs with one sampled key clock for transforms and materials, including start
  offsets and positive nonunit frequency. Sequence timing and arbitration choices,
  and remaining actor / effect scope: `known_ambiguities/sequence-material-controllers.md`
- Environment maps: lighting shaders with environment (or eye environment) mapping
  reflect their cube map (texture set slot 4, sampled with the world's axes: the vanilla
  maps keep the sky on +Z) along the reflected view, masked by the environment mask
  (slot 5) or else the normal map's specular alpha, scaled by the shader's
  environment map scale (and its controllers) and lit like the surface: Dwemer bronze,
  ore veins, glass, ice, eyes. Textures named in a slot of the wrong kind (a cube map as
  a mask) fall back to the slot's default; `VRM_NO_ENVMAP=1` turns them off for
  comparison; vrm-tool `envmap-shapes` counts the vanilla shapes and cube maps.
  Open questions: `known_ambiguities/environment-maps.md`
- Decal shapes (`sf1` Decal / Dynamic Decal: head parts over the face, moss and grime
  overlays on architecture) drawn with their depth biased towards the camera so they
  don't fight the surface under them; vrm-tool `sf1-files <bit>` lists the models with a
  shader flag (about 5600 vanilla models have decal shapes)
- Grass: landscape textures' grasses (LTEX `GNAM` -> GRAS) scattered over loaded cells on
  a 20-unit grid where those textures show, by density, slope, water distance, position /
  height / colour range, fitted to the slope and tinted by the landscape's vertex colour;
  drawn instanced per cell and grass, swaying in waves with the wind by their vertex
  alpha and fading out from 7000 units. `VRM_NO_GRASS=1` turns it off; vrm-tool `grass`
  lists the grasses. Open questions: `known_ambiguities/grass.md`
- Projected decals: references to texture sets with decal data (`DODT`: blood sprays,
  burn marks, giants' paint, puddles) cast from the reference along its +Y onto the cell's
  collision and drawn as boxes over the opaque scene's depth, lit like the surface they
  land on (sizes between the data's minimum and maximum, 2x2 subtextures, enable state;
  normal maps, shininess specular, parallax with a height map, hard-edged cutouts).
  `VRM_NO_DECALS=1` turns them off; vrm-tool `decals` lists them. Open questions:
  `known_ambiguities/decals.md`
- Soft effects: effect shaders with the soft effect flag (`SLSF1` bit 30) fade where
  they meet the opaque scene, by their soft falloff depth in world units (fog cards,
  steam, smoke and fire at the floor and walls; particles with such a shader too): the
  opaque pass's depth is copied before the blended draws, and only in frames that have
  such draws. `VRM_NO_SOFT_EFFECTS=1` turns it off; vrm-tool `soft-effects` lists the
  flag and depths. Open questions: `known_ambiguities/soft-effects.md`
- Sound categories: a sound descriptor's volume carries its category's (`GNAM` -> SNCT:
  `VNAM` static volume multipliers and, for the Audio menu's four categories, their
  `UNAM` defaults, up through the master category); music plays at the music
  category's volume and dialogue lines at the voice category's. Open questions:
  `known_ambiguities/audio-categories.md`
- Sound descriptors' variance: each playing of a one-shot, a reference's loop, thunder
  or a weather loop draws its playback rate (`BNAM` frequency shift and variance,
  percent) and gain (dB variance) afresh; the mixer plays voices at a rate. vrm-tool
  `sndr-stats` lists the values. Open questions: `known_ambiguities/sound-variation.md`
- Sound output models (SOPM): a descriptor's `ONAM` gives its minimum and maximum distance
  and the five-point attenuation curve its gain follows between them (the mixer's
  `Falloff`); 2D models (the menus', dialogue's) play unpositioned; dialogue lines fade by
  the default dialogue model. Open questions: `known_ambiguities/sound-output-models.md`
- Dialogue responses' speaker idles: a response whose INFO names one (`SNAM`: pointing,
  giving, saluting, reading...: 475 vanilla responses) gestures with it instead of the
  random `ActionTalking` one, in conversations, scenes and barks; one the speaker's graph
  won't take falls back to the random gesture. vrm-tool `info-idles` lists them. Open
  questions: `known_ambiguities/dialogue-idles.md`
- NIF float / colour `CONST_KEY` interpolation retains its source type and
  selects nearest endpoints for shader and alpha-test channels, named material
  sequences and particle float curves. Boolean keys retain their hold behavior.
  Midpoint ties and remaining interpolation limits:
  `known_ambiguities/value-key-interpolation.md`
- Quadratic `NiFloatData` / `NiPosData` curves retain and evaluate their
  tangents for standalone and named-sequence material / alpha-test channels
  and particle float controllers. Transform interpolation remains separate
  work; evidence and limits:
  `known_ambiguities/quadratic-value-interpolation.md`
- TBC `NiFloatData` / `NiPosData` curves derive Hermite tangents for the same
  material and particle channels, with nonuniform time weights. Public sources
  disagree on bias / continuity order; the schema-order policy and remaining
  parity limits are recorded in `known_ambiguities/tbc-value-interpolation.md`
- Impacts: melee blows, bashes and arrows look up the impact (IPCT) their impact data
  set (weapon `INAM`, bash `BIDS`, the race's `NAM5` bare-handed) has for the material
  struck (the target race's `NAM4`, the blocking shield's or weapon's `BAMT`, the
  surface's for arrows in the world): both sounds (`SNAM`, `NAM1`) play,
  including for footstep impacts; its effect model runs (blood
  sprays, emitting for the impact's duration) and its decal lands, for a wound on the
  floor or wall behind the target (at most 64 kept) and on its body: a box held by the
  bone passing nearest the wound, moving with the pose and ragdoll (8 an actor, 48 in all).
  Effect models play on a clock of their own from their text keys' `start`: their
  meshes drawn for the impact's duration, the nodes with controllers of their own
  (`NiTransformController`, `NiVisController`) moved, scaled and shown by them (the
  sprays' emitters swept round, ice shards flying), their shader controllers
  started with them (`APP_INIT` ones too), world-space particles left where they
  were emitted as their emitters move. An impact's hazard (`NAM2`) is left where
  it lands: its spell touches whoever is within its radius on its interval for its
  record's lifetime, the oldest of its kind going at its limit.
  Console `impact`; `VRM_NO_IMPACTS=1`
  leaves out effects and decals; vrm-tool `impacts`. Open questions:
  `known_ambiguities/impacts.md`
- Objects' behaviour graphs (`BSBehaviorGraphExtraData`: traps, pressure plates, levers,
  portcullises, puzzle pillars): the project runs on the model's own nodes, the nodes it
  moves (those with transform controllers) drawn apart and their collision carried;
  `PlayAnimation` / `PlayAnimationAndWait` / `Debug.SendAnimationEvent` send it events,
  `PlayGamebryoAnimation` plays keyframe sequences. A pressure plate goes `Down`, its
  rigged rockfall `break`s and the rocks fall. Scripts get `OnCellLoad` once a
  cell's scripts are all attached; `GetOpenState` (Papyrus and condition) follows
  animated doors and graphs' `Open` / `Close` events, the Papyrus call answering
  at the end of the frame. The rotating pillar puzzles turn, solve and open their
  gates; a pillar turned wrong afterwards closes the gate again.
  Open questions: `known_ambiguities/object-graphs.md`
- Animation events for scripts: `RegisterForAnimationEvent` /
  `UnregisterForAnimationEvent` (forms, aliases, magic effects) send the events an
  actor's or object's graph raises as `OnAnimationEvent`; `PlayAnimationAndWait`
  suspends its script until the graph raises the awaited event (swinging blades
  wait for `reset`, portcullises for `opening` / `closing`); objects' graphs play
  their `SoundPlay.<sound>` events. Parts the graphs move hit as keyframed traps
  (swinging blades and battering rams deal their damage); user-controlled and
  ping-pong clips; references' scripts keep their base's properties they don't set;
  `Get/SetAnimationVariableBool/Int/Float` on actors' and objects' graphs (a script's
  value holds against the AI's own writes: `known_ambiguities/object-graphs.md`). NPCs
  getting into furniture activate it (`OnActivate`): a wood pile's
  `CarryFurnitureScript` hears the pick-up's `AddToInventory` and gives the actor
  its firewood. Carry furniture: the idle tree's ways on are picked for users with
  and without the load (`GetItemCount`), so a wood pile picks up for the
  empty-handed and puts down for those carrying firewood (`RemoveFromInventory`
  takes it back); after the pick-up the actor leaves by the tree's instant exit
  (`IsExitingInstant`: `OffsetCarryLogStart`) and carries the logs on its shoulder
  until it puts them down
- Messages and the journal: `Message.Show` puts up a message box (title, text,
  the buttons whose conditions pass, `sOk` when none) and its script waits for
  the button, resuming with its index (`Vm::signal_with`); `Debug.MessageBox`;
  other messages as notifications, with MESG `TNAM` display times in the
  fallback HUD (the SWF's animation still controls its notification lifetime).
  Text tags: `%.0f` from `Show`'s arguments,
  `<Alias=...>` with `.ShortName` and pronoun forms, `<Global=...>`, `[Control]`
  as the key bound to it. Help messages (`ShowAsHelpMessage`: shown for a while,
  coming back after an interval up to a number of times, until the player does
  the input event; `ResetHelpMessage`). Quest stages apply each log entry whose
  conditions pass (its text into the journal, completing or failing the quest,
  and only its fragment runs). Objectives displayed, completed and failed
  (`CompleteAllObjectives`, `FailAllObjectives`...); "Quest added" /
  "completed" / "failed" banners. A journal (J): quests with a type, the
  miscellaneous ones together, finished ones apart, log and objectives. Console
  `sqs`, `sqo`, `journal`, `msgbox`, `inputevent`; vrm-tool `messages`,
  `quest-log`. Open questions: `known_ambiguities/messages.md`
- Menu mode: every menu but dialogue pauses the game (message boxes, journal,
  inventory, containers, books, lockpicking, the console: CommonLibSSE's
  `kPausesGame`). The world, the game clock and real-time timers stand still;
  scripts run on, `Utility.Wait` waits for menu mode to end while
  `WaitMenuMode` counts on (`NativeResult::WaitMenuMode`, `Vm::menu_time`);
  `IsInMenuMode`, `GetCurrentRealTime` with menus included. Offscreen runs press
  the last button of a box left up (`--hold-boxes` keeps it). Console
  `menumode`, `closemenu`
- Image space modifiers (IMAD): the frame drawn through a post pass when one is
  on: saturation, brightness, contrast, tint, fade colour, blur, double vision
  from their curves (animatable ones play over their duration and end, static
  ones hold until removed); Papyrus `Apply`, `ApplyCrossFade`, `PopTo`,
  `Remove`, `RemoveCrossFade`. `Game.FadeOutGame` (to and from black or white)
  and `Game.ShakeCamera`. Console `imod`, `rimod`, `imods`, `fadeout`, `shake`.
  The base image space (IMGS) under them: the interior's (XCIM, else
  `DefaultImageSpaceInterior`) or the weather's for the time of day (IMSP),
  its saturation, brightness, contrast (about the frame's average) and tint
  drawn every frame; vrm-tool `image-spaces`.
  Open questions: `known_ambiguities/image-space.md`
- Weather over time: the current and outgoing weather with a transition
  (sky, light, fog, clouds and image space mixed), regions' weathers from the
  player's cell (RDWT, by chance), rolled again every few hours, leaving a
  region bringing in one of the new region's; overrides. Papyrus `Weather`
  `ForceActive`, `SetActive`, `ReleaseOverride`, `GetCurrentWeather`,
  `GetOutgoingWeather`, `GetCurrentWeatherTransition`, `GetSkyMode`,
  `FindWeather`, `GetClassification`; conditions `IsRaining`, `IsSnowing`,
  `GetWindSpeed`, `GetCurrentWeatherPercent`, `IsPleasant`, `IsCloudy`.
  Weather sounds outdoors: rain and wind loops fading with the transition,
  thunder now and then with a lightning flash (console `thunder`). Rain and snow (the weather's MNAM precipitation,
  SPGD) as particles in a box about the camera, falling at their gravity
  velocity, snow turning about its falling centre, both blown by the
  weather's wind (DATA direction and range); coming in from the
  precipitation's begin fade in, going out by its end fade out. Rain and snow
  kept off under roofs and canopies by a top-down depth map of the opaque scene about the
  camera. Sky statics
  (the cloud meshes placed about the world, TNAM) shown only in the weathers
  listing them, fading with the transition, in the sky statics colour, the
  full LOD ones loaded with the worldspace however far. Auroras
  (MODL) at night or always (`kPermAurora`), turning with the sun
  (`kAuroraFollowsSun`). Console `fw`,
  `sw`, `rwo`, `weather`. Open questions: `known_ambiguities/weather.md`
- Destructible object definitions, initial stage queries and scripted
  `ObjectReference.DamageObject` health/stage progression: read base
  `DEST`/`DSTD` health, flags, stage records, explosion/debris references and
  optional replacement model paths; `ObjectReference.GetCurrentDestructionStage`
  and CTDA `GetDestructionStage` (470) report untouched references as `-1`.
  `DamageObject` mutates loaded references independently, with documented
  threshold and cap policy choices. `ClearDestruction` clears one reference's
  runtime health/stage state without events, including when it is unloaded.
  `OnDestructionStageChanged` queues old/final stage integers when damage
  changes the stored stage, through the standard reference/effect/running
  alias dispatcher; multi-stage skip timing is a documented policy choice.
  The engine's optional `DSTR` save section persists and replaces this runtime
  health/stage map without replaying events; older snapshots without it load
  empty. This does not implement Skyrim `.ess` destruction saves:
  `known_ambiguities/destruction.md`
- Weapon, bash and arrow blows on loaded destructible references take their
  damage as health (`strike` -> `damage_object_by_blow`), skipped while the
  current stage has Ignore External Damage (DSTD 0x08); power attacks add no
  extra damage to objects (unconfirmed, see `known_ambiguities/destruction.md`).
- `GetScriptVariable` reads a reference's Papyrus variable named in `CIS2`
  (`known_ambiguities/condition-variables.md`)
- Positional conditions: `GetPos`, `GetAngle`, `GetStartingPos`, `GetStartingAngle`
  (axis 'X'/'Y'/'Z'; starting transforms use editor placement or the saved
  origin of a reference created in the world), `GetHeadingAngle`,
  `GetRelativeAngle`, `GetWithinDistance` (package float inputs),
  `IsCloserToAThanB`, `GetLineOfSight`; choices and gaps in
  `known_ambiguities/condition-geometry.md`
- Hostile spells that damage health take their magnitude off destructible
  references they land on (touch on destructibles, projectile, beam and location spells) or
  whose area reaches them (`spell_lands` -> `damage_object_by_blow`), under the
  same Ignore External Damage gate; explosions are not done.

## Next
1. **Animation: behaviour graphs at runtime** (humanoids run their graphs; see Done)
   - Pose modifiers: twist chains, non-additive modes and branch blending
     (`known_ambiguities/twist-modifier.md`), keyframe bones; foot IK locking planted feet, aligning the
     body with the ground (`alignWorldFromModelGain`), legs' ungrounded events;
     creatures' look-at modifiers (unbound:
     the game picks and aims them itself); NPCs looking at each other in conversation
   - Objects' behaviour graphs (see Done): graph state in saves (kept across
     cell loads in memory)
   - Chooser start states, selectors' own blends
     (`known_ambiguities/selectors.md`)
   - NPCs strafing (`Direction`: see the player's body), sprinting; character
     properties on remaining generator members (clip `enforcedDuration` and
     blender `indexOfSyncMasterChild` are supported), blender-child members
     beyond `weight` and `boneWeights`, and transition-effect members beyond `duration`,
     `selfTransitionMode`, `eventMode`, the supported subset of `blendCurve`,
     compound-target behavior for `toGeneratorStartTimeFraction`, and source
     end-time behavior beyond direct single-play clips and transparent wrappers,
     runtime overrides and lifecycle gates (`known_ambiguities/character-properties.md`;
     `vrm-tool hkb-props` lists the bindings: the humanoid and dragon projects bind
     only two modifiers' `enable`);
     feeding `hkbRotateCharacterModifier`'s yaw (computed: see Done) into ordinary NPC
     heading in place of the AI's own turning (`known_ambiguities/rotate-character.md`); the other action trees
     (`ActionTurnLeft`...) as the way events are chosen
   - Carry furniture (wood piles done, see Done): packages or sandboxing that
     take a carrier to where the load goes (now any wood pile it happens to pick);
     buckets (`OffsetCarryBucketStart`: nothing in the idle tree sends it) and
     DLC2 stone piles (no instant exit to carry them away)
   - Delayed state changes (`FLAG_DELAY_STATE_CHANGE`; `vrm-tool hkb-flags` counts
     and lists the flags);
     change-notification activation / completion / interruption timing
     (`known_ambiguities/state-machine-events.md`)
   - Engine variables and events still missing: combat, first person;
     INFO listener idles (the speaker's are done, see Done); listeners' reactions
   - Footsteps: swimming (and swimming itself), splashes, decals
   - Later / separate: standalone controllers on skinned geometry, particle systems, attachments,
     distant and loose objects; collision following standalone transforms,
     persistence and driver arbitration (`known_ambiguities/node-controllers.md`);
     texture flip controllers (`NiFlipController`;
     `known_ambiguities/texture-flip-controllers.md`); actor / effect graph shader
     controllers inside sequences and sequence blending (placed rigid objects'
     named material sequences: see Done;
     `known_ambiguities/sequence-material-controllers.md`)
2. **Story Manager** (core and seventeen events done, `LOCK` and `TRES` among them; see Done)
   - Events waiting on their systems: crafting, NPCs'
     spell casts (combat suppression and concentration cadence need evidence;
     `known_ambiguities/story-cast-events.md`), new voice powers, bribe /
     intimidate / flatter; hellos between NPCs and creatures'
   - Secret relationship behavior (`known_ambiguities/relationships.md`)
3. **Detection (stealth)** (see Done); what's left waits on other systems or
   sources:
   - Quiet Casting, Shadow Warrior (with casting); spells
     and shouts as action sounds (the formula's slots for them are in
     `crate::detection`)
   - Weapon and impact sounds (action sounds, detection events): need the
     `iSoundLevel*` values, which no public source gives; weapons are silent
     until then
   - Finding bodies by detection
   - Combat searches following a target through load doors; automatic alert-flag
     changes and weapon-posture effects of `SetAlert` need evidence
     (`known_ambiguities/detection.md`); witnesses who hear a sneak attack
     alerted (with action sounds)
4. **AI depth**
   - Actor execution across loading and processing transitions: durable state
     independent of loaded animation and physics, keeping living actors' positions,
     current packages and procedure progress when they unload. Define completion,
     failure, interruption and resumption, package lifecycle events / fragments,
     and scene progress while actors are away; attach / remove animation and
     physics without restarting the actor's execution. This is the foundation
     for the package-tree, off-screen travel, unloaded actor-value and save work.
   - Processing policies: detailed updates nearby and coarse advancement away,
     with travel progress through cells and load doors, separate game / real-time
     timestamps and elapsed-time handling for waiting, sleeping and fast travel.
     Define continuity for wounds, regeneration, bleedout and timed effects;
     include execution state, travel progress and timestamps in saves.
     CommonLibSSE exposes High, MiddleHigh, MiddleLow and Low processing levels,
     but their exact eligibility, update cadence and subsystem behavior need
     investigation. Keep room for intermediate processing policies; matching
     observable behavior is the goal, not copying four internal allocations.
   - Comparative tests (planned, against Skyrim SE): walk away and return, then
     compare with interior transitions and buffered cells; follow a long journey
     across cells and load doors; change schedules and run multi-step packages
     while distant; interrupt travel, wait / sleep, fast travel and save / reload
     mid-journey; unload and reload wounded actors and actors with timed effects.
     Record exact processing tier (not threshold helper results), 3D / cell
     attachment, position, package / procedure progress, game / real time and
     events. Check re-entry placement and furniture state, event ordering and
     continuity; control for instrumentation such as quest aliases changing
     persistence. Use the observations to establish transition and catch-up rules
     before treating the current approximations as compatibility requirements.
   - Whole-cell sandboxers as their cell loads: the game has them at
     something to use anywhere in the cell (`test-mod autotest7`); ours start
     where they stood
   - Alias fills: created actors cleaned up; "from alias" locations
     followed through travel, not only at refresh; packages for "from event"
     aliases, and package targets by object id / type and interrupt data
   - Remaining procedures: guard (restricted areas), dialogue,
     carry, flee, the sawmills' lever and log-pile cycle (object graph events), orbit packages; ambush triggers (sleepers'
     reduced detection: see Detection);
     escorts' follower min / max distances and several followers, riding,
     the follower object list and `PreferPreferredPath?`; fleeing to doors, cover and allies (`fCombatFlee*`),
     threat
     avoidance outside combat (`fCombatAvoidThreatsChance`, `AVTH`); actors
     that can't reach their target backing away; force-greets' "moving into a
     new space", player keyword and riding checks, "Greet Using Preferred
     Path?", `ForceGreetWaitSitting` and alias trigger locations
   - Followers: teammates' following through load doors, fast travel and
     waiting, their other combat behaviour (`known_ambiguities/followers.md`);
     `AllyKilled` lines
   - Locks: sleepers warning the player before locking and putting them out;
     actors stepping round loose clutter instead of shoving it
   - Off-screen travel between worldspaces; paths round locked animated doors
   - Package trees: branches of `Sequence` / `Simultaneous` roots (multi-location
     sandboxes' stops done: see Done; their "in cell" stops elsewhere, per-stop
     unlocking and progress kept across loads remain),
     guard / wait / find / acquire procedures; numeric package-data defaults,
     runtime changes and contexts beyond candidate package selection
     (`known_ambiguities/package-trees.md`)
   - Vehicles: carts pulled by horses (`TetherToHorse`; the carriage held to the
     horse by physics constraints), passengers riding seated in them (`SetVehicle`)
   - Dragons: flying, circling, landing and perching; breath and shout attacks
     (with magic)
   - NPC encumbrance: `IsOverEncumbered` covers the player only (`src/encumbrance.rs`);
     whether NPCs over their carry weight are slowed or changed is unconfirmed, so
     other actors fail it (`known_ambiguities/comparison-conditions.md`)
5. **Game logic**: combat (crossbows and bolts, arrows in hand while drawing and
   stuck in actors, sneak shots, tempering
   and enchantments, killmoves and the decapitations they end in, the player's own weapon and
   animations), magic (see Done; next: native healing thresholds and timing, spells chosen by their
   scores, NPCs raising wards against spells already in flight and arrows (`known_ambiguities/wards.md`), summons and buffs in combat, runes, effect visuals (spell projectiles' models run
   behaviour graphs, particles and shader controllers; casting and hit art,
   lights) and sounds, the other archetypes (Illusion's calm, fear, frenzy
   and rally, summons, bound weapons done: see Done): NPCs summoning, reanimation,
   bound weapons in the casting
   hand, light, paralysis,
   invisibility's constant sources, potion rules and other interaction paths
   (`known_ambiguities/invisibility.md`); illusion spells chosen by NPCs; enchantment charges,
   NPCs poisoning their weapons; `known_ambiguities/magic.md`), shouts (see Done: NPCs and dragons
   shouting, the push throwing actors, "Spend Soul?"), perks (see Done: the entry points
   waiting on casting, crafting, barter, attack types;
   `known_ambiguities/perks.md`), inventory (player's, armor from
   inventory, ammo / quivers; torches in dark interiors, burning out), crime
   (escape routes, yielding, skill loss in jail; witnesses running to the
   guards; fences; see Done)
   - Death conditions: `GetCauseofDeath` (the cause's values are unknown,
     `known_ambiguities/death.md`) and `GetKillingBlowLimb` (the killing blow's
     hit limb isn't kept)
   - The player's body (see Done): first-person arms and weapons (the
     `_1stperson` skeleton, `1stperson*` models, its own graph); casting,
     Critical Charge, basic attacks by direction and sprinting
     (`attackStartSprint`, the left hand's `...SprintLeftHand`: left out of
     the attack data with the mounted ones), `attackStop`; bashes landing at
     the graph's `HitFrame` (bashes land at once now); footsteps from the graph's events (now the
     first-person stride), foot IK and head tracking; a ragdoll when they die;
     the camera stopping short of actors, a vanity camera; the head's morphs
     (`.tri`), tint layers (`TINI` / `TINC`), hair colour (`HCLF`) and
     texture sets (`HDPT` `TNAM`), with character creation
   - `GetQuestVariable` (79): its variable is a number in the CTDA with no CIS2 name
     (vanilla uses it 29 times); waits on a number-to-variable mapping
     (`known_ambiguities/condition-variables.md`)
   - Player control from scripts: `SetPlayerAIDriven` (player package execution,
     including scene and quest-alias packages)
   - Sneak attacks (see Done): NPCs sneaking up on the player
   - Skills (see Done): the other skills' uses as their systems come (magic,
     crafting, speech); Guardian Stone activation and verifying sleeping's
     Rested spell selection (UESP sources in `known_ambiguities/skills.md`).
     Active effects' "Perk to Apply" is supported (see Done); temporary perk
     ability / quest sections remain unresolved (`known_ambiguities/effect-perks.md`)
   - Physics: making fixed objects dynamic (`SetMotionType` on a static
     model), simulating constraint motors (none in the vanilla meshes), the
     player throwing what they hold, traps (pushback, disarming, trip wires; see
     `known_ambiguities/traps.md`), arrows knocking down beehives, NPCs' blows and spells pushing them
     (and falls hurting NPCs: `fJumpFall*NPC`, `crate::falling`)
   - Scenes: the player's own lines (skipped for now; see `known_ambiguities/scenes.md`);
     priority between scenes
   - Physics, more: arrows sticking in loose objects; opened display-case lids
     dropping their collision; prismatic and malleable constraints (none in the
     vanilla meshes); ragdoll arms' odd resting poses with the turned limits;
     parts moved by graphs (trap blades) pushing bodies aside
   - Destructible object damage and lifecycle (`DEST` definitions, initial
     stage queries and scripted `DamageObject` health/stage progression are
     done; see Done): cobwebs blocking dungeon paths, barricades, bee hives,
     trap oil pools and gas. Explosions (`EXPL` damage within their radius)
     and self damage; the
     stage's model (`BSDamageStage` children or a replacement model with its
     collision, freeing navmesh obstacles), explosion and debris;
     destruction visuals/effects and flags.
     `ClearDestruction` now resets runtime state (see
     Done), but game effect restoration remains unresolved. Choices:
     `known_ambiguities/destruction.md`
   - Water (loaded types and `IsInDangerousWater`: see Done): swimming and
     `IsSwimming`, unloaded actors' contact, contact spells (`XNAM`);
     `WATR` `DATA` is unused in SSE per xEdit, not damage per second.
     See `known_ambiguities/water.md` and
     `known_ambiguities/triggers.md` (lava is scripted triggers, fires
     are placed hazards: see Done)
   - Magic, more: detect life, throwing with telekinesis; the voice's casting
     conditions (source 2 of `GetCurrentCastingType`, `GetCurrentDeliveryType`,
     `IsCurrentSpell`, `SpellHasKeyword`: powers are cast at once and shouts
     have no casting state, so they stay unsupported); dropped soul gems'
     per-item souls (public reports describe emptying: see
     `known_ambiguities/soul-trap.md`; inventory transfers now carry souls);
     taper, area, addictions, the rest of potion and food flags; image-space modifiers
     applied by effects (`TriggerIfNotActive`'s target)
   - Crime, more: arrests (guards following through
     doors, opening the cell after five hits, The Chill, the follower wait
     marker, the jail outfit); `GetArrestingActor` already checks the conversation
     guard, and greeting selection now sees that context, but native return
     semantics and pursuit timing remain unverified (`known_ambiguities/crime.md`);
     Cidhna Mine and the Orc
     strongholds (`DialogueCrimeOrcs`); bribes, persuasion and Thanes; the
     bounty collector; horse theft and werewolf crimes; children telling
     adults, `PICC` lines, hired thugs; trespassers chased out, "Warn To
     Leave", `sNoWaitTrespass` / `sNoSleepTrespass`, sleepers waking to warn;
     `GetAmountSoldStolen` (research-gated: legacy counter behavior in SE
     unresolved; `known_ambiguities/barter.md`); the unreported crime faction;
     trespass for opening a lock
     - Crime conditions waiting on state the engine lacks (`known_ambiguities/crime.md`):
       `GetCrime` and `GetActorCrimePlayerEnemy` (NPCs' crimes by type, and the
       faction's player-enemy flag, which nothing sets); the player's faction flags
       `GetPCExpelled`, `GetPCFactionMurder`, `GetPCFactionAttack` and
       `GetPCEnemyofFaction`, with what sets them; `IsPCAMurderer` (a murder flag
       kept past the Murders stat)
   - Combat, more: attackers forgetting a lost fleeing target
     (`fCombatDetectionFleeingLostRemoveTime`,
     `fCombatLowFleeingTargetHitPercent`); flee dialogue's distance rules
     (`fCombatDialogueFleeDistanceMult`; timer scheduling: see Done);
     impacts' results (bounce, impale, stick); their hazards' models, lights,
     sounds and flags
   - Encumbrance, more: fast travel refused over-encumbered (with the map)
   - Perks, more: staggers, bow zoom; NPCs' skills advancing
   - Combat conditions waiting on the attack phases: `GetAttackState` (the
     graph's draw, swing, hit, next attack, follow-through and bash; the engine
     has only the swing timer and whether a blow landed) and `GetShouldAttack`
     (the attack-on-sight decision readable from `&Engine`). Open questions:
     `known_ambiguities/combat-conditions.md`
   - Reference conditions waiting on state the engine lacks: `GetInContainer` (which
     container a reference is in: inventories hold counts of base forms, taken
     references are only disabled, and a created reference keeps the container it
     was made in), `GetLockIsBroken` (a broken lock: picks break, but no lock keeps
     that state, and no public source describes it), `Exists` (what makes a
     reference exist: no public description or vanilla use yet)
   - World state: moved references appearing at once when moved out of an
     unloaded cell and following their new cell between loaded ones; looping sounds of enabled references checked
     with an audio device
6. **UI**: inventory (categories, equipping, item details), map, quest
   targets on the map (on the compass: see Done)
   - The game's menus through `crates/ui-swf` (the HUD, message boxes and the
     item menus, the journal, the gift, magic, favorites and tween menus
     done: see Done): waiting's rest: getting into the bed first; the gamepad's d-pad
     favorites; the name popup with character creation; the gamepad (gilrs, as
     the spike)
   - The map's rest (the world map and `map.swf` are done: see Done; markers, discovery
     and travel permission are in `src/map_markers.rs`, the view in
     `src/map_view.rs`), in order: the map's clouds, lighting, sea and fog
     (research-gated); the local map for interiors and nearby cells
     (`SetLocalMapExtents`, the movie's `img://Local_Map`); fast travel: picking a
     marker (`MarkerClick` is logged), the confirmation, refusals
     (over-encumbered, enemies near, guards pursuing, disabled by script),
     moving to the marker's destination, time passing with distance, and
     followers coming along; then quest targets and custom markers on the
     map, the journal's Show on Map; markers of child worldspaces on their
     parent's map; distant LOD around the loaded cells on by default (the
     blocks are now placed right: `VRM_LOD=1`)
   - The inventory's rest: the item's model beside the card (`UpdateItem3D`,
     rotating and zooming it), rows grouped by captured soul with per-item
     selection and transfers (`known_ambiguities/inventory-cards.md`); alchemy
     teaching the effects two ingredients share (waits on crafting),
     equipping torches in a hand (the player's body doesn't show a carried
     light), the survival warmth rating
   - The HUD: notifications' display
     time (`TNAM`) in the SWF HUD (fallback done; `INAM` is unused per xEdit;
     research-gated: the movie's queue and fade timeline expose no duration
     input; `known_ambiguities/messages.md`); controller rumble for camera shakes
   - The journal's rest: the System page (saves, settings, controls, help,
     quitting), Show on Map; the statistics whose systems aren't there yet
     (crafting, dragon souls collected, horses stolen)
   - The HUD's compass: the player's own marker (with the map), location
     aliases as quest targets (research-gated: no direct examples found in
     the inspected Bethesda plugins; `known_ambiguities/quest-targets.md`); the charge and
     survival meters, load door info, favor mode
   - Character creation: the race menu (`ShowRaceMenu`, `SetInChargen`) and the
     player's face built live from FaceGen data (sliders, head parts, tints);
     NPCs only use pre-baked heads
7. **Audio**: lip sync; categories' mute when submerged and menu / dialogue ducking, the
   Audio menu's sliders; output models' reverb send and speaker output (acoustic spaces),
   sounds' priorities when voices run out
8. **Rendering**: point light and interior shadows, static shadow caching (per-cell
   caster batches), HDR: eye adaptation, bloom, tone mapping and the image
   spaces' HDR values (sunlight / sky scale), depth of field; the modifiers'
   radial blur and motion blur (cinematic values and tint: see Done), particles'
   remaining modifiers (animated colliders, skinned/deformed and animated mesh emission,
   strips, LOD; see
   `known_ambiguities/particles.md`), distant LOD (BTR/BTO/trees),
   grass shadows and the grass cache, spells' impacts; eye environment maps' per-eye centres, effect shaders' env maps; weather: auroras' brightness and order against the
   clouds (see `known_ambiguities/weather.md`); weather's volumetric lighting, sun
   glare and damage, lightning bolts, wind
   moving trees, grass and clouds; grass's vertex lighting flag and bending
   round actors;
   `ENVMAP_LIGHT_FADE` (`sf2` bit 15); unscripted trigger volumes (acoustic
   spaces, room bounds and portals); effect models' skinned meshes,
   `BSProceduralLightningController` and actor / effect graph sequence-owned
   shader / alpha-test controllers (placed rigid objects: see Done); particles'
   decay, turbulence, drag's exact formula and animated objects, rotation axis,
   speed-to-aspect runtime fidelity, subtexture
   loops, bombs, inherited velocity and addon nodes' caps, flags and keyframes
9. **Saves** (the container, the engine's state and the Papyrus VM are done:
   see Done): active magic effects, scene state,
   AI progress, objects' behaviour graphs, detection; reading the
   original's `.ess` saves later; a save / load menu (the System page),
   load orders that changed since the save, and the original's refusals to
   save (combat, menus, scripted)
10. **Performance**: async loading, GPU-driven culling
11. **New game and the opening** (an eventual goal; it needs most of the items above)
   - A new game separate from the developer launch, which drops the player at
     Tamriel (4, -12) outside Riverwood with only start-game-enabled quests running
   - How the game starts `MQ101` (not start-game-enabled) and where it places
     the player (`PlayerRef` has no placement in the data): needs a public
     source; record it in `known_ambiguities/`
   - Alternate start mods as the nearer target: their own start-game-enabled
     quest, a `MoveTo` and message boxes, all of which run already
   - The real opening in steps: new game and placement; the cart ride with a
     default race (vehicles, player control); the execution and Alduin
     (killmoves, dragons, particles, image spaces); Helgen Keep (the player's
     combat, magic, leveling, help messages, journal); character creation last
