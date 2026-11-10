# Combat assistance (allies joining a fight)

Implemented in `src/ai/combat.rs` (`detect_enemies`, `fight_to_join`). Based on
public documentation and the game's data files only.

## What is known

- The CK wiki's *AI Data Tab*: **Assistance** is "Helps Nobody", "Helps Allies" or
  "Helps Friends and Allies" (`AIDT` byte 5: 0, 1, 2).
- Ally / friend come from faction relations (`XNAM` group combat reaction: 0
  neutral, 1 enemy, 2 ally, 3 friend). Members of the same faction count only
  as the faction lists itself: in Skyrim.esm 160 factions list themselves as
  allies, 29 as friends, 8 as enemies (`C00JorrvaskrFightingFaction`,
  `ArenaSelfLoathingFaction`, `WE19BanditFaction`...) and 3 as neutral, and
  188 factions with relations don't list themselves.
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
  (`autotest5`: each bolt landed, 27.4 health with its taper).

## Confirmed in the game (`test-mod autotest21`, 2026-11-01)

`research/autotest21.md`, Assistance (in `WarehouseFurniture`): Red attacks
a member of Teal, a plugin faction with no `XNAM`. The victim fights; Teal
members helping allies, 500 and 1200 units off, never joined in 10 s; a
helper of no faction at relationship rank 3 with the victim joined at once
and targeted the attacker. autotest22 read `GetFactionCombatReaction` of
Teal towards itself as 0. So a shared faction alone doesn't make allies
(ours since 2026-11-07); relationship ranks bring help (item 1 below).

## What the implementation does

- Every detection pass (once a second), an actor not in combat with assistance
  looks at the fights within 4000 units (no source)
  where it detects either side (`known_ambiguities/detection.md`). The player
  fights whoever fights them.
- It joins the nearest fight where the fighter is an ally (assistance ≥ 1) or a
  friend (assistance 2), by the factions' entries (a shared faction only as it
  lists itself) or their relationship rank, attacking the fighter's target
  unless that target is its own ally or friend (a shared faction counting
  here, as for hostility). Enemies seen on their own come first.
- Helpers can be unaggressive; helpers that are cowards flee at once, as when
  they start any fight.
- Helpers can be helped in turn, so a fight spreads through a group.

- Followers: `CurrentFollowerFaction` (`0005C84E`) lists the player's faction
  and one more as allies (`XNAM` reaction 2) in Skyrim.esm. Scripts' faction
  changes (`AddToFaction`, `SetFactionRank`, `RemoveFromFaction`) update a
  loaded actor's factions for fights at once and are applied again when it
  loads; the player's own count as the player's side. A hired follower thus
  helps the player in a fight as its AI data's assistance has it (Mjoll 2,
  Faendal 1). Unconfirmed: that the game's followers help by this alone; its
  teammate flag (`SetPlayerTeammate`; CommonLibSSE `Actor` flag
  `kPlayerTeammate`) is not read yet, and no public source says what else it
  changes in a fight.

## Unconfirmed

1. **Relationships.** A relationship rank brings help: Skyrim SE
   (autotest21, autotest23 and autotest24, `research/autotest23.md`,
   RelationshipHelp) with a helper of no faction, a helper of allies joined
   from rank 2 (Confidant) up, not at 1 (Friend); a helper of friends and
   allies at every rank from 1 to 4. So rank 2 and up count as allies, 1 as a
   friend; `ShouldHelp` reads the same. Unobserved: the ranks between
   helper and fighter that are set one way only, and negative ranks.
2. **Crime.** The player's assaults and murders add crime gold and send the
   guards near by to arrest them (`known_ambiguities/crime.md`); the townsfolk who
   join an attack on the player do so through assistance alone, and stay
   hostile only for that fight.
3. **Friendly hits.** `iFriendHitCombatAllowed` (4 in Skyrim.esm) suggests friends
   forgive some hits before fighting back. Every hit starts a fight here. In the
   game 8 script-cast bolts from a faction friend landed and started none
   (above): whether spells, script casts or friends' hits don't count is open.
