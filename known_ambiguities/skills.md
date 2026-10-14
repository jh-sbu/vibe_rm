# Skills and leveling

Implemented in `src/skills.rs`, with the uses wired in `src/ai/combat.rs`,
`src/locks.rs` and `src/pickpocket.rs`. Sources: UESP's Skyrim:Leveling and
skill pages; the skills' `AVIF` `AVSK` values and the game settings named
below (Skyrim.esm / Update.esm).

## Settled

- **AVSK** is four floats: use mult, use offset, improve mult, improve offset.
  The values match UESP's table (one-handed 6.3 / 0 / 2 / 0, lockpicking
  45 / 10 / 0.25 / 300...). A skill's `AVIF` is `0x446 + index`.
- **XP for a use**: use mult x base XP + use offset.
- **Character XP**: the skill level reached x `fXPPerSkillRank` (absent from
  the data, so 1); a level up takes `fXPLevelUpBase` (75) + `fXPLevelUpMult`
  (25) x level.
- **Level ups**: +`iAVDhmsLevelUp` (10) health, magicka or stamina, a perk
  point; health, magicka and stamina restored. Stamina also gives carry weight
  (`fLevelUpCarryWeightMod`, 5).
- **Base XP of the uses done**: a landed blow, its weapon's base damage
  (one-handed, two-handed, archery); a melee sneak attack,
  `fSneakAttackSkillUsageMelee` (30); a lock picked,
  `fSkillUsageLockPick<VeryEasy..VeryHard>` (2, 3, 5, 8, 13); a broken pick
  `fSkillUsageLockPickBroken`; sneaking hidden, `fSkillUsageSneakPerSecond`.
- **Level ups** wait until the player takes them in the skills menu (K here; the
  game opens the skills from the Tab menu) or with the console `levelup`.
  Perk points buy perks from the skills' perk trees (`known_ambiguities/perks.md`).
- **Skill books** teach once per book form (`DATA` flag 1, skill at offset 4).
  The game clears the flag on the book's base, so another copy doesn't teach;
  that is the same thing.

- **NPC levels** (`GetLevel`): `ACBS` level, or with the PC level mult flag
  (0x80) the player's level x mult / 1000, rounded down (1.5 and 1.667 at
  level 1 gave 1), at least calc min (`test-mod autotest`) and at most calc
  max, 0 meaning none (x5 with max 2 gave 2, with max 0 gave 5); calc min 0
  allows 0 (x0.5 gave 0). A fixed level ignores calc min and max (7 with max
  2 gave 7) (`test-mod autotest2`).

- **The cost of a level** (`test-mod autotest4`): `AdvanceSkill(skill, 10)`
  calls until a skill rose: One-Handed 21 to 22 took 12 (the current level's
  cost, 757.5 XP, with what was left over from the last rise; the level
  before's would take 11) and 51 to 52 took 67 (678 by the current level,
  653 by the one before); Lockpicking 1 each (`AdvanceSkill`'s value goes
  through the use mult and offset: 10 x 45 + 10 = 460, over its 386). So
  the cost uses the current level, XP past a level carries over, and
  `AdvanceSkill`'s value is a use's base XP.

## Open (choices made here)

- **No UESP number has a setting**: a sneak attack shot is 2.5 Sneak XP, a
  bash 5 Block XP.
- **Block** trains by the damage stopped (before armor), **armor** by the rest
  (before armor), whichever kind most worn pieces are (heavy on a tie). How the
  game splits mixed armor isn't public.
- **Pickpocket** trains by the item's value x count (gold: its amount).
  `fPickpocketSkillUsesCurve` (0.8) may bend that; how isn't public.
- **Sneaking hidden** counts only while some actor is close enough to work out
  a detection of the player and none detects them (the `[HIDDEN]` eye). UESP
  says "within the detection radius of an enemy"; friendly actors count here.
  The 2.5 XP for becoming hidden isn't given.
- **Blows**: every landed blow trains, blocked or not; hitting objects or
  corpses doesn't. Power attacks earn the same as basic ones.
- **Training** (UESP Skyrim:Trainers; `src/training.rs`). Settled: the
  trainer's class `DATA` has the skill it teaches at byte 4 (from
  One-handed) and the level it teaches up to at byte 5 (50, 75, 90 in the
  vanilla `Trainer*` classes); `sCanNotTrainHigher`, `sCanNotTrainAnymore`,
  `sInsufficientGoldToTrain`; the gold goes into the trainer's inventory.
  UESP's prices for a session from skill `b` (`10b + 50` up to 50, `30b +
  50` past it, `50b + 50` past 75) are `50 + fTrainingMultCost` (10) `x b`,
  times `iTrainingExpertCost` (3) or `iTrainingMasterCost` (5) past 50 and
  75; the 50 and the five sessions a level aren't settings in Skyrim.esm,
  so they are constants here. Choices: the class is read from the NPC's
  Stats template part; the rank shown is `sJourneyman` ("Common") up to 50,
  `sExpert` up to 75, `sMaster` past it; the session count starts again when
  a level up is taken (not when it becomes available); a session keeps the
  skill XP already earned toward the next level (as `IncrementSkill`), and
  passes no time; refusals are notifications; the menu has no sounds (no
  sound descriptor is named for it).
