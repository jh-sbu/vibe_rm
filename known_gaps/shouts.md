# Shouts

Implemented in `src/shouts.rs` (words, shouts, the player's voice), with
the shout's spell cast through `src/casting.rs`, the magic menu's shouts in
`src/swf_ui/magic_menu.rs` and the HUD's shout meter in `src/swf_ui.rs`.
Sources: UESP's Shouts and Dragon Shouts pages; the `SHOU` / `WOOP`
records; the game settings; `magicmenu.swf` (the shout card) and
`hudmenu.swf` (`SetShoutMeterPercent`, `FlashShoutMeter`).

## Settled

- **Records**: `SHOU` `SNAM` (word, spell, recovery seconds) for up to
  three words; `WOOP` `FULL` (the dragon word) and `TNAM` (its meaning).
  Unrelenting Force: Fus / Ro / Dah, 15, 20 and 45 seconds.
- **Holding the key**: one word under `fShoutTime1` (0.2 s), two under
  `fShoutTime2` (0.9 s), three after, no more than are unlocked.
- **Recovery** is the last word's recovery time times
  `ShoutRecoveryMult` (base 1).
- **Scripts learn and unlock words** with `Game.TeachWord` /
  `Game.UnlockWord` (word walls, MQ104B's Fus); no vanilla script gives the
  player a shout with `AddShout`, so knowing a word gives its shout.
- **The magic menu's shout card** (type 10): `dragonWord0..2`,
  `word0..2`, `unlocked0..2`, `effects`, `spellCost` (shown as RECHARGE),
  `showUnlocked` (the Unlock button), `soulSpent` (plays the word being
  learned after `DragonSoulSpent`); the bottom bar's `dragonSoulText`
  (`sSingleDragonSoulCount` / `sMultipleDragonSoulCount`). Unlocking is on
  the Ready Weapon key (R); without a soul, `sFailSpendSoul`.

## Open (choices made here)

- **When the shout goes out**: when the key comes up, or as soon as it has
  been held long enough for every word unlocked.
- **The card**: words not known show nothing (neither the dragon word nor
  its meaning); the description is the shout's `DESC`; RECHARGE is the
  recovery of the last word unlocked.
- **Spending a soul** unlocks at once: the game first asks "Spend Soul?"
  (`sConfirmSpendSoul`), not shown yet.
- **Cones** (Unrelenting Force's projectile): every live actor whose body's
  middle is within the projectile's range and its cone spread (`PROJ`
  `DATA` 0x44, 15 degrees, taken as either side of the middle) with nothing
  solid between. The push is the effect's stagger; nobody is thrown.
- **NPCs' shouts**: their record's (`SPLO`) and `AddShout`'s count for
  `HasShout`; NPCs don't shout yet.
- **Stats**: Words Of Power Learned / Unlocked on each word, Shouts Learned
  on a shout's first word known, Shouts Unlocked on its first word
  unlocked, Shouts Mastered when all are unlocked, Times Shouted. Favorite
  Shout isn't counted.
- **The Story Manager's new voice power event** (`NVPE`) isn't sent: what
  starts it and its members aren't published.
- **Nothing is saved**: words, shouts and the voice's rest.
