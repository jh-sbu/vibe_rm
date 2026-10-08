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
- Stolen items (UESP Crime): marked stolen for good; confiscated into the
  jail's evidence chest (the faction's `STOL`) on paying or going to jail.
  The prompts are `sSteal` / `sStealFrom` (red). The player keeps a value
  stolen per faction, unwitnessed and witnessed (CommonLibSSE
  `StolenItemValueStruct`), which `GetStolenItemValueNoCrime` and
  `GetStolenItemValue` read (no condition in the base game uses them).

- Trespass (UESP Crime): one warning to leave, then a bounty (5) "after 30
  seconds" if the player lingers, NPCs calling the guards or attacking;
  some areas give the bounty at once. Cells: "Public Area" (`DATA` 0x20):
  nobody is ever trespassing there; "Off Limits" (record flag 0x20000,
  CommonLibSSE `TESObjectCELL`): caught trespassing is a crime at once,
  without the warning (GECK / CK wiki, through search excerpts). Community
  research (Nexus "Trespassing Mechanic Research Findings", the Fandom wiki):
  trespass comes from packages with the Lock Doors option; once the owner
  locks up, the cell is private and being there is trespassing; public
  cells still lock their doors. The Trespass topic (`TRES`, in
  `DialogueGeneric`) has lines by `GetTrespassWarningLevel` 0 (leave), 1
  (last warning), 2 ("Guards! Trespasser!"), on `IsTrespassing` of the
  player; `fAITrespassWarningTimer` is 5 in Skyrim.esm. `vrm-tool
  trespass-cells` counts interiors by these flags (8 off limits, none warn
  to leave); `ctda-uses <data> 144 145` lists the lines.

- Pickpocketing (UESP Skyrim:Pickpocket): only while sneaking; chance `15 +
  skill - target skill / 4 - 4 x weight - gold / 10`, less 25 when the
  target detects the player, at most 90; a failure is detected by the target
  and reported (25 gold). The game settings give the same numbers as
  `fPickPocketActorSkillBase` 20 + `ActorSkillMult` 1 x skill +
  `TargetSkillMult` -0.25 x (`TargetSkillBase` 20 + target skill) +
  `WeightMult` -4 x weight + `AmountMult` -0.1 x gold, between
  `fPickPocketMinChance` 0 and `MaxChance` 90. Strings `sPickpocket` (red
  prompt), `sPickpocketFail`, `sNoPickPocketAgain`, `sInvalidPickpocket`.
  Topics: `PICN` (non-combat: "I guess I can look the other way, this
  time.") and `PICC` (combat; victims and witnesses by `IsActorAVictim`).

## Choices made without a source

- Witnesses: living loaded actors who detect the player at the time
  (detection above 0, without stealth points; `known_gaps/detection.md`).
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
- Stolen goods carry their owner in the inventory (`Inventory::owned`,
  standing for the game's ownership on the stack) and keep it wherever the
  player puts them, unless they go back to the owner; taking from a container
  judges the theft by the container's owner, so a stolen item in the player's
  own chest isn't stolen again. Removing items by script takes those
  belonging to nobody first.
- Paying with "remove stolen" takes every stolen item, whoever it was stolen
  from, and forgets the stolen value with that faction. The value stolen
  counts against the owner's crime faction (or the owning faction).
- Lockpicking (UESP: 5 gold, "even if nothing was taken") has no crime type
  of its own (`sCrimeType*` names steal, pickpocket, trespass, attack,
  murder, escape, werewolf), so it is reported as trespass, whose default gold
  is the same. It is committed on starting to pick a lock whose door,
  container or load door's far side someone else owns, once per attempt.
- Trespassing: an interior that isn't a public area, either off limits or
  owned by someone other than the player (and their factions) while an
  actor whose home it is (where its lock-doors sleep package puts it) runs
  a package that locks doors. Who owns it isn't otherwise checked.
- The warner is whoever in the cell detects the player most, awake and not
  fighting; level 0, then 1 and 2 each `fAITrespassWarningTimer` later and
  once someone sees the player again, the crime reported (by all witnesses,
  as any crime) at level 2 against the cell's owner. UESP's 30 seconds
  isn't matched: it is 10 by the setting. Leaving the cell starts it over;
  after level 2 nothing more happens there.
- Pickpocketing: equipped items can't be taken (UESP: the Perfect Touch
  perk allows it); taking several at once counts their weight (or the gold)
  together; UESP's "Sneak_bonus" is left out (not explained). Taken items
  are the victim's, stolen (gold never is marked); the add item event says
  pickpocketed (3). Caught, the menu closes, the crime is reported with the
  victim as a witness, the victim says `PICN` (`IsActorAVictim` true for it)
  and won't be pickpocketed by the player again (for the session). Being
  caught doesn't start a fight.
- The console's `setcrimegold` / `paycrimegold` default to the crime faction
  of the nearest location up from the current one that names one (`FNAM`).

## Open questions

- No arrests: guards don't come to the player, there is no jail, no
  "attack on sight" (`CRVA` flag) and the arrest dialogue
  (`DialogueCrimeGuards`) only sees the gold through its conditions.
- Horse theft, escape and werewolf crimes aren't committed anywhere.
- Crime responses: victims and witnesses don't attack (the `PICC` lines,
  morality / aggression); pickpocketing perks (Light Fingers, Night Thief,
  Cutpurse, Misdirection, Perfect Touch, Poisoned), skill gain and the
  thugs hired after a theft aren't there.
- Trespassers aren't attacked or chased out after "Guards! Trespasser!", the
  "Warn To Leave" flag (`DATA` 0x200, unused in the base game) does nothing,
  the warning lines near restricted places ("That's close enough", guard
  posts) need guard packages, and `sNoWaitTrespass` / `sNoSleepTrespass`
  wait for waiting and sleeping. Sleepers don't wake up to warn.
- Merchants refusing stolen goods and fences buying them: no bartering yet
  (`GetAmountSoldStolen`).
- A crime gets its gold at once; in the game it is withdrawn when the last
  witness dies before reporting it (CK wiki), and guards respond to alarms.
- Crimes by NPCs; `GetCrime`, `IsGuard`, `GetActorCrimePlayerEnemy`; the
  locations' unreported crime faction otherwise.
- Bounties aren't saved.
