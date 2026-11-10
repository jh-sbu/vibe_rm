# Autotest audit of `known_ambiguities/` (2026-10-31, redone 2026-11-10)

Every open item in `known_ambiguities/`, sorted by whether a strictly automatic
test plugin could settle it: the user loads a save with the plugin and touches
nothing; the script uses only vanilla Papyrus natives, keeps its readings in
`Result*` variables and requests a save, and `vrm-tool test-results` reads them.
This supersedes the tier lists in `plans/autotest-survey.md` (2026-10-13 to
10-21), whose open items are all included here.

An item is one question a test would answer. A file's bullet that holds
several separable questions counts as several; closely tied ones count as one.
Items that Skyrim SE readings have answered are left out: autotest to
autotest22, `regen`, `ammo`, `controltags` and the user's own observations,
as each ambiguity file records them. An item that was partly answered keeps
only its open part. Our engine implementing something doesn't settle it. A
probe that faulted (no reading) leaves its item open.

## Verdicts

| Code | Meaning | Items |
| --- | --- | ---: |
| A | Automatic with the record writers test mods already have | 551 |
| A* | Automatic, but needs a new writer (PACK trees of a shape no vanilla package has, dialogue, PHZD, LVSP, WATR, REFR overrides, perk ranks, region weather, quest stage fragments) or a method still unproven | 160 |
| U | Needs the player: a menu (barter, crafting, dialogue, sleep/wait, lockpicking, difficulty), input (attacks, aiming, casting by hand) or a new game | 203 |
| R | Needs a save to be loaded again (persistence questions) | 39 |
| V | Only visible or audible: rendering, UI layout, sound | 355 |
| N | Not observable in the game (internal formats, engine-only policies) | 74 |
| | **Total** | **1382** |

**Strictly automatic: 711** (551 A, 160 A*).

Conditions count as automatic: a test ability whose effect carries the
condition, read by the OneHanded gain it gives (autotest22 showed this
works; `HasMagicEffect` reports an effect whose condition fails). Skill XP
isn't readable with vanilla natives, so XP items are A*: they need enough
uses to count level-ups. Packages: test mods copy vanilla packages, their
trees included, and patch their inputs (`records::copy`, as autotest13 and
autotest16 do), so a question a vanilla template can show is A. A tree
shape that no vanilla package has is A*. Vanilla Papyrus has no
difficulty setter and no `UsePower`. The two autotest20 items (swim
depth, NPC charging) are included but stay deferred by the user.

## Time estimate

Each A and A* item has a serial estimate of in-game minutes: 1,323 min for A
and 566 min for A*, about 31 hours if every item ran alone. By length:
373 items of 1–2 min, 328 of 3–5 min, 9 statistical items of 6–15 min.
Past runs were faster than serial: autotest3 to autotest5 ran 15 to 25 tests
each in 6 to 8 minutes, because tests share setup and run side by side.
Allowing for that, as in the first audit:

- One-minute and two-minute items (natives, conditions; 373): about 0.4 min each, ~2.5 h.
- Three- to five-minute items (fights, packages, unloaded actors; 328): ~10.5 h.
- Long statistical items (flee confidence, weather frequency, ADIA, CSFL
  flight styles, UseWeapon barrages; 9): ~1.5 h.
- Per-plugin overhead (load, save, read results) at ~20 items a plugin, ~36 plugins: ~2.5 h.
- Reruns: autotest2, autotest4, autotest19, autotest21 and autotest22 each
  left a share unsettled by test faults; budgeting 30 to 50% of runs again
  adds 5 to 8 h.

About 17 hours of unattended game time across roughly 36 to 45 runs, 22 to 25
hours with reruns. Building the plugins comes on top, mostly the new record
writers behind the A* items.

## Redone 2026-11-10

The first audit (2026-10-31) listed 1055 items (455 A, 81 A*, 155 U, 18 R,
286 V, 60 N), before autotest21 and autotest22 ran. Every file was gone
over again, against its current text and `research/autotest21.md` /
`autotest22.md`:

- 80 items settled since then were dropped, and many more cut down to their
  open part (status conditions, crime, factions, formlists, protection,
  magic, story manager, remaining natives and others).
- 17 files written since were added: `bone-weight-tails`, `carriages`,
  `cell-ownership`, `chargen`, `dragon-flight`, `find-acquire-flee`,
  `first-person`, `local-map`, `movement-gaits`, `new-game`,
  `npc-mounts`, `offscreen-navigation`, `package-dialogue`,
  `quest-stages`, `ragdolls`, `speech-checks` and `translation` (159
  items).
- Files that grew with later work gained items: `remaining-natives` (+18),
  `package-trees` (+16), `escort` and `detection` (+7 each), and others.

SE readings that the ambiguity files still contradict (the items are
settled, so they aren't listed below; the files and the engine need
correcting):

- `protection.md`: invulnerability spares health from `DamageActorValue`.
  autotest21's invulnerable base lost 10 health and 10 stamina to it
  (Protection, "Settled").
- `combat-movement.md`: an unwanted draw is sheathed after two seconds.
  autotest21's unalerted NPC sheathed about 0.6 s after `DrawWeapon`
  (AlertFlag).
- `crime.md`: an NPC given better armor puts it on. autotest21's plain NPC
  didn't, and only teammates do (TakeUpArmor). The file's autotest2 note
  agrees with SE. Its general rule doesn't.
- `journal.md`: a vampire counts once, under People. autotest21 counted it
  under People and Undead, and the engine was changed to agree; the file's
  text is stale. Its "Fines Paid" line (the gold paid) also disagrees with
  its own confirmed reading (the bounty paid off).

## Covered by `test-mod autotest21` (2026-10-31)

`test-mod autotest21` (`crates/tools/src/test_mods/autotest21/`) ran 192 of
the first audit's 536 automatic items. autotest21 (run 2026-11-01,
`research/autotest21.md`) settled about 45 tests' items; its condition
probes were void (the probe effect raised OneHanded's damage modifier, and
`HasMagicEffect` reports an effect whose condition fails). `test-mod
autotest22` reran the faults (run 2026-11-01, `research/autotest22.md`):
the probes worked, answering the condition items, created aliases,
vanilla casters, staffs, the fights and others. What they answered is
dropped from the lists below. Still open from both, for a follow-up batch
(autotest22's last section and both notes' "Compared with ours"):
`GetXPForNextLevel`'s value, `GetTimeDead`'s unit, `GetRelativeAngle` (no
axis given), drawn torch and shield reads, `SetAttackActorOnSight`, crime
Attack on Sight, Disarm (a script-cast Disarm did nothing), Holds, Healer,
Wards, create level, Slow Time's share, NPCs' enchantment and staff
charge, `IsGuard` and the `KILL` of an essential `Kill(None)`.

Not yet probed by any batch: packages and procedures (activate,
use-weapon, use-magic, package-trees, package-dialogue, escort,
find-acquire-flee, locks' packages, npc-mounts, carriages, dragon-flight),
unloaded actors (actor-execution, unloaded-placement,
offscreen-navigation), the Havok graph questions (object-graphs,
state-machine events), ragdolls (`test-mod ragdolls` is built, not run),
criticals and perk tabs (perk entries not written yet), traps and
triggers, telekinesis, harvest, falling, the remaining combat-conditions
read during fights.

## By file

Each row: verdict, the question, and the serial minutes for A / A* items.
Line numbers point into the file as of 2026-11-10.

### `acoustic-spaces.md` (3 V)

- V: RDSA loop admission, one-shot roll interval and placement (listening comparison needed)
- V: space bed crossfade time (1.5 s is a provisional listening choice)
- V: overlap precedence between placed acoustic volumes (smallest volume, ties by reference ID)

### `activate.md` (3 A)

- A: walk to furniture and use it like Sit; start beside target on load (2 min)
- A: Number to Activate counting; done after the count (2 min)
- A: furniture-use duration 3-6 s / one-shot clip (2 min)

### `actor-execution.md` (13 A, 2 A*, 1 U, 1 R, 1 N)

- A: OnPackageEnd for non-completing runs (Sit/Sleep/Sandbox/Follow/Patrol/ForceGreet) (2 min)
- A: OnPackageChange/OnPackageStart logged for combat packages (2 min)
- A: a scene resumes a run still going; what a stopped scene's package logs as it lets go (2 min)
- A: package fragment vs event order (1 min)
- A: fights and searches end as an actor unloads (2 min)
- A: other place packages (sandbox/sit/sleep/patrol) placed while away (3 min)
- A: Follow/Escort keep with an away target; put on it once more than 256 past the follow distance (3 min)
- A: bleedout away, 12 s getup (2 min)
- A: jump-at-once range 4,032-4,399 in 3D, and whether distance left or time decides a late jump (5 min)
- A: coming back seated after a longer absence (3 min)
- A: buffer reach: which cells keep 3D after the player leaves (5 min)
- A: actors that stayed had no path (inferred, untested) (3 min)
- A: a Sit or Sleep scene action's seat is taken as the actor loads (2 min)
- A*: unloaded speaker's scene line holds, is skipped or plays unheard (3 min)
- A*: multi-step package trees away (3 min)
- U: sleeping vs waiting with unloaded travellers
- R: reloaded seated actor
- N: process level per distance

### `actor-natives.md` (6 A, 1 U)

- A: ActorBase.GetRace for a leveled-traits base: which race a leveled pick gives (SE gave an unrelated race; ours takes the first pick) (1 min)
- A: StopCombatAlarm: attack-on-sight actors restart at once (1 min)
- A: StopCombatAlarm ends unloaded fights (2 min)
- A: EnableAI(false) freezes the animation graph (1 min)
- A: EnableAI(false): speech and hit reactions (2 min)
- A: EnableAI flag survives the actor unloading (2 min)
- U: StopCombatAlarm forgives a resisted arrest

### `actor-values.md` (2 A)

- A: skill-share rule past 100 for PC-level-mult NPCs: the fit misses DLC2EbonyWarrior (Alteration 89 and Conjuration 95 stored, 92 and 98 from ours) (1 min)
- A: health's wait after a blow (fDamagedHealthRegenDelay 0.5 s): whether a landed blow or spell starts it (3 min)

### `alias-fills.md` (7 A, 2 A*)

- A*: ALNT nonzero meaning, and how far "near" reaches (3 min)
- A*: ALCL create level changes a leveled actor's pick (2 min)
- A: which of several matches is taken without Closest; whether it searches beyond persistent references when "In loaded area" is unset (2 min)
- A: unique-actor alias: an unlisted actor in a master vs in an esp (2 min)
- A: location unique lists merged across plugins (2 min)
- A: created references after their cell unloads or resets (5 min)
- A: items created in a container (not the player's): taken out on quest stop, or kept (2 min)
- A: a refill of a still-running quest disables and forgets its created references (2 min)
- A: RefLocation override lasts until the source changes; travel-derived location of a reference alias (5 min)

### `alias-packages.md` (1 A)

- A: at the same priority, filled last vs started last vs lower alias id (2 min)

### `alpha-test-controllers.md` (1 V)

- V: byte quantization and clamping of alpha keys (round or truncate, clamp out-of-range keys)

### `archery-ammunition.md` (2 A, 1 V)

- A: NPC ammunition choice: worn arrows, else strongest, else iron arrows (2 min)
- A: a stack that ran out and came back: keeps its old place in the tie order? (1 min)
- V: the quiver's drawn shapes do not change with the count left (visual)

### `audio-categories.md` (2 V)

- V: a sound's volume multiplies the descriptor's attenuation, each category's VNAM, and the menu slider's UNAM default up the chain (SFX 0.81, footsteps 0.65, voice 0.65, music 0.32)
- V: dialogue fade gain (-6 dB), timing (3 s out, 1 s in), and which sounds it covers (player conversations only, not scenes or INFO barks)

### `audio-defaults.md` (1 V)

- V: DOBJ values merged per key across load order (last named value wins; zero keys skipped; named nulls kept)

### `barter.md` (9 U)

- U: GetAmountSoldStolen counter behaviour (Skyrim SE implements it or not)
- U: what a merchant won't buy: unlisted vs refused
- U: Mod Buy/Sell Prices perk subject (the merchant as the third tab)
- U: Speech XP from gold changed hands
- U: fenced stolen goods stay stolen
- U: merchant short of gold: pays what it has once the player accepts the warning
- U: VENV hours not checked on opening
- U: restocking rules (iDaysToRespawnVendor, whole chest replaced, merchants without a chest not restocked)
- U: Investor / Master Trader gold timing (the gold comes with the next stocking)

### `blender-flags-properties.md` (5 N)

- N: scalar property / binding conversion and precedence
- N: invalid input handling (finite integral 0..65535 accepted; others keep authored flags)
- N: bit-index bindings (conversion, combination and precedence)
- N: sampling and resync timing (activation/update sampling, synchronization changed while active)
- N: conflicting-binding behaviour (variable vs duplicate property; the no-fall-through rule)

### `blender-sync-master.md` (1 N)

- N: sync master trigger selection (heaviest-child rule, explicit index, additive fallback)

### `body-weight.md` (3 V)

- V: _0/_1 shape pairing (same name and vertex count; unpaired shapes drawn as they are)
- V: what blends (tangents and bitangents are the heavy model's)
- V: weight 50 when NAM7 is missing

### `bone-weight-tails.md` (2 V, 2 N)

- V: blender children: trailing bones past a short authored array get weight 1 (inferred from authored data, not measured; first-person bow and torch-sprint masks depend on it)
- V: bone switch: tail 0 and the werewolf's cameraOut fallback [1.0], which rests on one vanilla array
- N: overlapping short bone-switch children: add up and normalise, or does one child win
- N: empty bone-switch child array: means no bones

### `carriages.md` (3 A, 2 A*, 1 U, 3 V)

- A: a passenger's root heading and offset from the cart origin after SetVehicle (2 min)
- A: a player passenger's RidingInCarriage keyword, and when SetVehicle(None) releases it (2 min)
- A: an unloaded cart keeps its relative angle and hitch position while following the horse; passengers follow their saved frames (5 min)
- A*: hitch point, cart yaw, rear-axle trailing, cart pitch and roll on slopes under a moving horse, against SE's Havok constraint (5 min)
- A*: whether a hit moves a hitched (keyframed) cart (3 min)
- U: movement input does not walk a passenger out of the cart
- V: the seated body's facing (torso, head or seat heading), the view's camera rig, tilt and sway, and the SetSittingRotation(-55) view
- V: collider timing at SetVehicle(None)
- V: wheel spin and the front wheels not steering while hitched

### `casting.md` (13 A, 5 A*, 8 U)

- A: magicka's regen wait: 0.5 s setting vs 0.26-0.44 s seen; no regen while an NPC's spell is held (2 min)
- A: concentration landing interval (2 min)
- A: area: whether multi-effect and self-delivered area spells (Fire Storm rings, Fast Healing, Battle Cry) reach others as modelled (3 min)
- A: who casts in combat (magic vs melee multiplier) (3 min)
- A: how casters fight: closing distance (the SE warlock stopped near 840 from 900, ours does not close in) and cast interval (4 min)
- A: player struck by beams and target spells within the capsule (2 min)
- A: Story Manager CAST members (2 min)
- A: cast kind picked per delivery (2 min)
- A: a spell beside a weapon: no cast animation (2 min)
- A: which hand NPC casters ready their spells in (2 min)
- A: NPC casters' graph event sequence (2 min)
- A: NPC dual-cast share, 2.8x cost (4 min)
- A: SpellHasKeyword matches effects' keywords (1 min)
- A*: target-actor spell range (SPIT, 4096 without) and contact range 160 (3 min)
- A*: skill XP from base cost before or after auto-calc (5 min)
- A*: EffectWasDualCast and Impact stagger only on dual cast (3 min)
- A*: casting-source conditions (idle hand -1; dual cast reads as left; a let-go spell not yet fired counts as cast) (3 min)
- A*: scroll casting type (SPIT vs fire and forget) (2 min)
- U: cost checked at press or release
- U: target-location spells without a projectile
- U: greater powers: midnight vs 24 h
- U: when the player's hands are told
- U: cast at SpellFire; 1.5 s timeout
- U: hand stopping events
- U: dual casting magnitude and duration effects, and XP
- U: dual cast start window 0.2 s

### `cell-ownership.md` (3 A)

- A: which exterior cell a loaded actor belongs to (GetParentCell) after it crosses into another loaded cell (5 min)
- A: cell attach and detach events and OnCellLoad timing for an actor that changed cell this way (5 min)
- A: an actor beyond the loaded cells stays with its kept cell and goes away when it unloads (5 min)

### `character-properties.md` (10 N)

- N: scalar property conversion; runtime property overrides and output-to-property semantics
- N: blender-child bone-weight edge cases (missing/mistyped bindings, empty arrays, skeleton length mismatch, per-bone normalisation)
- N: blender-child scalar weight edge cases (missing/duplicate bindings, non-finite values, update timing)
- N: transition duration sampling and missing/duplicate-binding policy
- N: blend curve asymmetric formulas (smoothstep fallback for 2 and 3) and sampling timing
- N: target start fraction formula, out-of-range policy and skipped-trigger policy
- N: transition end modes duration arithmetic and runtime timing
- N: cyclic wrapper blend parameter: conflicting or shared child bindings
- N: cyclic blender range inputs: sampling and lifecycle, invalid or conflicting bindings
- N: modifier enable handling at activation and deactivation boundaries (added)

### `chargen.md` (1 A, 3 U, 5 V, 2 N)

- A: Actor.SetRace keeps the face and fires OnRaceSwitchComplete even when the race is unchanged (2 min)
- U: race menu content: categories, slider labels, order and ranges, choosable head parts, playable race list
- U: race menu keys (A/D, R) and the Done name popup (cancel keeps the menu open)
- U: changing race, sex or preset resets to that preset NPC, keeping the weight
- V: race menu camera (170 units back, 45 to the face), placement and absence of a light
- V: hair colour of a face with no HCLF (the player's), taken from the race default
- V: vampire morph taken from the race's Vampire keyword
- V: normals of morphed face parts (model's own, not recomputed)
- V: tint masks sampled nearest at 512 × 512
- N: 19th face slider: values outside −16..16 taken as unset
- N: Game.PrecacheCharGen / PrecacheCharGenClear do nothing

### `cleared.md` (1 A, 1 R, 1 V)

- A: boss clearing rule (all Boss refs dead, parents not) (2 min)
- V: cleared banner text and music
- R: cleared cells never respawn (iHoursToRespawnCellCleared and OnReset unused)

### `clip-crop-properties.md` (1 N)

- N: invalid crops; crop update timing

### `clip-enforced-duration.md` (1 N)

- N: enforced-duration rate formula

### `clip-mode-properties.md` (1 N)

- N: mode change on a running clip; enum conversion

### `combat-assistance.md` (4 A, 2 A*)

- A: assistance reach between 2500 and 5000 units (4000 assumed), and whether detection of a side is needed to join (3 min)
- A: followers help by assistance alone, without the teammate flag (2 min)
- A: RELA ranks 1, 2 and 4, and helpers that help only allies (2 min)
- A: townsfolk joining an attack on the player stay hostile only that fight (3 min)
- A*: friends' blows start no fight (iFriendHitCombatAllowed); spells settled (3 min)
- A*: what else the teammate flag changes in a fight (3 min)

### `combat-conditions.md` (13 A, 1 A*)

- A: IsCombatTarget while searching: is the combat target cleared (GetCombatState 2 seen) (2 min)
- A: IsShieldOut drawn (sheathed settled) (1 min)
- A: IsTorchOut drawn or lit (sheathed settled) (2 min)
- A: combat group membership rule (group counts) (2 min)
- A: IsLastHostileActor (2 min)
- A: ShouldAttackKill: hostility/yielding (2 min)
- A: GetShouldHelp for allies and friends (yes case; only the no case read) (1 min)
- A: IsProtected: a loaded actor's own flag when it differs from its base (base flag settled) (1 min)
- A: GetActorAggroRadiusViolated warn radius / after leaving (2 min)
- A: GetCombatTargetHasKeyword (1 min)
- A: GetAttackState values 7-12 (3 min)
- A: IsIgnoringCombat / GetActorsInHigh meaning (2 min)
- A: GetShouldAttack rule (2 min)
- A*: IsPowerAttacking for NPC power attacks; the player's power bash needs input (3 min)

### `combat-fleeing.md` (14 A, 1 A*, 1 N)

- A: DPS estimate in strength (10 min)
- A: armor term in strength (5 min)
- A: confidence modifier: how applied, distribution (10 min)
- A: group membership for the ratio (4 min)
- A: personal survival check (4 min)
- A: damage requirement to flee (3 min)
- A: check timing (3 min)
- A: confidence boost when hurt/approached (3 min)
- A: where to flee (doors, cover, allies) (5 min)
- A: ending a flee: what fFleeIsSafeTimer (30) measures, and which distance settings apply (3 min)
- A: fled NPCs returning: regeneration ruled out at HealRate 0; potions and ratio shifts open (3 min)
- A: forgetting fleeing combatants (LostRemoveTime, LowFleeingTargetHitPercent) (4 min)
- A: why an Average NPC at 18% against a level 20 fighter did not flee in 12 s (4 min)
- A: fCombatDialogueFleeMin/MaxElapsedTime values (3 / 5 from public lists) read in SE (1 min)
- A*: flee dialogue scheduling, fCombatDialogueFleeDistanceMult (4 min)
- N: group strength timer phases

### `combat-movement.md` (1 A, 2 A*)

- A: idle sheathe: repeated DrawWeapon restarts it (2 min)
- A*: orbit and flank pacing and distances (statistical) (5 min)
- A*: NPC charge: start beyond 1000 units, sprint state (NPCSprinting vs AIControlledNPCSprinting), sprint attacks during a charge (3 min)

### `combat-state.md` (4 A)

- A: event on switching targets (2 min)
- A: player's GetCombatState: searching (2) and when it returns to 0 (2 min)
- A: StopCombat: timing of a still-hostile actor's re-acquisition (0.39-0.43 s in SE; ours a quarter second) (1 min)
- A: IsInCombat integer parameter beyond 0, 1 and 2 (2 min)

### `comparison-conditions.md` (2 A)

- A: SameFactionAsPC treats rank -1 as no membership (1 min)
- A: GetPCMiscStat with no name (1 min)

### `condition-geometry.md` (5 A, 1 A*)

- A: GetStartingPos/Angle of other created refs (PlaceAtMe objects; inventory-created unsupported) (1 min)
- A: null ref = combat target in heading/relative/LOS (2 min)
- A: GetRelativeAngle sign and wrap (1 min)
- A: GetLineOfSight: eye points, target body, the player's line ignoring facing, and the view-cone edge (2 min)
- A: references in different interiors or worldspaces are never within GetWithinDistance (1 min)
- A*: package flag 0x08 on the second parameter (2 min)

### `condition-variables.md` (1 A*, 1 N)

- A*: GetQuestVariable (79): which Papyrus variable each number names, by a probe scanning numbers (method unproven) (2 min)
- N: what GetScriptVariable reads when it does not read Papyrus variables (no SCRV in Skyrim records)

### `crafting.md` (14 U, 2 V, 2 N)

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
- U: Create Object weapon/armor statistics count output items (not presses)
- N: whether unused created forms are collected from saves and the runtime plugin
- N: experiment station movie routing (no vanilla station uses an experiment type)

### `created-refs.md` (4 A, 1 R)

- A: deleted once nothing holds it (script-held refs) (1 min)
- A: Delete on a plugin reference: disabled, nothing, or deleted in the session (1 min)
- A: Delete on a created item in a container (1 min)
- A: a script still holding a deleted reference sees its calls do nothing (1 min)
- R: Delete on a plugin reference: whether it is removed from the save (needs a reload)

### `crime.md` (15 A, 3 A*, 14 U, 1 N)

- A: NPCs taking up given shields, and which weapon a plain NPC wields among several it carries (3 min)
- A: witnesses = detecting actors; victim always knows (3 min)
- A: members report crimes against members only as victim (2 min)
- A: GetCrimeGold with no faction: which hold's faction it reads, if any (1 min)
- A: dropped stolen goods keep the tag; needs a control with an undropped stolen item (2 min)
- A: mixed stack weapon charges on drop; drop spacing (1 min)
- A: remove-stolen pay takes all factions' stolen goods (1 min)
- A: IsGuard for hold guards: what they have that a plugin NPC of IsGuardFaction or Guard class lacks (2 min)
- A: attack on sight at 1000 (2 min)
- A: witness leaving loaded world / in bleedout (3 min)
- A: which witnesses attack (3 min)
- A: changes to an NPC's faction sit on the reference; the record stands for its first reference (2 min)
- A: items removed by script take the unowned ones first (1 min)
- A: taking from a container judges the theft by the container's owner (2 min)
- A: picking up dropped unowned goods in an owner's house is not theft (2 min)
- A*: GetCrime / IsPCAMurderer / GetPCFactionMurder / GetPCFactionAttack (3 min)
- A*: trespass conditions and warning timing (10 s vs 30 s) (4 min)
- A*: witness reaction line choice (3 min)
- U: lockpicking reported as trespass on starting a pick
- U: pickpocketing rules
- U: guards within 2048 come to arrest; PURS cadence
- U: resisting arrest
- U: yielding
- U: go to jail / SendPlayerToJail
- U: blocking branches before HELO
- U: arrest greeting context (GetArrestingActor)
- U: escaping jail
- U: GetAmountSoldStolen (same as barter)
- U: placing items in pockets: XP, owner
- U: poison placed in pockets
- U: pickpocket retry gate
- U: Harmony making targets forget
- N: console setcrimegold default faction

### `critical-hits.md` (6 A, 1 A*, 1 V)

- A: CRDT percent mult gates criticals (4 min)
- A: entry point 17 subjects (3 min)
- A: unarmed critical multiplier (3 min)
- A: blocked criticals (3 min)
- A: staff hits can't crit (2 min)
- A: one critical roll per blow, checked after the damage perks; no roll when chance <= 0 (3 min)
- A*: critical effect timing / on death (3 min)
- V: Critical Strike HUD text

### `cursor-menu.md` (2 V)

- V: native cursor scaling
- V: cursor visibility policy

### `death.md` (5 A, 1 A*)

- A: GetTimeDead: real minutes or tenths of a game hour (3 min)
- A: IsKillerObject run on the victim with a list of the killer's base or reference (1 min)
- A: GetCauseofDeath values beyond -1 (script Kill(killer)) (3 min)
- A*: GetKillingBlowLimb (3 min)
- A: GetKiller for summons (follower case is settled) (2 min)
- A: placed-dead and unloaded-death subjects have no GetTimeDead value and fail (2 min)

### `decals.md` (8 V)

- V: reach along +Y, collision vs render geometry
- V: size pick per reference
- V: 2x2 subtexture atlas
- V: decal specular model
- V: parallax constant
- V: no-blend no-test decals
- V: DODT colour alpha
- V: screen-space box projection: which surfaces take a decal, and the facing fade

### `destruction.md` (5 A, 1 A*, 3 V)

- V: damage-stage nodes by name or fields
- V: stage without replacement model keeps last
- V: whether the Bleak Falls web's stage 1 model changes (replacement FXspiderWebKitDoorSpecialDest.nif)
- A: thresholds: 100% stages, several caps, skipped stages (2 min)
- A*: weapon hit damage to objects (3 min)
- A: which spells damage objects; frost and shock (2 min)
- A: Ignore External Damage from current stage (2 min)
- A: OnDestructionStageChanged: one event per call when several stages are skipped (1 min)
- A: whether later damage reapplies effects after ClearDestruction (stage and health after the clear are settled) (2 min)

### `detection.md` (16 A, 7 A*, 2 R, 1 V)

- A: blindness 100 out of combat: vision only, or also sound and observer-skill terms (3 min)
- A: view cone eye height (110 above feet) and half-angle (SE: seen at 90°, not at 135°; ours 95) (3 min)
- A: light level scale; iLightLevelInteriorMod and iLightLevelMax indoors (3 min)
- A: fights start/joined only within 4000 (3 min)
- A: alert behaviour (walk to last place, weapons out) (3 min)
- A: lost after 10 s undetected (3 min)
- A: combat search behaviour (where, how long) (5 min)
- A: SetAlert flag vs detection states (2 min)
- A: which noise level each weapon swing makes (dagger Silent vs UESP's none); iSoundLevel values settled (1 min)
- A: observer skill term through walls at range (3 min)
- A: unseen hit (non-sneak, sneak-attack survivor): fight or search (2 min)
- A: CreateDetectionEvent level scale, 1.25 s duration, owner needed (3 min)
- A: noise elsewhere: listener alerts and searches at the noise (3 min)
- A: arrow noise: bow shot or impact, and the projectile's VNAM level (3 min)
- A: lost target that fled: gives up after 30 s; what restarts the time (5 min)
- A: Muffle doesn't quiet action noises (4 min)
- A*: target skill factor fSneakSkillMult (4 min)
- A*: exterior distance and light mults (4 min)
- A*: stealth point drain/refill (4 min)
- A*: detection update frequency (3 min)
- A*: detection dialogue topics, idle cadence, CombatToNormal (4 min)
- A*: roles of fPlayerDetectionSneakBase/Mult, fPlayerDetectActorValue, fSneakFlyingDistanceMult, fSneakAmbushNonTargetMod (5 min)
- A*: sleeping observers see nothing (4 min)
- R: load-time lifecycle of searches and stealth points
- R: SetAlert flag across load, death and reset
- V: sneak eye opening rule

### `dialogue-idles.md` (3 V)

- V: named idle replaces the random gesture, or joins it (dialogue-idles.md:16-22)
- V: how long a looping gesture lasts; no hold through pauses (dialogue-idles.md:23-27)
- V: listener idles for player conversations, scenes and barks (dialogue-idles.md:28-31)

### `dialogue-menu.md` (5 U, 2 V)

- U: new (white) vs heard (grey) topics (dialogue-menu.md:30-31)
- U: selected topic when the list comes back after a click (dialogue-menu.md:36-37)
- U: mouse down counted twice per press (dialogue-menu.md:40-42)
- U: click during transition: does the game lock up the topics too (dialogue-menu.md:43-51)
- U: no Goodbye topic; Tab and Esc leave, Esc skips a greeting line (dialogue-menu.md:38-39)
- V: when the menu is told what (ShowDialogueText, NotifyVoiceReady, PopulateDialogueList) (dialogue-menu.md:22-29)
- V: ShowDialogueList newList: slide in vs fade in (dialogue-menu.md:32-35)

### `dialogue-selection.md` (1 A*, 8 U)

- U: rumor weighting per eligible INFO, per topic, or other (dialogue-selection.md:159-163)
- U: equal-priority ordering: FormID, plugin order, quest start order (dialogue-selection.md:164-165)
- U: blocking and exclusive branch flags; several RUMO topics in an explicit linked list (dialogue-selection.md:166-169)
- U: pooled prompt text when a pool mixes FULL texts; GMST or drawn topic's FULL (dialogue-selection.md:170-175)
- U: priority position of the pooled entry when contributing topics' PNAM differ (dialogue-selection.md:176-177)
- U: when a new rumor is chosen (rebuild, end of response, click); mixed heard state (dialogue-selection.md:178-182)
- U: whether a Random End INFO is in the draw (Geldis Sadri's "Haven't heard anything") (dialogue-selection.md:99-108, 134-135)
- U: whether a lower-priority quest joins an unended random group (dialogue-selection.md:134-137)
- A*: Say Once / Hours Until Reset / Do All Before Repeating scope and reset (5 min) (dialogue-selection.md:183-187)

### `disarm.md` (7 A, 1 N)

- A: where the dropped weapon lands (1 min)
- A: Can't Drop weapons: taken out of hand, kept in inventory (1 min)
- A: two-handed weapons and bows: dropped as the right hand's weapon (1 min)
- A: drawn state tested at landing vs windup (2 min)
- A: disarmed NPC fights with what it carries (2 min)
- A: hit reaction / OnHit / sound (1 min)
- A: magnitude 12 / 20 / 30 as the level cap; draugr immune past level 31 (3 min)
- N: player without body graph

### `dragon-flight.md` (15 A*, 1 R)

- A*: cruise at FLMV walk speed; turn-rate limit (dragon-flight.md:7-10, 18-19) (5 min)
- A*: orbit: midpoint of the radius band, authored height, turn limit (dragon-flight.md:16-18, 49) (5 min)
- A*: hover completion within 96 units; hovering while the next leaf runs (dragon-flight.md:10-12) (3 min)
- A*: away flight independent of NAVM; waiting advances the controller (dragon-flight.md:10-12) (5 min)
- A*: takeoff hold 1 s; landing wait 1.5 s; glide 2 s into cruise (dragon-flight.md:13-17) (3 min)
- A*: perch-and-shout sequences complete; a standalone perch holds (dragon-flight.md:14-16) (4 min)
- A*: perch choice (nearest loaded furniture, stable ties); landing search (dragon-flight.md:21-22) (5 min)
- A*: CSFL flight-style weights and hover/ground/perch durations (dragon-flight.md:18-22) (10 min, statistical)
- A*: missing combat ground/perch falls back to orbit; no landing surface holds in air (dragon-flight.md:23-24) (4 min)
- A*: forced landing marker constrains landing; SetAllowFlying(false) requests landing (dragon-flight.md:24-25) (4 min)
- A*: FlightGrab: victim attached at RLegFoot; release at graph payload or 1.067 s (dragon-flight.md:26-30) (4 min)
- A*: FlightGrab damage and collision criteria; no-collision-check flag (dragon-flight.md:26-33) (5 min)
- A*: interrupted or unloaded grab releases the victim (dragon-flight.md:29-30) (4 min)
- A*: shout word (last non-null word), 1 s charge, 3 s concentration, recast after load (dragon-flight.md:34-37) (4 min)
- A*: combat voice choice: first hostile SHOU by FormID, 4096 range fallback, zero CSGD weight (dragon-flight.md:38-42) (4 min)
- R: FLGT save of permissions, clocks, marker, perch, grab and combat phase; save mid-charge; old saves without FLGT (dragon-flight.md:44-47, 51)

### `effect-perks.md` (4 A, 1 R)

- A: temporary perks' ability and quest-stage sections (2 min)
- A: rank one, duplicate grants, priority ties with permanent perks, AddPerk/RemovePerk overlap (2 min)
- A: derived grants re-evaluated on the ~1 s ability cadence (SE: abilities' conditions recheck about once a second, research/autotest22.md "The probes work now") (2 min)
- A: Perk to Apply on an NPC's effect: granted only to the player (2 min)
- R: derived grants after save and load: follows the active effects `AEFF` (2 min)

### `effect-shaders.md` (6 V)

- V: membrane equation: fades, pulse phase, key-time units, edge/fill compositing, palette row (effect-shaders.md:17-24)
- V: depth and extrusion amount (effect-shaders.md:25-29)
- V: which shapes: Affect Skin Only and No Weapons classification (effect-shaders.md:30-32)
- V: stacking order and cap (effect-shaders.md:33-36)
- V: shader particles: density, vector frame, loop, palette timing (effect-shaders.md:37-45)
- V: addon models: spawn placement and timing (effect-shaders.md:46-48)

### `encumbrance.md` (2 A, 3 U)

- A: over means more than CarryWeight, not equal (player; the NPC native reads False whatever the load in SE) (1 min)
- A: NPC worn-armor slowing: formula per locomotion mode, Conditioning/Unhindered, creatures, weapon weight, refresh timing (6 min)
- U: player over-encumbered speed: walk or crawl (encumbrance.md:11-13)
- U: player worn-armor slowing: drawn value, sprint and sneak share, Mod Armor Weight counted as zero weight or as a flag (encumbrance.md:14-25)
- U: over-encumbered message once, when the player becomes over (encumbrance.md:26-27)

### `environment-maps.md` (2 V)

- V: reflection combining: added to the lit surface, multiplied by the light, not by diffuse colour (environment-maps.md:23-27)
- V: mask fallback: normal map alpha, then none; model-space normal maps (environment-maps.md:28-31)

### `escort.md` (8 A, 1 R)

- A: Run If Behind Distance: the escort catching up with an escorted actor ahead, or the followers running to keep up (escort.md:19-21, 52-53) (3 min)
- A: escort turning in place to face the one it waits for, past ~35° off to one side; the threshold, and whether it happens at all (escort.md:22-24) (2 min)
- A: follower selection: list order, furthest lagging follower sets the pace, a follower in another space holds (escort.md:40-45) (4 min)
- A: waiting band: close to FollowerMax past DistanceToWait, wait until FollowerMin; the geometry (escort.md:50-51, 54-55) (4 min)
- A: herd follows PathStart's linked patrol while its parallel Wait controls the interval (escort.md:46-48) (4 min)
- A: away escorts use the same selection and waiting rules (escort.md:55-56) (5 min)
- A: preferred-path cost factor 4 for Travel/Patrol/Escort; Sandbox/Wander PreferredOnly routes (escort.md:58-66) (5 min)
- A: Hadvar's MQ102 near/far branch switch (escort.md:68-71) (4 min)
- R: waiting-band hysteresis survives unload and save (escort.md:52)

### `factions.md` (9 A, 1 R)

- A: fights end when the factions stop being enemies, or fighters keep fighting until they lose the target (factions.md:36-39) (2 min)
- A: a faction listing another more than once: every entry changed, or only the last (factions.md:40-42) (1 min)
- A: SetEnemy / SetAlly booleans: neutral or enemy (ally or friend) per side (factions.md:27-31) (2 min)
- A: whether clearing the expulsion restores the rank; what else expulsion changes (factions.md:71-73) (1 min)
- A: allies of the player-enemy faction are exempt from attack (factions.md:92-96) (2 min)
- A: aggression-0 members of a player-enemy faction: flee, or only move away (factions.md:94-95) (2 min)
- A: GetActorCrimePlayerEnemy reads the subject's crime faction alone (factions.md:80-83) (2 min)
- A: SetAttackActorOnSight: which aggression attacks a marked actor, and whether unaggressive actors flee (factions.md:110-111) (2 min)
- A: SetAttackActorOnSight: allies of the marked actor are exempt (factions.md:111-112) (2 min)
- R: attack-on-sight mark survives saves (factions.md:113)

### `falling.md` (2 A, 1 U)

- A: fall height: highest point to landing, centre to centre, restarted by a >256-unit move; fJumpFallVelocityMin's role (falling.md:11-15) (3 min)
- A: water depth at which a landing is spared: surface at or above capsule middle (falling.md:16-17) (3 min)
- U: difficulty scaling of fall damage (falling.md:18-19)

### `fast-travel.md` (6 A, 3 U)

- A: gate re-enabled by a script MoveTo: a non-player MoveTo, SetPosition, angle change, and deferred movement timing (fast-travel.md:20-23) (2 min)
- A: marker icon and discovery vs EnableFastTravel (fast-travel.md:27-32) (1 min)
- U: refusal radius and priority: nearby hostiles, combat, search (fast-travel.md:43-49) 
- A: arrival link priority with several unkeyed links; interior-linked and edge cases (fast-travel.md:56-64) (2 min)
- A: elapsed time: measured from marker or arrival, horizontal or 3D, scaling with another timescale (fast-travel.md:81-91) (3 min)
- A: catch-up parity: effects, regeneration, mortality, schedules, weather (fast-travel.md:93-98) (3 min)
- A: native ordering of time advance vs arrival, pauses, loading failures (fast-travel.md:99-100) (2 min)
- U: map confirmation modal timing and autosaves (fast-travel.md:110-112)
- U: travel autosave ordering against the clock and cell change (fast-travel.md:117-124)

### `favorites.md` (3 U, 3 V)

- V: what is listed: carried favorite items and known spells and powers; a favorite that leaves and returns (favorites.md:12-31)
- U: Accept and attack-button equipping: spells in the hand, powers as the voice, items as Accept does (favorites.md:32-37)
- V: menu stays up after equipping; Tab, Esc, Q fade it out; reopens on the last selection (favorites.md:38-39)
- U: hotkeys 1 to 8 in the menu and in the world; number keys in dialogue (favorites.md:40-44)
- U: mouse buttons equip the selected entry wherever the cursor is (favorites.md:36-37)
- V: sounds: only the movie's own list movement (favorites.md:45-46)

### `find-acquire-flee.md` (13 A, 1 R)

- A: Find: which item is chosen (nearest first, own inventory first, shuffle) (find-acquire-flee.md:6-11, 18-20) (4 min)
- A: Find: holder inventories inside the area (containers, corpses) count (find-acquire-flee.md:6-8, 18-20) (3 min)
- A: Find: nested and faction FLST criteria, cycles, script additions (find-acquire-flee.md:9-10, 21) (3 min)
- A: Find and Acquire: empty lists retry every 3 s beside Wander; repeat restarts Find (find-acquire-flee.md:12-13, 27-28) (3 min)
- A: Acquire: 96-unit approach, loose stacks taken whole, container and corpse transfers stop at quota (find-acquire-flee.md:23-27, 32-33) (4 min)
- A: Acquire: owned items and living holders never eligible, whatever AllowStealing or AllowPickpocketing say (find-acquire-flee.md:29-32) (3 min)
- A: Flee: finishes at the goal radius or when named threats are at DistanceToFlee; destination choice (find-acquire-flee.md:35-37, 43-44) (3 min)
- A: Flee: unnamed targets fall back to current attackers; dead threats dropped only with ForgetDeadThreats (find-acquire-flee.md:37-38) (3 min)
- A: Flee: distance zero uses fFleeDistanceInterior 3000 / Exterior 5000 (find-acquire-flee.md:38-39) (3 min)
- A: Flee: Quiet suppresses FLEE; the bark channel tried every four real seconds (find-acquire-flee.md:39-40) (3 min)
- A: Flee: avoid-node radii as destination tolerance (find-acquire-flee.md:42) (3 min)
- A: Flee: CountsAsIsFleeing honoured; older Flee trees default true (find-acquire-flee.md:41) (3 min)
- A: away movement at the walker speed until routing (find-acquire-flee.md:43-44) (5 min)
- R: saved Find lists keep their ordering; save and reload between Find and Acquire (find-acquire-flee.md:15-16, 21, 33)

### `first-person.md` (1 A, 2 A*, 2 U, 1 R, 4 V)

- V: vanilla suppression of helmets and sheathed arms; MOD2/3 fallback for shields; addons omitted (first-person.md:7-17)
- V: camera alignment, eye height, aim pitch, Camera Control and camerafrom variables (first-person.md:19-29)
- V: projection and depth: near 1, FOV 65 (first-person.md:39-43)
- V: gameplay origins: hand art, bow nock, shield guard, eye-to-hand convergence (first-person.md:45-52)
- U: which graph vanilla script listeners hear for draw and release events; HitFrame and bow release across a view switch (first-person.md:31-37)
- U: scroll wheel into and out of first person (first-person.md:54-57, 61-62)
- A: script Force calls bypass camera input locks (first-person.md:56-57) (1 min)
- A*: furniture with the DOBJ FFFP keyword forces first person for its session, restoring the view on exit (first-person.md:57-62) (4 min)
- A*: the i1stPerson contract: animation variable values on the first-person rig and third-person rig (first-person.md:64-71) (3 min)
- R: saves preserve the underlying view, not the furniture override (first-person.md:58-60)

### `followers.md` (7 A, 1 U, 2 R, 1 V)

- A: teammate joins a fight the player is in, against the player's attacker; whether factions or AI assistance or own-ally status stop it (followers.md:42-46) (3 min)
- A: player answers for a teammate's or summon's first blow on a calm, law-abiding target (followers.md:47-51) (3 min)
- A: whether a teammate's kill counts in the player's kill statistics (followers.md:50-51) (2 min)
- A: teammate count includes unloaded teammates (followers.md:52) (1 min)
- A: non-teammate followers (package Follow) join the party through doors, fast travel and waiting; waiting and dead ones excluded (followers.md:71-79, 86-97) (5 min)
- A: a teammate wears a given armour only when it is better (followers.md:53-54) (2 min)
- A: after a follower's death, dismissal clears the teammate flag and the slot, after the wait (followers.md:99-108, 120-121) (3 min)
- U: teammate carry-weight refusal: stack that crosses the limit; partial moves; dead teammates (followers.md:55-69)
- R: TEAM flags in saves (followers.md:81-84)
- R: follower party after a save load just before fast travel or waiting (followers.md:120-121)
- V: AllyKilled barks: witnesses, per-fight cooldown, latest pending death, multiple deaths (followers.md:110-118, 121-122)

### `foot-ik.md` (4 V)

- V: gain semantics: per 30 Hz frame; on/off fade and 0.2 out; ground ascending/descending; feedback; error bias; planted/raised; ankle orientation (foot-ik.md:28-40)
- V: step height for standing on the collision surface: 40 up, 32 down, giants 100/128 (foot-ik.md:43-47)
- V: planted-foot locking: 0.8 acquire threshold, 30 Hz unlock gain, 5% lag limit (foot-ik.md:69-89)
- V: align world-from-model: support-plane fit, gain, ankle-limit cap (foot-ik.md:90-100)

### `force-greet.md` (4 U)

- U: greeting again after 10 s while the package applies and the player stays in the trigger (force-greet.md:28-30)
- U: "player must be detected": detection above 0, without stealth points (force-greet.md:31-32)
- U: "in cell" trigger locations: package location rule vs the player in that cell (force-greet.md:33-34)
- U: seated greeters speak from the seat once the player is in the trigger (force-greet.md:35-36)

### `formlists.md` (1 R)

- R: additions persist in saves: all lists or only lists from loaded plugins (formlists.md:33-35)

### `furniture.md` (6 A, 1 V)

- A: an NPC activates furniture as it begins to enter; scripts hear OnActivate; activate children (furniture.md:7-13) (2 min)
- A: actors placed in furniture as a cell loads don't activate it (furniture.md:14) (2 min)
- A: ways on are filtered by item-count idle conditions (furniture.md:15-18) (3 min)
- A: one-shot furniture: the clip's length, the exit sequence (IdleForceDefaultState, exit event) (furniture.md:19-23) (2 min)
- A: paired furniture: which registrations hear a paired notification, ordering, duplicates (furniture.md:38-46) (5 min)
- A: Wait's StopMovement input: true interrupts a walk, false lets it continue (furniture.md:47-51) (3 min)
- V: carrying: anim objects kept, OffsetStop on entering furniture, objects dropped on standing (furniture.md:24-28)

### `gifts.md` (2 U, 1 V)

- U: favor points: how the game counts a gift's value (cap of 100 in the script) (gifts.md:15-17)
- U: what is listed: the giver's filtered items, keywords, gold never, stolen items only with abShowStolenItems (gifts.md:18-21)
- V: bottom bar shows favor points through UpdatePlayerInfo (gifts.md:25-26)

### `grass.md` (4 V)

- V: candidate grid (20 units), texture pick, chance per grass (grass.md:22-26)
- V: ranges: position, height and colour (grass.md:27-29)
- V: wave period: length of the waves; blade bend (grass.md:30-32)
- V: fade from 7000 units over 1000, alpha test (grass.md:33-34)

### `hands.md` (6 A, 1 A*, 2 U, 3 V)

- A: unasked hand placement for NPCs and bound weapons, and the first-hand fallback (hands.md:71-74) (1 min)
- A: displacing and pairs: which hand empties when one of two equal weapons leaves (hands.md:75-80) (2 min)
- A: NPC takes its weapon back from a torch to fight (hands.md:94-98) (3 min)
- A: blows per HitFrame; dual power attack stamina; frames under 0.25 s are one blow (hands.md:124-131) (3 min)
- A: leftWeaponSpeedMult from the off-hand weapon; WeaponSpeedMult 0 counting as 1 (hands.md:132-137) (1 min)
- A: NPC dual wielders do not block or bash (hands.md:138-143) (3 min)
- A*: asked hand for one-handed weapons, by slot-aware equip or attack button; EitherHand's effect on weapons (hands.md:65-70) (3 min)
- U: attack buttons per hand: right, left, both, and the fists' rules (hands.md:108-116)
- U: which attack the race gives for the hands (hands.md:117-123)
- V: equip icons (hands.md:87-90)
- V: left-hand weapon shown only drawn (hands.md:91-93)
- V: mouse in item menus (hands.md:99-104)

### `harvest.md` (2 A, 2 A*, 1 U, 1 V)

- A: owned plants: theft (1 min)
- A: Ingredients Harvested counts each ingredient (1 min)
- A*: PFPC seasonal percent: does it change the produce? (the all-zero cabbage is harvestable) (3 min)
- A*: regrowth after 240 h (3 min)
- U: activation picking of plants (non-collidable shapes, nearest hit)
- V: red prompt on owned plants; produce marked as the owner's

### `head-tracking.md` (1 A*, 2 V)

- A*: AHEL greeting range: proximity sending it (fAIMinGreetingDistance 150, 32-unit exit band) (3 min)
- V: head-tracking target = scene listener
- V: head-tracking target rules: species-specific attention, line of sight, head points

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

### `integer-generator-bit-bindings.md` (2 N)

- N: explicit sync master semantics
- N: bit-selector semantics: bit assignment and conversion, evaluation order, duplicate bindings

### `interp-value-modifier.md` (2 A*)

- A*: step: chase vs linear from activation, read by sampling SpeedDamped each frame after an attack starts (3 min)
- A*: does the game set SpeedDamped from code, and does an attack on the move start from its last value? (3 min)

### `inventory-cards.md` (9 V, 1 N)

- V: scroll card payload (card type, fields, cost shown as 0)
- V: black soul label
- V: school meter refresh (script skill changes while open)
- V: best-in-class classes
- V: best-in-class compared numbers
- V: best-in-class ties and singles
- V: best-in-class scope
- V: ingredient learned-effect label
- V: egui quantity chooser's appearance, default slider position and focus
- N: other ways to learn ingredient effects

### `inventory-restrictions.md` (1 A, 2 U, 1 V)

- A: RemoveAllItems on bound weapons (1 min)
- U: console drop bypasses Can't Drop
- U: menu transfers of a playable bound weapon (Bound Weapon flag set, Non-playable clear)
- V: Can't Drop feedback

### `invisibility.md` (6 A, 3 U)

- A: attack-start / hit-frame ordering, including UseWeapon practice (2 min)
- A: constant invisibility left alone (1 min)
- A: mixed self spells, failed renewal, scripted casts whose effects fail, concentration timing (3 min)
- A: eating an ingredient breaks it (1 min)
- A: arrow interrupts at spawn, not draw (NPC) (2 min)
- A: an NPC's own activation interrupts (2 min)
- U: player's own activation of a locked door or chest (lock message, lockpicking start)
- U: player's inventory actions, grabbing, crafting and dialogue started outside the Activate path
- U: player charging a hand spell without delivering it

### `journal.md` (5 A, 1 A*, 4 U, 13 V, 1 N)

- N: opening callbacks
- V: quest first selected
- V: quest/objective order
- V: objective states
- V: misc tracking
- V: closing
- V: date format
- V: level meter with pending level-ups
- V: which statistics per category, order
- V: magic favorites scoring
- V: Favorite Weapon
- V: quit list default selection
- V: Show on Map: which target the camera centres on when several
- V: Show on Map opening zoom
- A: crime counts when reported (assault, murder, trespass once each) and Items Stolen (2 min)
- A: jail counts: Times Jailed, Days Jailed, Jail Escapes (2 min)
- A: holds' Bounty statistics (the bounty with each hold's crime faction) (1 min)
- A: Gold Found, Chests Looted, Most Gold Carried (1 min)
- A: type statistics a completed quest counts in besides Quests Completed (main, guilds, Daedric, Civil War, Dawnguard, Dragonborn) (2 min)
- A*: Skill Increases from skill use, not scripts (3 min)
- U: crafting statistics
- U: Books Read / Skill Books Read
- U: Barters
- U: Pockets Picked and Items Pickpocketed

### `loading-screen.md` (1 A, 1 U, 13 V, 1 N)

- A: game paused through load and fades (2 min)
- U: mouse turning
- V: fade durations
- V: minimum time on screen
- V: player info on screen
- V: which screen is chosen
- V: later texts drawn from
- V: mist model and colour
- V: model transform
- V: camera path, FOV
- V: lighting
- V: scene fade
- V: default logo
- V: sound during load
- V: whether a quick load or the death reload shows the full screen, and where "Quickloading..." / "Reloading the most recent save game" appear
- N: random stream

### `local-map.md` (13 V)

- V: interior area bounds (OBND of references, markers and Not On Local Map excluded; the game's rule and any missing geometry)
- V: interior with no usable reference bounds (512-unit fallback box)
- V: which objects the map draws; back-face culling and ceilings
- V: effects with sentinel OBND boxes omitted from the fit and draw
- V: look: fixed neutral grey lighting; no actors, effects, weather or fog
- V: interior with no NorthMarker (zero rotation)
- V: world map marker when opened from an interior (the exterior door chosen, its position)
- V: fit margin (5% of the larger side) and minimum view size
- V: map camera update method and frequency
- V: reveal grid: 256-unit squares in 16 by 16 pages, one grid per interior and worldspace
- V: reveal range: exact value (about 20 body lengths) and whether it differs indoors and outdoors
- V: reveal's line of sight, lights and height
- V: unrevealed squares' fog colour and opacity, and the custom destination's visibility there

### `locks.md` (11 A, 2 A*, 1 U)

- A: unloaded actors' doors: evaluation cadence (5 min)
- A: animated doors open for walkers with key or ownership (3 min)
- A: alias ALFC faction rank and conflict policies (2 min)
- A: alias CNTO inventory: permanent or not; re-grant on refill (2 min)
- A: Lock(abAsOwner) edge cases: several entrances, Off Limits, package changes after an override (3 min)
- A: which doors a package locks; whether unlocking waits on the owner being home (5 min)
- A: after asking the player to leave: lock later, or warn again on return (5 min)
- A: leaving through a locked pair from inside (2 min)
- A: what lets NPCs through locked load doors, including the Falk-style package jumps (5 min)
- A: leveled alias CNTO rolls: recipient level, rerolls on refill or failed start (3 min)
- A: warning approach distance, bed entry and escalation (5 min)
- A*: XRNK owner rank (3 min)
- A*: procedure-tree location locks: which doors (owned, unowned, radius) (5 min)
- U: lockpicking curve, Locksmith arc, Quick Hands crime chance

### `loose-objects.md` (6 A, 3 U, 6 V, 3 N)

- A: references with "Don't Havok Settle" (0x20000000) keep their placed place on load (2 min)
- A: ApplyHavokImpulse magnitude as kg m/s, and the masses (3 min)
- A: damping values (linear 0.1, angular 0.5) (2 min)
- A: where dropped items appear: 50 ahead, 70 up in ours; the game's were 140.5 ahead and 107.8 up (2 min)
- A: corpses follow their ragdoll's first body (partial: PushActorAway 5 moved -9.9) (2 min)
- A: a cart driven into another loaded exterior cell goes on in it (5 min)
- V: triangle meshes on moving bodies become convex hulls
- V: constrained multi-body models (hand cart, sign, chain, board)
- V: angle limit sign
- V: constraint types (ball-and-socket, rope)
- V: breakable constraint threshold units and impulse summed
- V: SetMotionType leaves NIF-fixed bodies in place
- N: constraint pivot frames
- N: motors (read, not simulated)
- N: falling out of the world (put back at 4096 units; held fixed)
- U: grabbing (hold, pull, drop limits, owned things)
- U: player blows push objects (BLOW_PUSH 4 kg m/s)
- U: throwing held objects

### `magic-menu.md` (2 A, 1 U, 4 V)

- A: negative school modifiers raise cost (2 min)
- A: SpellHasSkill matches secondary effects (3 min)
- U: readying with buttons; hands; powers to the voice slot
- V: categories
- V: level label
- V: card cost, and per-second display for concentration spells
- V: active effects listing

### `magic.md` (16 A, 1 A*, 3 U, 2 V)

- A: absorb magicka of the College staff (MG07StaffEffect took 100 for 50, gave the caster none in the game; ours gives what it takes) (2 min)
- A: a fire-and-forget effect whose condition fails: does it land? (2 min)
- A: calm, fear, frenzy, rally hold the actor value at their magnitudes; stacking holds (2 min)
- A: calm, fear, frenzy level cap after resistance (2 min)
- A: IsUndead caching rule (1 min)
- A: calm: broken by an attack, and what follows (2 min)
- A: fear: a coward checks every frame (2 min)
- A: frenzy: target attacks the nearest actor; clean-up on end (3 min)
- A: cloaks: reach, rate, who is spared (3 min)
- A: ethereal: a blow or bash to an ethereal actor lands with no damage, OnHit or poison (3 min)
- A: ethereal: an NPC fights an ethereal player (3 min)
- A: Slow Time: world share and game clock sampled each second through and after the effect (4 min)
- A: Mod Spell Magnitude perk subjects (owner, tabs) (2 min)
- A: NPC enchanted weapons: charge per blow against the enchantment's cost (3 min)
- A: effects end on the dead (except No Death Dispel) and on unloaded actors (3 min)
- A: Spells Learned counts scripted AddSpell and tome spells (2 min)
- A*: summons: placement, limit, follow distances (4 min)
- U: a player's hand-cast hostile spell: assault and fight
- U: player's attack, cast or activation ends ethereal (shouts, powers, potions don't)
- U: player's walk under Slow Time
- V: spell tome wording
- V: consume sounds default (ingredients, potions naming none; listener)

### `main-menu.md` (2 U, 4 V)

- U: Continue's save choice (newest across characters, passing corrupt and other-plugin saves)
- U: Load list's starting selection on PC (does Enter go on to the saves?)
- V: title music loop, category and volume; backdrop
- V: credits scroll speed units; fade back in when closed
- V: Quit to Main Menu fade duration
- V: "No save can be loaded." message wording

### `map-markers.md` (2 A, 1 A*, 5 V)

- A: reveal distance (between 2000 and 4000 units observed) and check cadence (3 min)
- A: markers on the map from the start (FNAM 1) discovered when reached (2 min)
- A*: discovery grants character XP (iXPRewardDiscoverMapMarker not given) (3 min)
- V: discovery music per marker type
- V: compass frame mapping
- V: compass marker distances and fade; custom compass marker in exteriors and interiors
- V: AddToMap message ("Map updated.")
- V: placing, moving or removing the custom destination: sound (silence chosen)

### `map-view.md` (8 V)

- V: map extent (corner cells in or out)
- V: camera height, pitch and FOV
- V: what is drawn (including the game's lighting, clouds and sea)
- V: player marker projection
- V: markers on the map (height, hollow icons)
- V: hover description (what else the game lists)
- V: child worldspace projection
- V: camera on opening: whether the turn, zoom and tilt also reset

### `math-natives.md` (4 A)

- A: out-of-domain inputs other than 2 and -2 (we return 0 outside [-1, 1]); nonfinite inputs (1 min)
- A: float to Int cast of in-range fractions, truncation or rounding (1 min)
- A: pow of zero to other negative powers (1 min)
- A: how infinity prints (1 min)

### `melee-reach.md` (3 A, 2 N)

- A: reach from chest to target body height (3 min)
- A: 1.3 reach margin (4 min)
- N: heights without BSBound
- N: player capsule as body
- A: unreachable targets: wait and swing (3 min)

### `messages.md` (2 U, 14 V)

- U: help message "done" counting: an event counts only while its help is up; does the game also count an input made before any help showed? (2 min) Needs the player's input, known_ambiguities/messages.md:91-96
- U: whether the console counts as menu mode (timers stop with it open)? (3 min) Needs the console opened, known_ambiguities/messages.md:163-165
- V: notification TNAM vs the HUD's fixed fade animation
- V: TNAM edge cases (zero, truncated, huge), fade timing, queue delay
- V: one help message at a time?
- V: untyped quests' objectives in the HUD
- V: journal log entries shown: earlier entries, ordering
- V: quest banners timing and wording
- V: text instance data (keep flag); Global tags in any quest's text
- V: world sounds and notifications/help timers in menu mode
- V: PIMC resume fade
- V: loading screens keep control tags as written (messages.md:135-136)
- V: art for a key with no bitmap: bracket kept or key name (messages.md:137-144)
- V: which binding wins: a later context's keyboard over an earlier context's mouse; exact-case name before case-insensitive; event names over key names (messages.md:145-151)
- V: other languages' key names (messages.md:157)
- V: PDMC fade, default 3 s out / 1 s in (messages.md:161-163)

### `movement-gaits.md` (1 A)

- A: fast-walk speed for humans and other horses: is a tenth of the walk-to-run gap right beyond the one measured ride? (5 min)

### `music.md` (2 R, 7 V)

- R: script-added music survives saves (`Add` a type, save, load)
- R: fades, ducking envelopes and playback cursors restart on load
- V: transition duration owner (incoming WNAM vs outgoing)
- V: ducking: whether underlying tracks advance and how attenuation stacks
- V: gap between tracks (20-60 s provisional); shuffle
- V: equal-priority ties; whether `Add` of a queued type moves it
- V: track and palette interpretation (layers, palette fades, weights)
- V: default music triggers (BTMS, DTMS, LUMS, SCMS, DCMS)
- V: graceful type change: cut at the next cue, BNAM outro over the transition; cue and outro timing

### `new-game.md` (4 U, 1 N)

- U: the engine starts MQ101 on a new game, after the start-game-enabled quests: which order, and how (new-game.md:32-35)
- U: startup stages run before the world loads: when relative to the load, and what is loaded before MoveTo (new-game.md:36-40)
- U: a new game with nothing moving the player: where SE starts them (new-game.md:41-44)
- U: a new game's calendar weekday carried over from a game loaded before (new-game.md:50-52)
- N: the message for a new game that can't start (new-game.md:45-47)

### `node-controllers.md` (7 V)

- V: APP_INIT clock origin; restart on reload; catch-up after Disable/Enable
- V: competing drivers order (standalone, named sequences, behaviour poses)
- V: loop endpoint sampling (in-range stop sample)
- V: NiFlipController on BS shaders (slot mapping, frame rounding, out-of-range)
- V: a named sequence's keyed parts keep their final pose after it stops (known_ambiguities/node-controllers.md:56-58)
- V: standalone transforms move render parts only, collision stays put (known_ambiguities/node-controllers.md:82-83; see object-graphs' collision item)
- V: hidden placed emitters clear live particles; SE may keep them (known_ambiguities/node-controllers.md:85-86)

### `npc-healing.md` (6 A)

- A: heal start/stop thresholds, cooldowns (4 min)
- A: which spells count as heals (2 min)
- A: costliest affordable heal chosen (2 min)
- A: concentration heals: one second's cost to stay selected, quarter-second payments, 1.5 to 3 s cadence (3 min)
- A: healing before a weapon or bow attack regardless of the style's magic multiplier (2 min)
- A: self healing: no enemy range, line of sight or facing needed (3 min)

### `npc-mounts.md` (9 A*, 1 R)

- A*: horse choice: XHOR, then linked, then nearest owned by the NPC base; faction-owned and unowned horses not used (npc-mounts.md:7-10) (5 min)
- A*: two riders competing for one horse, in rider order (npc-mounts.md:6-8, 36) (5 min)
- A*: approach within 128 units; HorseEnter; enter the idle after 2.5 s; exit 2.5 s, placed 100 units aside (npc-mounts.md:10-12) (3 min)
- A*: IsOnMount / IsRidingMount during enter and exit (approach excluded); Dismount's return value (npc-mounts.md:12-13, 36) (3 min)
- A*: combat, package change or death release the ride; load-door interruption (npc-mounts.md:27-28, 36-37) (5 min)
- A*: away travel: a rider's speed at walk and run gaits (npc-mounts.md:29-31, 36-37) (5 min)
- A*: Dismount inhibits remounting that package (npc-mounts.md:28) (3 min)
- A*: a freshly spawned rider starts mounted at once (HorseEnterInstant) rather than mounting; Tullius was mounted at fade-in, which doesn't show when (npc-mounts.md:21-25) (2 min)
- A*: a change to another riding package keeps the ride for other actors (npc-mounts.md:16-20) (3 min)
- R: MOUN: is a mid-ride save reloaded mounted, and what is restored (npc-mounts.md:33-34) (3 min)

### `npc-templates.md` (10 A, 1 R, 2 N)

- A: empty spell list inheritance cases: an unflagged NPC's list empty, or an inherited source empty with its own template's entries (npc-templates.md:57-60) (1 min)
- A: ability sync timing on load (npc-templates.md:61-64) (2 min)
- A: leveled spell list roll time; reset rerolls (npc-templates.md:65-66) (3 min)
- A: leveled spell list level: actor's vs player's (npc-templates.md:90-93) (2 min)
- A: HasSpell of the LVSP form (npc-templates.md:101-102) (1 min)
- A: copied actor's leveled spell list yields nothing (npc-templates.md:103-104) (1 min)
- A: HasKeyword: worn items / magic effects (npc-templates.md:118-125) (1 min)
- A: a child's spell list replaces the template's spells and perks, not merged (npc-templates.md:34-39, 23-24) (2 min)
- A: racial spells come from the race of the traits part, not the spell-list source's (npc-templates.md:25-26) (2 min)
- A: an NPC templated without the factions flag and with none of its own is in none of its template's factions (npc-templates.md:111-114) (2 min)
- R: a leveled spell roll kept across save and load, including an empty result (npc-templates.md:94-100) (3 min)
- N: missing scalar parts taken from template (npc-templates.md:110-112)
- N: NPC ATKD replaces race's entirely (npc-templates.md:115-117)

### `object-graphs.md` (10 A, 1 A*, 4 V)

- V: transform-controller nodes drawn apart (object-graphs.md:34-35)
- V: collision moved as fixed colliders; bodies fall when those move away (object-graphs.md:36-38)
- V: tail auxiliary graph feeding (object-graphs.md:94-101)
- V: graph retention across outfit changes (object-graphs.md:120-125)
- A: graphs and scripts kept across unload; time doesn't pass for graphs (object-graphs.md:39-45) (2 min)
- A: OnCellLoad on reload and its order vs OnLoad (object-graphs.md:43-49) (2 min)
- A: GetOpenState for activators (object-graphs.md:58-60) (1 min)
- A: how long natives like GetOpenState wait (object-graphs.md:61-71) (2 min)
- A: animation registrations kept across the sender unloading (object-graphs.md:75-78) (2 min)
- A: graph-event waits ignore case, as registrations do (the blade's `Reset` / `reset`) (object-graphs.md:79-81) (1 min)
- A: script-set graph variables vs the engine's own writes (object-graphs.md:82-93) (2 min)
- A: payload events other than SoundPlay: withheld from scripts? (object-graphs.md:139-141) (2 min)
- A: PlayAnimationAndWait on a payload event: does the wait reach its timeout? (object-graphs.md:143-145) (1 min)
- A: PlayAnimationAndWait with a missing event: does the game's wait end, and when (10 s here)? (object-graphs.md:72-74) (5 min)
- A*: a new graph attached: initial state and event replay; competing base-skeleton graphs (object-graphs.md:120-125) (3 min)

### `offscreen-navigation.md` (5 A, 2 A*)

- A: a long bent route: timing and positions around terrain and through an interior; which of two routes is taken (centre-distance cost) (offscreen-navigation.md:9-16, 32-33) (5 min)
- A: loose clutter steering: 90-unit look-ahead, 24-unit bounds expansion, 35/65 diagonal sidestep, 30-unit floor check, waiting when neither side is free (offscreen-navigation.md:24-27, 33-34) (5 min)
- A: tiny or light loose objects (under 0.2 kg or 12 units high) keep capsule contact, not avoidance (offscreen-navigation.md:28-29, 33-34) (3 min)
- A: crossing an XTEL edge: no distance added between coordinate spaces (offscreen-navigation.md:15-16) (5 min)
- A: no route means no movement, apart from measured explicit jumps (offscreen-navigation.md:17-18) (3 min)
- A*: actor-specific locked portals, and permitted crossing of animated door obstacles; a second overlapping obstacle still blocks (offscreen-navigation.md:19-23, 33-34) (5 min)
- A*: NVPP groups do not create nonadjacent connections (offscreen-navigation.md:9-10) (5 min)

### `outfits.md` (3 A)

- A: old outfit's armour the NPC holds but doesn't wear: removed or left? (2 min)
- A: ActorBase.SetOutfit's effect on existing actors of the base and on actors made later (1 min)
- A: outfit leveled lists re-rolled on re-equip (2 min)

### `package-dialogue.md` (10 A*, 1 R, 1 V)

- A*: Say's response end under each Finish Speaking value; an abandoned INFO's remaining responses never start (package-dialogue.md:33-38) (3 min)
- A*: multi-response INFO: fragment timing; the end fragment runs on normal completion only (package-dialogue.md:35-38, 57-58) (3 min)
- A*: Say with no eligible or empty INFO completes without speech; Say stays where its Travel left it (package-dialogue.md:25-28) (2 min)
- A*: unloaded speaker: no response starts, queued responses aren't consumed; an interrupted response replays from its start (package-dialogue.md:39-42) (5 min)
- A*: DialogueActivate's completion relative to the ADIA scene; the HELO greeting fallback with the player (package-dialogue.md:48-54) (3 min)
- A*: IsPlayerMovingIntoNewSpace: when it is true across a load door (package-dialogue.md:72-75) (5 min)
- A*: a nearby greeter during both sides of a load door's fade (package-dialogue.md:92-93) (5 min)
- A*: alias and link trigger areas: unfilled and refilled trigger aliases (package-dialogue.md:86-88, 93) (3 min)
- A*: sitting wait versus from-sitting greets (package-dialogue.md:79-80, 93-94) (3 min)
- A*: a player riding in a carriage: the RidingInCarriage keyword check (package-dialogue.md:88-90, 93-94) (5 min)
- R: conversation end and save/reload with a started leaf; it completes on return (package-dialogue.md:79-84, 94-95) (3 min)
- V: forced subtitles: the distance boundary, and with general subtitles disabled (package-dialogue.md:29-32, 57-58) (2 min)

### `package-trees.md` (2 A, 17 A*, 1 U, 2 R, 3 V)

- A*: numeric package data: invalid, negative or large inputs; run_on 6 (package-trees.md:59-79) (3 min)
- A*: package location: 3D vs flat distance; near package start; exterior "in cell" as grid square (package-trees.md:116-124) (4 min)
- A*: multi-location Wait in real or game seconds (package-trees.md:150-151, 223-224) (6 min)
- A*: multi-location clock starts on arrival; whether Sandbox delays its simultaneous Wait (package-trees.md:154-159) (3 min)
- A*: Random picks the same stop twice (package-trees.md:160-162, 220-222) (3 min)
- A*: Stacked-root trees: first passing branch (package-trees.md:170-172) (3 min)
- A*: Simultaneous ends on its first completing child, or on all (package-trees.md:211-215, 230) (3 min)
- A*: Stacked recheck cadence; a returned-to branch restarts or resumes (package-trees.md:216-219, 230-231) (3 min)
- A*: Wait with zero seconds holds indefinitely; a Wait-only phase keeps its seat past furniture time (package-trees.md:253-255) (3 min)
- A*: a statically unset Activate target or zero use count completes; dynamically unresolved aliases and null sentinels (package-trees.md:255-259) (3 min)
- A*: a Wait or Sleep across phase boundaries: a newly entered Wait gets only later seconds (package-trees.md:261-264) (5 min)
- A*: nearest suitable reference, equal distances broken by FormID; search radius default 768; Beds/Chairs marker kinds (package-trees.md:285-292, 298-302) (5 min)
- A*: object-type memberships: Combat Wearable, Wearable, Spell Range Target, Weapons None (package-trees.md:294-299) (3 min)
- A*: UseIdleMarker with a missing or occupied marker waits (package-trees.md:306-309) (3 min)
- A*: EndPursuitArea: pursuit semantics (package-trees.md:311-312) (5 min)
- A*: GroupMovement and SingleFileFollow: follower order and 192-unit spacing; FollowTo ends on arrival; TravelCrouchWhenNear's standing and crouched legs (package-trees.md:320-329) (5 min)
- A*: a PACK stays current when the other procedures of a sequence finish; only root completion ends it (package-trees.md:225-228) (3 min)
- A: default package lists consulted only when none of the NPC's own applies, or in another order (package-trees.md:173-175) (3 min)
- A: patrolling from Self walks the actor's own linked reference chain (package-trees.md:176) (3 min)
- U: menu time: a Wait in a menu advances a loaded Wait; native menu timing for those transitions (package-trees.md:261-269) (needs a menu opened by the player, 6 min)
- R: multi-location progress kept across loads (package-trees.md:141-144) (4 min)
- R: SetLookAt / ClearLookAt continuity across away travel and save/load (package-trees.md:314-315) (3 min)
- V: a furniture exit animation skipped during the menu advance; carried objects survive (package-trees.md:265-267)
- V: KeepAnEyeOn observation-area boundaries, head tracking while moving and body facing when idle (package-trees.md:309-312)
- V: abPathingLookAt permitting an idle body turn as well as head tracking (package-trees.md:316-318)

### `particles.md` (19 V)

- V: particle space: a world-space system's particles carried back when its instance moves (particles.md:33-36)
- V: emission rate and clock; emitter shapes; stop at the maximum count (particles.md:37-44)
- V: direction and speed variations (declination, planar angle, speed/radius/life variation "made up") (particles.md:45-48)
- V: size and aspect (particles.md:49-53)
- V: forces: planar and spherical gravity (particles.md:54-57)
- V: rotation (particles.md:58-60)
- V: subtextures (particles.md:61-63)
- V: spawning on death (particles.md:64-66)
- V: colour and fade: vertex colour and alpha mapping (particles.md:67-71)
- V: run distance (6000 units) and warm-up (3 s ahead) (particles.md:72-74)
- V: addon nodes and master systems; per-node copies; empty value nodes (particles.md:75-84)
- V: palette and vertex colour: whether vertex colour tints the palette colour; vertex alpha with a colour-only palette; row convention (particles.md:86-119)
- V: multi-target emitters: addon transforms, and particles per node (particles.md:121-166)
- V: directional drag: falloff curve, additive vs absolute endpoint, zero range, and the temporal formula (particles.md:168-214)
- V: rigid mesh emission: weighting, repeated references, node references, surface sampling (particles.md:216-263)
- V: particle colliders: sidedness, overlap recovery, tolerance, swept vs discrete, impact spawning details (particles.md:265-321)
- V: speed to aspect: interpolation, speed space, current or birth velocity, width vs height (particles.md:323-356)
- V: initial rotation controllers: deactivation, spawned descendants keeping the parent's spin (particles.md:358-401)
- V: blended particles sorted by their system's centre, not each particle (particles.md:73-74)

### `perks.md` (3 A, 3 A*, 2 U)

- A: SetStage on a quest that ran and was stopped: restart or not (1 min)
- A*: perk ranks: an entry applies above its rank (2 min)
- A*: entry point tab subjects, as listed in the file (5 min)
- A: order of damage, armour and block modifications (4 min)
- A*: stealth perk formula: Mod Detection Sneak Skill's percentage in the observer's skill factor (4 min)
- U: buying perks: one point per rank, if the rank's conditions pass (needs the skills menu)
- A: arrow recovery rolled on strike or on search (3 min)
- U: Wax Key repeat rewards: does the game suppress a second reward after the first pick (needs lockpicking, perks.md:73-76) (5 min)

### `player-body.md` (1 A, 2 A*, 9 U, 4 V, 1 N)

- V: head part replacement per type (player-body.md:23-28)
- N: Direction binding through cyclic wrapper (player-body.md:29-33)
- U: sheathed third-person facing: backpedal and strafe, or turn round (player-body.md:34-37)
- A*: falling event threshold (0.3 s off the ground) (player-body.md:38-39) (2 min)
- V: first-person shadow of the third-person body (player-body.md:40-41)
- U: attacking with the weapon put away draws it (player-body.md:42-43)
- U: swing gating during the draw (player-body.md:44-47)
- A: equipment change while drawn: drawn state kept, blocking survives, and whether a weapon-type switch replays the equip transition (player-body.md:87-104) (2 min)
- U: power attack by movement direction (player-body.md:111-116)
- U: blow timing and attack gating (HitFrame, 1.5 s, 0.4 s) (player-body.md:117-123)
- U: bow release timing (player-body.md:124-139)
- U: interrupted and duplicate bow releases (player-body.md:140-156)
- V: third-person camera starting distance (player-body.md:157-160)
- V: shoulder offsets and their pivot (player-body.md:161-168)
- U: third-person aim: the crosshair's match (player-body.md:169-172)
- U: disabled camera switching stops the wheel's zoom (player-body.md:173-174)
- A*: draw events for spells, mixed hands, and the redraw when hands change (player-body.md:48-86) (4 min)

### `player-controls.md` (3 A, 1 U, 1 V)

- A: IsCamSwitchControlsEnabled with both POV owners (1 min)
- A: unknown POV types (1 min)
- A: POV owners overlap / repeated calls (1 min)
- U: SetPlayerAIDriven arbitration between user input and package motion
- V: cart mode precedence vs dialogue

### `poisons.md` (1 A, 1 A*, 7 U, 3 V)

- A: NPCs poisoning their weapons in combat (3 min)
- A*: EquipItem on a poison (run last: may open a box) (1 min)
- U: which hand the poison goes on
- U: question wording
- U: bound weapons take poisons
- U: blocked blows use a dose
- U: bows: dose leaves with arrow
- U: one poisoned instance per weapon form
- U: dose count perk tabs
- V: refusals as notifications
- V: when sPoisoned shows
- V: poison icons on grouped weapons

### `power-bash.md` (1 A, 4 U)

- A: NPC power bash stamina fallback (3 min)
- U: hold threshold
- U: cancellation
- U: hit timing
- U: torch bashes, dual wield guard

### `protection.md` (6 A, 1 U)

- A: alias flag before start-up stage (1 min)
- A: alias flag on an already essential base (1 min)
- A: invulnerable: whether blows still stagger and count as assaults; whether magicka is spared (2 min)
- A: ForceActorValue on an invulnerable actor (1 min)
- A: ghost: melee blows pass or land; enemies target it (3 min)
- A: a base flagged ghost (ACBS 0x20000000) makes its actors ghosts (2 min)
- U: ghost activation / talking

### `quadratic-value-interpolation.md` (1 N)

- N: duplicate/unsorted keys

### `query-natives.md` (6 A)

- A: GetClass/GetRace of leveled templates: where SE's answers come from (1 min)
- A: GetEditorLocation for references other than created references and the player (1 min)
- A: GetCurrentLocation edge cases (inventory refs, dragons) (2 min)
- A: location counts: reported plugin/master double counting; enabled editor references vs actors wandering in (2 min)
- A: location alias fills and IsCleared looking inside child locations (2 min)
- A: IsLoaded parent inclusion (1 min)

### `quest-stages.md` (3 A, 1 A*)

- A: SetStage below the current stage: return value (1 min)
- A: lower stage marked done, its log entries run once; a done stage does nothing (2 min)
- A: setting the current stage, or a done stage above it, runs it again (2 min)
- A*: lower stage's script fragments run (needs a stage-fragment writer) (2 min)

### `quest-targets.md` (7 V)

- V: which targets shown
- V: where a target is
- V: route through load doors
- V: marker alpha/scale, merging
- V: enemy markers
- V: location aliases as targets
- V: world-map overlays

### `ragdolls.md` (12 A, 1 A*, 2 U, 1 R, 7 V)

- A: knock states 1–4 and 8: what sets them (3 min)
- A: recovery thresholds: 1 s down, 0.5 s at rest, 35 units/s, 2 rad/s (3 min)
- A: push velocity mapping (force × 35 units/s, +0.35 up, clamp ±128); negative force still lifts (2 min)
- A: coincident source and target push upward (+Y) (1 min)
- A: area effects: magnitude below 1 staggers, 1 or more knocks down; radius is a hard limit; caller excluded (2 min)
- A: refused pushes and area knockdowns stagger at full magnitude; 1.5 for ragdoll-immune creatures (2 min)
- A: furniture and mount refusal (2 min)
- A: combat targets and package runs kept through a knock (2 min)
- A: root motion of walking paused until recovery ends (2 min)
- A: fall damage per landing, rest-on-ledge re-hurt, water/ethereal/paralysis prevent it, delayed damage before recovery (3 min)
- A: ForceAdd / ForceRemove on living actors (3 min)
- A: living ragdoll cap of 16 NPCs; overflow staggers; closer actors first (5 min)
- A*: dismount-to-ragdoll from a mount or dragon fall (3 min)
- U: pickpocket opening while getting up (100% if unaware and out of combat)
- U: jumping during recovery and after it
- R: knocked actors across unload and save (saved pose restart, cell not yet loaded, dead on load)
- V: get-up type numbers 0 / 1 / 2 (ordinary, reanimate, swimming)
- V: impulse proportional to mass across bodies
- V: Allow Ragdoll Collision meaning
- V: control approximation (zero maxForce limp, gains, paralysis holds struck pose)
- V: matching, get-up blends and root alignment
- V: stagger magnitude clamps to 1.0 (blend graph)
- V: weapons, attacks and voices during a knock

### `reference-conditions.md` (4 A, 1 U, 1 N)

- A: GetDestroyed from DSTD flag (1 min)
- A: IsWeaponInList left hand (1 min)
- A: IsUnlockedDoor after a script unlock vs never locked (1 min)
- A: GetDefaultOpen on created refs (1 min)
- U: IsUnlockedDoor after key or pick vs never locked
- N: GetIsObjectType unlisted types

### `relationships.md` (3 A)

- A: HasParentRelationship with no actor given (2 min)
- A: secret relationships filtered from queries (1 min)
- A: any-family queries: whether dead relatives and template-based NPCs count (1 min)

### `remaining-natives.md` (51 A, 5 A*, 2 U, 3 R, 7 V)

- A: alias timers: multiple scripts, zero/NaN (1 min)
- A: ingredient hostility: learned effects only (1 min)
- A: cell attach/detach timing (2 min)
- A: GetKey with two different keys (1 min)
- A: HasCommonParent counting the inputs (1 min)
- A: HasRefType disabled references (1 min)
- A: GetAllItemsCount for quest and unplayable items (1 min)
- A: WornHasKeyword hidden-skin inclusion (1 min)
- A: GetCurrentPackage timing and interrupts (2 min)
- A: GetWidth/GetLength of actors (bounds) (1 min)
- A: GetScale of player / race change (1 min)
- A: GetWorldSpace unloaded travellers / inventory (2 min)
- A: GetDeadCount for placed starts-dead references never killed in play (2 min)
- A: GetGoldValue potions / lights (1 min)
- A: duplicate keyword editor IDs (2 min)
- A: NPC sprint (IsSprinting) (2 min)
- A: IsAlarmed for non-guard witnesses (2 min)
- A: Is3DLoaded for persistent references after detachment (5 min)
- A: leveled list at 255 entries; duplicates (1 min)
- A: SetDestroyed: what the flag shows and whether it blocks activation (2 min)
- A: SetScale on actor keeps height factors (1 min)
- A: CalculateEncounterLevel multipliers (1 min)
- A: IgnoreFriendlyHits tolerance (2 min)
- A: Resurrect of a placed starts-dead actor (2 min)
- A: UnequipAll on spells (1 min)
- A: IsHostileToActor for unloaded and dead actors (1 min)
- A: SetDoingFavor effects (2 min)
- A: ObjectReference.Reset: lock, destruction, open state, harvest (2 min)
- A: Cell.Reset resurrects / re-enables (3 min)
- A: finders on unloaded actors' positions; random finders and dead actors (5 min)
- A: GetSunPosition vector by hour; sungazing angle (2 min)
- A: CalculateFavorCost formula (1 min)
- A: PlayerKnows other learning (1 min)
- A: GameTimeToString date on a later save (2 min)
- A: SetINI* unknown names (1 min)
- A: SendAssaultAlarm (actor and faction): ASSU sent, which members report (2 min)
- A: SendStealAlarm: other thieves do nothing (2 min)
- A: FastTravel in combat (3 min)
- A: MoveToPackageLocation for a whole-cell location (2 min)
- A: WaitForAnimationEvent without the 10 s limit (3 min)
- A: AttachAshPile default pile (2 min)
- A: MoveToNode rest transform (2 min)
- A: IsAllowedToFly in some exteriors (2 min)
- A: StartSneaking on an NPC: package stance (3 min)
- A: SendTrespassAlarm: other criminals report (2 min)
- A: EndDeferredKill kills at once or at the next blow (2 min)
- A: SendVampirismStateChanged reaching other actors' scripts (2 min)
- A: CaptureFrameRate string format (1 min)
- A: SetGodMode: magicka, stamina, carry weight (1 min)
- A: ClearTempEffects: placed hazards (2 min)
- A: SendWereWolfTransformation: crime report (2 min)
- A*: GetDialogueTarget listeners (2 min)
- A*: IsTrespassing for NPCs (3 min)
- A*: SetActorCause credit for experience from hazards (3 min)
- A*: dependent animated objects (3 min)
- A*: conflicting INFO group membership across plugins (3 min)
- U: IsRunning for a sprinting player and the backward quirk
- U: IsArrestingTarget for a pursuing guard
- R: script loops survive save
- R: leveled list additions survive save
- R: sound category changes survive saves
- V: script sound follows moving source
- V: quest text globals copied when
- V: SetFrequency above 1
- V: SetInChargen message text
- V: SetAlpha fade length; objects and player not faded
- V: SetSittingRotation sign and seat view
- V: ShaderParticleGeometry falling indoors unsheltered

### `reverb.md` (2 V)

- V: REVB field mapping (delay scaling)
- V: reverb network tone/decay

### `rotate-character.md` (3 N)

- N: rotation step formula and axis convention
- N: composition with root motion / facing
- N: TurnDelta units

### `saves.md` (1 A, 1 A*, 9 R, 1 V)

- A: RequestSave / RequestAutoSave file names and rotation (1 min)
- A*: RequestSave refused while the player is dead (5 min)
- V: "Autosaving..." notice on RequestAutoSave
- R: weather on load
- R: story manager memory persistence
- R: continuity adjuncts (harvest, cleared, crime memory, controls, regen delays, camera...)
- R: further continuity policies
- R: active effects and tapers saved
- R: behaviour graph open states saved
- R: faction/ghost/outfit/AI-off/form-list sections
- R: form ids across a load-order change (save loads only with the same plugins in the same order)
- R: optional sections not in the bullets above: LOOK, TEAM, MAPS, PWCD, SOUL, MBXW/ANMW waits, FORM/COPY crafted forms, WSTA, AEXC

### `scenes.md` (4 A, 6 A*, 1 R, 1 V)

- A: Start/ForceStart take actors from other scenes: when they stop (2 min)
- A: Start fails when the scene's conditions fail; ForceStart ignores them (2 min)
- A: package action done rules (radius+150, furniture, greet) (3 min)
- A: Stop runs the end fragment; repeat while true (2 min)
- A*: topicless dialogue actions; reading time without voice (2 min)
- A*: speakers must be loaded: a line waits for its speaker (3 min)
- A*: procedures counted done after a second (shout, use magic, activate, hover, orbit, own trees) (3 min)
- A*: scene flag 0x4 and action LNAM (3 min)
- A*: observe-combat / death pause / interruptible / scene packages only (4 min)
- A*: looping dialogue repeats or re-picks (2 min)
- R: scene state in saves
- V: subtitle distance; barks quiet during scenes

### `scrolls.md` (2 A, 5 U, 1 V)

- A: GetEquippedSpell on a held scroll (1 min)
- A: Favorite Spell and Favorite School statistics don't count a held scroll (1 min)
- U: scroll charge time (automatic: greater of SPIT and longest effect); dual casting
- U: scroll XP (Blizzard gives none)
- U: unwind after an early ritual release (0.3 s retained; SE under 0.5 s)
- U: airborne refusal beyond jumping (falling and other airborne states)
- U: ritual scroll identification by a both-hand equip slot for each scroll; Telekinesis concentration channel
- V: scroll equip icon

### `selectors.md` (2 V, 2 N)

- V: whether selector switches cross-fade, and what interrupts or forwards triggers during a blend (9-18)
- V: behaviour when the selected index is invalid (16-18)
- N: selector bytes 0x59..0x5F (44-47)
- N: chooser start state (39-51)

### `sequence-material-controllers.md` (1 R, 4 V)

- R: whether a selected sequence's clock survives save and reload (80-82)
- V: sequence vs controller timing, priorities, weights, blending and transitions (72-78)
- V: multiple manager scopes with duplicate sequence names, and simultaneous sequence ownership (74-78)
- V: whether a clamped sequence's terminal material sample stays after it completes, and competing drivers (67-70)
- V: unlinked / external KF blocks, chains interrupted by unknown controllers, initially hidden shapes (83-86)

### `shouts.md` (2 A, 1 U, 1 V)

- A: shout cone reach and spread (3 min)
- A: Story Manager NVPE event trigger and members (2 min)
- U: when the shout goes out: on key release, or once held long enough (36-38)
- V: shout card (39-41)

### `skills-menu.md` (5 U, 11 V)

- U: level up popup timing (152-158)
- U: key and mouse bindings other than Space (159-168)
- U: Legendary confirmations (169-172)
- U: whether a press in the level up popup takes one level (155-158)
- U: whether taking a perk asks for confirmation (159-168)
- V: opening image (123-124)
- V: camera (125-129)
- V: skydome sequences (130-143)
- V: MoveCamera / StartCameraAnimation (144-150)
- V: perk requirement text (173-181)
- V: label colours (182-184)
- V: perk placement in the sky (185-199)
- V: stars and lines sequences (200-215)
- V: sounds (216-219)
- V: transition effect, beast trees, ring acceleration (221-244)
- V: when the lines' FadeOut / FadeIn and the stars' reset play (213-215)

### `skills.md` (4 A*, 5 U)

- A*: sneak attack shot and bash XP (5 min) (68-69)
- A*: block vs armor XP split, mixed armor (5 min) (70-72)
- A*: sneaking hidden XP conditions (5 min) (75-78)
- A*: Mod Skill Use applied before use mult / offset (5 min) (105-108)
- U: Legendary edge cases (57-66)
- U: pickpocket XP curve (73-74)
- U: blocked blows and power attacks train the same (79-80)
- U: training choices (81-96)
- U: which Rested spell a sleep selects (bed, owned bed, spouse) (116-117)

### `skin-tone.md` (2 V)

- V: tone blend formula (22-26)
- V: all-zero tones and whether the game draws those Dremora bodies black (27-31)

### `sleep-wait.md` (1 A, 8 U, 1 V)

- A: script-set owners for the player's checks (beds, trespass, locks, stealing prompts, Story Manager owner) (2 min) (67-70)
- U: opening wait and sleep (16-18)
- U: refusal reasons and their order (19-25)
- U: whether InCell places refuse waiting (23-25)
- U: hours pass: effects, regeneration, statistics (26-31)
- U: other actors placed per hour (32-41)
- U: actors' health over a wait (42-45)
- U: what interrupts a wait (46-51)
- U: sleep events (52-54)
- V: sleep fade timing (55-66)

### `sneak-attacks.md` (1 A, 6 U)

- A: NPCs sneak attacking the player (3 min) (29-30)
- U: crossbows get the bow's 2x (and Deadly Aim) (22-24)
- U: staves strike without a sneak bonus (25)
- U: the multiplier applies before armor (26-27)
- U: bashes are never sneak attacks (28)
- U: unused sneak settings: what they scale (31-33)
- U: backstab statistic (40-46)

### `soft-effects.md` (5 V)

- V: soft fade formula (26-29)
- V: which surfaces count as the scene behind (30-36)
- V: particles with a flagged shader (37-38)
- V: effects where the sky is behind them (39-40)
- V: effects without a depth test (41-42)

### `soul-trap.md` (7 A, 1 R, 1 V, 1 N)

- A: black souls from Dremora and NoSoulTrap actors (21-25) (2 min)
- A: white souls in black gems (26-27) (2 min)
- A: filling common gems' linked filled forms, and gems that keep the soul (28-31) (2 min)
- A: souls kept through script transfers (45-52) (2 min)
- A: dropping filled gems without linked forms (54-57) (2 min)
- A: whether a wolf's TrapSoul gives a soul at all (21-25) (2 min)
- A: a second TrapSoul from a body already trapped (14-19) (2 min)
- R: whether a spent body stays spent across save and reload, and after Resurrect (14-19)
- V: "No soul gem large enough." message (32-34)
- N: mixed owners' soul gems (58-62)

### `sound-output-models.md` (4 V)

- V: the curve is a gain; interpolation and the first point within the minimum distance (27-31)
- V: dialogue output model choice (DOP2 / DOP3) (32-38)
- V: speaker layouts and channel assignment (39-46)
- V: HRTF approximation for rear sounds (52-57)

### `sound-variation.md` (4 V)

- V: frequency shift and variance units (16-19)
- V: variance distribution (symmetric or one-sided) (20-24)
- V: which sounds vary (25-29)
- V: mixer gain smoothing (33-38)

### `speech-checks.md` (7 A, 1 U, 2 V)

- A: GetBribeAmount across NPC level, Speech, morality and player level; settles fBribeMoralityMult, fBribeNPCLevelMult and the formula's grouping (5-16) (3 min)
- A: WillIntimidateSucceed across NPC confidence, level and Speech; settles fIntimidateSpeechcraftCurve and the five fIntimidateConfidenceMult settings (5-10) (3 min)
- A: Mod Bribe Amount perk entry point on GetBribeAmount (14-16) (2 min)
- A: GetBribeSuccess means the player carries at least the bribe (17-19) (1 min)
- A: GetIntimidateSuccess equals WillIntimidateSucceed (20) (1 min)
- A: whether bribed and intimidated flags wear off with time (21-23) (2 min)
- A: whether Activate on an actor without PC dialogue (or in bleedout without AllowBleedoutDialogue) starts a conversation (24-27) (2 min)
- U: whether bribed or intimidated changes the NPC's behaviour otherwise (21-23)
- V: the "Talk" prompt for actors without PC dialogue (24-27)
- V: whether `<CrimeGold>` shows violent and non-violent bounty together (28-29)

### `spell-visuals.md` (1 R, 9 V)

- V: casting art selection (20-22)
- V: graph events and hand variables (23-29)
- V: attachment fallback (30-34)
- V: quiet and invisible targets (35-39)
- V: hit art refresh and ending (40-62)
- V: flame, beam and cone visuals (63-81)
- V: dual scale and enchantment art (82-87)
- V: sequence generator blend modes (88-97)
- V: RFCT transforms (98-100)
- R: whether script-played indefinite shaders survive SE saves (101-103)

### `staffs.md` (9 A, 2 A*, 3 U)

- A: staff charge cost formula, for NPC staff users (16-44; 105-111) (4 min)
- A: NPCs' worn school reductions lower their charge use (41-44) (3 min)
- A: empty hand beside a staff reads 8, 9 or 0 (spells read 0 in autotest21) (76-79) (1 min)
- A: NPCs taking up staffs (style multiplier) (96-99) (3 min)
- A: NPCs fighting with staffs rather than spells (100-104) (3 min)
- A: charge kept through drop and pickup (115-138) (2 min)
- A: enchanted bows pay per arrow that strikes (148-151) (3 min)
- A: which depleted copies a partial drop takes (132-134) (2 min)
- A: NPC enchanted blows' charge cost, by the attacker's skill (148-162) (3 min)
- A*: ModSpellCost entry point on enchantments (35-40; 45-57) (3 min)
- A*: which copy's charge is used and kept (62-74) (2 min)
- U: insufficient charge behaviour (58-61)
- U: magicka regenerates while casting staffs (75)
- U: recharging list, overflow, reusable gems and meter (164-183)

### `state-machine-events.md` (1 A*, 3 N)

- N: transition notification boundaries and ordering: no vanilla machine sets one (29-37)
- N: FLAG_DELAY_STATE_CHANGE (39-51)
- A*: self-transition modes (CONTINUE / RESET / BLEND) (70-76) (3 min)
- N: event modes, PROCESS_ALL (77-79)

### `state-machine-outputs.md` (1 A, 2 N)

- A: whether a script or expression write to i1HMState survives frames without a state change, and what happens during a blend (49-51) (2 min)
- N: publishing before the new state's child activates (44-46)
- N: the ID published when an invalid start ID falls back to the first state (47-48)

### `state-machine-start-properties.md` (1 N)

- N: bound start mode and sync index sampling, invalid and duplicate binding policy, sync output ordering, enum and integer conversions (54-59)

### `status-conditions.md` (31 A, 4 A*, 6 U)

- A: GetIsPlayableRace: template race; non-actors fail (33-39) (1 min)
- A: GetKeywordItemCount: two different keyworded forms, and starting-inventory items (53-56) (1 min)
- A: HasTwoHandedWeaponEquipped for bows, crossbows, staves and sheathed two-handers; HasBoundWeaponEquipped by hand (75-83) (1 min)
- A: IsOwner / IsCellOwner: subject or caller reading (104-105) (1 min)
- A: omitted owner names the player (110-111) (1 min)
- A: owner faction matching (exact owner, not member) (112-113) (1 min)
- A: IsInMyOwnedCell: direct, by base, or by faction; the player's exteriors (114-117) (1 min)
- A: GetXPForNextLevel: what the value is, by a finer scan (132-135) (1 min)
- A: GetInCurrentLocFormList: whether list entries' parents count (174-176) (1 min)
- A: IsLocationLoaded: what counts as loaded (177-180) (1 min)
- A: null reference in HasSameEditorLocAsRef / IsInSameCurrentLocAsRef (181-183) (1 min)
- A: GetFactionCombatReaction: a faction listing itself as an ally (203-206) (1 min)
- A: GetFactionRelation: meaning of the strongest-reaction value (207-209, 345-349) (2 min)
- A: location ref-type counts: the dead rule and empty alias (232-236) (2 min)
- A: GetKeywordDataForAlias (237-238) (1 min)
- A: player regions from XCLR (267-270) (1 min)
- A: GetInZone (271-272) (1 min)
- A: GetPlayerControlsDisabled: the fighting parameter (277-283) (1 min)
- A: GetSecondsPassed (287-289) (1 min)
- A: GetCurrentAIPackage / GetCurrentAIProcedure values (297-299) (2 min)
- A: GetScale of a placed reference's XSCL with no actor loaded (339-340) (1 min)
- A: GetIsCreature for other creature kinds (341-344) (1 min)
- A: GetFactionRankDifference with both members, and whether it is always 0 (350-353) (1 min)
- A: GetWeaponAnimType: left-hand weapon alone, none, spell in hand (354-356) (1 min)
- A: IsCarryable: quest items and weight limits (357-360) (1 min)
- A: GetTargetHeight sign (361-362) (1 min)
- A: GetClothingValue: value of worn clothing (366-369) (1 min)
- A: GetDisease on the diseased player (370-375) (1 min)
- A: IsActorUsingATorch with weapons drawn (376-379) (1 min)
- A: GetInCellParam: named reference or subject (380-382) (1 min)
- A: IsRunning for NPCs: graph speed or movement input (409-412) (2 min)
- A*: IsPC1stPerson in vanity, horse and furniture cameras (154-157) (2 min)
- A*: IsNullPackageData (290-294) (2 min)
- A*: GetActivatorHeight (363-365) (2 min)
- A*: IsRidingMount (419-429) (2 min)
- U: IsPCSleeping: when it starts and ends (149-153)
- U: MenuMode parameter (273-276)
- U: IsTimePassing while waiting, and dialogue and scene states (284-286)
- U: IsWaiting (300-302)
- U: GetCannibal (383)
- U: IsRunning for the player (409-412)

### `story-cast-events.md` (1 A, 1 A*, 2 U)

- A: whether NPC and script casts send CAST (2 min) (44-47)
- A*: CAST suppressed in combat, and whose combat state matters (2 min) (48-51)
- U: when concentration casts send CAST (52-56)
- U: whether staffs, scrolls, powers and shouts send CAST (57-60)

### `story-manager.md` (11 A, 5 A*, 2 U)

- A: ADIA conversation cadence, chance and range (10 min) (108-111)
- A: reset hours for quests started otherwise (2 min) (112-113)
- A: quests' own event conditions, and whom they run on (2 min) (116, 178-179)
- A: assault event members (2 min) (121-130)
- A: an NPC's kill's crime status when it fights the victim, is its enemy or the player's teammate (2 min) (131-139)
- A: finding bodies rule (3 min) (134-135)
- A: OnStory event before the startup stage (1 min) (144-149)
- A: skill and level increase event values (1 min) (144-149)
- A: alias scripts' timing for quests started from Papyrus (2 min) (150-153)
- A: REMP owner and item members (2 min) (183-187)
- A: max concurrent counting the running quests below a node (2 min) (114-115)
- A*: TRES event members and V1 (4 min) (100-107)
- A*: hello event timing (passing greeting) (3 min) (119-120)
- A*: crime events' members, arrest event and ADCR (3 min) (136-143)
- A*: faction favor cap for stealing (3 min) (159-168)
- A*: generic faction member counts as dead by its first reference (2 min) (176-177)
- U: LOCK event members and failed pick (100-107)
- U: selling sends no event; given = 5 (188-192)

### `swf-hud.md` (4 U, 20 V)

- U: whether a message box can be cancelled (Esc or another key); the movie's `setIsCancellable` is never called
- U: opening and reopening the menus; list positions restored on the next opening
- U: keys: each control-map key's action in the menus (Accept, Ready Weapon, Toggle POV, attack buttons, Tab, Esc)
- U: which keys reach the menus while a message box or item menu is up
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
- V: Message boxes (text, line breaks, button layout).
- V: Answers.
- V: Categories.
- V: Entries.
- V: The item card
- V: The bottom bar
- V: Both sides in one list.
- V: Containers
- V: Bartering
- V: The card

### `system-page.md` (6 U, 6 V)

- U: difficulty on spell damage by attacker (no vanilla native sets the difficulty)
- U: save on pause interval and the journal/tween opening (needs the player)
- U: autosave rotation: how many autosave slots the game keeps
- U: look sensitivity scale (slider mapping)
- U: difficulty on weapon hits and bashes (needs a swing)
- U: remapping conflicts: swap or refusal behaviour after `FinishRemapMode`
- V: save row numbering
- V: missing preference default (fSpecularLODStartFade)
- V: brightness mapping
- V: distance slider mappings
- V: tree billboard presentation
- V: how far ordinary geometry fades before culling (last 20 percent, screen-door)

### `tbc-value-interpolation.md` (2 N)

- N: bias/continuity order
- N: zero time spans and nonfinite TBC parameters (no malformed-input parity established)

### `telekinesis.md` (1 A, 2 A*, 1 V)

- A: NPC/enchantment telekinesis (2 min)
- A*: reach formula and the minimum distance (how near the held object is kept) (2 min)
- A*: heaviest lifted (2 min)
- V: no-recast message

### `texture-flip-controllers.md` (2 V)

- V: flip controller frame mapping
- V: whether `NiFlipController` applies to Skyrim shader properties at all

### `timer-properties.md` (2 N)

- N: deadline sampling time
- N: disabled-time / reactivation semantics

### `transition-event-mode-properties.md` (3 N)

- N: event mode sampling and routing
- N: default-mode semantics and invalid/duplicate binding policy
- N: `IGNORE_TO_GENERATOR` routing (intended source-only, approximated as destination-only)

### `transition-self-mode-properties.md` (2 N)

- N: self mode sampling time
- N: reset / blend / cyclic-check algorithms' original-engine parity, and invalid, conflicting or bit-specific bindings

### `translation.md` (6 A, 1 R)

- A: an undrawn reference (marker, water) or a loaded actor moves on `TranslateTo`; an unloaded one gets `OnTranslationFailed` at once (2 min)
- A: `OnTranslationFailed` when its cell unloads mid-way (5 min)
- A: `StopTranslation` leaves the reference where it is and sends no event (2 min)
- A: the rotation turns the short way round, capped per axis (2 min)
- A: the spline's Hermite tangents (start and end facings times the magnitude) and speed kept along the length (3 min)
- A: when `OnTranslationAlmostComplete` fires: 90% of the way, the CK wiki's default, never checked in the game (2 min)
- R: a save or load mid-way leaves the reference where it was, without events

### `traps.md` (2 A, 3 A*, 1 U)

- A: reference VMAD inherits base properties (1 min)
- A: trap kills belong to no one (2 min)
- A*: touch margin; sleeping bodies don't hit (3 min)
- A*: which objects are traps; event arguments (5 min)
- A*: graph-moved parts hit like keyframed traps (3 min)
- U: held objects don't hit the player

### `triggers.md` (6 A, 2 A*, 1 V)

- A: phantom volumes reach 20 units as boxes do (2 min)
- A: who sets triggers off (projectiles?) (2 min)
- A: actors inside on cell load enter on first update (2 min)
- A: script-moved trigger keeps its volume (1 min)
- A: OnTrigger interval and first event (2 min)
- A: activate parent: children get the parent as activator; a parent that blocks activation passes nothing on (2 min)
- A*: placed hazards: radius units, scale, interval (4 min)
- A*: trigger collision layers (which objects each reacts to) (4 min)
- V: whether a placed hazard draws its own model (`FXFire01New.nif`) besides the placed fires

### `tween-menu.md` (3 U, 3 V)

- U: choosing (Skills, Magic, Items) and the menus that open from it
- U: coming back from the inventory or magic menu (`ShowTweenMenu`, Left, clicking outside, Right, Tab)
- U: closing with Tab or Esc
- V: opening animation
- V: bottom bar
- V: sounds

### `twist-modifier.md` (2 V, 2 N)

- V: angle units
- V: chain distribution / RAMPED
- N: other bindings
- N: pose order

### `unloaded-placement.md` (10 A, 2 U)

- A: upstairs over the target (5 min)
- A: stop distance formula for other radii (5 min)
- A: other procedures (Sleep, Eat, UseWeapon) placement (5 min)
- A: coming from another cell/worldspace (5 min)
- A: whole-cell sandbox marker choice (5 min)
- A: which overlapping actor moves apart (3 min)
- A: 30/40/50 short distribution (5 min)
- A: Travel heading threshold (3 min)
- A: a long absence at furniture: where a Sit actor is after it (autotest16's Sitter stood) (5 min)
- A: a Travel actor's gradual turn after load: rate (2 min)
- U: waiting/sleeping moves loaded actors
- U: new game puts actors on target at once

### `use-magic.md` (7 A*)

- A*: cast time and cooldown meaning (4 min)
- A*: number to cast; per-package counts (4 min)
- A*: hold when blocked (3 min)
- A*: package dual cast and its cost (3 min)
- A*: hands drawn/put away; magicka during package (2 min)
- A*: hostile spell at a practice target aimed at its bounds centre (3 min)
- A*: finite cast count while away: completion timing, count continuity across unloading, magicka (5 min)

### `use-weapon.md` (8 A, 4 A*)

- A: unresolvable triggers; distance from origins (3 min)
- A: Always Power Attack selection and exhaustion (4 min)
- A: finite package completion (porch bowmen) (10 min)
- A: completed package passed over vs resumed (4 min)
- A: random power attack share (6 min)
- A: distance gate boundary, cadence, resumption (4 min)
- A: MultiTarget target selection lifetime (4 min)
- A: finite barrage count across unloading and re-entry (5 min)
- A*: count 0 with Never End off; time limit start (4 min)
- A*: Hold When Blocked, Always Hit, damage/combat flags (4 min)
- A*: missing template inputs: an absent trigger imposes no gate, a missing radius is 0 (4 min)
- A*: bow timings while away (draw, hold, between-shot delay) (5 min)

### `value-key-interpolation.md` (1 N)

- N: midpoint ties, repeated timestamps

### `voices.md` (3 V)

- V: voice cap and priority direction
- V: weather beds in the budget
- V: virtualization and fade when a voice is dropped or returns

### `wards.md` (11 A, 3 A*, 1 U, 1 N)

- A: ward build rate and maximum (NPC warders) (3 min)
- A: spell strength against a ward (3 min)
- A: breaking: spell stopped, graph event (2 min)
- A: OnHit for warded spells (1 min)
- A: gear wards add, not worn down (3 min)
- A: Ward Absorb magicka (2 min)
- A: channelled effects linger (2 min)
- A: NPCs: when they raise wards (4 min)
- A: NPCs: ward priority vs heals/spells (3 min)
- A: NPCs: keep time and gap; IsWardState hold (4 min)
- A: combat styles weigh warding (4 min)
- A*: ward arc (4 min)
- A*: what wards stop (3 min)
- A*: ward Restoration XP (4 min)
- U: dual-cast wards start full
- N: WardDeflection actor value

### `water.md` (8 A, 1 A*, 2 V, 2 N)

- A: immersion threshold, unloaded/dead actors (3 min)
- A: swim entry depth (chosen: over 100) and whether Waterbreathing changes swimming (autotest20, deferred by user) (4 min)
- A: Swims/Walks race flags and swimming (3 min)
- A: breath capacity and drowning damage (4 min)
- A: creature breath / float heights (4 min)
- A: swim speeds: humanoid NPCSwimming walk/run, and creatures' authored swim speeds (3 min)
- A: the point that counts as the head under water: 90% of skeleton height for NPCs, the eye for the player (3 min)
- A: whether a swimming fighter draws or attacks (3 min)
- A*: XNAM contact spell (3 min)
- V: submerged sound: fade time (150 ms ramps) and whether the underwater REVB applies
- V: which swim blend input the graph drives (`Direction` vs `SwimDirection`)
- N: water surface cell edges
- N: overlapping water planes

### `weather.md` (6 A, 1 A*, 8 V, 1 N)

- A: transition speed law: whether a constant term (about 1.3 s) is real or the probes' polling (5 min)
- A: FindWeather: why cloudy (Fog's class) found none in Riverwood (2 min)
- A: how often the weather changes; volatility (15 min)
- A*: RDWT global chances add or replace (5 min)
- N: region priority / override flag
- A: leaving a region: new weather transition (3 min)
- A: a game started indoors: GetCurrentWeather is None until it goes out (2 min)
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
