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

## Unconfirmed: choices made

- **Categories.** Favorites, All, the five schools (Alteration, Illusion,
  Destruction, Conjuration, Restoration), Shouts, Powers, Active Effects.
  All holds spells, powers and shouts, not the active effects. A spell's
  school is its costliest effect's skill (base cost × magnitude^1.1 ×
  (duration / 10)^1.1).
- **The level** (Novice to Master) is that effect's minimum skill
  (`MGEF` `DATA` 0x28): under 25, 50, 75, 100.
- **The cost** is the record's (`SPIT`: the Creation Kit stores the cost it
  works out when it isn't set by hand), times 1 − (0.0025 × skill)^0.65, then
  the `ModSpellCost` perks. Fortify school effects aren't counted.
  Concentration spells show it per second (`castTime` 0 picks the card's
  per-second label).
- **Readying.** Accept readies a spell in the right hand; the attack
  buttons (the control map's) in their hand. Readying it again in the same
  hand puts it away. A spell in the right hand puts the weapon away; in the
  left, the shield; a two-handed weapon goes either way; and a weapon or
  shield equipped later takes the spell's hand. Powers go to the voice slot.
- **Active effects** are listed per item (a spell, potion, ability...), under
  its name (its first effect's when it has none), without effects hidden from
  the UI; the time left is its soonest-ending effect's. Negative when any
  effect is hostile or detrimental.
- **The magic menu key** is P (the control map's Quick Magic); egui
  (`--egui-hud`) has no magic menu.
