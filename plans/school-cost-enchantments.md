# Plan: school cost reduction for staffs and enchanted weapons

**Status: drafted** (2026-10-30). ROADMAP Next item 5, "Magic, NPCs and
items": "staff charge reduced by the school's cost reduction". Each numbered
step is one commit; run `cargo fmt` before each.

Items that lower a school's magicka cost (Fortify Destruction on a ring...)
also lower the charge used by staffs and enchanted weapons of that school.
Here staff casts and weapon blows pay the enchantment's cost less skill
alone.

## The evidence

- UESP [Skyrim:Enchanting](https://en.uesp.net/w/index.php?oldid=3538992)
  (revision 3538992, line 220 of the dump): "Items with enchantments that
  reduce the magicka cost of spells will also affect charge levels on
  weapons associated with that school of magic." Its example: four pieces at
  −25% Destruction give Chillrend (a sword with a frost enchantment)
  unlimited uses. So the rule covers all enchanted weapons, not just staffs.
- UESP [Skyrim:Fortify Destruction](https://en.uesp.net/w/index.php?oldid=3429893)
  (revision 3429893), Enchanting and Notes: the enchantment "decreases the
  cost of Destruction spells and the use cost of Destruction-based
  enchantments", "This also works with staves".
- UESP [Skyrim:Magic Overview](https://en.uesp.net/w/index.php?oldid=3582204)
  (revision 3582204), Spell Cost: cost = base × skill multiplier × perk
  multiplier × equipment multiplier (× dual cast), where the equipment
  multiplier is 1 minus the sum of the reductions, and four 25% pieces bring
  it to zero.
- The records (already in `known_ambiguities/magic-menu.md`, "School cost
  reduction"): `EnchFortify<School>ConstantSelf` are Peak Value Modifiers on
  actor values 108..112 (`AlterationMod`..`RestorationMod`; skill index +
  90). Spells get the reduction through `PerkSkillBoosts`' five
  `ModSpellCost` entries (Multiply 1 + AV × −0.01, conditioned on
  `SpellHasSkill`), which the player owns.

## What exists (2026-10-30)

- `Engine::spell_cost` (`src/spells.rs`): `SPIT` × skill factor, then the
  `ModSpellCost` entry point. That covers the school reduction for spells
  (the player's, through `PerkSkillBoosts`), so spells need no change.
- `Engine::staff_cost` (`src/staffs.rs`): `ENIT` cost × skill factor. It
  doesn't run the entry point, so there's no school reduction.
  `pay_weapon_charge` uses it for weapons without a crafted `use_cost`, and
  `use_cost × skill factor` for crafted copies.
- Callers: the player's staff casts (`casting.rs`, `player_spell_cost`),
  NPCs' staff choice and casts (`npc_staff_spell`, `npc_casts`), and
  weapon blows (`ai/combat.rs`, via `pay_weapon_charge`).
- `vrm-tool perks <Data>`: 43 `ModSpellCost` entries in the vanilla data,
  with spell-tab conditions `SpellHasCastingPerk` (50 uses),
  `EPMagic_SpellHasSkill` (15), `GetIsID` (12) and `HasKeyword` (9).

## Design

- **One multiplier for enchantments.** Add `Engine::school_cost_mult(actor,
  school_skill) = max(0, 1 - actor_value(actor, skill + 90) / 100)`.
  `staff_cost` and the crafted `use_cost` path both multiply by it after the
  skill factor. A negative value raises the cost, as for spells. Clamp the
  result at zero, as `spell_cost` does.
- **Not the whole entry point.** Running `ModSpellCost` on enchantments
  would bring in the other 38 entries, which aren't sourced for items.
  `staffs.md` records that whether spell cost perks apply to staffs isn't
  published. So only the actor-value reduction is applied, and it isn't
  applied twice because spells keep their perk path. Record this in
  `known_ambiguities/staffs.md`.
- **Who.** Any actor: the multiplier reads the actor's own value. This
  differs from spells, where the reduction comes from a perk only the player
  owns (`magic-menu.md` leaves NPCs open). Record it as a choice.
- **Which school.** `spell_school(ench)`, the costliest effect's skill, as
  staffs use already.

## Steps

0. **Survey.** Extend `vrm-tool perks` (or add `vrm-tool cost-entries`) to
   list, for each `ModSpellCost` entry, whether its conditions could match
   an enchantment: `SpellHasCastingPerk` can't (an `ENCH` has no `SPIT`
   perk), `SpellHasSkill` can, and `GetIsID` / `HasKeyword` depend on the
   form. Write the result into `known_ambiguities/staffs.md` as the
   evidence for not running the entry point. Commit with the tool change.
1. **The multiplier.** `school_cost_mult` in `src/spells.rs` beside
   `skill_cost_factor`. Use it in `staff_cost` and in
   `pay_weapon_charge`'s `use_cost` branch. Tests in `src/staffs.rs`
   (synthetic records, as `staff_engine` builds them):
   - a Destruction staff with `DestructionMod` 25 costs 0.75 of before;
   - at 100 it costs nothing and casts with an empty charge;
   - a frost-enchanted sword's blow spends less at 50;
   - a Restoration enchantment isn't changed by `DestructionMod`;
   - a spell's cost with `DestructionMod` 25 is unchanged by this commit (the
     perk path covers it), guarding against double counting.
2. **NPCs and the item card.** NPC staff affordability
   (`npc_staff_spell`) and casts follow from `staff_cost`; add one test
   with an NPC carrying the reduction. Check what the inventory card shows
   for staffs (`swf_ui/item_menu.rs`). It shows charge, not uses, so it
   needs no change; note that in the commit.
3. **Docs.** Update `known_ambiguities/staffs.md` ("The cost"): the
   reduction, its source, why it isn't the full entry point, and that NPCs
   get it. Remove the clause from the ROADMAP entry.

## Verification

- `cargo test` for the cases above.
- Console check in a run: give the player Fortify Destruction rings
  (`player.additem`, `player.equipitem`) and a Staff of Flames, cast, and
  compare `GetActorValue RightItemCharge` before and after with and
  without the rings.

## Open questions

- Whether the game runs the full `ModSpellCost` entry point for
  enchantments, so that other cost perks also lower charge use. Nothing
  public says so. An SE test could tell: a plugin perk with a
  `ModSpellCost` entry and no spell conditions, given to the player, with
  `RightItemCharge` read before and after one staff cast. That is
  autotest-friendly.
- Whether NPCs' worn reductions apply in the game.
