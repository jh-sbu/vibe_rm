# Speech checks

Research: `research/pex-natives/speech-batch11-checks.md`.

- **Settings the plugins lack.** `fBribeMoralityMult`, `fBribeNPCLevelMult`,
  `fIntimidateSpeechcraftCurve` and the five
  `fIntimidateConfidenceMult<Cowardly..Foolhardy>` aren't in Skyrim.esm, and
  no public source found gives their defaults (the CK wiki is unreachable from
  here). All are 1.0 for now. An SE probe could settle them: read
  `GetBribeAmount` and `WillIntimidateSucceed` for NPCs of known level,
  Speech, morality and confidence at several player levels.
- **The bribe formula's grouping.** UESP's line has unbalanced brackets; we
  read it as `NPC level * mult + player level - ((player Speech - NPC Speech)
  - 100) * fBribeSpeechcraftMult`. A negative base is taken as 0. The result
  passes through the player's "Mod Bribe Amount" perks (no vanilla perk uses
  it) and is rounded to the nearest gold.
- **`GetBribeSuccess`** is taken to mean the player carries at least the
  bribe, matching `FavorDialogueScript`'s own gold test. No source describes
  the condition.
- **`GetIntimidateSuccess`** is `WillIntimidateSucceed`.
- **Bribed and intimidated** are only flags (`IsBribedbyPlayer`,
  `IsIntimidatedbyPlayer`, saved). Whether either wears off, or changes the
  NPC's behaviour otherwise, is unknown.
- **Talking.** An actor whose race lacks "Allow PC Dialogue" can't be talked
  to unless `AllowPCDialogue(true)`; a bleeding-out actor only after
  `AllowBleedoutDialogue(true)`. Activating one does nothing, and the prompt
  still says "Talk" (the vanilla prompt for such actors isn't observed).
- **`<CrimeGold>`** is the total bounty with the speaker's crime faction;
  whether SE shows the violent and non-violent bounty together is unverified.
