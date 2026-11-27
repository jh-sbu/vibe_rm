# Creature diseases caught from blows

Implemented in `src/magic.rs` (`Engine::deliver_attack_spell`,
`has_caught_disease`) and `src/ai/combat.rs` (`resolve_swings`): an
attack's spell (`ATKD` at 0x08) lands with each blow of that attack.

## What the records and Skyrim SE show

- Each race's attack data names a spell its blows carry: the bear's and
  skeever's on every attack, the wolf's only on its power attacks and
  lunges. For these creatures the spell is the disease itself (the bear's
  `DiseaseBoneBreakFever`), and it reaches only targets struck by those
  attacks (UESP *Skyrim:Wolf*: "Power Attacks inflict Rockjoint").
- Each disease effect carries its chance as a condition:
  `GetRandomPercent <= N` (Ataxia and Rockjoint 2, Bone Break Fever,
  Rattles and Witbane 5, Brain Rot and Sanguinare Vampiris 10, the last also
  `GetIsID Player`). `GetRandomPercent` runs 0 to 99, so these are 3%, 6% and
  11%. Survival Mode's overrides add only "Survival Mode off" to these and
  carry their own versions for when it is on.
- Skyrim SE, autotest29 (`research/autotest29.md`): nine NPCs each struck by
  a bear for 150 s. The first infecting blow was 2, 3, 7, 10, 10, 13, 32
  and 58 (one NPC struck 8 times wasn't infected): 8 in 143 blows, 5.6%,
  against the condition's 6%. So each blow rolls, and NPCs catch diseases.
  The delivery sent no `OnHit` of its own.
- Skyrim SE, autotest28 and autotest29: `HasSpell(the disease)` read false
  while the disease was in force, and `RemoveSpell` cured it; `GetDisease`
  read 1 for the player's caught disease and 0 for one added by `AddSpell`
  (autotest22, autotest23).

## Choices

- **The roll is made once, as the blow lands.** Each effect's conditions
  are evaluated then; those that fail don't come at all, and those that
  pass aren't looked at again (other constant effects are, every second).
  autotest29 doesn't settle the second half: of the eight NPCs infected,
  four kept the full 25-point stamina drain to the end, three had none at
  the end (with `HasMagicEffect` still true, which reports an effect whose
  condition fails), and one had 9.2 of it. A re-check every second would
  have left few with the drain; none at all would have left all eight.
  UESP's *Skyrim:Disease* notes as a confirmed bug that Bone Break Fever
  "rarely" drains 0. A follow-up batch polling max stamina after the
  infecting blow could settle it (`plans/ROADMAP.md`).
- **A caught disease isn't one of the target's spells** (`HasSpell` false).
  It is an effect marked `delivered` that the ability sync leaves on, until
  `RemoveSpell`, Cure Disease or a dispel ends it. It is saved with that
  mark.
- **Disease Resist is a separate roll.** A disease is resisted outright
  with the target's Disease Resist as a percent chance, before the
  effects' own rolls. UESP's *Skyrim:Resist Disease* says it "decreases by
  <mag>% your chances of contracting a disease", which this gives: a 6%
  chance at 50% resist is 3%. The disease effects name no resist value
  (their `DATA` resist is none), so the game must apply it by spell type;
  how is unread.
- **It lands quietly** (`apply_item_quietly`'s way): the blow was the hit
  and the assault, so the disease adds no `OnHit`, fight or crime of its
  own. A blocked blow delivers it too, as enchantments are; unread.
- **`GetDisease`** reads 1 while the subject has any caught disease. Read
  in the game only for the player; for NPCs it is the same rule, unread.
