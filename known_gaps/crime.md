# Crime

Implemented in `src/crime.rs`; `vrm-tool crime-factions <data>` lists the
factions that track crime with their crime gold, crime group, jail and stolen
goods chest, how many NPCs name each as their crime faction and how many
locations name it.

## What is known

- Records (UESP `FACT`, CommonLibSSE `TESFaction`): `DATA` flags 0x40 track
  crime, 0x80 / 0x100 / 0x200 / 0x400 / 0x2000 / 0x10000 ignore murder /
  assault / stealing / trespass / pickpocket / werewolf, 0x800 do not report
  crimes against members, 0x1000 crime gold uses defaults; `CRVA` arrest,
  attack on sight, murder, assault, trespass, pickpocket gold, steal multiplier,
  escape and werewolf gold; `CRGR` the crime group (a form list), `JAIL`,
  `WAIT`, `STOL`, `PLCN`, `JOUT`. NPCs' crime faction is `CRIF`; locations
  name an "unreported crime faction" (`LCTN` `FNAM`: the nine holds).
- Default crime gold (UESP Crime): murder 1000, assault 40, trespass 5,
  pickpocket 25, stealing half the item's value rounded down, escape 100,
  werewolf 1000. The holds' factions say to use the defaults and carry the
  same numbers in `CRVA`. Violent: assault, murder, werewolf; the rest are
  non-violent.
- Crimes are reported to an actor's crime faction, only when witnessed by one
  that reports crimes (CK wiki "Crime"); "ignore crimes against non-members":
  a crime of a flagged type against a member of another crime faction isn't
  reported (CK wiki "Factions Tab"). The player's crime gold per faction is
  violent and non-violent, plus infamy (CommonLibSSE `CrimeGoldStruct`).
- Kill events' V1 (CK wiki "Kill Actor Event"): 0 not murder or no crime
  faction, 1 murder not reported, 2 reported; V2 the relationship rank
  between killer and victim before the death.
- Papyrus (`Faction`): `GetCrimeGold` (both kinds), `GetCrimeGoldViolent` /
  `NonViolent`, `ModCrimeGold(amount, violent)`, `SetCrimeGold` (non-violent),
  `SetCrimeGoldViolent`, `GetInfamy*`, `PlayerPayCrimeGold(removeStolen,
  goToJail)`, `CanPayCrimeGold`; `Actor.Get` / `SetCrimeFaction`.
- Conditions: `GetCrimeGold`, `GetCrimeGoldViolent`, `GetCrimeGoldNonviolent`
  (the faction, else the subject's crime faction), `GetIsCrimeFaction`,
  `CanPayCrimeGold`, `GetInSharedCrimeFaction`.

## Choices made without a source

- Witnesses stand in for detection, which isn't implemented (roadmap:
  Detection). None of these numbers has a source: living loaded actors within
  1400 units (combat's detection distance, itself unsourced), a third of that
  while the player sneaks (invented), facing the player within 95 degrees
  unless closer than 200 (invented), with nothing between their eyes (110
  units up, as for finding bodies) and the player's. No light, sound, sneak
  skill or perception; the game's settings for them (`fSneakMaxDistance`
  2500, `fSneakExteriorDistanceMult` 2.1, `fSneakLight*`...) aren't used yet.
  The victim of an assault always knows of it.
- Each crime faction told adds its gold once, however many of its members saw.
  Whether a witness reports to its own crime faction (done here) or to the
  victim's is not settled by the sources found.
- "Do not report crimes against members": members report crimes against
  members only when they are the victim.
- Assault: the player's first blow on someone calm who keeps the law (see the
  Story Manager's `ASSU`). Murder: the player killing someone they assaulted.
- `GetCrimeGold` with no faction on the player sums every faction.
- `PlayerPayCrimeGold` takes what gold the player has towards the bounty and
  clears it, enough or not.
- The console's `setcrimegold` / `paycrimegold` default to the crime faction
  of the nearest location up from the current one that names one (`FNAM`).

## Open questions

- No arrests: guards don't come to the player, there is no jail, no
  "attack on sight" (`CRVA` flag) and the arrest dialogue
  (`DialogueCrimeGuards`) only sees the gold through its conditions.
- Pickpocketing, trespassing (`fAITrespassWarningTimer`), lockpicking owned
  locks, horse theft, escape and werewolf crimes aren't committed anywhere.
- Stolen items aren't marked in the inventory (`GetStolenItemValue*`), nor
  taken away when paying.
- A crime gets its gold at once; in the game it is withdrawn when the last
  witness dies before reporting it (CK wiki), and guards respond to alarms.
- Crimes by NPCs; `GetCrime`, `IsGuard`, `GetActorCrimePlayerEnemy`; the
  locations' unreported crime faction otherwise.
- Bounties aren't saved.
