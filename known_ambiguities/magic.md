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

## Confirmed in the game (`test-mod autotest4`, 2026-10-13)

- **Conditions' subject** is the effect's target and their target the
  caster: a fire spell needing `GetLevel >= 5` cast by a level 1 NPC at a
  level 20 one struck it run on the subject and didn't run on the target.
- **Resistance**: a 50 point fire spell took 54.9 from an NPC at fire
  resistance 0, half at 50, a tenth at 90, nothing at 100 or 200; all of it
  with the spell's Ignore Resistance flag; a quarter at fire and magic
  resistance 50 each (the two multiply). The player lost the same at 90 and
  100 (`fPlayerMaxResistance`, 85, the game's value). The extra over the
  magnitude is the effect's taper (below): 4.9 of 50, Fireball's 7.9 of 40.

## Confirmed in the game (`test-mod taper`, 2026-10-13)

- **Taper** (`MGEF` `DATA` 0x1C weight, 0x34 curve, 0x38 duration): after an
  instant value change, and after a lasting one ends, the value goes on
  changing at weight x magnitude a second, falling as `(1 - t / duration) ^
  curve`. The game applies it every frame (60 a second) at the frame's end
  rate, so the total is a little under `magnitude x weight x duration /
  (curve + 1)`: for 50 points, Firebolt's (0.3, 2, 1 s) 4.875, Fireball's
  (2 s) 9.875, Fire Storm's (4 s) 19.875, a trap poison's (0.2, 2, 1 s) 3.25,
  Storm Call's bolt (1, 1, 1 s) 24.583, all to the thousandth; Healing
  Hands' (1, 0, 0.3 s) healed 15 more; Lightning Bolt's (0.01, 0, 0.1 s) 0.05.
  Frost has none. Fire at 10 a second for 3 s took 30.886: its lasting
  change lands a fraction each frame (not in whole points) and tapers after.

## Confirmed in the game (`test-mod autotest5`, 2026-10-13)

- **Taper's reach** (50 points, cast with target actor delivery, the
  actors' regeneration off): Firebolt's fire took 54.875 as when aimed. An
  absorb health's taper reaches the caster too: the draugr axe's (0.2, 2,
  1 s) took 53.25 and gave 53.25. A dual effect's second value takes the
  taper at its weight: Storm Call's bolt 74.583 health and 37.292 magicka,
  Lightning Bolt's 50.05 and 25.025, a lightning rune's (0.3, 2, 3 s) 64.875
  and 32.437; Ice Spike's 50 and 50 stamina. The spriggans' swarm (0.5, 2,
  3 s) took 74.792 and a trap's fire 53.25. (In `taper` the trap rune and
  trap fire copies kept their condition, `GetLevel` of the player >= 30.)
- **Magnitude perks**: Augmented Flames (x1.25) made the player's 50 point
  fire spell take 68.594: 62.5 and that magnitude's taper.
- **Ability effects**: an ability of a fortify effect on One-handed (+7)
  raised it while the player had it, from `AddSpell` or a perk's ability
  section; a self spell of it (+10 for 60 s) too, until dispelled. The
  potions' fortify skill effects raise the skill's damage modifier (actor
  value 135), not the skill.

## Open (choices made here): taper

- **Absorb magicka**: the College staff's effect (`MG07StaffEffect`: 1, 0,
  1 s) took 100 magicka for 50 and gave the caster none, where the absorb
  health gave all it took; taken here as giving what it takes, as absorb
  health does.
- **Absorb health** of 20 took 20 from the target and gave the caster 20.
- **Calm, fear, frenzy, rally** held aggression -1 (Calm), confidence 0
  (Fear), aggression 3 (Fury, whose other effect sets confidence 4) and
  confidence 4 (Courage: rally's value, here taken as foolhardy, is 4).
  Calm, Fear and Fury (magnitude 9, 9, 6) left a level 20 NPC alone and
  took a level 5 one; Courage took both (no level cap).

## Open (choices made here)

- **Conditions** of fire-and-forget effects with no duration that fail
  don't land; lasting and constant ones wait, looked at again every second,
  and stop (`OnEffectFinish`) or start again (`OnEffectStart`) as they
  change (the game's interval isn't known: `autotest4`'s lasting effect
  was a potion's, which raises another value; `autotest5`'s was frost,
  whose No Duration flag makes it land once, so neither measured it).
- **Resistance**: whether NPCs have a cap isn't public (taken as 100).
- **Perks**: Mod Spell Magnitude (owner: the caster; tabs: spell, target)
  and Mod Incoming Spell Magnitude (owner: the target; tab: spell) scale
  magnitudes; not for constant effects or potions.
- **A hostile spell landing** sends `OnHit` (the spell as its source, no
  projectile) and, like a first blow, starts the target's fight and an
  assault. Instant health damage goes through the combat damage path (a
  flinch); damage over time wears health down a little each frame without
  one.
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
  value of their own (above). Two such effects on one value stack as holds
  (each holds the difference from what it found).
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
  their AI data says. The player's summons go with them through load
  doors (and `coc`), arriving side by side 160 units behind them at once
  (the game's followers come through a moment later). When the effect
  ends, or the summoned is killed or left in an unloaded cell, it goes at
  once: no banishing effect, no body left. The distances are chosen by
  eye; the game's follow and catch-up rules for commanded actors aren't
  public.
- **Bound weapons** (archetype 17; the weapon is the related form): the
  weapon is put in the target's inventory and equipped in the right hand; at the end it is
  unequipped and every one taken away. Put away (another weapon taken up)
  it lasts on: in the game Bound Sword's effect was still on 5 s after an
  iron sword took its hand (`test-mod autotest4`). Bound Bow's arrows come
  from its effect's own script.
