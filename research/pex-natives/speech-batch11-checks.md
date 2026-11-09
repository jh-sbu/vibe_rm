# Speech checks and who can be talked to

Natives: `Actor.GetBribeAmount`, `WillIntimidateSucceed`, `SetBribed`,
`IsBribed`, `SetIntimidated`, `IsIntimidated`, `AllowPCDialogue`,
`AllowBleedoutDialogue`, `IsUnconscious`. Conditions: `IsIntimidatedbyPlayer`
(116), `IsBribedbyPlayer` (402), `GetBribeAmount` (653), `GetBribeSuccess`
(654), `GetIntimidateSuccess` (655). Implemented in `src/speech_checks.rs`.

## Sources

- The Papyrus Index (papyrus.bellcube.dev, consulted 2026-11-09), pages for
  each native; descriptions attributed to the Creation Kit wiki (CC BY-SA 4.0):
  - `GetBribeAmount`: "the amount of gold required to bribe this actor".
  - `WillIntimidateSucceed`: whether the player's intimidation of the actor
    will succeed; points to the "Mod Player Intimidation" perk entry point.
  - `SetBribed(abBribe = true)` / `IsBribed`: flags the actor as bribed by
    the player.
  - `AllowPCDialogue(abTalk)`: whether the actor can be talked to,
    "overriding the race flag".
- UESP [Skyrim:Speech](https://en.uesp.net/w/index.php?oldid=3627051),
  "Bribe Formula" and "Intimidate Formula" (source claims):
  - NPC morality = Morality AV * `fBribeMoralityMult` + 1.
  - v = NPC level * `fBribeNPCLevelMult` + player level
    - ((player Speech - NPC Speech) - 100) * `fBribeSpeechCraftMult`.
  - bribe = v ^ `fBribeCostCurve` * (`fBribeScale` * NPC morality).
  - Player's scariness = player level * (1 + max(-1, (player Speech - NPC
    Speech) / 100)) ^ `fIntimidateSpeechcraftCurve`, modified by "Mod Player
    Intimidation"; NPC's = NPC level * `fIntimidateConfidenceMult<Confidence>`.
- UESP [Skyrim talk:Speech](https://en.uesp.net/w/index.php?oldid=2848144),
  "Intimidate formula": a community transcription of the 2011 release's
  routine (public community research). It agrees with the page and adds that
  the comparison is NPC scariness <= player scariness, and that the levels are
  the actor-base levels with "PC level mult" applied.
- xEdit `wbDefinitionsTES5.pas`: entry points 43 "Mod Player Intimidation" and
  46 "Mod Bribe Amount"; race `DATA` flag 0x00200000 "Allow PC Dialogue".
- `vrm-tool gmst`: Skyrim.esm defines only `fBribeCostCurve` (1.4),
  `fBribeScale` (0.5) and `fBribeSpeechcraftMult` (0.1). The morality and
  level multipliers and the intimidation settings are not in the plugins.

## Game-asset observations

- `vrm-tool ctda-uses ... 654 655`: about thirty vanilla INFOs test
  `GetBribeSuccess == 1` or `GetIntimidateSuccess == 1` (Nelacar, Verulus,
  the Orc strongholds' suitors, Riften's gate guard and others), one tests
  `GetIntimidateSuccess == 0` for the failure line.
- `FavorDialogueScript` (`vrm-tool pex-dump`): `Bribe` compares
  `GetBribeAmount` with the player's `GetGoldAmount`, removes that much gold,
  and calls `SetBribed(true)`; `Intimidate` calls `SetIntimidated(true)`.
- The English strings files hold 37 prompts with `<BribeCost>` (any case),
  five with `<CrimeGold>` ("I'll pay off my bounty. (<CrimeGold> gold)"). We
  replaced neither before; dialogue prompts now go through
  `Engine::dialogue_text`.
- Race flags read from Skyrim.esm: Nord, Elder, Dremora and Dog races allow
  dialogue; Wolf, Horse, Dragon, Giant, Falmer and Chicken races don't.

Inference and choices are in `known_ambiguities/speech-checks.md`.
