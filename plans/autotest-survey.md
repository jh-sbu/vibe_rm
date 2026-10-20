# Which ambiguities an autotest could settle

A survey (2026-10-13) of every open item in `known_ambiguities/` for a test
mod whose script settles it unattended (see `crates/tools/src/test_mods/`):
the user loads a save with the plugin and leaves the game alone; the script
calls only vanilla Papyrus natives (no SKSE), keeps each report in a
`Result<Name>` variable and asks the game to save, and `vrm-tool
test-results` reads the reports from that save. Menus stop the script
(`Utility.Wait` doesn't advance in them), so anything that opens one
(barter, gifts, sleep / wait, dialogue, books, force greets, arrest) can't
run unattended.

Verdicts: **autotest** (a script settles it), **partial** (a script settles
part of it), **manual** (a test mod the user plays and watches), **no**.
Natives were checked against the game's scripts with `vrm-tool pex-dump`
(`Game.SetPlayerLevel` doesn't exist: the player's level can't be raised by
script). Settled items move out of this list as their tests report.

## Where it stands (2026-10-14)

- **Tier 1**: done (`autotest2`, `autotest3`).
- **Tier 2**: mostly done (`autotest4`, `autotest5`); the leftovers below
  are the next autotest's.
- **Tiers 3 and 4**: not started.
- **Manual only** and **not settleable**: unchanged; no autotest planned.

## Tier 1: cheap, record types test mods already write, no travel (done)

Built as `test-mod autotest2`; its two runs and `test-mod autotest3`'s
follow-up (2026-10-13) settled all of it, the answers in the files named:

- Weather (`weather.md`): transitions in game time (about 7.6 s at delta
  125, TimeScale 60) against real time (22.8 s); acceleration does nothing,
  with an override or without.
- Settings' values read from the running game (`fSneakLight*`,
  `iSoundLevel*`, flee and confidence settings...).
- NPC level cap and rounding at level 1 (`skills.md`).
- Hands (`hands.md`, `magic-menu.md`); player graph variables
  (`player-body.md`).
- Crime money and witnesses (`crime.md`); NPCs taking up better gear.
- Cleared (`cleared.md`); relationships and quests started by script
  (`story-manager.md`); map marker flags (`map-markers.md`); templates
  (`npc-templates.md`); animation events (`object-graphs.md`).
- Combat events (`combat-state.md`: weapons' `OnHit` carry no projectile);
  light level (`detection.md`); statistics (`journal.md`).

## Tier 2: needs record types not written yet

Built as `test-mod autotest4` (one plugin, as asked), with `autotest3`'s
Firebolt follow-up folded in. Its run (2026-10-13) left several tests
unsettled (the tests, not the game, at fault: see `autotest4`'s module
doc); `test-mod autotest5` rebuilt them, with `taper`'s open ends, and its
run (2026-10-13) settled all but three (`autotest5`'s module doc).

### Settled

- **Story manager** (`story-manager.md`): node conditions run on the
  player; members' order; reset hours; max running;
  `SendStoryEventAndWait` returns before the quest's handler is done.
- **Magic** (`magic.md`, `actor-values.md`, `casting.md`): the
  `GetActorValuePercent` condition's scale (0..1); resistance, the player's
  85 cap, Ignore Resistance; calm / fear / frenzy / rally values and level
  cap; conditions' subject; absorb health; bound weapons; area radius (those
  behind a body spared); magnitude perks (Augmented Flames scales the taper
  too); dual effects' second values take the taper.
- **Perks** (`perks.md`): priority (highest first); quest sections
  (`SetStage` starts a quest not running); ability sections.
- **Skills** (`skills.md`): a level costs the current level's XP, the rest
  carries over, `AdvanceSkill`'s value is a use's base XP.
- **Shouts** (`shouts.md`): word and shout statistics; `HasShout` sees
  `AddShout`.
- **Alias packages** (`alias-packages.md`): the second quest's alias wins.
- **Alias fills** (`alias-fills.md`): forced / unique aliases on the dead
  or disabled stay empty; create level makes no difference to a PC level
  mult NPC; without Closest the first placed of two.
- **Combat** (`combat-assistance.md`, `combat-fleeing.md`): assistance
  radius (within 2500, not 5000), ally and friend; a faction friend's
  script-cast bolts start no fight; NPCs don't heal (`HealRate` 0); a coward
  flees at once.
- **Triggers** (`triggers.md`): the 20 unit margin, what fires it, disabled
  triggers.
- **Scenes** (`scenes.md`): Start / ForceStart rules, completion conditions
  end a phase at once, failing start conditions, death and stop-quest flags.
- **NPC dual wielding and torches** (`hands.md`): the combat style flag
  decides; a two-handed weapon is put away for a torch.

### Still to do

- **HasParentRelationship** (`relationships.md`): a quest condition on
  `HasParentRelationship` evaluated against NPC pairs linked by vanilla
  Parent / Child, in-law, grandparent and boss / employee RELA records, in
  both directions, logged by script: which end counts and which ASTPs.

- **Remove item event's members** (`story-manager.md`): a test quest of ours under
  the game's REMP node (`OnStoryRemoveFromPlayer`) can log R1 / R2 / V1 after the script
  calls `DropObject`, `RemoveItem` into a container or an NPC: does R1 come out the player, and does
  removing to an NPC or a container send 5 / 6, selling nothing?

Tried and not settled:

- **Magic conditions' recheck interval** (`magic.md`): needs a lasting
  effect (not No Duration, unlike `autotest5`'s frost) on a value it
  doesn't change itself, the condition toggled under it.
- **Absorb magicka's gain** (`magic.md`): the caster got nothing.
- **Traps** (`traps.md`): when the trap events fire and their arguments.
  Our scripted bowl came to rest beside the NPC with no event; the game's
  rock given `SetMotionType` dynamic didn't fall; a copy of the rock crashes
  the game. Needs another way to make a trap move (activating the game's
  trap trigger?).

- **Script-set animation variables vs the engine's writes**
  (`object-graphs.md`): `SetAnimationVariableFloat("Speed", 0)` on a walking
  NPC (or `Direction` / `IsBlocking`), then `GetAnimationVariableFloat` a few
  frames later: does the script's value stick or does the engine's own write
  replace it?

- **Blending effects' modes** (`state-machine-events.md`): a script can't
  send graph events to an NPC's state machine and read which clip plays, but
  `SendAnimationEvent` of an event with a self-transition (a re-entered
  weapon-state wildcard) then `GetAnimationVariable*` / timing of the
  following animation would show whether the state restarted.

Left out so far:

- **Summons** (`magic.md`): placement, limit, follow distances (no way to
  the summoned actor's reference without an effect script).
- **Perk ranks** (`perks.md`).
- **Alias fills** (`alias-fills.md`): near alias reach (`ALNT`); create
  level on a leveled actor's pick; whether a match without Closest searches
  beyond persistent references.
- **Alias packages** (`alias-packages.md`): "filled last" against started
  last against the lower alias id (they went together in the run).
- **Story manager** (`story-manager.md`): quests' own event conditions'
  subject; item pickup (AIPL) owner and stolen rule; `XRNK` (needs a REFR
  override).
- **Scenes** (`scenes.md`): flag 0x4, looping dialogue (dialogue records),
  observe-combat and interruptible flags.
- **Combat**: friendly hits by blows rather than script-cast spells
  (`combat-assistance.md`); assistance chains; the flee check's timing and
  ending a flee (`combat-fleeing.md`, with tier 3's statistics).
- **Torches** (`hands.md`): does an NPC that put its weapon away for a
  torch take it back to fight?
- **Triggers** (`triggers.md`): collision layers (which objects each kind
  reacts to); `OnTrigger`'s interval and whether one comes on the entering
  frame (an NPC standing in a scripted box, the script logging
  `Utility.GetCurrentRealTime` at each event).
- The "do all before repeating" Story Manager node flags.
- **UseWeapon completion** (`use-weapon.md`): the vanilla finite packages
  (the Dragonsreach porch bowmen, `WhiterunDragonsreachPorchUseBow8x4`: Never
  End off, 3 barrages of 3 to 8 attacks) can be watched unattended: a script on
  the actor logging `GetCurrentPackage` and `Utility.GetCurrentRealTime` each
  second (and `OnAnimationEvent` for `attackRelease` / `attackStart` if
  registered) shows whether the package ends right after the third barrage's
  last attack or after its pause, what the actor then runs, and whether the
  package returns only after its schedule window reopens. Count of attacks per
  barrage and the share of power attacks among unforced swings (melee training
  packages with Always Power Attack off, e.g. `GuardSolitudeMeleeTrainingPackage`)
  come from the same event log.
- **UseMagic counts and timing** (`use-magic.md`): a test NPC of ours given a
  copy of a `UseMagic` package (a test mod writes the `PACK` from the template
  `000504F5`) with a self-cast fire-and-forget spell, cast counts 3 and 3,
  cooldown 5 and 5; its script logs `OnSpellCast` (registered for the NPC's
  own events) and `GetCurrentPackage` with `Utility.GetCurrentRealTime` each
  second. That settles whether the package ends after the last cast, how the
  cooldown counts, and, with min 1 and max 4 across several NPCs, whether the
  count is picked at random per run. The same with a concentration spell shows
  what the cast time holds (the log of `OnSpellCast`/`OnMagicEffectApply`
  intervals), and a 3-casts package with the target behind a placed wall
  whether "Hold When Blocked" waits.
- **NPC stats** (`actor-values.md`): `GetBaseActorValue` health of a
  non-auto-calc NPC with a health offset (a placed copy of Khayla: 155 or
  UESP's 50?); Sneak of a deer (15 + offset 70); a "PC level mult" copy of
  `EncDremoraMelee06` with calc min 46 (skills past 100 at runtime: as its
  `DNAM`, or held at 100?).
- **Settings to read** (`GetGameSettingFloat / Int`, values the plugins
  lack): `iSoundLevelNormal`, `Loud`, `VeryLoud` (`detection.md`: weapons
  stay silent without them; `autotest2` read `iSoundLevelLoud` but the
  value wasn't kept), `fCombatDetectionFleeingLostRemoveTime`,
  `fCombatLowFleeingTargetHitPercent` and the `fCombatDialogueFlee*`
  timers the roadmap's combat items wait on.
- **Cloaks** (`magic.md`): Flame Cloak cast on the player (`Spell.Cast`),
  NPCs held (`SetDontMove`) at 150, 200 and 250 units, one fighting the
  player and one not: health each half second (the reach, the rate, who
  it spares).
- **Soul trap** (`soul-trap.md`): `Kill` then `TrapSoul` from the player
  on a placed wolf and a bandit, the player holding chosen gems; reading
  `GetItemCount` of the empty and filled forms after: which gem a white
  soul takes beside a black one, whether a grand soul into an empty grand
  gem becomes `SoulGemGrandFilled`, and `TrapSoul`'s result for a black
  soul with no black gem.
- **Placed hazards** (`triggers.md`): a test cell with a placed
  `FireLgPlacedHazard` (scale 1, and a copy at 2): the player moved
  (`MoveTo` with offsets) 40, 60, 80 and 120 units from it, held, health
  read after 2 s at each: the reach (feet? scaled?) and the rate. Needs a
  `PHZD` record writer.
- **Wards** (`wards.md`), partial: the player holds Lesser Ward (the
  script can't cast it held), told to by a message box answered before the
  script goes on; the script reads `GetActorValue("WardPower")` every 0.1 s
  with `Utility.GetCurrentRealTime` for 3 s (how fast it builds, to what:
  40, 52?). Then a held NPC facing the player casts (`Spell.Cast` from it)
  Flames-strength and Fireball-strength test spells at them: `WardPower`
  and the player's health after each, `OnWardHit` statuses (an alias
  script), magicka after with Ward Absorb added (`AddPerk`), and the same
  from an NPC placed behind the player (the arc). Once dual cast (the
  perk added: does it start whole?). NPCs' wards: a Vigilant of Stendarr
  (`EncVigilantOfStendarr01NordM04`: Lesser Ward in the left hand) and a
  fire warlock placed apart and set on each other (`StartCombat`), the
  Vigilant's `WardPower` and the warlock's magicka (its drops mark its
  casts; `IsCasting` is a condition, not a native) read every 0.1 s for
  20 s against `GetCurrentRealTime`: how soon a ward comes up after a cast
  starts, how long it outlasts it, and the gap before the next
  (`wards.md`'s keep and wait). Then the Vigilant set on an NPC with only
  a sword: does it ward at all with no spells coming?
- **Staffs** (`staffs.md`), partial: staffs are cast with the attack
  button, so the user casts a Staff of Firebolts once by hand when a
  notification asks; the script reads `GetActorValue("RightItemCharge")`
  before and after, at Destruction 15 and at 100 (`SetActorValue`), with
  and without Novice / Apprentice Destruction (`AddPerk`): the cost and
  what lowers it. Then `RightItemCharge` after the staff is dropped and
  picked up again (charge carried?). Weapons: an NPC held with an Iron
  Sword of Burning set on a held target (`StartCombat`), its
  `RightItemCharge` read after each `OnHit` the target's alias sees
  (spent per blow? for blocked blows? do NPCs' weapons drain at all?);
  the player's charge after `RemoveItem` into a container and back
  (`staffs.md`'s enchanted weapons). Recharging is the inventory's (a
  menu): manual, the user charging a half-spent sword with a petty soul
  and then a grand one, the script reading `RightItemCharge`, the gems'
  `GetItemCount` and Enchanting's `GetBaseActorValue` before and after
  (XP itself isn't readable without SKSE).
- **Ethereal** (`magic.md`): `VoiceBecomeEthereal3` cast by script on
  a held NPC, then a Firebolt cast at it and `DamageActorValue` health on
  it: health after each (magic spared, scripts not?), and whether the
  Firebolt's `OnHit` reaches its script.
- **Poisons** (`poisons.md`), partial: `EquipItem` of a poison on the
  player holding an Iron Sword, and on an NPC holding one: `GetItemCount`
  of the poison after (used, kept, drunk: `HasMagicEffect` of its
  effect on the one equipping it). Last in a run: if it asks the player,
  the message box stops the script. Should the NPC's weapon take it, that
  NPC fighting a held target would show whether a blocked blow uses the
  dose (the target's health over its poison's duration).
- **Slow Time** (`magic.md`): `VoiceSlowTime1` cast by script on the
  player, with `Utility.GetCurrentRealTime` and `Game.GetRealHoursPassed`
  read at the start and when `HasMagicEffect` of its effect turns false
  (the duration in real seconds: 8, or 26 if scaled), and a held NPC with
  `Utility.Wait` marks (waits in slowed or real time?). A dropped object
  (`PlaceAtMe` a book, `SetMotionType` dynamic) timed against it falling
  with and without the effect; an NPC walking a `TravelToRadius` leg timed
  likewise; the player's walk can't be scripted unattended.
- **Telekinesis** (`telekinesis.md`): objects of increasing weight
  (`PlaceAtMe` an iron ingot, a sword, an anvil-weight furniture piece, a
  barrel) in the player's line of sight at 500 / 1500 / 2500 units
  (`MoveTo` with offsets, the player held still), `Spell.Cast` of
  `Telekinesis` on the player with `Game.GetPlayerGrabbedRef` after a
  `Utility.Wait` (does a script cast grab; the reach; the heaviest lifted),
  then the same with an object already grabbed (the no-recast message
  can't be read, the held ref can). Crosshair aim may need the player
  turned with `SetAngle`.
- **Leveled spell lists** (`npc-templates.md`): test NPCs placed by
  `PlaceAtMe`, each naming a test `LVSP` in its spell list: entries at
  levels 1, 10 and 30 (no "all levels") on a level-30 NPC with the player
  below 10 (`HasSpell` of each: the actor's level or the player's); twenty
  of one with three level-1 entries (each pick's share, and whether
  `Disable` / `Enable` or a second `PlaceAtMe` of the same reference's base
  keeps a pick); twenty with 50% chance of none; `HasSpell` of the list's
  own form. Needs an `LVSP` writer.
- **Actors' keywords** (`npc-templates.md`): `HasKeyword` on an NPC wearing
  armor with `ArmorHeavy`, and under a spell whose effect has a keyword
  its race lacks.
- **Runtime faction changes** (`crime.md`): a test NPC outside a faction;
  `GetFactionRank` after `AddToFaction` (0?), after `AddToFaction` on one
  already at rank 2 (kept?), `SetFactionRank` and `ModFactionRank` on one
  that is out (joins? stays out?), after `RemoveFromFaction` (-1), and
  `IsInFaction` after each. Plain vanilla natives on a placed NPC.
- **Impact hazards** (`impacts.md`): whether a hazard an impact's `NAM2`
  leaves is observable by script (an arrow's impact at a known place, then
  `Game.FindClosestReferenceOfType` for the `HAZD` base within its radius)
  and how long it lasts against its record's lifetime.
- **Package locations** (`package-trees.md`), `GetWithinPackageLocation`:
  an NPC held (`SetDontMove`) in the test cell with two packages, the first
  conditioned on `GetWithinPackageLocation` (its own condition, as the
  Dawnguard disguised vampire package has it), the second a fallback; after
  each step `EvaluatePackage` and `GetCurrentPackage` say whether it passed.
  Run on a probe reference (`run_on` 2) with a "near reference" location of
  radius 200: the probe moved (`MoveTo` with offsets) 190 and 210 units
  away horizontally, then 150 across and 150 up (212 in 3D, 150 flat):
  3D or flat, and whether the edge counts. Run on the NPC with a "near
  package start" (kind 2) location of radius 200: the package started, the
  NPC moved 400 units, re-evaluated: still within (its current position) or
  not (where the package started), and again after the fallback has run in
  between (whether the start resets). Needs a `PACK` writer (own conditions,
  `PKCU` to a vanilla template such as `TravelToRadius`, `PLDT` inputs). An
  exterior "in cell" (the grid square or not) needs travel: tier 4.

- **Pick lock and trespass Story Manager events** (`story-manager.md`):
  no vanilla node listens, so a Story Manager event node of our own (as
  `autotest4`'s SMBN/SMQN) for `LOCK` and `TRES`, its quest's script
  reporting `OnStoryPickLock` / `OnStoryTrespass` arguments. `LOCK` needs
  the player to pick a lock (manual) unless `Lock(false)` / `Activate` by
  script sends it too (that is itself worth reporting); `TRES` needs an
  owned interior and a warner: what V1 holds per warning, and whether a
  failed pick sends `LOCK`.

## Tier 3: long or statistical (minutes each)

- **Multi-location sandboxing** (`package-trees.md`): an NPC with a
  `SandboxMultiLocation` package (Random? false, then true) over three
  markers 1,500 units apart in the test cell, times 20, 40 and 0 seconds.
  Poll `GetDistance` to each marker every second for five minutes, with
  `Utility.GetCurrentRealTime` and `GetCurrentGameTime`: which marker it
  is at, when it arrives and when it sets off. Run once at TimeScale 20 and
  once at 200: real or game seconds, whether the clock starts on arrival or
  when the package starts, the start stop, the zero-time stop skipped, and
  (random) whether the same marker comes twice running. Needs the `PACK`
  writer of the package location test.

- Detection (`detection.md`): view cone, searching after losing a target,
  the 10 s lost timer, observer skill at range, the exterior multiplier,
  sound level to hearing distance (`CreateDetectionEvent`).
- Melee reach standing still and the 1.3 margin (`melee-reach.md`).
- Flee confidence statistics (`combat-fleeing.md`), 10-15 minutes.
- Critical strikes (`critical-hits.md`): two actors with a test sword whose
  `CRDT` percent mult is 0 and 1.0, a test perk setting the chance to 100,
  fight (`StartCombat`) a tough target; compare the health lost per `OnHit`
  (`GetActorValue` before and after) against the blow's damage: whether
  the percent mult gates criticals, what a blocked or bare-handed critical
  adds, and whether blocking stops it.
- How often the weather changes, at a high TimeScale (`weather.md`).
- Loose objects (`loose-objects.md`): `ApplyHavokImpulse` units and damping,
  drop position, corpses following the ragdoll.
- Story manager conversations (ADIA) timing (`story-manager.md`), 10+ min.

## Tier 4: disruptive (travel, guards, jail, time skips)

Last in a run, or a run of their own:

- Attack on sight and arrest thresholds with real guards (`crime.md`); an
  arrest opens dialogue and stalls the script.
- Jail and paying with jail (`crime.md`): takes the inventory, ends the run.
- Trespass timing, 30 s against 5-10 s (`crime.md`).
- Map marker discovery distances (`map-markers.md`).
- Unloaded regeneration, positions kept (`world-state.md`,
  `loose-objects.md`).
- Harvest: produce, seasons, growing back after 240 hours (`harvest.md`).
- NPCs and locked load doors, packages locking doors (`locks.md`).
- Leaving through a locked door from inside (`locks.md`).
- When an unloaded sleeper's home locks: `Utility.Wait` / `GetCurrentGameTime`
  polling `IsLocked` on a shop door across its closing hour from another
  worldspace, to see whether it flips at the package's start or later
  (`locks.md`).
- Created references after their quest stops: a quest of ours with aliases
  created at the player, in a chest and as an NPC, `Stop()`ped; then
  `IsDisabled` / `GetParentCell` / `GetItemCount` on each, and the NPC alive
  or not (`alias-fills.md`).
- Owners set by script for stealing (`sleep-wait.md`).

## Manual only (a test mod the user plays)

- **Best-in-class mark** (`inventory-cards.md`): drawn by the UI only, no
  Papyrus reading. Seen in play: give the player several swords, daggers,
  helmets and light/heavy body pieces, and note which carry the icon (classes,
  ties, light against heavy, enchanted items).
- **Rumor pooling and response arbitration** (`dialogue-selection.md`):
  create a dedicated NPC and running quests with RUMO topics, distinct
  prompts and INFO fragments recording counters and sequence numbers.
  Compare RUMO with identical-text Custom topics, and test missing FULL,
  absent branches, branch flags and explicit linked choices. Check that
  the displayed prompt predicts the INFO whose fragment runs on click.
  Use one topic with one INFO and another with three to distinguish
  per-topic from per-INFO sampling over repeated selections; vary quest
  and topic priorities separately, Random End and nonrandom boundaries
  across quests, and reverse FormIDs/plugin/start order for priority ties.
  Include heard/unheard mixtures, Say Once, reset intervals and Do All
  Before Repeating, then leave/reopen dialogue and advance game time.
  A script can record counters and save results after the user closes
  dialogue, but cannot drive the player menu unattended with vanilla
  Papyrus. Random-group behavior can be probed partially through Say(),
  but that alone does not establish player-menu pooling or prompt choice.
  Vanilla cases to check by hand: whether Rorik offers rumors (his RUMO
  topic is outside a top-level branch); which prompt Orgnar shows when the
  MS13 rumor ("Have you heard any rumors lately?") is the one drawn, and
  for a test topic whose FULL differs from the GMST when it is pooled with
  others; and Geldis Sadri's "Haven't heard anything lately" line turning
  up while his other rumors still qualify.
- Rendering: image space, particles, impacts, decals, environment maps,
  grass, foot IK.
- Weather looks: rain, snow, lightning, clouds, sky statics, auroras; sounds.
- Audio levels (`audio-categories.md`): the relative loudness of effects, footsteps,
  voice and music at the Audio menu's defaults, heard against the engine's.
- Menus and HUD: `swf-hud.md`, `journal.md`, `dialogue-menu.md`,
  `quest-targets.md`, `messages.md`, `tween-menu.md`, `favorites.md`,
  `magic-menu.md`.
- Gifts and barter (a script could save numbers for the reader after the
  user closes the menu); sleep and wait (a script can log the sleep events
  and stats).
- Player input: lockpicking, pickpocketing, grabbing and throwing, force
  greets, shout timing, the camera, attack buttons.

## Not settleable in the game

The console's `setcrimegold` default faction; condition-only rules
(`GetCrimeGold` with no faction, `IsGuard`); NPC attack data (ATKD);
ragdoll joint limits; region weather priority and global chances.
