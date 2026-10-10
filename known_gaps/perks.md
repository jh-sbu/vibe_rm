# Perks

Implemented in `src/perks.rs`, with the hooks in `src/ai/combat.rs`,
`src/detection.rs`, `src/locks.rs` and `src/pickpocket.rs`, and the perk trees in
the skills menu (`src/ui.rs`). Sources: CommonLibSSE's `BGSPerk`,
`BGSPerkEntry`, `BGSEntryPoint` and `BGSEntryPointPerkEntry` headers (entry
point and function numbers), `BGSSkillPerkTreeNode` (the `AVIF` tree fields),
UESP's PERK record format and skill pages, the records themselves (`vrm-tool
perks`).

## Settled

- **Record**: `DATA` (trait, level, ranks, playable, hidden), the conditions
  before the first section (what it takes to buy it), `NNAM` (the next rank),
  then sections `PRKE` (type, rank, priority) .. `PRKF`. Quest sections:
  `DATA` quest + stage; abilities: a spell; entry points: `DATA` (entry point,
  function, tab count), `PRKC` (tab index) before each tab's conditions, `EPFT`
  (1: one float, 2: two floats, 3+: leveled list, spell, text...) and `EPFD`.
- **Functions**: 1 set, 2 add, 3 multiply, 4 add range, 5 add actor value x
  factor, 6 absolute, 7 negative absolute, 12 set to actor value x factor, 13
  multiply by actor value x factor, 14 multiply by 1 + actor value x factor.
  The two-float functions' first float is the actor value's index
  (`PerkSkillBoosts`: 96.0 .. 107.0 with 0.01).
- **Owners**: the NPC record's `PRKR` (perk, rank), from the template giving
  the spell list (`template::SPELLS`); the player's from `NPC_ 00000007` (12
  perks: skill boosts, Allow Shouting, Well Fitted...).
- **The tree**: each node is `PNAM` perk, `FNAM`, `XNAM` / `YNAM` grid cell,
  `HNAM` / `VNAM` offsets, `SNAM` skill, `CNAM` children, ended by `INAM` (its
  index); the first node (no perk) is the root. Multi-rank perks are chains
  of records by `NNAM`, each with its own requirements (Stealth20: Sneak 20 and
  Stealth00).
- **Numbers checked** against UESP's perk descriptions (console `perkep`):
  Backstab 6x and Assassin's Blade 15x with daggers, Deadly Aim 3x with bows,
  Armsman 1.2x, Juggernaut 1.2x the heavy piece's rating (an iron cuirass goes
  from 27 to 32 here), Stealth 20..40%, Light Fingers +20, Novice Locks twice
  the sweet spot below Apprentice locks.

## Open (choices made here)

- **Priority order**: entries apply lowest priority first. The CK's meaning
  of the number isn't documented in a public source.
- **Ranks**: an entry applies while the owner's rank of the perk is above the
  entry's rank. The vanilla perks have one rank per record, so this is
  untested.
- **Tab subjects** as wired here (the tab count and the conditions used in
  the vanilla records fit them; the CK's tab names aren't in a public
  source): Mod Attack Damage, Mod Power Attack Damage, Mod Sneak Attack Mult,
  Mod Target Damage Resistance: owner, weapon, target. Mod Incoming Damage:
  owner, attacker, attacker's weapon. Mod Armor Rating: owner, armor piece.
  Mod Bashing Damage: owner, shield (else weapon). Mod Power Attack Stamina:
  owner, weapon. Mod Percent Blocked, Mod Detection Sneak Skill, Make Lockpicks
  Unbreakable: owner. Mod Detection Light / Movement: owner (the observer),
  target. Mod Lockpick Sweet Spot: owner, the locked reference. Mod Pickpocket
  Chance: owner, victim, item. Conditions on any tab also see the last subject
  as their target.
- **Where each number is changed** (the order in the game isn't published):
  attack damage, then power attack damage, then the sneak multiplier (itself
  through Mod Sneak Attack Mult); armor penetration scales the share the
  target's armor takes; Mod Incoming Damage after armor, before blocking;
  Mod Percent Blocked before the 85% cap; Mod Armor Rating on each piece's
  rating after the skill scaling, before rounding up.
- **Stealth perks**: Mod Detection Sneak Skill is a percentage taken off the
  observer's skill factor while the target sneaks (UESP: the Sneak skill and
  its perks reduce the noticer's skill factor).
- **Pickpocket perks** add to the chance before the 90% cap (UESP's formula);
  Keymaster sets 100, so keys come out at the cap.
- **Quest sections** set their stage when the perk is added (the CK wiki's
  description); nothing undoes it on removal.
- **Buying**: one perk point per rank, if the rank's conditions pass on the
  player. No refunds, no "Legendary" reset.
- **Ability sections** are spells the owner knows: constant ones take effect
  (`known_gaps/magic.md`).
- **Not done**: entry points without their
  systems: critical hits (1, 2), Calculate Weapon Damage (0), barter (8, 60),
  activate choices (14: Vampire Feed, cannibalism), spells applied on hits,
  bashes and swings (51, 52, 67), magic but Mod Spell Magnitude and Mod Incoming Spell Magnitude (30, 38...), crafting (66, 76,
  77...), Mod Skill Use (22), carry weight (10), falling damage (58), armor
  weight (32), staggers (33, 34), recovering arrows (21), bow zoom (20),
  Ignore Running During Detection (15), the lockpick starting arc and key
  rewards (63, 90). `IsAttackType` (697) has no attack to answer for, so
  entries asking it fail. Perks aren't saved (no saves).
