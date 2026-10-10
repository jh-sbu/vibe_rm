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

## Open (choices made here)

- **Which level the cost uses.** UESP gives `improve mult x (level - 1)^1.95 +
  offset` but works its example with the current level (lockpicking 15 to 16:
  349.13). The example is followed: the current level.
- **XP past a level carries over** into the next.
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
- **`Game.AdvanceSkill`'s value** is treated as a use's base XP (through the use
  mult). The CK wiki calls it "skill usage", which suggests that.
- **Level ups** wait until the player takes them in the skills menu (K here; the
  game opens the skills from the Tab menu) or with the console `levelup`.
  There is no perk tree yet: perk points only add up.
- **Skill books** teach once per book form (`DATA` flag 1, skill at offset 4).
  The game clears the flag on the book's base, so another copy doesn't teach;
  that is the same thing.
- **Not done**: magic schools, smithing, alchemy, enchanting, speech (no
  barter or persuasion yet), trainers, the Rested / Well Rested / Lover's
  Comfort / Guardian Stone bonuses, Legendary skills. Only the player's skills
  advance; NPCs' come from their records. The progress isn't saved (no saves).
- **NPC levels** (`GetLevel`): `ACBS` level, or with the PC level mult flag
  (0x80) the player's level x mult / 1000, rounded, between calc min and max
  (max 0: none). How the game rounds isn't public.
