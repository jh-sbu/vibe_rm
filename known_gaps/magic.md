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
- **Spell tomes** (`BOOK` `DATA` flag 4, the spell after the flags) used
  from the inventory teach their spell and are used up, with the
  notification "New Spell Learned: Flames" (`sSpellAdded`, then the
  spell's name; the game's exact wording isn't published); a spell already
  known says "You already know Flames" (`sAlreadyKnown` and the name) and
  the tome stays. A tome in the world opens as a book when activated, as
  other books do. Spells Learned counts every spell (type 0) the player
  gains, from tomes, scripts or the console.
- **Calm, fear, frenzy, rally** (archetypes 6, 7, 8, 38; turn undead 24
  as fear) hold the effect's actor value (aggression, confidence) at a
  value of their own: CommonLibSSE's `TargetValueModifierEffect` subclasses
  give calm -1, demoralize 0 and frenzy 3; rally's isn't given, so it is
  foolhardy (4), which fits "won't flee". Two such effects on one value
  stack as holds (each holds the difference from what it found).
- **Their level cap**: calm, fear and frenzy land only on targets whose
  level is at most the magnitude (UESP: "creatures and people up to level
  N"; "Target Level does not apply to Rally effects"). The magnitude is the
  one after perks (Animage, Kindred Mage, Hypnotic Gaze... are Mod Spell
  Magnitude entries) and, for the hostile ones, after magic resistance, so
  a giant's resistance brings Fear's 9 down to 6. Whether resistance
  touches the cap in the game isn't published. A target too strong for it
  is still struck by a hostile spell (it fights its caster).
- **`IsUndead`** is the subject having the `ActorTypeUndead` keyword; the
  game caches its own reading (CommonLibSSE `CalculateCachedOwnerIsUndead`),
  whose rule isn't public. **`IsCommandedActor`**: a summoned actor.
- **Calm** ends the target's fight; while calm it looks for no fights (nor
  joins its allies'). Any blow or hostile spell landing on it ends the calm
  and it fights back (UESP: no will to fight "unless the target is
  attacked first"; what follows the attack isn't described).
- **Fear**: a coward (confidence 0) thinks about fleeing every frame it
  isn't fleeing, not on the threat ratio timer, so the frightened run at
  once; when it ends the next check brings them back.
- **Frenzy**: the target attacks the nearest actor (or the player) it
  finds, allies too (aggression 3 is hostile to everyone). Casting it is an
  assault as other hostile spells are (UESP: Frenzy "not considered a
  crime" if the caster is hidden), but the target doesn't turn on its
  caster for it. When it ends, the target leaves a fight with someone it
  wouldn't fight now, and those fighting it who wouldn't fight it leave
  theirs; the game's own clean-up isn't described.
- **Summons** (archetype 18; the NPC is the effect's related form): the
  effect is the caster's, and its actor appears where a target location
  spell lands (cast some other way, beside the caster), facing the caster.
  A commander has one summon, more by its Mod Commanded Actor Limit perk
  entries (Twin Souls: +1; tabs: owner, spell); a new one sends the oldest
  away. The summoned follow their commander (keeping 160 to 320 units off,
  running), are moved beside it when more than 4096 units behind out of a
  fight, fight the nearest actor fighting their commander (or the one it
  fights), within 3000 units, never their commander, and pick no fights
  of their own (their own aggression is ignored). Others treat them as
  their AI data says. When the effect ends, or the summoned is killed or
  left in an unloaded cell (its commander through a load door), it goes at
  once: no banishing effect, no body left. The distances are chosen by
  eye; the game's follow and catch-up rules for commanded actors aren't
  public.
- **Not done**: NPCs casting by hand (the player's casting:
  `known_gaps/casting.md`), runes, dual casting, effect visuals (hit shaders,
  art objects, light) and sounds, magic in NPC combat. Archetypes other than
  value / peak / dual value modifiers, absorb, script, cure disease / poison /
  paralysis, invisibility, stagger and the target value modifiers (calm,
  fear, frenzy, rally, turn undead) and summons: paralysis only sets the
  actor value (no ragdoll); NPCs summoning, reanimating, bound weapons, light, detect life, telekinesis, soul
  trap, reanimate, cloaks, slow time, wards and the rest do nothing yet. Taper, area, enchantment
  charges, poisons on weapons, addictions, potion and food flags beyond
  "use it". The active effects aren't shown in a menu. Nothing is saved.
