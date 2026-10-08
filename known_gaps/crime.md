# Crime

Implemented in `src/crime.rs` and `src/arrest.rs`; `vrm-tool crime-factions <data>` lists the
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

- Arrest (UESP Crime / Jail, CK wiki Faction): guards "will attempt to
  arrest you" in the hold where the player has a bounty and chase them; the
  faction's Arrest flag makes its guards arrest, Attack on Sight makes them
  attack once the bounty is high enough, and without Arrest they attack
  whenever there is a bounty. Resisting turns the guards hostile;
  cancelling the arrest dialogue is resisting. Paying a low bounty is on
  the spot, else the player is "transported outside the nearest town jail";
  either way stolen goods are confiscated. Jail: at most seven days, served
  for 700 or more; "sleep in a cell bed"; one lockpick kept; the rest kept
  in a chest by the cell.
- The arrest itself is data (`vrm-tool quest-topics <data>
  DialogueCrimeGuards`): blocking branches `DGCrimeForcegreet` (its start
  topic, subtype `PFGT`, on `GetAlarmed` and `GetCrimeGold`) and
  `DGCrimeBlockingHello` (the wanted player talking to a guard, "Wait... I
  know you"), the pursuit lines (`PURS`), and fragments calling
  `PlayerPayCrimeGold(true, GetAlarmed)`, `SendPlayerToJail(true, true)` and
  `SetPlayerResistingArrest`. The topic quest's condition is `IsGuard`.
- Guards: the hold guards are members of `IsGuardFaction` at rank 0 (their
  templates list it at -1); their class is the soldiers' `CWSoldierClass`.
  Four classes carry the class Guard flag (`CLAS` `DATA`'s last byte 0x1:
  `GuardImperial`, `GuardSonsSkyrim`, `GuardOrc1H` / `2H`).
- A faction's `JAIL` is a prison marker (a `DOOR` marker, base 0x4) linked
  by `XTEL` to its pair in the jail cell, like a load door; `PLCN` is the
  player's belongings chest, `STOL` the evidence chest, `JOUT` the jail
  outfit (`BeggarOutfit` in Whiterun), `WAIT` the follower wait marker. The
  holds' factions set Arrest and Attack on Sight (Winterhold Arrest only;
  `CrimeFactionImperial` Attack on Sight only).

- Witnesses (CK wiki *Crime*): "if the last Actor who viewed a crime is
  killed before guards arrive, the crime gold is removed"; an actor without
  a crime faction who witnesses a crime warns the player or starts combat.
  UESP: killing all the witnesses before they report it wipes the bounty,
  with a message; essential actors can't be. Strings `sWitnessKilled`
  ("Last witness killed."), `sAddCrimeGold` ("bounty added to"),
  `sRemoveCrimeGold` ("bounty removed from").
- Escaping jail (UESP *Skyrim:Jail*): "Simply activating an escape route
  or unlocking the door to a jail cell is considered a crime" (100, the
  factions' escape gold); "If you do escape, your bounty will remain"; the
  belongings stay in the evidence and belongings chests. The cell doors run
  `JailDoorScript`, which only makes a detection event (sound level 25).
- Reaction topics (`vrm-tool topic-lines <data> STEA`...): `STEA` ("Hey!
  Hands off!"), `ASSA` ("Help! I'm being attacked!" for victims by
  `GetActorValue Confidence` < 3, "Last mistake." from 3; "Help! Someone's
  being attacked!" for others, not guards), `MURD` ("Help! Murder!"), and
  the non-combat `STFN` ("I guess you can have that."), `ASNC` ("None of my
  business."), `MUNC` ("What's done is done."); the settings name `ASSA`
  `CombatAssault` and `ASNC` `CombatAssaultNC`.

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
- `IsGuard`: a member of `IsGuardFaction` or of a class flagged Guard.
- The guards of the faction told, within 2048 units of the player, awake
  and not fighting, come to arrest them (UESP: witnesses report to the
  guards); they run to the player calling a `PURS` line every 6 to 10
  seconds and the nearest within 160 units opens the blocking topic that
  passes. They give up when the bounty is gone, they start fighting or
  they unload.
- Attack on Sight from a bounty of 1000 (`ATTACK_ON_SIGHT_GOLD`; UESP's
  murder entry). Guards of a faction without Arrest attack at any bounty.
- Resisting (the fragment, or the arrest conversation ending without the
  bounty settled and not on a goodbye line, so "never mind" lets the player
  go) makes the faction's guards attack until it is paid; only guards, not
  UESP's "NPCs with high responsibility".
- Paying with "go to jail" sends the player through the jail's inner prison
  marker to the outer one (UESP: outside the jail); it is the alarmed
  guard's line that passes true.
- `SendPlayerToJail`: a day per 100 gold, one to seven; stolen goods to
  `STOL`, everything else but one lockpick to `PLCN`, the `JOUT` outfit
  worn, through the prison marker. Activating a bed in the jail's cell asks
  `sServeSentenceQuestion`; serving adds the days, gives back the chest's
  contents and lets the player out the same way. `GetDaysInJail` counts
  days served. `abRealJail` is ignored.
- Blocking branches (`DLBR` flag 0x2) are tried before `HELO` whenever the
  player talks to anyone, highest topic priority first.
- Escaping: unlocking a lock in the jail's interior (picked or with a key),
  or the player being anywhere else, while jailed. The bounty the sentence
  was for comes back without adding infamy, then the escape gold (with
  infamy, no witnesses needed), and the guards near by are alarmed.
  Picking the jail's locks while jailed isn't the lockpicking crime too.
- A crime's witnesses are those who reported it to a faction; it is
  unreported until a guard of that faction reaches the player to arrest
  them, a witness is itself one of its guards, or the bounty is paid or
  served. A witness leaving the loaded world has reached the guards (the
  crime stands); when all of them have died, its gold is withdrawn (not its
  infamy). Witnesses in bleedout aren't dead.
- One witness reacts with a line, the victim if it saw it, else the nearest:
  the combat topic when it reports or fights, the non-combat one otherwise.
  Pickpocketing and trespass keep their own lines (`PICN`, `TRES`).
- Witnesses who attack: wronged (the victim, or a member of the faction
  that owns what was stolen), with no crime faction that tracks crime,
  aggressive (aggression 1 or more) and not cowardly; the others only
  speak (the "warning").
- The console's `setcrimegold` / `paycrimegold` default to the crime faction
  of the nearest location up from the current one that names one (`FNAM`).

## Open questions

- Arrests: yielding by sheathing (the player has no drawn state), guards
  following the player through doors, escape routes (crumbling walls,
  sewers: as activators they count only once the player is out), the guards
  opening the cell after five hits, The Chill's escape without a bounty,
  skill progress
  lost in jail, the follower wait marker, Cidhna Mine and the Orc
  strongholds' `DialogueCrimeOrcs`, bribes, persuasion and Thane influence
  (perks, speech checks and quest variables), the bounty collector, and
  `GetArrestingActor` while being taken to jail. The jail outfit stays on.
- Horse theft (`iCrimeGoldStealHorse` 100) and werewolf crimes aren't committed anywhere.
- Crime responses: witnesses don't run to the guards (the guards near by
  hear at once), children don't tell the nearest adult (UESP), morality
  plays no part (UESP: it is for followers ordered to commit crimes), the
  `PICC` lines aren't used; pickpocketing perks (Light Fingers, Night Thief,
  Cutpurse, Misdirection, Perfect Touch, Poisoned), skill gain and the
  thugs hired after a theft aren't there.
- Trespassers aren't attacked or chased out after "Guards! Trespasser!", the
  "Warn To Leave" flag (`DATA` 0x200, unused in the base game) does nothing,
  the warning lines near restricted places ("That's close enough", guard
  posts) need guard packages, and `sNoWaitTrespass` / `sNoSleepTrespass`
  wait for waiting and sleeping. Sleepers don't wake up to warn.
- Merchants refusing stolen goods and fences buying them: no bartering yet
  (`GetAmountSoldStolen`).
- Crimes by NPCs; `GetCrime`, `IsGuard`, `GetActorCrimePlayerEnemy`; the
  locations' unreported crime faction otherwise.
- Bounties aren't saved.
