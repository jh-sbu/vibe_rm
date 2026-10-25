# Knowledge and the uncalled remainder

Public pages consulted 2026-10-24 (Papyrus Index / community CK wiki text,
summarized), https://papyrus.bellcube.dev/skyrimse/script/ `ingredient/function/`
`learneffect` (0-based index), `learnnexteffect` (the next unknown effect, its
index returned), `learnalleffects`; `form/function/playerknows` (the "Known"
flag of a MagicEffect, WordOfPower or Enchantment; for an enchantment it is the
base one that counts); `utility/function/gametimetostring` ("MM/DD/YYYY HH:MM" on
a 24-hour clock from game days passed); `spell/function/preload` / `unload`
(preload or drop a spell's art; the page discourages them); `soundcategory/
function/mute`, `unmute`, `pause`, `unpause`, `setvolume` (0..1), `setfrequency`
(0..1); `utility/function/setinibool` etc. (only pre-defined settings in
Skyrim.ini / SkyrimPrefs.ini, only for the session, nothing written back);
`armor` / `light` / `actor` `getwarmthrating`. Vanilla calls only `Spell.Preload` /
`Unload` (5 each) and one `GetWarmthRating` each (Survival); the rest are for mods.

Implementation (`src/knowledge.rs`, `script_sounds.rs`, `audio/mod.rs`, `stats.rs`;
inference where marked). No new saved section: learned effects ride in the world
state's `learned_effects`:
- `LearnEffect` / `LearnNextEffect` / `LearnAllEffects` fill `learned_effects`
  (the ingredient's effects are its `EFID` list); an index past the list or a
  non-ingredient does nothing; `LearnNextEffect` gives -1 when all are known.
- `PlayerKnows`: a word of power is known when taught; a magic effect when some
  learned ingredient effect names it; enchantments are never known (nothing here
  teaches one); anything else is false.
- `GameTimeToString`: month, day and four-digit year from the calendar the
  journal's date uses (the game starts 17th of Last Seed, 4E 201; 365 days a
  year); the time of day from the fraction. A time past a thousand years is held
  to it, and a non-finite one is the beginning.
- `Spell.Preload` / `Unload`: nothing to do (art streams in when needed).
- `SoundCategory`: sounds started from a sound descriptor (`Sound.Play`, world
  and object sounds) are tagged with their `SNCT` category and each parent up to the
  master; the mixer then mutes, pauses (holds the voice where it is), scales the
  volume of and speeds up or slows down every voice in the category or below it.
  Dialogue, barks, music and weather loops are not tagged yet. The state is kept for
  the session only (inference: the mixer settings aren't saved). Volume and
  frequency are held to 0..1 (frequency at least 0.01).
- `Utility.SetINI*`: kept in memory by lower-cased name; any name is accepted
  (the game accepts only existing settings). Nothing reads them yet.
- `GetWarmthRating`: 0; warmth is part of Survival Mode, which does not exist here.

Tests: synthetic ingredient, effect, word and category records; mixer unit tests
of mute, pause, volume and frequency.
