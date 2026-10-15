# The magic menu (magicmenu.swf) and spells in hand

Implemented in `src/swf_ui/magic_menu.rs` (the menu) and `src/spells.rs`
(spells in hand, what the card says). Sources: the movie's ActionScript
(`MagicMenu` extends the inventory's `ItemMenu`; `ItemCard` reads `type`,
`spellCost`, `castTime`, `castLevel`, `effects`, `timeRemaining`,
`negativeEffect`; the bottom bar `magicSchoolName` / `Level` / `Pct`); the
game settings for the categories (`sAlteration`..., `sShouts`, `sPowers`,
`sActiveEffects`) and levels (`sNoviceDisplayName`...); the CK wiki for
`Actor.EquipSpell` / `UnequipSpell` / `GetEquippedSpell` (`aiSource` 0 the
left hand, 1 the right, 2 other); UESP for the skill's cost reduction
(`fMagicCasterPCSkillCostBase`, `fMagicPCSkillCostScale`); CommonLibSSE for
the `ModSpellCost` entry point (38).

## School cost reduction

`AlterationMod` .. `RestorationMod` reduce the corresponding school's
magicka cost by their percentage, separately from skill and perks. The
current actor value includes worn enchantments and script changes; the
same cost serves the card, casting, and NPC spell affordability. At 100%
or above, the cost is zero. `PowerMod` values are not cost reductions.
Public first-hand modding reports describe `DestructionMod` +30 as a 30%
cost reduction and distinguish it from `DestructionPowerMod`:
[Creation Kit and Modders discussion](https://forums.nexusmods.com/topic/9027933-difference-between-onehanded-onehandedmod-onehandedpowermod/).
The [Magicka Cost Level Scaling author's documentation](https://www.nexusmods.com/skyrimspecialedition/mods/21557)
also gives separate skill, perk and equipment multipliers.
The installed game's `EnchFortify<School>ConstantSelf` effects all use the
Peak Value Modifier archetype and actor values 108 through 112, in the
order Alteration, Conjuration, Destruction, Illusion, Restoration (`MGEF`
`DATA` 0x40 and 0x44). These are the skill modifiers, not the power modifiers.

`PerkSkillBoosts` (`000CF788`, overridden by Update.esm) supplies five
`ModSpellCost` entries (38), with the Multiply 1 + Actor Value Mult function
(14), each conditioned on the spell's school (`SpellHasSkill`). Their
`EPFD` pairs are the corresponding actor value index and -0.01. The player
owns this perk; the existing perk evaluator therefore already implements
these reductions. Applying another multiplier in `spell_cost` would apply
the reduction twice. At 100% or above the final cost is clamped to zero.
The roadmap's missing-cost entry was stale and has been removed.

The Novice .. Master perks additionally require `SpellHasCastingPerk`
(condition 713). It compares the spell's `SPIT` FormID at byte 32 with the
requested perk, resolving it in that spell's plugin. It does not check
whether the caster owns the requested perk: the entry point's ownership
check does that separately. Firebolt names `DestructionApprentice25`, whose
entry halves cost for spells naming it. That condition was missing here
and is now implemented. A [first-hand CK perk example](https://forums.nexusmods.com/topic/1192384-how-to-modify-destruction-spells-without-moding-enchanting/)
documents using the same condition on the Spell tab to select tiers and
exclude enchantments; xEdit's SPEL definition gives the `SPIT` field layout.

The entries' priorities also determine their order against other cost perks;
the engine uses the existing record-driven priority ordering. Still open:
whether NPCs without this perk receive any school cost reduction outside
perks. No fallback is invented here. Negative school modifiers increase
cost through the same perk function; that case needs an in-game comparison.
No engine binary was examined.

### Mixed-school effects remain unresolved

`EPMagic_SpellHasSkill` (696) currently matches any effect's skill, including
conditional secondary effects. Firebolt's records have `FireDamageFFAimed`
(Destruction, 20), `PerkImpactStaggerPushFFAimed` (Restoration, 22), and
`PerkIntenseFlamesConfDownFFAimed` (Illusion, 21). Consequently, this engine
can apply Restoration and Illusion cost modifiers to Firebolt as well.
The [public CK perk example](https://www.gamesas.com/creating-script-make-spell-damage-scale-based-player-t260890.html)
describes selecting the spell's school but does not distinguish any-effect
from principal-effect matching or explain conditional effects. The records
alone cannot establish that behavior. It needs an in-game comparison with
only each secondary school's modifier changed, with and without the
secondary effect's perk. This turn leaves that existing choice unchanged.

### Verification in this engine (2026-10-15)

Offscreen casting with the installed game records and other schools'
modifiers zero: 25% reductions changed Candlelight 21 -> 15.75,
Conjure Familiar 107 -> 80.25, Firebolt 41 -> 30.75, Muffle 144 -> 108,
and Fast Healing 73 -> 54.75 magicka. Firebolt at Destruction 100 with
Apprentice Destruction and 25% reduction spent 9.13. Flames spent 14
magicka per second, 10.5 with 25% reduction, and zero with 100%; Firebolt
also spent zero at 100% and 150%. These verify this implementation,
not a comparison with the original runtime.

## Unconfirmed: choices made

- **Categories.** Favorites, All, the five schools (Alteration, Illusion,
  Destruction, Conjuration, Restoration), Shouts, Powers, Active Effects.
  All holds spells, powers and shouts, not the active effects (shouts'
  cards: `known_ambiguities/shouts.md`). A spell's
  school is its costliest effect's skill (base cost × magnitude^1.1 ×
  (duration / 10)^1.1).
- **The level** (Novice to Master) is that effect's minimum skill
  (`MGEF` `DATA` 0x28): under 25, 50, 75, 100.
- **The cost** is the record's (`SPIT`: the Creation Kit stores the cost it
  works out when it isn't set by hand), times 1 − (0.0025 × skill)^0.65, then
  the `ModSpellCost` perks and the school's cost reduction above.
  Concentration spells show it per second (`castTime` 0 picks the card's
  per-second label).
- **Readying.** Accept readies a spell in the right hand; the attack
  buttons (the control map's) in their hand, unless its equip slot names
  only the other (`IncinerateLeftHand`); spells cast with both hands
  (`BothHands`) take both. Readying it again in the same hand puts it away.
  A spell takes the place of what its hand holds (a weapon, the shield; a
  two-handed weapon either way), and a weapon or shield equipped later
  takes the spell's hand (`known_ambiguities/hands.md`). Powers go to the
  voice slot.
- **Active effects** are listed per item (a spell, potion, ability...), under
  its name (its first effect's when it has none), without effects hidden from
  the UI; the time left is its soonest-ending effect's. Negative when any
  effect is hostile or detrimental.
