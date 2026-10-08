# Combat assistance (allies joining a fight)

Implemented in `src/ai/combat.rs` (`detect_enemies`, `fight_to_join`). Based on
public documentation and the game's data files only.

## What is known

- The CK wiki's *AI Data Tab*: **Assistance** is "Helps Nobody", "Helps Allies" or
  "Helps Friends and Allies" (`AIDT` byte 5: 0, 1, 2).
- Ally / friend come from faction relations (`XNAM` group combat reaction: 0
  neutral, 1 enemy, 2 ally, 3 friend). Members of the same faction count as allies.
- A negative faction rank isn't membership (potential followers carry
  `CurrentFollowerFaction` at -1), so such factions are ignored for combat.

## What the implementation does

- Every detection pass (once a second), an actor not in combat with assistance
  looks at the fights within 4000 units (no source)
  where it detects either side (`known_gaps/detection.md`). The player
  fights whoever fights them.
- It joins the nearest fight where the fighter is an ally (assistance ≥ 1) or a
  friend (assistance 2), attacking the fighter's target unless that target is its
  own ally or friend. Enemies seen on their own come first.
- Helpers can be unaggressive; helpers that are cowards flee at once, as when
  they start any fight.
- Helpers can be helped in turn, so a fight spreads through a group.

## Gaps

1. **Detection.** The helper must detect a fighter, through the detection
   formula; fights make no noise of their own yet (no action sounds or
   detection events).
2. **Relationships.** `RELA` ranks (ally, confidant, friend...) between two NPCs
   probably count too. Only factions are used.
3. **Crime.** The player's assaults and murders now add crime gold
   (`known_gaps/crime.md`), but guards don't come: the townsfolk who join an
   attack on the player do so through assistance alone, and stay hostile only
   for that fight.
4. **Friendly hits.** `iFriendHitCombatAllowed` (4 in Skyrim.esm) suggests friends
   forgive some hits before fighting back. Every hit starts a fight here.
5. **Helping the player.** Followers would help the player through
   `CurrentFollowerFaction`'s relations; no follower system exists yet.
6. **Assault dialogue.** `CombatAssault` / `CombatAssaultNC` and `AllyKilled` topics
   aren't said.
