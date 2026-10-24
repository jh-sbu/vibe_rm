# Essential, protected, invulnerable and ghost actors

## Public sources

- UESP [Skyrim Mod:Mod File Format/QUST, revision 3507775](https://en.uesp.net/w/index.php?oldid=3507775),
  Aliases: reference alias `FNAM` 0x40 "Essential (makes the alias target
  essential for the duration of the quest)" and 0x400 "Protected".
- UESP [Skyrim Mod:Mod File Format/NPC, revision 3650076](https://en.uesp.net/w/index.php?oldid=3650076):
  `ACBS` flags 0x02 Essential, 0x800 Protected, 0x20000000
  "Ghost/non-interactable (Ghosts, Nocturnal)", 0x80000000 Invulnerable.

## Quest aliases' flags

- A reference filling a reference alias with `FNAM` 0x40 (0x400) of a
  running quest is essential (protected), whatever its base says; clearing
  the alias or stopping the quest ends it. Nothing is stored: the flags are
  read from the current fills, as alias factions are. `IsEssential`,
  `IsProtected`, `Kill`, damage and bleeding out all read the same answer.
- The player-only rule for the protected is the base flag's: the player can
  kill them, others only bring them down.
- Unconfirmed: whether "for the duration of the quest" means while the
  alias is filled (our reading) or until the quest stops even if the alias
  is cleared, and whether an alias flag counts for a quest that has started
  but whose start-up stage hasn't run. Whether a flagged alias pointing at a
  base that is already essential changes anything (we answer no).

## Scripts changing the base's flags (`SetEssential`, `SetProtected`, `SetInvulnerable`)

- `ActorBase.SetEssential`, `SetProtected` and `SetInvulnerable` (vanilla
  `ActorBase.pex`; 134, 34 and 64 call sites, `vrm-tool pex-natives`) set or
  clear the base's `ACBS` flag, true if the argument is missing; `IsEssential`,
  `IsProtected` and `IsInvulnerable` on the base read it, and
  `Actor.IsEssential` reads the actor's answer (base or alias). The change is
  the base's, so every actor of that base follows it, loaded or not, and it
  wins over a template's flags. Saved in optional `BFLG`.
- CommonLibSSE keeps essential and protected on the actor as well
  (`Actor::BOOL_FLAGS` `kEssential` 1 << 18, `kProtected` 1 << 19), with no
  rule for when they are copied from the base. We read the base each time,
  so a change reaches actors already loaded. Unconfirmed: whether a loaded
  actor only follows its base's change after it reloads.

## Invulnerable

- `ACBS` 0x80000000 (CommonLibSSE `TESActorBaseData::kInvulnerable`,
  `GetInvulnerable`), from the record or `SetInvulnerable`: nothing lowers
  the actor's health, neither blows, spells and falls (`damage`) nor
  `DamageActorValue("Health")`; raising it still works. The player's base
  (`0x7`) counts: the vampire lord's script makes the player invulnerable
  while transforming (UESP talk page
  [Skyrim talk:Vampire Lord/Archive 2, revision 2224214](https://en.uesp.net/w/index.php?oldid=2224214)
  quotes it, beside a commented-out `SetGhost`).
- Unconfirmed: whether `DamageActorValue` and `ForceActorValue` bypass it,
  whether blows still stagger and count as assaults (we drop the blow
  entirely), and whether stamina and magicka are spared too (we spare health
  only).

## Ghosts (`SetGhost`, `IsGhost`, `GetIsGhost`)

- An actor is a ghost when `Actor.SetGhost` made it one (true if the
  argument is missing), else when its base has `ACBS` 0x20000000 (UESP:
  "Ghost/non-interactable (Ghosts, Nocturnal)"). The script's choice is the
  reference's alone and wins over the base, as CommonLibSSE's `ExtraGhost`
  (a per-reference `bool ghost`) suggests. `Actor.IsGhost` and `GetIsGhost`
  (237) read it. Saved in optional `GHST`; vanilla calls `SetGhost` 222
  times.
- A ghost takes no damage from blows, spells or falls (`damage`).
  `DamageActorValue` still lowers its health.
- Sources for what a ghost is are Oblivion's: UESP
  [Oblivion Mod:PCS Library/NPCs cannot hurt the player character, revision 2269838](https://en.uesp.net/w/index.php?oldid=2269838)
  (a ghost player can't be hurt) and
  [Oblivion Mod:Resurrecting the Player, revision 2308496](https://en.uesp.net/w/index.php?oldid=2308496)
  ("can be used to make the player invulnerable (in most ways)", and a ghost
  "should be ignored by enemies"). Not Skyrim evidence.
- Unconfirmed in SE: whether blows pass through ghosts' collision or land
  without effect, whether enemies ignore or stop targeting ghosts (we leave
  targeting alone), whether ghosts can be activated or talked to
  ("non-interactable"), whether `DamageActorValue` hurts them, and whether a
  base flag changed by script would apply (no native sets it).
