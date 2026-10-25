# Followers and the player's teammates

Implemented in `src/followers.rs` (the flag, who answers for a blow), with
`src/ai/combat.rs` (`fight_to_join`), `src/script/natives.rs`
(`SetPlayerTeammate`, `IsPlayerTeammate`) and `src/condition/mod.rs`
(`GetPlayerTeammate`, `GetPlayerTeammateCount`).

## What is known

- Scripts: `Actor.SetPlayerTeammate(bool abTeammate = true, bool abCanDoFavor =
  true)` and `Actor.IsPlayerTeammate()` (Papyrus API). The game's own scripts
  call the setter for every follower they hire or dismiss (`vrm-tool pex-calls
  <data> SetPlayerTeammate`: `DialogueFollowerScript` with `(true, true)` for a
  person and `(true, false)` for an animal, the Companions' and Dark
  Brotherhood's housekeeping, the Dawnguard's thralls) and read it
  (`defaultActivateSelf` and the Dawnguard's ambush trigger let a teammate set
  them off like the player; `RPDefault_OnAggro` and kin ask it of their
  target).
- Conditions: `GetPlayerTeammate` (453, on the subject) and
  `GetPlayerTeammateCount` (454), UESP *Mod File Format/Raw Function Data*
  (https://en.uesp.net/w/index.php?oldid=3411473); 157 lines of dialogue in the
  game's data test one of them (`vrm-tool ctda-uses <data> 453 454`).
- CommonLibSSE: `Actor` has a flag `kPlayerTeammate`; `PlayerCharacter` keeps a
  `teammateCount`.
- Crime (UESP *Skyrim:Crime*, https://en.uesp.net/w/index.php?oldid=3639929,
  Assault): "Attacks by followers, summoned, commanded, or reanimated allies
  also count as assault", and "if you ask a follower to commit a witnessed crime
  you will be held responsible".
- Carry limit (UESP *Skyrim:Carry Weight*, oldid=3630670, "Circumventing":
  "Followers and Spouses ... have their own limit that cannot be exceeded",
  about 290 units; *Skyrim:Followers* has no figure). The limit is the
  follower's `CarryWeight`, from its race (`DATA` offset 48: Nord 300, dog 200
  in the game's data), the same number the player's is.
- The refusal is the game's GMST `sTeammateOverencumbered` = "can't carry any
  more." (`vrm-tool gmst`; UESP *Skyrim Mod:Mod File Format/GMST/Defaults*
  lists it). Its lowercase start suggests the follower's name comes first.
- Followers help by faction: `CurrentFollowerFaction` is an ally of the
  player's factions in the data (see `combat-assistance.md`).

## Choices made without a source

- **A teammate takes the player's side in a fight**: it joins any fight it sees
  the player in (`fight_to_join`), against the player's attacker, whatever its
  factions or AI assistance say, unless the target is its own ally or friend.
  No public source says what else the flag changes in combat (friendly fire
  tolerance, following orders, fleeing); none of it is done.
- **The player answers for a teammate's or a summon's first blow** on someone
  calm who keeps the law, as for their own: the assault is a crime reported to
  the victim's crime faction if witnessed. The witnesses are those who detect
  the *player* (not the follower). A teammate's kill is not the player's
  murder (the UESP page doesn't say), and does not count among the player's kills.
- The teammate count is the flagged actors that are alive, loaded or not.
- A teammate given armor or a shield by `AddItem` wears it when it is better,
  as for weapons (`test-mod autotest2` saw a teammate wear a cuirass).
- **A teammate takes no more than its carry weight**: `Engine::transfer_item`,
  which the item menus, the fallback UI, gifts and pickpocket placing share,
  refuses a stack from the player when the teammate's carried weight plus the
  stack's would exceed its `CarryWeight` (equal fits, as for the player's
  "over" test). The refusal is the whole stack, so a quantity that doesn't
  fit moves nothing; the notice is "<name> can't carry any more.". A
  teammate already over its limit refuses even weightless stacks. Dead
  teammates are bodies and take anything (UESP's dead drops). Unconfirmed:
  UESP's Carry Weight page says that when a fortified follower's original
  limit is exceeded "only one item or stack of items can be added", which may
  mean the game checks only the weight before a transfer, so one stack can
  cross the limit. Taking a fitting part of a stack (partial move) is not
  described anywhere. Manual test: give a Nord follower 300 of weight in a
  chest, then a stack that crosses 300 and one that fits; compare the notice and
  the counts.

## Travel and load-door continuity

Local UESP [Followers revision 3646982](https://en.uesp.net/w/index.php?oldid=3646982),
Notes, describes fast travel and zone changes bringing stuck followers back.
The shared party hooks now include living loaded teammates whose active goal
is Follow targeting the player. Waiting, dismissed/nonteammate and dead actors
are excluded; summons retain their existing inclusion and duplicates are removed.
The normal move path retains destination placement in WRLD. Exact attachment,
combat and disabled-actor behavior is not verified in SE.

Optional native TEAM retains teammate/canDoFavor flags by replacement; old saves
clear them rather than keeping stale membership. Original `.ess` representation
is not established. Tests use synthetic actor runtimes and no visual assets;
spawn/animation attachment is not claimed to have been visually validated.
