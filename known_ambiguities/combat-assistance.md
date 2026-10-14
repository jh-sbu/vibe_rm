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

## Confirmed in the game (`test-mod autotest4`, 2026-10-13)

An enemy set on a victim, a helper of ours placed off to the side; after 8 s:

- An ally of the victim's faction helping allies joined from 500 and from
  2500 units away, not from 5000 (taken here as within 4000).
- One helping nobody didn't; a friend of the victim's faction helping only
  allies didn't; a friend helping friends and allies did.
- A member of a faction friendly to itself, struck 8 times 2 s apart by our
  25 point fire bolt cast by script from another member, never turned on it
  (whether the bolts landed wasn't checked).

## What the implementation does

- Every detection pass (once a second), an actor not in combat with assistance
  looks at the fights within 4000 units (no source)
  where it detects either side (`known_ambiguities/detection.md`). The player
  fights whoever fights them.
- It joins the nearest fight where the fighter is an ally (assistance ≥ 1) or a
  friend (assistance 2), attacking the fighter's target unless that target is its
  own ally or friend. Enemies seen on their own come first.
- Helpers can be unaggressive; helpers that are cowards flee at once, as when
  they start any fight.
- Helpers can be helped in turn, so a fight spreads through a group.

## Gaps

1. **Relationships.** `RELA` ranks (ally, confidant, friend...) between two NPCs
   probably count too. Only factions are used.
2. **Crime.** The player's assaults and murders add crime gold and send the
   guards near by to arrest them (`known_ambiguities/crime.md`); the townsfolk who
   join an attack on the player do so through assistance alone, and stay
   hostile only for that fight.
3. **Friendly hits.** `iFriendHitCombatAllowed` (4 in Skyrim.esm) suggests friends
   forgive some hits before fighting back. Every hit starts a fight here. In the
   game 8 script-cast bolts from a faction friend started none (above).
