# Perks

Implemented in `src/perks.rs`, with the hooks in `src/ai/combat.rs`,
`src/detection.rs`, `src/locks.rs` and `src/pickpocket.rs`, and the perk trees in
the skills menu (`src/ui.rs`). Sources: CommonLibSSE's `BGSPerk`,
`BGSPerkEntry`, `BGSEntryPoint` and `BGSEntryPointPerkEntry` headers (entry
point and function numbers), `BGSSkillPerkTreeNode` (the `AVIF` tree fields),
UESP's PERK record format and skill pages, the records themselves (`vrm-tool
perks`).

## Settled

- **Record**: `DATA` (trait, level, ranks, playable, hidden), the conditions
  before the first section (what it takes to buy it), `NNAM` (the next rank),
  then sections `PRKE` (type, rank, priority) .. `PRKF`. Quest sections:
  `DATA` quest + stage; abilities: a spell; entry points: `DATA` (entry point,
  function, tab count), `PRKC` (tab index) before each tab's conditions, `EPFT`
  (1: one float, 2: two floats, 3+: leveled list, spell, text...) and `EPFD`.
- **Functions**: 1 set, 2 add, 3 multiply, 4 add range, 5 add actor value x
  factor, 6 absolute, 7 negative absolute, 12 set to actor value x factor, 13
  multiply by actor value x factor, 14 multiply by 1 + actor value x factor.
  The two-float functions' first float is the actor value's index
  (`PerkSkillBoosts`: 96.0 .. 107.0 with 0.01).
- **Casting costs**: `SpellHasCastingPerk` (713) reads the spell's authored
  casting perk (`SPIT` byte 32), so the Novice .. Master perks select the
  right spells. `PerkSkillBoosts` supplies Fortify school cost reductions
  through `ModSpellCost`, not a separate hardcoded multiplier. Sources and
  remaining questions: `known_ambiguities/magic-menu.md`.
- **Owners**: the NPC record's `PRKR` (perk, rank), from the template giving
  the spell list (`template::SPELLS`); the player's from `NPC_ 00000007` (12
  perks: skill boosts, Allow Shouting, Well Fitted...).
- **Temporary owners**: active player magic effects' `DATA` 0x88 perks join
  `HasPerk` and entry-point evaluation without altering permanent ownership.
  Duplicate grants contribute once; ability and quest sections are deferred.
  Sources and lifecycle limits: `known_ambiguities/effect-perks.md`.
- **The tree**: each node is `PNAM` perk, `FNAM`, `XNAM` / `YNAM` grid cell,
  `HNAM` / `VNAM` offsets, `SNAM` skill, `CNAM` children, ended by `INAM` (its
  index); the first node (no perk) is the root. Multi-rank perks are chains
  of records by `NNAM`, each with its own requirements (Stealth20: Sneak 20 and
  Stealth00).
- **Numbers checked** against UESP's perk descriptions (console `perkep`):
  Backstab 6x and Assassin's Blade 15x with daggers, Deadly Aim 3x with bows,
  Armsman 1.2x, Juggernaut 1.2x the heavy piece's rating (an iron cuirass goes
  from 27 to 32 here), Stealth 20..40%, Light Fingers +20, Novice Locks twice
  the sweet spot below Apprentice locks.
- **Pickpocket perks** add to the chance before the 90% cap (UESP's
  Skyrim:Pickpocket formula: Light Fingers is a flat term in the sum, and
  Cutpurse's +50 "will still max out at 90%"); Keymaster sets 100, so keys
  come out at the cap.

## Confirmed in the game (`test-mod autotest4`, 2026-10-13)

- A perk with a quest section, added to the player, set the stage on a
  quest of ours that wasn't running, which started it; removing the perk
  left the stage set and the quest running.
- A perk with an ability section gave the player the ability spell, and
  removing it took the spell away.

## Confirmed in the game (`test-mod autotest5`, 2026-10-13)

- **Priority order**: entries apply highest priority first. With Mod Spell
  Magnitude +10 at priority 0 and x2 at 1, the player's 50 point frost
  spell took 110 (x2 first); with +10 at 1 and x2 at 0, 120.
- An ability section's constant effect works: +7 One-handed while the perk
  was had.

## Open (choices made here)

- **Quest sections on quests not running.** Most perks' stages are on
  `PerksQuest`, which isn't start-game-enabled; its fragments need the quest's
  properties. A stage set on a quest that has never run starts it first (the
  CK wiki's `Quest.SetStage`: setting a stage starts a quest that isn't
  running). A quest that ran and was stopped isn't restarted: stage changes
  wait for the end of the script run here, so `SetStage` followed by `Stop`
  in one fragment would otherwise restart it.

- **Ranks**: an entry applies while the owner's rank of the perk is above the
  entry's rank. The vanilla perks have one rank per record, so this is
  untested.
- **Tab subjects** as wired here (the tab count and the conditions used in
  the vanilla records fit them; the CK's tab names aren't in a public
  source): Mod Attack Damage, Mod Power Attack Damage, Mod Sneak Attack Mult,
  Mod Target Damage Resistance: owner, weapon, target. Mod Incoming Damage:
  owner, attacker, attacker's weapon. Mod Armor Rating: owner, armor piece.
  Mod Bashing Damage: owner, shield (else weapon). Mod Power Attack Stamina:
  owner, weapon. Mod Percent Blocked, Mod Detection Sneak Skill, Make Lockpicks
  Unbreakable: owner. Mod Detection Light / Movement: owner (the observer),
  target. Mod Lockpick Sweet Spot: owner, the locked reference. Mod Pickpocket
  Chance, Can Pickpocket Equipped Item, Should Apply Placed Item: owner,
  victim, item. Conditions on any tab also see the last subject
  as their target.
- **Where each number is changed** (the order in the game isn't published):
  attack damage, then power attack damage, then the sneak multiplier (itself
  through Mod Sneak Attack Mult); armor penetration scales the share the
  target's armor takes; Mod Incoming Damage after armor, before blocking;
  Mod Percent Blocked before the 85% cap; Mod Armor Rating on each piece's
  rating after the skill scaling, before rounding up.
- **Stealth perks**: Mod Detection Sneak Skill is a percentage taken off the
  observer's skill factor while the target sneaks (UESP: the Sneak skill and
  its perks reduce the noticer's skill factor, but gives no formula for how
  the perk's 20..40% enters it).
- **Buying**: one perk point per rank, if the rank's conditions pass on the
  player.
- **Ability sections** are spells the owner knows: constant ones take effect
  (`known_ambiguities/magic.md`).
- **Recovering arrows**: Mod Recover Arrow Chance (Hunter's Discipline sets
  66) changes `iArrowInventoryChance` (33), the chance an arrow striking an
  actor ends up in its inventory, rolled as it strikes (living or not);
  UESP's "from dead bodies" may mean the game rolls only when the body is
  searched.
