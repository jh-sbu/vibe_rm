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

## Tier 1: cheap, record types test mods already write, no travel

Built as `test-mod autotest2`; its two runs and `test-mod autotest3`'s
follow-up (2026-10-13) settled all of it (acceleration does nothing, with
an override or without).

1. **Weather, game or real time** (`weather.md`, transition speed): delta
   125 again at TimeScale 60. Game time: about 7.6 s; real time: about
   22.8 s. Read TimeScale first and restore it on every way out.
2. **Weather, accelerating without an override** (`weather.md`):
   `ReleaseOverride`, settle, then `SetActive(false, accel)` with and
   without. 22.8 s against about 1.4 s if accelerating applies. The region's
   weather may come in instead: log the outgoing weather and flag a run
   whose current weather isn't the one set. Last of the weather tests.
3. **Settings' values** (`Game.GetGameSettingFloat / Int`): the settings
   whose defaults the data doesn't hold (`fSneakLight*`, `fSneakCombatMod`,
   `iSoundLevel*`, `fCombatFleeBoostConfidence*`, `fConfidence*`, flee and
   search timers...), read from the running game.
4. **NPC level cap at level 1** (`skills.md`): products above the cap
   (x5 max 2, x5 max 0) show the clamp and whether max 0 means none; min 0
   with x0.5 shows rounding at a tie.
5. **Hands** (`hands.md`, `magic-menu.md` readying): equip and unequip
   weapons, spells, shields, torches in turn, reading `GetEquippedWeapon`,
   `GetEquippedItemType`, `GetEquippedShield`, `GetEquippedSpell` after each.
   Gear restored.
6. **Player graph variables** (`player-body.md`): `IsNPC` and the variables
   driving the graph via `GetAnimationVariable*`.
7. **Crime money and witnesses** (`crime.md`): `PlayerPayCrimeGold` takes
   what gold there is and clears the bounty; killing the only witness wipes
   the bounty; a crime faction adds gold once however many saw; a witness
   reports to its own faction or the victim's. The alarm is raised by
   script (`SendAssaultAlarm`), which may not be a real blow.
8. **Cleared** (`cleared.md`): Dungeons Cleared across `SetCleared` true,
   false, true.
9. **NPCs take up better gear** (`crime.md`): a spawned NPC with no outfit
   given better armor, weapon, shield: does it wear them?
10. **Relationships** (`story-manager.md`): symmetric; kept per NPC.
11. **Quests started by script** (`story-manager.md`): has `OnInit` run when
    `Start()` returns, after one wait, later?
12. **Map markers** (`map-markers.md`): what the FNAM flags give
    (`IsMapMarkerVisible`, `CanFastTravelToMarker`), `AddToMap`.
13. **Templates** (`npc-templates.md`): a missing name / factions taken from
    the template.
14. **Animation events** (`object-graphs.md`): registering on an object with
    no graph fails; matching ignores case.
15. **Combat events** (`combat-state.md`): which sources send `OnHit`
    (script damage, spells, arrows, blows); re-engaging after `StopCombat`.
16. **Light level** (`detection.md`): the player's `GetLightLevel` each hour
    under a fixed weather.
17. **Statistics** (`journal.md`): `QueryStat` before and after scripted
    kills by type, skill increases, food and potions, bounty and fines,
    quests completed by type. Whether a scripted `Kill()` counts as the
    player's is one of the answers.

## Tier 2: needs record types not written yet

Spells and magic effects, perks, packages, quest aliases, story manager
nodes, scenes, trigger volumes. The writer takes any record; each needs its
field layout (`test_mods/records.rs`).

Built as `test-mod autotest4` (one plugin, as asked), with `autotest3`'s
Firebolt follow-up folded in (its two `OnHit`s: the target's health and
`OnMagicEffectApply` at each, and a spell with only its fire damage). Left
out: summons (no way to the summoned actor's reference without an effect
script), perk ranks, near aliases' reach (ALNT), item pickup ownership and
`XRNK` (they need overrides of vanilla references), scene flag 0x4 and
looping dialogue (dialogue records), the flee check's timing and ending a
flee (tier 3's statistics), and the "do all before repeating" node flags.

Its run (2026-10-13) left the magic conditions' scale and recheck,
`HasShout`, the ability section's effect, the player's magnitude perks,
health regeneration, friendly hits, scene completion, the area radius and
the traps unsettled (the tests, not the game, at fault: see `autotest4`'s
module doc). `test-mod autotest5` rebuilds them, with `taper`'s open ends
(absorbs' gain, dual effects' second values, the copies that never landed).
Its run (2026-10-13) settled all but the recheck interval, trap events and
absorb magicka's gain (`autotest5`'s module doc).

- **Story manager** (`story-manager.md`): which actor node and quest event
  conditions run on (the file's open question); members' order; reset
  hours; max running; `SendStoryEventAndWait`; item pickup (AIPL) owner and
  stolen rule; `XRNK` (needs a REFR override). A recorder quest on `OnStory*`
  events makes these cheap once there.
- **Magic** (`magic.md`, `actor-values.md`, `casting.md`): the
  `GetActorValuePercent` condition's scale (0..1 or 0..100); resistance and
  the player's 85 cap, Ignore Resistance; values calm / fear / frenzy /
  rally hold and their level cap (with resistance); absorb; conditions'
  subject and recheck interval; bound weapons; area radius (middle of the
  body, caster spared); magnitude perks; summons (placement, limit, follow
  distances).
- **Perks** (`perks.md`): priority order (add vs multiply), ranks, quest
  sections (`SetStage` on a quest not running), ability sections.
- **Skills** (`skills.md`): count `AdvanceSkill` calls to the next level:
  which level the cost uses, carry-over, `AdvanceSkill`'s units. Changes the
  player's skills; cheaper if the save reader reads skill XP.
- **Shouts** (`shouts.md`): word and shout stats around `TeachWord` /
  `UnlockWord` (permanent: use words of our own); `HasShout` for script
  given shouts (through a condition).
- **Alias packages** (`alias-packages.md`): which package wins among
  aliases of equal priority quests; one quest's aliases.
- **Alias fills** (`alias-fills.md`): create level; forced / unique aliases
  on the dead or disabled; which of several matches without Closest; near
  alias reach (ALNT).
- **Combat** (`combat-assistance.md`, `combat-fleeing.md`): assistance
  radius, ally and friend, chains; friendly hits before a fight; health
  regeneration in combat; the damage requirement; ending a flee.
- **Triggers** (`triggers.md`): one trigger volume of our own: what fires it,
  the 20 unit margin, disabled and moved triggers.
- **Traps** (`traps.md`): a scripted object of ours pushed into an NPC: when
  the trap events fire and their arguments.
- **Scenes** (`scenes.md`): Start / ForceStart / Stop rules, completion
  conditions, death and combat flags, flag 0x4.
- **NPC dual wielding and torches** (`hands.md`): combat style flags; torches
  need darkness.

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
