# Critical strikes

Implemented in `Engine::critical_strike` (`src/ai/combat.rs`), called from the
blow path for melee hits and arrows alike. Sources: UESP's
[Skyrim:Damage, Critical Strikes](https://en.uesp.net/w/index.php?oldid=3485809)
(revision 3485809), CommonLibSSE's `BGSEntryPoint` (entry points 1, 2 and 17)
and `TESObjectWEAP::CriticalData`, xEdit's `CRDT` definition (the SSE layout is
24 bytes: damage, unused, percent mult, flags, unused, effect, unused), and the
vanilla perks (`vrm-tool perks <Data> ep 1`).

## What the data and the sources settle

- **Chance** comes only from perks: "Calculate My Critical Hit Chance" (entry
  point 1) starts at 0 and the perks *set* a percent: Bladesman, Deep Wounds
  and Critical Shot 10 / 15 / 20 (by weapon keyword, tab 1; Deep Wounds and
  Bladesman also test that the target isn't the player's teammate, tab 2),
  Critical Charge and Great Critical Charge 100 for a sprinting power attack
  (`IsSprinting`, `IsAttackType` `PowerAttackTypeForward`). Because priorities
  decide who sets last, UESP's note that a sword with Bladesman gets only 10 %
  critical charges comes out of the entry point order, not special code.
  `MQBladesBlessing` adds 10 against a keyword on the target; `DLC2CriticalHit`
  (Rieklings) sets 10 with no conditions.
- **Damage** is the weapon's `CRDT` damage (UESP: usually half the weapon's
  base damage, floored, but set independently) through "Calculate My Critical
  Hit Damage" (entry point 2: Bladesman / Deep Wounds / Critical Shot x1.25 and
  x1.5, the Critical Charges x2). It is added to the blow after armor and the
  target's incoming-damage perks: "extra damage which bypasses armor". Skill,
  tempering, sneak and power attack multipliers and the ammunition don't touch
  it. Sneak attack and critical strike are separate (both can apply).
- `GetLastHitCritical` (508) answers whether the latest blow an actor took was
  critical; `IsSprinting` (568) is the player's sprint (NPCs don't sprint yet).
- The player's "Critical Strikes" statistic counts their critical blows.

## Unconfirmed choices

- **`CRDT` percent mult.** Vanilla weapons have 0 or 1.0 here (an iron sword 0, a
  Daedric sword 1.0) with no visible pattern, and UESP says every weapon
  can critical with the right perks. Taken as unused. If the game scales
  the chance by it, iron weapons would never critically strike, contradicting UESP.
- **Entry point 17 ("Mod Enemy Critical Hit Chance")** runs on the *target's*
  perks, with the attacker and weapon as its tabs 1 and 2. No vanilla perk uses
  it, so the subjects are a guess; the name only says it modifies the chance
  of enemies critically striking the owner.
- **Bare hands.** `fCombatUnarmedCritDamageMult` (1) is read as a multiplier of
  the blow's damage, giving creatures such as Rieklings (`DLC2CriticalHit`, which
  has no weapon condition) something to add. The setting's use isn't documented.
- **Blocking.** A blocked critical strike is reduced with the rest of the blow
  by the guard's share; whether the game lets blocks stop criticals outright,
  or criticals ignore blocking, isn't documented.
- **Critical effect (`CRDT` effect).** A spell cast on the target when the
  strike lands. Weapons with "Critical Effect on Death" set the flag on nearly
  every vanilla weapon with no effect, and no installed weapon has an effect,
  so neither the timing nor the on-death meaning can be checked. Effects that
  aren't on-death land after the damage; on-death ones are not cast.
- **"Critical Strike on" (`sCriticalStrike`)** exists as a setting, but the
  game's HUD isn't known to print it in Skyrim: nothing is shown.
- **Staves** don't strike critically here only because their hits aren't
  blows; unarmed and spell casting have no separate rule.
- Check timing: one roll per blow, `chance` percent, checked after the damage
  perks; a missing roll (chance <= 0) draws no random number.

Possible in-game settle (Papyrus, vanilla natives, unattended): give an actor
a sword and perks setting the chance to 100 and 50, strike a target with
known health many times and read the health lost per blow
(`plans/autotest-survey.md`).
