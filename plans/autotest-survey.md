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
- **Actors' keywords** (`npc-templates.md`): `HasKeyword` on an NPC wearing
  armor with `ArmorHeavy`, and under a spell whose effect has a keyword
  its race lacks.

## Tier 3: long or statistical (minutes each)

- Detection (`detection.md`): view cone, searching after losing a target,
  the 10 s lost timer, observer skill at range, the exterior multiplier,
  sound level to hearing distance (`CreateDetectionEvent`).
- Melee reach standing still and the 1.3 margin (`melee-reach.md`).
- Flee confidence statistics (`combat-fleeing.md`), 10-15 minutes.
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
- Owners set by script for stealing (`sleep-wait.md`).

## Manual only (a test mod the user plays)

- Rendering: image space, particles, impacts, decals, environment maps,
  grass, foot IK.
- Weather looks: rain, snow, lightning, clouds, sky statics, auroras; sounds.
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
