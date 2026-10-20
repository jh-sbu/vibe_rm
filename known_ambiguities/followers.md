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
- `canDoFavor` is kept but nothing asks for it yet.

## Not done

- Teammates in fast travel, waiting, sleeping and load doors (the engine moves the
  player's summons through load doors; teammates are not moved), dismissal
  by death, the follower packages' follow distances (`escort.md`), and the
  favor menu.
