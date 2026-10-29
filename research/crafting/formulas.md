# Crafting formulas

These are source claims from the local UESP dump, not measurements in Skyrim SE.

- [Smithing revision 3612986](https://en.uesp.net/w/index.php?oldid=3612986),
  lines 148–200: qualities 1..6 need skill 14,31,65,100,134,168 without a
  material perk; 14,22,40,57,74,91 with it. Chest bonuses 2,6,10,13,17,20;
  other armor and weapons 1,3,5,7,9,10. Value multiplier 1 + quality/6.
  Further levels continue (table and increments on that page); displayed
  quality is capped at Legendary. Fortify Smithing modifies improvement
  directly rather than raising Smithing skill. Exact fractional health mapping
  and rounding need the manual test plugin.
- Same page, lines 374–378: smithing XP = 3 * value^0.65 + 25. Value is the
  cumulative value of **all** outputs, correcting the plan's single-item
  wording. GMSTs fConstructibleSkillUseMult=3, fConstructibleSkilluseExp=.65,
  fConstructibleSkillUseConst=25. Tempering GMSTs:
  fTemperingSkillUseItemValMult=8, Exp=.6, Const=25; fTemperingSkillUseMult=1.
  Exact tempering XP rounding remains a manual-test question.
- [Alchemy revision 3596102](https://en.uesp.net/w/index.php?oldid=3596102),
  lines 152–160: initial multiplier 4, skill factor 1.5; strength = 4 *
  ingredient base magnitude * (1 + .5 * skill/100) * applicable perks *
  Fortify Alchemy * Seeker of Shadows. Negative results fall back to the base.
  Lines 409–415 say XP is proportional to value, including small failed-mix XP.
  Lines 146–148 say two or three ingredients, effects shared by two, learning
  through mixing, and greyed ingredients for **previously failed** combinations.
  This corrects the plan's proposed omniscient no-shared-effect greying.
- [Enchanting Effects revision 3595887](https://en.uesp.net/w/index.php?oldid=3595887),
  lines 698–777: skill multiplier 1 + x*(x-.14)/3.4, x=effective skill/100.
  Vanilla adds potion magnitude to effective skill (the alternative multiplicative
  potion formula is an Unofficial Patch change, excluded here). Apparel soul
  multipliers petty..grand: 1/12,1/6,1/3,2/3,1. Capacities: 250,500,1000,2000,3000.
  Weapon charges per use = 3*(base cost*magnitude/maximum)^1.1 *
  (1-sqrt(skill/200)); two effects' costs add. Maximum magnitude is floored.
- [Enchanting revision 3538992](https://en.uesp.net/w/index.php?oldid=3538992),
  lines 200–210: enchanting gives one base skill use, regardless of item/gem;
  disenchanting varies with effect and magnitude. The GMST fDisenchantSkillUse
  is .0025; its exact cost input is not established by that page.
- [ENCH revision 2652910](https://en.uesp.net/w/index.php?oldid=2652910),
  lines 69–71 and [ALCH revision 2308919](https://en.uesp.net/w/index.php?oldid=2308919),
  lines 59–61: effect cost = base cost*(max(magnitude,1)*max(duration,10)/10)^1.1.

GMSTs read from the installed records: fEnchantingSkillFactor=1.25,
fEnchantmentPointsMult=.12, fEnchantmentEffectPointsMult=8,
fAlchemySkillFactor=1.5. A matching constant's presence alone is not proof of
how it participates in a formula. Use documented formulas rather than guessing
from names. Ingredient tie-breaking, exact potion naming, mixed-effect poison
classification, failed-mix consumption and Story Manager sources still need
additional public or original-game evidence.

Smithing revision 3612986 lines 264–273 additionally gives the exact formula:
effective skill = (base skill - 13.29) * (1 + material perk) *
(1 + sum of apparel bonuses) * (1 + potion bonus) + 13.29;
quality level = floor((effective skill + 38) * 3 / 103).
The exact rating bonus is (3.6 * level - 1.6), halved outside chest armor.
Some rounded entries in the summary table conflict with this; see the decision
in known_ambiguities/crafting.md.

Tempering's material multiplier comes from PERK entry point 76, not from a
material HasPerk condition in its recipe. TemperWeaponSteelSword only tests
EPTemperingItemIsEnchanted OR Arcane Blacksmith. SteelSmithing instead multiplies
entry point 76 by 2 with item-keyword conditions. PerkSkillBoosts and
AlchemySkillBoosts also modify entry point 76 from SmithingMod and SmithingPowerMod.
Feed (Smithing - 13.29) through that entry point, then add 13.29 and apply the
quality formula. This includes material perks, fortify apparel/potions and any
authored Seeker modifier without applying them twice.

Alchemy Effects [revision 3536120](https://en.uesp.net/w/index.php?oldid=3536120),
lines 9–14 and 85–129, clarifies that one highest-priority ingredient supplies
all effect parameters; power flags independently scale magnitude and duration,
then each is rounded. NoMagnitude/NoDuration force zero. A zero duration uses
one in the cost multiplier; a positive duration uses duration/10, including
one-second hidden durations. Do not clamp those to ten seconds: this matters
for the documented Damage Health value bug. ALCH's final value floors the sum
of exact effect costs (ALCH format page), rather than summing per-effect floors.
The mathematical relationships here are independently implemented; no wiki
pseudocode is copied into project code.

The authored ALCH DefaultPotion (0005661F) is named **Unknown Potion**. It supplies
the unknown-result title; DefaultPoison supplies poison art. This settles the
plan's missing unknown-potion GMST: its text is a localized FULL, not a GMST.
