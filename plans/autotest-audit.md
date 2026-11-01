# Autotest audit of `known_ambiguities/` (2026-10-31)

Every open item in `known_ambiguities/`, sorted by whether a strictly automatic
test plugin could settle it: the user loads a save with the plugin and touches
nothing; the script uses only vanilla Papyrus natives, keeps its readings in
`Result*` variables and requests a save, and `vrm-tool test-results` reads them.
This supersedes the tier lists in `plans/autotest-survey.md` (2026-10-13 to
10-21), whose open items are all included here.

An item is one question a test would answer. A file's bullet that holds
several separable questions counts as several; closely tied ones count as one.
Items already settled by autotest to autotest19 are not listed.

## Verdicts

| Code | Meaning | Items |
| --- | --- | ---: |
| A | Automatic with the record writers test mods already have | 455 |
| A* | Automatic, but needs a new writer (PACK trees, dialogue, PHZD, LVSP, WATR, REFR overrides, perk ranks, region weather) or a method still unproven | 81 |
| U | Needs the player: a menu (barter, crafting, dialogue, sleep/wait, lockpicking), input (attacks, aiming, casting by hand) or a new game | 155 |
| R | Needs a save to be loaded again (persistence questions) | 18 |
| V | Only visible or audible: rendering, UI layout, sound | 286 |
| N | Not observable in the game (internal formats, engine-only policies) | 60 |
| | **Total** | **1055** |

**Strictly automatic: 536** (455 A, 81 A*).

Conditions count as automatic: a test ability whose effect carries the
condition, read with `HasMagicEffect` after a wait longer than the recheck
interval (itself an item under `magic`). That readout is untested.
Skill XP isn't readable with vanilla natives, so XP items are A*: they need
enough uses to count level-ups. The two autotest20 items (swim depth,
NPC charging) are included but stay deferred by the user.

## Time estimate

The `min` column is a serial estimate of in-game time per item: 975 min
for A and 267 min for A*, about 21 hours if every item ran alone.
Past runs were faster than that: autotest3 to autotest5 ran 15 to 25
tests each in 6 to 8 minutes, because tests share setup and run side by
side. Allowing for that:

- One-minute and two-minute items (natives, conditions; 343): about 0.4 min each, ~2.5 h.
- Three- to five-minute items (fights, packages, unloaded actors; 185): ~6 h.
- Long statistical items (flee confidence, weather frequency, ADIA, UseWeapon barrages; 8): ~1.5 h.
- Per-plugin overhead (load, save, read results) at ~20 items a plugin, ~27 plugins: ~2 h.
- Reruns: autotest2, autotest4 and autotest19 each left a share unsettled by
  test faults; budgeting 30 to 50% of runs again adds 3 to 5 h.

About 12 hours of unattended game time across roughly 30 to 35 runs, 15 to 20 hours
with reruns. Building the plugins comes on top, mostly the new record writers
behind the A* items.

## Covered by `test-mod autotest21` (2026-10-31)

`test-mod autotest21` (`crates/tools/src/test_mods/autotest21/`) runs these
in one batch: 192 of the 536 automatic items, by file, with the test
that asks each. Items not listed are still open for a later batch.

**Run 2026-11-01** (`research/autotest21.md`): about 45 tests settled their
items. Every condition probe was void: the probe effect raised OneHanded's
damage modifier, and `HasMagicEffect` reports an effect whose condition
fails. So status-, comparison-, geometry-, death- and variable-condition
items, combat-conditions and magic's recheck interval stay open. Also open:
created aliases (nothing was created), our NPC casters (never cast: casting,
magic-menu, npc-healing, wards, staffs), fights that ended early, and the
smaller faults listed at the end of the note. The settled readings that
differ from our choices are applied (`plans/ROADMAP.md`, autotest21).
`test-mod autotest22` reruns the faults: every condition probe (now read by
the OneHanded gain alone), Recheck, created aliases, casters (vanilla copies),
fights, and the small ones.

**autotest22 run 2026-11-01** (`research/autotest22.md`): the probes work,
so the condition items are answered (status, comparison, geometry, death,
variables, combat-conditions, `IsInCombat`'s parameter, the 1 s recheck).
Created aliases, vanilla casters, staffs, the fights, `PlayerEnemy`,
`IsUnique`, invisibility's food and activation, story pickups, the view
cone and the sun gave readings too. Still open, for a follow-up batch (the
note's last section): `GetXPForNextLevel`'s value, `GetTimeDead`'s unit,
`GetRelativeAngle` (no axis given), drawn torch and shield reads,
`SetAttackActorOnSight`, crime Attack on Sight, Disarm, Holds, Healer,
Wards, create level.

| File | Items | Tests |
| --- | ---: | --- |
| actor-natives | 4 | ActorNatives (leveled traits race, `GetNthLinkedRef` 0 / -1 / keyword-only link), EnableAI |
| actor-values | 5 | NpcStats (tie order, PC-level-mult skills past 100, Khayla, skill offsets), QueryNatives (class from the stats template) |
| alias-fills | 3 | CreatedRefs (clean-up on Clear / Stop / Complete), AliasMatch (no Closest, in loaded area; create level) |
| casting | 4 | HandTypes (empty hand beside a spell), Casters (hand, distance, cast cadence), StoryCombat (CAST members) |
| combat-assistance | 4 | Assistance (reach, RELA rank), Teammate, FriendlyHits |
| combat-conditions | 4 | ItemConditions (`IsShieldOut`, `IsTorchOut`), FactionConditions (`GetShouldHelp`), ActorConditions (`IsProtected`) |
| combat-fleeing | 3 | Flee (damage requirement, check timing, ending), SoundLevels (the forgetting settings' values) |
| combat-movement | 1 | AlertFlag (idle sheathe delay, repeated `DrawWeapon`) |
| combat-state | 4 | Teammate (player's state), StopCombat (re-acquiring, switching targets), InCombatParam |
| comparison-conditions | 4 | FactionConditions (`SameFaction` rank -1, `SameRace` templates), ActorConditions (stamina scale, NPC over-encumbered) |
| condition-geometry | 5 | Geometry (`GetRelativeAngle`, `GetHeadingAngle`, `GetWithinDistance` edge, `IsCloserToAThanB` tie, line of sight, starting position) |
| condition-variables | 1 | FactionConditions (`GetScriptVariable`) |
| created-refs | 3 | Delete |
| crime | 7 | FactionRanks (rank changes, `IsGuard`), Infamy, StolenDrop, AttackOnSight, TakeUpArmor |
| death | 5 | DeathConditions (`GetTimeDead`, `IsKiller`, `IsKillerObject`, `GetCauseofDeath`, teammate's kill) |
| destruction | 4 | Destruction (thresholds, stage numbering, events, `ClearDestruction`), ReferenceSetters |
| detection | 4 | ViewCone (cone, distance, blindness, light), AlertFlag (`SetAlert`), SoundLevels (`iSoundLevel*`) |
| disarm | 3 | Disarm (where it lands, a sheathed two-hander, events) |
| encumbrance | 1 | EncumbranceEdge |
| factions | 8 | FactionNatives (`SetReaction`, expelled, crime groups), PlayerEnemy (enemy flag, aggression 0, attack on sight, allies), FightAfterAlly |
| fast-travel | 2 | FastTravel (gate after `MoveTo`, time taken) |
| followers | 2 | Teammate (joins, kill not murder) |
| formlists | 2 | FormLists |
| hands | 1 | HandTypes (`weaponSpeedMult`) |
| inventory-restrictions | 1 | BoundWeapon |
| invisibility | 3 | InvisiblePotion (drinking, locked activation), InvisibleAttacker |
| journal | 2 | KillStats, UseStats |
| loose-objects | 6 | LooseObjects (lying still, falling, impulse, drop position, corpses), RememberBefore / RememberAfter |
| magic | 11 | AbsorbMagicka, Recheck, ResistanceCap, Holds (holds, level cap with resistance), Cloak, Ethereal, SlowTime, WornEnchantment, HostileSpell, EnchantedBlows |
| magic-menu | 2 | NpcCostReduction |
| map-markers | 2 | MapMarkers |
| math-natives | 3 | MathNatives |
| npc-healing | 1 | Healer |
| npc-templates | 1 | ActorKeywords |
| outfits | 3 | Outfits |
| protection | 5 | Protection, AliasProtection |
| query-natives | 5 | QueryNatives |
| relationships | 2 | Relationships |
| remaining-natives | 27 | ContainerCounts, WornKeyword, Dimensions, DeadCount, QuestStates, 3DLoaded, SoundIds, ReferenceSetters, ActorState, Hostility, Reset, Finders |
| soul-trap | 3 | SoulTrap |
| staffs | 1 | NpcStaff |
| status-conditions | 18 | PlatformConditions, ItemConditions, ActorConditions, FactionConditions |
| story-cast-events | 1 | StoryCombat |
| story-manager | 5 | StoryItems (REMP and AIPL members, owners), StoryCombat (assault, kills' crime status, skill increase) |
| wards | 3 | Wards (NPC warding, build, keep) |
| weather | 3 | Weather (transition law by delta, acceleration, `FindWeather`), BackOutside (indoors) |

Not in autotest21: packages and procedures (activate, use-weapon,
use-magic, package-trees, escort, locks' packages), unloaded actors
(actor-execution, unloaded-placement), the Havok graph questions
(object-graphs, state-machine events), criticals and perk tabs (perk
entries not written yet), traps and triggers, telekinesis, harvest,
falling, the remaining combat-conditions read during fights.

## By file

Each row: verdict, the question, and the serial minutes for A / A* items.

### `acoustic-spaces.md` (1 V)

- V: RDSA one-shot interval and placement

### `activate.md` (3 A)

- A: walk to furniture and use it like Sit; start beside target on load (2 min)
- A: Number to Activate counting, done after count (2 min)
- A: furniture-use duration 3-6 s / one-shot clip (2 min)

### `actor-execution.md` (12 A, 2 A*, 1 U, 1 R, 1 N)

- A: OnPackageEnd for non-completing runs (Sit/Sleep/Sandbox...) (2 min)
- A: OnPackageChange/Start logged for combat packages (2 min)
- A: scene resumes a run still going; what a stopped scene's package logs (2 min)
- A: package fragment vs event order (1 min)
- R: reloaded seated actor
- A: fights/searches end as an actor unloads (2 min)
- A: other place packages (sandbox/sit/sleep/patrol) placed while away (3 min)
- A: Follow/Escort keep with an away target (3 min)
- A: bleedout away, 12 s getup (2 min)
- A*: unloaded speaker's scene line holds/skips/plays (3 min)
- A: jump-at-once range 4032-4399 and late-jump timing rule (5 min)
- U: sleeping vs waiting with unloaded travellers
- A: coming back seated after a longer absence (3 min)
- A: buffer reach: which cells keep 3D after leaving (5 min)
- A: actors that stayed had no path (inferred) (3 min)
- A*: multi-step package trees away (3 min)
- N: process level per distance

### `actor-natives.md` (8 A, 1 U)

- A: ActorBase.GetRace for leveled-trait base (1 min)
- A: GetNthLinkedRef 0 / negative (1 min)
- A: GetNthLinkedRef via keyworded-only link (1 min)
- U: StopCombatAlarm forgives a resisted arrest
- A: StopCombatAlarm: attack-on-sight actors restart (1 min)
- A: StopCombatAlarm ends unloaded fights (2 min)
- A: EnableAI(false) freezes the graph (1 min)
- A: EnableAI(false): detection, combat, speech, hit reactions (2 min)
- A: EnableAI flag survives unloading (2 min)

### `actor-values.md` (5 A)

- A: leftover skill points tie order (higher AV index first) (1 min)
- A: PC-level-mult NPC skills past 100 held at 100 (1 min)
- A: non-auto-calc NPC = race + offset (Khayla 155 vs 50) (1 min)
- A: skill offsets added past 100 (1 min)
- A: class comes with the stats template (1 min)

### `alias-fills.md` (4 A, 2 A*)

- A: created refs cleaned up on Clear / on complete (2 min)
- A*: ALNT nonzero meaning, near reach (3 min)
- A*: ALCL create level changes leveled pick (2 min)
- A: which match without Closest; beyond persistent refs (2 min)
- A: unique-actor alias: unlisted actor in a master vs esp (2 min)
- A: location unique lists merged across plugins (2 min)

### `alias-packages.md` (1 A)

- A: filled last vs started last vs lower alias id (2 min)

### `alpha-test-controllers.md` (1 V)

- V: byte quantization / clamping of alpha keys

### `archery-ammunition.md` (2 A, 1 U)

- A: tie between several equipped arrows (1 min)
- U: player auto-pick when stack runs out / incompatible equipped
- A: NPC without arrows: iron-arrow fallback (2 min)

### `audio-categories.md` (2 V)

- V: slider defaults multiply into volume
- V: dialogue fade amount and timing

### `audio-defaults.md` (1 V)

- V: DOBJ merged per key across load order

### `barter.md` (9 U)

- U: GetAmountSoldStolen counter
- U: unbuyable items unlisted vs refused
- U: Mod Buy/Sell perk subject
- U: Speech XP from gold changed hands
- U: fenced stolen goods stay stolen
- U: merchant short of gold
- U: VENV hours not checked on opening
- U: restocking rules
- U: Investor / Master Trader gold timing

### `blender-flags-properties.md` (4 N)

- N: scalar property / binding conversion
- N: invalid input handling
- N: bit-index bindings
- N: sampling/resync timing

### `blender-sync-master.md` (1 N)

- N: sync master trigger selection

### `body-weight.md` (3 V)

- V: _0/_1 shape pairing
- V: what blends (tangents)
- V: weight 50 without NAM7

### `casting.md` (14 A, 5 A*, 8 U)

- A: magicka regen delay after spending / while held (3 min)
- U: cost checked at press or release
- A: concentration landing interval (2 min)
- A*: target-actor spell range / contact 160 (3 min)
- U: target-location spells without projectile
- A: area: per-effect radius, line of sight rule (3 min)
- A*: skill XP: base cost before/after autocalc (5 min)
- U: greater powers: midnight vs 24 h
- A: who casts in combat (magic vs melee mult) (3 min)
- A: how casters fight: distance, cast interval (4 min)
- A: player struck by beams within capsule (2 min)
- A: Story Manager CAST members (2 min)
- A: cast kind picked per delivery (graph events) (2 min)
- A: empty hand beside one spell = 9 (1 min)
- A: spell beside weapon: no cast animation (2 min)
- U: when the player's hands are told
- U: cast at SpellFire; 1.5 s timeout
- U: hand stopping events
- U: dual casting magnitude+duration effects, XP
- U: dual cast start window 0.2 s
- A: which hand NPC casters ready spells in (2 min)
- A: NPC casters' graph event sequence (2 min)
- A: NPC dual-cast share, 2.8x cost (4 min)
- A*: EffectWasDualCast / Impact stagger only on dual cast (3 min)
- A*: casting-source conditions: idle hand -1, dual cast = left (3 min)
- A: SpellHasKeyword matches effects' keywords (1 min)
- A*: scroll casting type (SPIT vs fire and forget) (2 min)

### `character-properties.md` (9 N)

- N: scalar property conversion / overrides
- N: blender-child bone-weight edge cases
- N: blender-child scalar weight edge cases
- N: transition duration sampling
- N: blend curve asymmetric formulas
- N: target start fraction formula
- N: transition end modes
- N: cyclic wrapper blend parameter
- N: cyclic blender range inputs

### `cleared.md` (1 A, 1 V)

- A: boss clearing rule (all Boss refs dead, parents not) (2 min)
- V: cleared banner text and music

### `clip-crop-properties.md` (1 N)

- N: invalid crops; crop update timing

### `clip-enforced-duration.md` (1 N)

- N: enforced-duration rate formula

### `clip-mode-properties.md` (1 N)

- N: mode change on a running clip; enum conversion

### `combat-assistance.md` (4 A, 1 U)

- A: 4000-unit reach, detection needed to join (3 min)
- A: followers help by assistance alone (teammate flag) (2 min)
- A: RELA ranks count for assistance (2 min)
- U: townsfolk joining an assault stay hostile only that fight
- A: friendly hits: blows vs spells, iFriendHitCombatAllowed (3 min)

### `combat-conditions.md` (14 A)

- A: IsCombatTarget/GetCombatState while searching (2 min)
- A: IsPowerAttacking for power bashes (2 min)
- A: IsShieldOut sheathed (1 min)
- A: IsTorchOut lit / sheathed (1 min)
- A: combat group membership rule (group counts) (2 min)
- A: IsLastHostileActor (2 min)
- A: ShouldAttackKill: hostility/yielding (2 min)
- A: GetShouldHelp (1 min)
- A: IsProtected loaded vs base flag (1 min)
- A: GetActorAggroRadiusViolated warn radius / after leaving (2 min)
- A: GetCombatTargetHasKeyword (1 min)
- A: GetAttackState values 7-12 (3 min)
- A: IsIgnoringCombat / GetActorsInHigh meaning (2 min)
- A: GetShouldAttack rule (2 min)

### `combat-fleeing.md` (12 A, 1 A*, 1 N)

- A: DPS estimate in strength (10 min)
- A: armor term in strength (5 min)
- A: confidence modifier: how applied, distribution (10 min)
- A: group membership for the ratio (4 min)
- A: personal survival check (4 min)
- A: damage requirement to flee (3 min)
- A: check timing (3 min)
- A: confidence boost when hurt/approached (3 min)
- A: where to flee (doors, cover, allies) (5 min)
- A: ending a flee: fFleeIsSafeTimer, distances (3 min)
- A: fled NPCs returning (health) (3 min)
- A*: flee dialogue scheduling, fCombatDialogueFleeDistanceMult (4 min)
- N: group strength timer phases
- A: forgetting fleeing combatants (LostRemoveTime, LowFleeingTargetHitPercent) (4 min)

### `combat-movement.md` (1 A)

- A: idle sheathe delay; repeated DrawWeapon restarts it (2 min)

### `combat-state.md` (5 A)

- A: alert-but-not-found reported as state 2 (2 min)
- A: event on switching targets (2 min)
- A: player's GetCombatState rule (2 min)
- A: StopCombat: hostile actor re-acquires when (1 min)
- A: IsInCombat integer parameter (2 min)

### `comparison-conditions.md` (5 A)

- A: SameFaction counts rank -1 (1 min)
- A: SameRace uses template race (1 min)
- A: GetStaminaPercentage scale (1 min)
- A: IsOverEncumbered for NPCs (1 min)
- A: GetPCMiscStat with no name (1 min)

### `condition-geometry.md` (6 A, 1 A*)

- A: GetStartingPos/Angle of created refs (1 min)
- A: null ref = combat target in heading/relative/LOS (2 min)
- A: GetRelativeAngle sign and wrap (1 min)
- A: GetWithinDistance <= at the edge (1 min)
- A*: package flag 0x08 on the second parameter (2 min)
- A: IsCloserToAThanB tie (1 min)
- A: GetLineOfSight eye points, target body (2 min)

### `condition-variables.md` (2 A)

- A: GetScriptVariable reference and missing variable (1 min)
- A: quest variable numbers (79) (2 min)

### `crafting.md` (13 U, 2 V)

- U: created potion/enchantment form identity and stacking
- U: GetItemHealthPercent encoding of tempered items
- U: player-enchanted weapon value
- V: player furniture animation at stations
- V: first-use help cadence
- U: recipe material ownership order
- U: tempering skill thresholds
- U: improvement XP formula
- U: alchemy dominant-effect tie
- U: potion name/polarity ties
- U: failed mix XP, triple teaches all pairs
- U: disenchanting copies; XP cost input
- U: SetChargeValues rounding, multi-effect min power
- U: player enchantment ENIT fields
- U: CRFT for tempering/alchemy/enchanting

### `created-refs.md` (3 A)

- A: deleted once nothing holds it (script-held refs) (1 min)
- A: Delete on a plugin reference (1 min)
- A: Delete on a created item in a container (1 min)

### `crime.md` (12 A, 3 A*, 14 U, 1 N)

- A: AddToFaction/SetFactionRank/ModFactionRank/RemoveFromFaction ranks (1 min)
- U: GetAmountSoldStolen (same as barter)
- U: placing items in pockets: XP, owner
- U: poison placed in pockets
- A: NPCs taking up given armor/weapons: rule and timing (3 min)
- A: witnesses = detecting actors; victim always knows (3 min)
- A: members report crimes against members only as victim (2 min)
- A: GetCrimeGold/GetInfamy with no faction; infamy after paying (1 min)
- A*: GetCrime / IsPCAMurderer / GetPCFactionMurder / GetPCFactionAttack (3 min)
- A: dropped stolen goods keep the tag (check by remove-stolen pay) (2 min)
- A: mixed stack weapon charges on drop; drop spacing (1 min)
- A: remove-stolen pay takes all factions' stolen goods (1 min)
- U: lockpicking reported as trespass on starting a pick
- A*: trespass conditions and warning timing (10 s vs 30 s) (4 min)
- U: pickpocketing rules
- A: IsGuard: faction or Guard class (1 min)
- U: guards within 2048 come to arrest; PURS cadence
- A: attack on sight at 1000 (2 min)
- U: resisting arrest
- U: yielding
- U: go to jail / SendPlayerToJail
- U: blocking branches before HELO
- U: arrest greeting context (GetArrestingActor)
- U: escaping jail
- A: witness leaving loaded world / in bleedout (3 min)
- A*: witness reaction line choice (3 min)
- A: which witnesses attack (3 min)
- N: console setcrimegold default faction
- U: pickpocket retry gate
- U: Harmony making targets forget

### `critical-hits.md` (5 A, 1 A*, 1 V)

- A: CRDT percent mult gates criticals (4 min)
- A: entry point 17 subjects (3 min)
- A: unarmed critical multiplier (3 min)
- A: blocked criticals (3 min)
- A*: critical effect timing / on death (3 min)
- V: Critical Strike HUD text
- A: staff hits can't crit (2 min)

### `cursor-menu.md` (2 V)

- V: native cursor scaling
- V: cursor visibility policy

### `death.md` (5 A, 1 A*)

- A: GetTimeDead unit (3 min)
- A: IsKiller direction (2 min)
- A: IsKillerObject: list of bases or refs (1 min)
- A: GetCauseofDeath values (3 min)
- A*: GetKillingBlowLimb (3 min)
- A: GetKiller substitutes player for follower/summon (2 min)

### `decals.md` (7 V)

- V: reach along +Y, collision vs render geometry
- V: size pick per reference
- V: 2x2 subtexture atlas
- V: decal specular model
- V: parallax constant
- V: no-blend no-test decals
- V: DODT colour alpha

### `destruction.md` (6 A, 1 A*, 2 V)

- V: damage-stage nodes by name or fields
- V: stage without replacement model keeps last
- A: thresholds, caps, overkill (2 min)
- A*: weapon hit damage to objects (3 min)
- A: which spells damage objects (2 min)
- A: Ignore External Damage from current stage (2 min)
- A: script-visible stage: DSTD index vs model damage stage (1 min)
- A: OnDestructionStageChanged dispatch rules (1 min)
- A: ClearDestruction resets, events (1 min)

### `detection.md` (11 A, 6 A*, 1 R, 1 V)

- A: blindness only out of combat, vision only (3 min)
- A*: target skill factor fSneakSkillMult (4 min)
- A*: exterior distance and light mults (4 min)
- A: light level scale, iLightLevel* settings (3 min)
- A: view cone angle and eye height (3 min)
- A*: stealth point drain/refill (4 min)
- A*: detection update frequency (3 min)
- A: fights start/joined only within 4000 (3 min)
- A: alert behaviour (walk to last place, weapons out) (3 min)
- A: lost after 10 s undetected (3 min)
- A: combat search behaviour (where, how long) (5 min)
- A*: detection dialogue topics and idle cadence (4 min)
- V: sneak eye opening rule
- A: SetAlert flag vs detection states (2 min)
- R: load-time lifecycle of searches/stealth points
- A: iSoundLevel* values for weapon swings (1 min)
- A*: fPlayerDetectionSneakBase/Mult etc. roles (5 min)
- A: observer skill term through walls at range (3 min)
- A: unseen non-sneak hit: fight or search (2 min)

### `dialogue-idles.md` (3 V)

- V: named idle replaces the random gesture
- V: looping gesture duration
- V: listener idles for barks/scenes

### `dialogue-menu.md` (1 U, 5 V)

- V: when the menu is told what
- U: new vs heard topics
- V: ShowDialogueList newList
- V: selected topic on return
- V: mouse down counted twice
- V: click during transition lockup

### `dialogue-selection.md` (1 A*, 6 U)

- U: rumor weighting per INFO vs per topic
- U: equal-priority ordering
- U: branch flags and RUMO in linked lists
- U: pooled prompt text
- U: priority position of the pooled entry
- U: when a new rumor is chosen; mixed heard state
- A*: Say Once / Hours Until Reset / Do All Before Repeating scope (5 min)

### `disarm.md` (6 A, 1 N)

- A: where the dropped weapon lands (1 min)
- A: Can't Drop weapons (1 min)
- A: two-handed weapons and bows (1 min)
- A: drawn state tested at landing vs windup (2 min)
- N: player without body graph
- A: disarmed NPC fights with what it carries (2 min)
- A: hit reaction / OnHit / sound (1 min)

### `effect-perks.md` (3 A)

- A: temporary perks' ability and quest sections (2 min)
- A: rank, duplicate grants, AddPerk/RemovePerk overlap (2 min)
- A: condition re-evaluation cadence for grants (2 min)

### `effect-shaders.md` (6 V)

- V: membrane equation
- V: depth / extrusion
- V: which shapes (skin only, no weapons)
- V: stacking order / cap
- V: shader particles semantics
- V: addon models

### `encumbrance.md` (2 A, 3 U)

- U: player over-encumbered speed (walk vs crawl)
- U: player worn-armor slowing formula
- U: over-encumbered message once
- A: over means > CarryWeight (not =) (1 min)
- A: NPC worn-armor slowing, refresh timing (6 min)

### `environment-maps.md` (2 V)

- V: reflection combining terms
- V: reflection mask fallback

### `escort.md` (2 A)

- A: Run If Behind Distance meaning (3 min)
- A: escort turning to face the escorted (2 min)

### `factions.md` (10 A, 1 R)

- A: fights end when factions stop being enemies (2 min)
- A: faction listed more than once (1 min)
- A: SetReaction: modifier vs combat reaction (2 min)
- A: what being expelled changes (1 min)
- A: player-enemy flag alone makes members attack (2 min)
- A: aggression-0 members flee the player enemy (2 min)
- A: faction allies exempt from player-enemy (2 min)
- A: SetAttackActorOnSight: unaggressive flee (2 min)
- A: SetAttackActorOnSight: allies exempt (2 min)
- R: attack-on-sight mark survives saves
- A: faction in its own crime group unlisted; other's list (1 min)

### `falling.md` (2 A, 1 U)

- A: fall height measure; fJumpFallVelocityMin (3 min)
- A: water depth that spares a fall (3 min)
- U: difficulty scaling of fall damage

### `fast-travel.md` (5 A, 3 U)

- A: fast-travel gate re-enabled by MoveTo: timing (1 min)
- A: marker icon/discovery vs EnableFastTravel (1 min)
- U: refusal radius and priority
- A: arrival link priority (Game.FastTravel) (2 min)
- A: elapsed time formula (3 min)
- A: catch-up parity (effects, regen, schedules) (3 min)
- U: map confirmation modal timing
- U: travel autosave ordering

### `favorites.md` (2 U, 3 V)

- V: what is listed
- U: accept / attack-button equipping
- V: menu stays up; reopen selection
- U: hotkeys
- V: sounds

### `followers.md` (5 A, 1 U, 1 R)

- A: teammate joins any fight the player is in (3 min)
- A: player blamed for teammate/summon assault (3 min)
- A: teammate kill not player's murder (2 min)
- A: teammate count includes unloaded (1 min)
- U: teammate carry weight refusal
- A: followers carried by fast travel/load doors (2 min)
- R: TEAM flags in saves

### `foot-ik.md` (4 V)

- V: gain semantics, feedback, bias
- V: step height on collision
- V: planted-foot locking
- V: align world-from-model limits

### `force-greet.md` (4 U)

- U: greeting again after 10 s
- U: player must be detected rule
- U: in-cell trigger location
- U: seated greeters

### `formlists.md` (2 A, 1 R)

- A: GetAt position of added forms (1 min)
- A: AddForm duplicates; RemoveAddedForm on plugin form (1 min)
- R: additions persist in saves

### `furniture.md` (4 A, 1 V)

- A: NPC activates furniture on entering (2 min)
- A: actors placed in furniture on load don't activate (2 min)
- A: ways on filtered by item-count idle conditions (3 min)
- A: one-shot furniture exit sequence (2 min)
- V: carrying anim objects

### `gifts.md` (2 U, 1 V)

- U: favor points
- U: what is listed
- V: bottom bar favor display

### `grass.md` (4 V)

- V: candidate grid / texture pick
- V: ranges
- V: wave period meaning
- V: fade distance

### `hands.md` (6 A, 3 U, 3 V)

- U: asked hand for one-handed weapons; EitherHand
- A: unasked hand placement (1 min)
- A: displacing rules, pairs (2 min)
- V: equip icons
- V: left-hand weapon shown only drawn
- A: NPC takes weapon back from torch to fight (3 min)
- V: mouse in item menus
- U: attack buttons per hand (player)
- U: which attack (player)
- A: blows per HitFrame, dual power attack stamina (3 min)
- A: weaponSpeedMult fed from weapon speed (1 min)
- A: NPC dual wielders don't block or bash (3 min)

### `harvest.md` (1 A, 2 A*, 1 U)

- U: activation picking of plants
- A*: PFPC seasonal percent (3 min)
- A: owned plants: theft (1 min)
- A*: regrowth after 240 h (3 min)

### `head-tracking.md` (1 A*, 1 V)

- A*: AHEL greeting range (3 min)
- V: head-tracking target = scene listener

### `image-space.md` (9 V)

- V: HDR values
- V: effect shader math
- V: blended tints
- V: blur / double vision units
- V: several modifiers at once
- V: Apply strength
- V: cross-fades
- V: camera shakes
- V: fades in menus

### `impacts.md` (1 A, 9 V)

- V: two sound slots playback
- A: NAM2 hazard placement and lifetime (3 min)
- V: sound-only records without DNAM
- V: where a blow lands
- V: blood decals
- V: decal limits/fade
- V: effect orientation axes
- V: effect clock
- V: effect parts
- V: blood on bodies

### `integer-generator-bit-bindings.md` (1 N)

- N: explicit sync master semantics

### `interp-value-modifier.md` (2 N)

- N: step: chase vs linear from activation
- N: SpeedDamped set by code

### `inventory-cards.md` (8 V, 1 N)

- V: scroll card payload
- V: black soul label
- V: school meter refresh
- V: best-in-class classes
- V: best-in-class compared numbers
- V: best-in-class ties and singles
- V: best-in-class scope
- V: ingredient learned-effect label
- N: other ways to learn ingredient effects

### `inventory-restrictions.md` (1 A, 1 U, 1 V)

- A: RemoveItem on bound weapons (1 min)
- U: console drop bypasses Can't Drop
- V: Can't Drop feedback

### `invisibility.md` (6 A)

- A: attack-start / hit-frame ordering (2 min)
- A: constant invisibility left alone (1 min)
- A: mixed spells, failed renewal, concentration timing (3 min)
- A: potion/food consumption breaks it (1 min)
- A: blocked/locked activation breaks it (1 min)
- A: arrow interrupts at spawn not draw (NPC) (2 min)

### `journal.md` (5 A, 3 U, 11 V, 1 N)

- N: opening callbacks
- V: quest first selected
- V: quest/objective order
- V: objective states
- V: misc tracking
- V: closing
- V: date format
- V: level meter with pending level-ups
- V: which statistics per category, order
- U: crafting statistics
- A: kill statistics by actor type keyword (QueryStat) (2 min)
- A: crime statistics counted when reported (2 min)
- A: bounty statistics (1 min)
- A: Gold Found / Chests Looted (1 min)
- U: Books Read / Skill Books Read
- A: Skill Increases, Food Eaten, Potions Used (1 min)
- U: Barters
- V: magic favorites scoring
- V: Favorite Weapon
- V: quit list default selection

### `loading-screen.md` (1 A, 1 U, 12 V, 1 N)

- V: fade durations
- V: minimum time on screen
- A: game paused through load and fades (2 min)
- V: player info on screen
- V: which screen is chosen
- V: later texts drawn from
- N: random stream
- V: mist model and colour
- V: model transform
- V: camera path, FOV
- V: lighting
- V: scene fade
- U: mouse turning
- V: default logo
- V: sound during load

### `locks.md` (9 A, 1 A*, 1 U)

- A: unloaded actors' doors: evaluation cadence (5 min)
- A: animated doors open for walkers with key/ownership (3 min)
- A: alias ALFC faction rank/conflict policies (2 min)
- A: alias CNTO inventory permanent; re-grant on refill (2 min)
- A: Lock(abAsOwner) edge cases (several entrances, Off Limits) (3 min)
- A*: XRNK owner rank (3 min)
- U: lockpicking curve, Locksmith arc, Quick Hands
- A: which doors a package locks; owner away (5 min)
- A: after asking the player to leave: lock later / warn again (5 min)
- A: leaving through a locked pair from inside (2 min)
- A: what lets NPCs through locked load doors (5 min)

### `loose-objects.md` (7 A, 3 U, 6 V, 3 N)

- A: placed objects lie still until disturbed (2 min)
- A: created objects fall at once (1 min)
- V: triangle meshes become convex hulls
- V: constrained multi-body models
- N: constraint pivot frames
- V: angle limit sign
- V: constraint types (ball, rope)
- N: motors
- V: breakable constraint threshold
- V: SetMotionType leaves NIF-fixed bodies
- A: ApplyHavokImpulse units, masses (3 min)
- A: damping (2 min)
- A: where dropped items appear (1 min)
- N: falling out of the world
- A: resting place remembered (2 min)
- U: grabbing
- U: player blows push objects
- A: corpses follow ragdoll position (2 min)
- U: throwing held objects

### `magic-menu.md` (3 A, 1 U, 4 V)

- A: NPC school cost reduction without perk (3 min)
- A: negative school modifiers raise cost (2 min)
- A: SpellHasSkill matches secondary effects (3 min)
- V: categories
- V: level label
- V: card cost
- U: readying with buttons
- V: active effects listing

### `magic.md` (16 A, 1 A*, 2 V)

- A: absorb magicka gain (2 min)
- A: condition recheck interval (3 min)
- A: NPC resistance cap (2 min)
- A: spell magnitude perk subjects (2 min)
- A: hostile spell OnHit / fight / assault (2 min)
- A: constant enchantments of worn items (1 min)
- A: weapon enchantments use charge per blow (3 min)
- V: spell tome wording
- A: stacking value holds (2 min)
- A: calm/fear/frenzy level cap after resistance (2 min)
- A: IsUndead caching rule (1 min)
- A: calm broken by attack; what follows (2 min)
- A: fear: coward checks every frame (2 min)
- A: frenzy: turns on caster? clean-up on end (3 min)
- A*: summons: placement, limit, follow distances (4 min)
- A: cloaks: reach, rate, who is spared (3 min)
- A: ethereal: hits land with no damage? scripts hurt (2 min)
- A: Slow Time: world share, durations, waits (player walk excepted) (4 min)
- V: consume sounds default

### `main-menu.md` (1 U, 3 V)

- U: Continue's save choice
- V: title music loop/category; backdrop
- V: credits scroll speed units
- V: Quit to Main Menu fade

### `map-markers.md` (2 A, 4 V)

- A: visible/reveal distances, check cadence (3 min)
- V: discovery music per marker type
- V: compass frame mapping
- V: compass marker distances/fade
- V: AddToMap message
- A: AddToMap with travel, discovered later (2 min)

### `map-view.md` (7 V)

- V: map extent
- V: camera height/pitch/FOV
- V: what is drawn
- V: player marker projection
- V: markers on it (height, hollow)
- V: hover description
- V: child worldspace projection

### `math-natives.md` (3 A)

- A: inverse trig out of domain, nonfinite (1 min)
- A: rounding (1 min)
- A: tangent at poles (1 min)

### `melee-reach.md` (3 A, 2 N)

- A: reach from chest to target body height (3 min)
- A: 1.3 reach margin (4 min)
- N: heights without BSBound
- N: player capsule as body
- A: unreachable targets: wait and swing (3 min)

### `messages.md` (10 V)

- V: notification TNAM vs HUD fixed animation
- V: TNAM edge cases, fade timing
- V: help message "done" counting
- V: one help message at a time
- V: untyped quests' objectives in HUD
- V: journal log entries shown
- V: quest banners timing/wording
- V: text instance data; Global tags in any quest
- V: world sounds/notifications in menu mode
- V: PIMC resume fade

### `music.md` (1 R, 6 V)

- V: transition duration owner
- V: ducking
- V: gap between tracks; shuffle
- V: equal-priority ties; Add moves an entry
- R: script-added music survives saves
- V: track/palette interpretation
- V: default music triggers

### `node-controllers.md` (4 V)

- V: APP_INIT clock origin, restart on reload
- V: competing drivers order
- V: loop endpoint sampling
- V: NiFlipController on BS shaders

### `npc-healing.md` (3 A)

- A: heal start/stop thresholds, cooldowns (4 min)
- A: which spells count as heals (2 min)
- A: costliest affordable heal chosen (2 min)

### `npc-templates.md` (7 A, 2 N)

- A: empty spell list inheritance cases (1 min)
- A: ability sync timing on load (2 min)
- A: leveled spell list roll time; reset rerolls (3 min)
- A: leveled spell list level: actor's vs player's (2 min)
- A: HasSpell of the LVSP form (1 min)
- A: copied actor's leveled spell list yields nothing (1 min)
- N: missing scalar parts taken from template
- N: NPC ATKD replaces race's entirely
- A: HasKeyword: worn items / magic effects (1 min)

### `object-graphs.md` (8 A, 4 V)

- V: transform-controller nodes drawn apart
- V: collision moved as fixed colliders
- A: graphs and scripts kept across unload (2 min)
- A: OnCellLoad on reload and order vs OnLoad (2 min)
- A: GetOpenState for activators (1 min)
- A: how long natives like GetOpenState wait (2 min)
- A: animation registrations kept across unload (2 min)
- A: script-set graph variables vs engine writes (2 min)
- V: tail auxiliary graph feeding
- V: graph retention across outfit changes
- A: all payload-carrying events withheld from scripts (2 min)
- A: PlayAnimationAndWait on a payload event (1 min)

### `outfits.md` (3 A)

- A: only outfit-marked items removed; removed vs unequipped (2 min)
- A: Actor.SetOutfit reference vs base (1 min)
- A: outfit leveled lists re-rolled on re-equip (2 min)

### `package-trees.md` (2 A, 7 A*)

- A*: numeric package data: invalid/negative/large, run_on 6 (3 min)
- A*: package location 3D vs flat; near package start; exterior in-cell (4 min)
- A*: multi-location wait in real or game seconds (6 min)
- A*: multi-location clock starts on arrival (3 min)
- A*: Random picks the same stop twice (3 min)
- A*: multi-location progress kept across loads/waits; in-cell stops (4 min)
- A*: Stacked-root trees: first passing branch (3 min)
- A: default package lists consulted when (3 min)
- A: patrolling from Self (3 min)

### `particles.md` (18 V)

- V: particle space
- V: emission rate/clock
- V: direction/speed variations
- V: size and aspect
- V: forces
- V: rotation
- V: subtextures
- V: spawn on death
- V: colour and fade
- V: run distance / warm-up
- V: addon nodes / master systems
- V: palette and vertex colour
- V: multi-target emitters
- V: directional drag
- V: rigid mesh emission
- V: particle colliders
- V: speed to aspect
- V: initial rotation controllers

### `perks.md` (3 A, 3 A*, 1 U)

- A: SetStage on a stopped quest doesn't restart it (1 min)
- A*: perk ranks: entry applies above its rank (2 min)
- A*: entry point tab subjects (5 min)
- A: order of damage/armor/block modifications (4 min)
- A*: stealth perk formula (4 min)
- U: buying perks
- A: arrow recovery rolled on strike or on search (3 min)

### `player-body.md` (1 A, 1 A*, 9 U, 4 V, 1 N)

- V: head part replacement per type
- N: Direction binding through cyclic wrapper
- U: sheathed third-person facing
- A*: falling event threshold (2 min)
- V: first-person shadow
- U: attacking sheathed draws
- U: swing gating during draw
- A: equipment change keeps drawn state (1 min)
- U: power attack by movement direction
- U: blow timing / attack gating
- U: bow release timing
- U: interrupted / duplicate bow releases
- V: third-person camera start distance
- V: shoulder offsets pivot
- U: third-person aim
- U: disabled camera switch stops zoom

### `player-controls.md` (4 A, 1 V)

- A: IsCamSwitchControlsEnabled with both POV owners (1 min)
- A: unknown POV types (1 min)
- A: POV owners overlap / repeated calls (1 min)
- V: cart mode precedence vs dialogue
- A: SetPlayerAIDriven arbitration (2 min)

### `poisons.md` (1 A, 1 A*, 7 U, 3 V)

- U: which hand the poison goes on
- U: question wording
- V: refusals as notifications
- U: bound weapons take poisons
- U: blocked blows use a dose
- U: bows: dose leaves with arrow
- U: one poisoned instance per weapon form
- V: when sPoisoned shows
- A*: EquipItem on a poison (run last: may open a box) (1 min)
- A: NPCs poisoning their weapons in combat (3 min)
- V: poison icons on grouped weapons
- U: dose count perk tabs

### `power-bash.md` (1 A, 4 U)

- U: hold threshold
- U: cancellation
- A: NPC power bash stamina fallback (3 min)
- U: hit timing
- U: torch bashes, dual wield guard

### `protection.md` (7 A, 1 U)

- A: alias essential flag: while filled vs quest running (1 min)
- A: alias flag before start-up stage (1 min)
- A: alias flag on an already essential base (1 min)
- A: loaded actor follows base flag change (1 min)
- A: invulnerable: DamageActorValue, stagger, stamina/magicka (2 min)
- A: ghost: blows pass or land; enemies target (3 min)
- U: ghost activation / talking
- A: ghost DamageActorValue (1 min)

### `quadratic-value-interpolation.md` (1 N)

- N: duplicate/unsorted keys

### `query-natives.md` (6 A)

- A: IsUnique on templated NPCs (1 min)
- A: GetClass of unresolved leveled template (1 min)
- A: GetEditorLocation policies (1 min)
- A: GetCurrentLocation edge cases (inventory refs, dragons) (2 min)
- A: location population recursion, double count (2 min)
- A: IsLoaded parent inclusion (1 min)

### `quest-targets.md` (7 V)

- V: which targets shown
- V: where a target is
- V: route through load doors
- V: marker alpha/scale, merging
- V: enemy markers
- V: location aliases as targets
- V: world-map overlays

### `reference-conditions.md` (4 A, 1 N)

- A: GetDestroyed from DSTD flag (1 min)
- N: GetIsObjectType unlisted types
- A: IsWeaponInList left hand (1 min)
- A: IsUnlockedDoor after key/pick vs never locked (1 min)
- A: GetDefaultOpen on created refs (1 min)

### `relationships.md` (3 A)

- A: HasParentRelationship direction and ASTPs (2 min)
- A: secret relationships filtered from queries (1 min)
- A: any-family counts dead; follows templates (1 min)

### `remaining-natives.md` (41 A, 1 A*, 1 U, 3 R, 4 V)

- A: alias timers: multiple scripts, zero/NaN (1 min)
- A: ingredient hostility: learned effects only (1 min)
- A: missing string setting value (1 min)
- A: cell attach/detach timing (2 min)
- A: IsStopped intermediate states (1 min)
- A: GetKey with two different keys (1 min)
- A: HasCommonParent counting the inputs (1 min)
- A: HasRefType descendants/disabled (1 min)
- A: GetAllItemsCount copies vs forms (1 min)
- A: WornHasKeyword hand items / skin (1 min)
- A: GetCurrentPackage timing and interrupts (2 min)
- A*: GetDialogueTarget listeners (2 min)
- A: NPC encumbrance native (1 min)
- A: GetWidth/Height/Length bounds and scale (1 min)
- A: GetScale of player / race change (1 min)
- A: GetWorldSpace unloaded travellers / inventory (2 min)
- A: GetDeadCount cumulative (2 min)
- A: GetGoldValue potions / lights (1 min)
- A: IsRunning backward quirk; NPC sprint (2 min)
- A: IsAlarmed for non-guard witnesses (2 min)
- U: IsArrestingTarget for a pursuing guard
- A: Is3DLoaded timing (1 min)
- A: sound instance ids reused (1 min)
- V: script sound follows moving source
- R: script loops survive save
- R: leveled list additions survive save
- A: leveled list at 255 entries; duplicates (1 min)
- A: SetOpen raises OnOpen/OnClose (1 min)
- A: SetDestroyed: explosion, event, stage (2 min)
- A: SetScale on actor keeps height factors (1 min)
- A: CalculateEncounterLevel multipliers (1 min)
- A: IgnoreFriendlyHits tolerance (2 min)
- A: IsDeleted for created refs (1 min)
- A: Resurrect: equipment worn; starts-dead actors (2 min)
- A: UnequipAll spells; UnequipItemSlot numbering (1 min)
- A: IsHostileToActor unloaded/dead (1 min)
- A: SetDoingFavor effects (2 min)
- A: ObjectReference.Reset scope (2 min)
- A: Cell.Reset resurrects / re-enables (3 min)
- V: quest text globals copied when
- A: Quest.Reset on a running quest (1 min)
- A: finders skip dead; unloaded positions (1 min)
- A: GetSunPosition magnitude; sungazing angle (2 min)
- A: CalculateFavorCost formula (1 min)
- V: SetInChargen message text
- A: PlayerKnows other learning (1 min)
- A: GameTimeToString year format (1 min)
- R: sound category changes survive saves
- V: SetFrequency above 1
- A: SetINI* unknown names (1 min)

### `reverb.md` (2 V)

- V: REVB field mapping (delay scaling)
- V: reverb network tone/decay

### `rotate-character.md` (3 N)

- N: rotation step formula and axis convention
- N: composition with root motion / facing
- N: TurnDelta units

### `saves.md` (7 R, 1 N)

- N: RequestSave file names
- R: weather on load
- R: story manager memory persistence
- R: continuity adjuncts (harvest, cleared, crime memory, controls, regen delays, camera...)
- R: further continuity policies
- R: active effects and tapers saved
- R: behaviour graph open states saved
- R: faction/ghost/outfit/AI-off/form-list sections

### `scenes.md` (3 A, 4 A*, 1 R, 1 V)

- A: Start/ForceStart take actors from other scenes: when they stop (2 min)
- A: package action done rules (radius+150, furniture, greet) (3 min)
- A*: topicless dialogue actions; reading time without voice (2 min)
- A: Stop runs the end fragment; repeat while true (2 min)
- V: subtitle distance; barks quiet during scenes
- A*: scene flag 0x4 and action LNAM (3 min)
- A*: observe-combat / death pause / interruptible / scene packages only (4 min)
- A*: looping dialogue repeats or re-picks (2 min)
- R: scene state in saves

### `scrolls.md` (1 A, 2 U, 1 V)

- A: GetEquippedSpell on a held scroll (1 min)
- U: scroll charge time; dual casting
- U: scroll XP
- V: scroll equip icon; favorite statistics

### `selectors.md` (2 N)

- N: selector bytes 0x59..0x5F
- N: chooser start state

### `sequence-material-controllers.md` (3 V)

- V: sequence vs controller timing/blending
- V: backwards playback, TBC, persistence
- V: unlinked/external blocks

### `shouts.md` (2 A, 1 U, 1 V)

- U: when a shout goes out (release vs held)
- V: shout card
- A: shout cone reach and spread (3 min)
- A: NVPE story event trigger and members (2 min)

### `skills-menu.md` (3 U, 10 V)

- V: opening image
- V: camera
- V: skydome sequences
- V: MoveCamera / StartCameraAnimation
- U: level up popup timing
- U: key/mouse bindings
- U: Legendary confirmations
- V: perk requirement text
- V: label colours
- V: perk placement in sky
- V: stars and lines sequences
- V: sounds
- V: transition effect / beast trees / ring acceleration

### `skills.md` (4 A*, 4 U)

- U: Legendary edge cases
- A*: sneak attack shot / bash XP (5 min)
- A*: block vs armor XP split, mixed armor (5 min)
- U: pickpocket XP curve
- A*: sneaking hidden XP conditions (5 min)
- U: blocked blows / power attacks train the same
- U: training choices
- A*: Mod Skill Use applied before use mult/offset (5 min)

### `skin-tone.md` (2 V)

- V: tone blend formula
- V: all-zero tones

### `sleep-wait.md` (1 A, 7 U, 1 V)

- U: opening wait/sleep
- U: refusal reasons and order
- U: hours pass: effects, regen, statistics
- U: other actors placed per hour
- U: actors' health over a wait
- U: what interrupts a wait
- U: sleep events
- V: sleep fade timing
- A: script-set owners for player checks (2 min)

### `sneak-attacks.md` (1 A, 6 U)

- U: crossbows 2x
- U: staves no sneak bonus
- U: multiplier before armor
- U: bashes never sneak attacks
- A: NPCs sneak attacking the player (3 min)
- U: unused sneak settings
- U: backstab statistic

### `soft-effects.md` (5 V)

- V: soft fade formula
- V: which surfaces count
- V: particles soft
- V: sky behind
- V: no depth test

### `soul-trap.md` (5 A, 1 V, 1 N)

- A: black soul rule (ActorTypeNPC creatures) (2 min)
- A: white souls in black gems (2 min)
- A: filling linked forms; partial gems (2 min)
- V: No soul gem large enough message
- A: souls kept through script transfers (2 min)
- A: dropping filled gems without linked forms (2 min)
- N: mixed owners' soul gems

### `sound-output-models.md` (4 V)

- V: curve as gain
- V: dialogue output model choice
- V: speaker layouts
- V: HRTF approximation

### `sound-variation.md` (4 V)

- V: shift/variance units
- V: variance distribution
- V: which sounds vary
- V: mixer gain smoothing

### `spell-visuals.md` (1 R, 9 V)

- V: casting art selection
- V: graph events and hand variables
- V: attachment fallback
- V: quiet/invisible targets
- V: hit art refresh and ending
- V: flame/beam/cone visuals
- V: dual scale, enchantment art
- V: sequence generator blend modes
- V: RFCT transforms
- R: script shaders survive saves

### `staffs.md` (7 A, 2 A*, 3 U)

- A: staff charge cost formula (NPC staff users) (4 min)
- A*: ModSpellCost entry point on enchantments (3 min)
- A: NPCs' worn school reductions lower charge use (3 min)
- U: insufficient charge behaviour
- A*: which copy's charge is used/kept (2 min)
- U: magicka regenerates while casting staffs
- A: empty hand beside a staff = 8 (1 min)
- A: NPCs taking up staffs (style multiplier) (3 min)
- A: NPCs fighting with staffs vs spells (3 min)
- A: charge kept through drop and pickup (2 min)
- A: enchanted bows pay per arrow that strikes (3 min)
- U: recharging list, overflow, reusable gems, meter

### `state-machine-events.md` (2 A*, 2 N)

- A*: transition notification boundaries and ordering (4 min)
- N: FLAG_DELAY_STATE_CHANGE
- A*: self-transition modes (CONTINUE/RESET/BLEND) (3 min)
- N: event modes (PROCESS_ALL)

### `state-machine-outputs.md` (1 N)

- N: publish order within an update; invalid start; competing writers

### `state-machine-start-properties.md` (1 N)

- N: bound mode/index sampling and conversions

### `status-conditions.md` (34 A, 3 A*, 6 U)

- A: IsWin32 true on SE PC (1 min)
- A: IsGoreDisabled (1 min)
- A: GetIsPlayableRace: template race; non-actors (1 min)
- A: GetKeywordItemCount: stacks, equipped, lists (1 min)
- A: HasTwoHandedWeaponEquipped hand; HasBoundWeaponEquipped sources (1 min)
- A: IsOwner/IsCellOwner: subject vs caller semantics (1 min)
- A: omitted owner = player (1 min)
- A: owner faction matching (1 min)
- A: IsInMyOwnedCell direct/faction (1 min)
- A: GetXPForNextLevel whole cost vs remaining (1 min)
- U: IsPCSleeping timing
- A: IsPC1stPerson (1 min)
- A: GetInCurrentLocFormList children (1 min)
- A: IsLocationLoaded parents (1 min)
- A: null ref in HasSameEditorLocAsRef etc. (1 min)
- A: GetFactionCombatReaction mutual; self = ally (1 min)
- A: GetFactionRelation (449) meaning (2 min)
- A: location ref-type counts, dead rule, empty alias (2 min)
- A: GetKeywordDataForAlias (1 min)
- A: player regions from XCLR (1 min)
- A: GetInZone (1 min)
- U: MenuMode parameter
- A: GetPlayerControlsDisabled parameters (1 min)
- U: IsTimePassing while waiting
- A: GetSecondsPassed (1 min)
- A*: IsNullPackageData (2 min)
- A: GetCurrentAIPackage / GetCurrentAIProcedure values (2 min)
- U: IsWaiting
- A: GetScale condition (1 min)
- A: GetIsCreature (1 min)
- A: GetFactionRankDifference (1 min)
- A: GetWeaponAnimType (1 min)
- A: IsCarryable (1 min)
- A: GetTargetHeight sign (1 min)
- A*: GetActivatorHeight (2 min)
- A: GetClothingValue (1 min)
- A: GetDisease (1 min)
- A: IsActorUsingATorch (1 min)
- A: GetInCellParam (1 min)
- U: GetCannibal
- U: IsRunning for the player
- A: GetUnconscious = bleeding out (1 min)
- A*: IsRidingMount (2 min)

### `story-cast-events.md` (1 A, 1 A*, 2 U)

- A: NPC and script casts send CAST (2 min)
- A*: CAST suppressed in combat (whose state) (2 min)
- U: concentration casts: when CAST fires
- U: staffs/scrolls/powers/shouts send CAST

### `story-manager.md` (11 A, 5 A*, 2 U)

- U: LOCK event members, failed pick
- A*: TRES event members and V1 (4 min)
- A: ADIA conversation cadence, chance, range (10 min)
- A: reset hours: quests started otherwise (2 min)
- A: quests' own event conditions run on whom (2 min)
- A: item pickup R1 owner; stolen rule (2 min)
- A*: hello event timing (passing greeting) (3 min)
- A: assault event: every blow or first; members (2 min)
- A: NPC kills' crime status (1 min)
- A: finding bodies rule (3 min)
- A*: crime events' members; arrest event; ADCR (3 min)
- A: OnStory event before startup stage (1 min)
- A: skill/level increase event values (1 min)
- A: OnInit for every quest at game start; alias scripts timing (2 min)
- A*: faction favor cap for stealing (3 min)
- A*: generic faction member dead by first ref (2 min)
- A: REMP owner and item members (2 min)
- U: selling sends no event; given = 5

### `swf-hud.md` (23 V)

- V: The crosshair's text.
- V: Meters.
- V: The compass.
- V: Notifications
- V: Quest banners
- V: The sneak eye.
- V: Enemy health.
- V: Modes.
- V: Key art.
- V: Sounds.
- V: Message boxes.
- V: Answers.
- V: Input to the menus.
- V: Opening.
- V: Categories.
- V: Entries.
- V: The item card
- V: The bottom bar
- V: Keys.
- V: Both sides in one list.
- V: Containers
- V: Bartering
- V: The card

### `system-page.md` (5 U, 5 V)

- V: save row numbering
- U: Save on Pause
- U: autosave rotation
- V: missing preference default
- U: look sensitivity scale
- U: difficulty damage paths
- V: brightness mapping
- V: distance slider mappings
- U: remapping conflicts
- V: tree billboard presentation

### `tbc-value-interpolation.md` (1 N)

- N: bias/continuity order

### `telekinesis.md` (1 A, 2 A*, 1 V)

- A*: reach formula (2 min)
- A*: heaviest lifted (2 min)
- V: no-recast message
- A: NPC/enchantment telekinesis (2 min)

### `texture-flip-controllers.md` (1 V)

- V: flip controller frame mapping

### `timer-properties.md` (2 N)

- N: deadline sampling time
- N: disabled-time / reactivation semantics

### `transition-event-mode-properties.md` (1 N)

- N: event mode sampling and routing

### `transition-self-mode-properties.md` (1 N)

- N: self mode sampling time

### `traps.md` (2 A, 3 A*, 1 U)

- A*: touch margin; sleeping bodies don't hit (3 min)
- A: reference VMAD inherits base properties (1 min)
- U: held objects don't hit the player
- A*: which objects are traps; event arguments (5 min)
- A*: graph-moved parts hit like keyframed traps (3 min)
- A: trap kills belong to no one (2 min)

### `triggers.md` (5 A, 2 A*)

- A: phantom reach 20 units (2 min)
- A: who sets triggers off (projectiles?) (2 min)
- A: actors inside on cell load enter on first update (2 min)
- A: script-moved trigger keeps its volume (1 min)
- A: OnTrigger interval and first event (2 min)
- A*: placed hazards: radius units, scale, interval (4 min)
- A*: trigger collision layers (which objects each reacts to) (4 min)

### `tween-menu.md` (6 V)

- V: opening animation
- V: bottom bar
- V: choosing
- V: coming back
- V: closing
- V: sounds

### `twist-modifier.md` (2 V, 2 N)

- V: angle units
- V: chain distribution / RAMPED
- N: other bindings
- N: pose order

### `unloaded-placement.md` (8 A, 2 U)

- A: upstairs over the target (5 min)
- A: stop distance formula for other radii (5 min)
- A: other procedures (Sleep, Eat, UseWeapon) placement (5 min)
- A: coming from another cell/worldspace (5 min)
- A: whole-cell sandbox marker choice (5 min)
- A: which overlapping actor moves apart (3 min)
- U: waiting/sleeping moves loaded actors
- U: new game puts actors on target at once
- A: 30/40/50 short distribution (5 min)
- A: Travel heading threshold (3 min)

### `use-magic.md` (5 A*)

- A*: cast time and cooldown meaning (4 min)
- A*: number to cast; per-package counts (4 min)
- A*: hold when blocked (3 min)
- A*: package dual cast and its cost (3 min)
- A*: hands drawn/put away; magicka during package (2 min)

### `use-weapon.md` (7 A, 2 A*)

- A: unresolvable triggers; distance from origins (3 min)
- A: Always Power Attack selection and exhaustion (4 min)
- A: finite package completion (porch bowmen) (10 min)
- A*: count 0 with Never End off; time limit start (4 min)
- A: completed package passed over vs resumed (4 min)
- A: random power attack share (6 min)
- A: distance gate boundary, cadence, resumption (4 min)
- A: MultiTarget target selection lifetime (4 min)
- A*: Hold When Blocked, Always Hit, damage/combat flags (4 min)

### `value-key-interpolation.md` (1 N)

- N: midpoint ties, repeated timestamps

### `voices.md` (2 V)

- V: voice cap and priority direction
- V: weather beds in the budget

### `wards.md` (11 A, 2 A*, 2 U, 1 N)

- A: ward build rate and maximum (NPC warders) (3 min)
- U: dual-cast wards start full
- A*: ward arc (4 min)
- A: spell strength against a ward (3 min)
- A*: what wards stop (3 min)
- A: breaking: spell stopped, graph event (2 min)
- A: OnHit for warded spells (1 min)
- A: gear wards add, not worn down (3 min)
- A: Ward Absorb magicka (2 min)
- A: channelled effects linger (2 min)
- U: ward Restoration XP
- A: NPCs: when they raise wards (4 min)
- A: NPCs: ward priority vs heals/spells (3 min)
- A: NPCs: keep time and gap; IsWardState hold (4 min)
- A: combat styles weigh warding (4 min)
- N: WardDeflection actor value

### `water.md` (5 A, 1 A*, 2 N)

- A: immersion threshold, unloaded/dead actors (3 min)
- N: water surface cell edges
- N: overlapping water planes
- A*: XNAM contact spell (3 min)
- A: swimming depth; Waterbreathing (autotest20, deferred by user) (4 min)
- A: Swims/Walks race flags and swimming (3 min)
- A: breath capacity and drowning damage (4 min)
- A: creature breath / float heights (4 min)

### `weather.md` (6 A, 1 A*, 8 V, 1 N)

- A: transition speed law; abAccelerate (5 min)
- A: FindWeather didn't find Fog (2 min)
- A: how often the weather changes; volatility (15 min)
- A*: RDWT global chances add or replace (5 min)
- N: region priority / override flag
- A: leaving a region: new weather transition (3 min)
- A: indoors: weather state; starting indoors (2 min)
- V: weather sounds
- V: precipitation particles
- V: rain under roofs
- V: lightning
- V: cloud speeds
- V: sky statics
- V: wind direction meaning
- V: auroras
- A: SetActive(false) not coming in after a forced override (2 min)

### `world-state.md` (4 A)

- A: unloaded regeneration timing; fast-travel recovery (5 min)
- A: NPC-opened doors close: distance and delay (3 min)
- A: NPC-left-open door across unload (4 min)
- A: Automatic Door / Minimal Use flags (3 min)
