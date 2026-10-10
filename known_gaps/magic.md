# Magic

Implemented in `src/magic.rs` (records, active effects, abilities, potions),
with the actor value store's temporary modifiers in `src/actor_values.rs`,
the natives in `src/script/natives.rs`, conditions in `src/condition/mod.rs`
and the hooks in `src/detection.rs` and `src/ai/combat.rs`. Sources: UESP's
MGEF, SPEL, ENCH and ALCH record pages; CommonLibSSE's `EffectSetting`,
`SpellItem`, `EnchantmentItem`, `AlchemyItem`, `MagicSystem` and
`ActorValues` headers; the records (`vrm-tool magic`).

## Settled

- **Records**: `MGEF` `DATA` (flags, base cost, related form, skill, resist
  value, archetype, primary / secondary actor values and weight, cast type,
  delivery...), keywords, conditions, `VMAD`. Items: `SPEL` `SPIT` (36 bytes:
  cost, flags, type, charge time, cast type, delivery, cast duration, range,
  perk), `ENCH` `ENIT`, `ALCH` `ENIT` (poison flag 0x20000), `INGR`, `SCRL`;
  each `EFID` with its `EFIT` (magnitude, area, duration) and conditions.
- **Value modifiers** (checked against the records): abilities and effects
  with the Recover flag hold their change until they end (Resist Frost 50 on
  Nords, Fortify Health potions +20 for 60 s, Muffled Movement 0.5); effects
  with a duration and no Recover apply magnitude per second (Flames: 8 for
  1 s); effects without one apply it once (Firebolt 25, Restore Health 25).
  Detrimental effects take away.
- **Peak value modifiers** sharing a keyword and actor value don't add up
  (two Fortify Health potions: +20).
- **The same item** landing again replaces its earlier effects.
- **Muffle** is `MovementNoiseMult` (Muffled Movement 0.5, Silence and the
  Muffle spell 1): movement noise in detection is scaled by 1 - it.
- **Invisibility** (archetype 11) sets `Invisibility`; an invisible target
  can't be seen (no visual factor).
- **Magic armor** (`DamageResist`: Oakflesh, the armor set bonuses) adds to
  the armor rating.
- **Effect scripts** run on an `ActiveMagicEffect` per effect:
  `OnEffectStart` / `OnEffectFinish` (target, caster), and the target's events
  (`OnHit`...) while active. Checked: Firebolt's `MG01FireEffectScript` sets
  `MG01`'s `PlayerHit` when it hits the player.
- **Spells known**: the record's `SPLO` (from the template giving the spell
  list), the race's, perks' ability sections (Muffled Movement, Silence...),
  `AddSpell`, less `RemoveSpell`.

## Open (choices made here)

- **Conditions** are evaluated on the effect's target as the subject, with the
  caster as the target. Fire-and-forget effects with no duration whose
  conditions fail don't land; lasting and constant ones wait, looked at again
  every second, and stop (`OnEffectFinish`) or start again (`OnEffectStart`)
  as they change.
- **Resistance**: hostile effects are scaled by 1 - magic resistance, and any
  effect with a resist value by 1 - that resistance; the player's resistances
  are capped at `fPlayerMaxResistance` (85), NPCs' at 100. Spells with the
  Ignore Resistance flag skip both. Whether NPCs have a cap isn't public.
- **Perks**: Mod Spell Magnitude (owner: the caster; tabs: spell, target)
  and Mod Incoming Spell Magnitude (owner: the target; tab: spell) scale
  magnitudes; not for constant effects or potions.
- **A hostile spell landing** sends `OnHit` (the spell as its source, no
  projectile) and, like a first blow, starts the target's fight and an
  assault. Instant health damage goes through the combat damage path (a
  flinch); damage over time wears health down in whole points without one.
- **Absorb** takes from the target and gives the caster as much as was
  taken (at once or per second).
- **Effects end** on the dead (except No Death Dispel) and on actors
  unloaded; abilities come back when they load. Finished effects' scripts are
  kept 30 real seconds for `OnEffectFinish`.
- **Constant enchantments** of what an actor wears apply while worn (a ring of
  Fortify Health +20). **Weapon enchantments** land with each blow (and a
  bow's with each shot) with no charge used; this hasn't been seen in a fight
  (NPCs pick their best weapon and the console can't equip theirs).
- **Not done**: casting by hand (the player's and NPCs' spells, magicka cost
  and regeneration, charge times, concentration, projectiles, areas, runes,
  dual casting, skill gain from casting), shouts, effect visuals (hit shaders,
  art objects, light) and sounds, magic in NPC combat. Archetypes other than
  value / peak / dual value modifiers, absorb, script, cure disease / poison /
  paralysis, invisibility and stagger: paralysis only sets the actor value
  (no ragdoll); calm, frenzy, fear, rally, turn undead, summons, bound
  weapons, light, detect life, telekinesis, soul trap, reanimate, cloaks,
  slow time, wards and the rest do nothing yet. Taper, area, enchantment
  charges, poisons on weapons, addictions, potion and food flags beyond
  "use it". The active effects aren't shown in a menu. Nothing is saved.
