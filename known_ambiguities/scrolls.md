# Scrolls

Implemented in `src/items.rs` (using a scroll from the inventory or favorites
puts it in the hand asked for, or away), `src/casting.rs` (`use_up_scroll`,
no magicka cost) and `src/spells.rs` (`equipped_spell` counts a held scroll).
Source: [UESP's Skyrim:Scrolls](https://en.uesp.net/w/index.php?oldid=3543120)
(revision 3543120).

## From the source

- Anyone can use a scroll whatever their skill; casting costs no magicka; each
  scroll is used once, and one cut short before the cast is not used and has to
  be started again; selecting it in the inventory (or favorites) assigns it to
  a hand, and some need both hands (taken from the scroll's equip slot, `ETYP`,
  as for spells); casting one gives no magic skill experience, apart from
  the Scroll of Blizzard.

## Choices

- A held scroll is an item (equipped and counted like a weapon), not a spell:
  `Actor.GetEquippedSpell` doesn't report it, and the "Favorite Spell" and
  "Favorite School" statistics don't count it (they count spells).
- The cast is the charge-and-release of a spell; dual casting isn't offered
  for scrolls. Automatic charge time uses effect casting times (see below).
- The Scroll of Blizzard gives no skill experience: the page gives no amount.
- The inventory marks a held scroll with the plain "equipped" icon, not a
  hand's.

## Boundaries inferred from SE observations and assets

The user's SE observations and asset cross-check are recorded in
`research/magic/scroll-casting-hold.md`. They supersede the original blanket
movement hold inferred from [UESP's Scrolls page](https://en.uesp.net/w/index.php?oldid=3543120):
Fireball permits walking; Blizzard ritual casting prevents it. Aiming and sneak
toggling work; sprint and jump are refused. These are observed behaviors, not
provisional choices. The remaining implementation decisions are:

- Identify ritual scrolls by an equip slot that requires both hands. Fireball
  and Blizzard's slots and the graph's RitualSpell state support this distinction,
  but every scroll has not been observed. Ordinary scrolls allow translation;
  ritual scrolls hold it through charging and the release clip's exit event.
- After an early ritual release, retain the hold for 0.3 seconds. The user
  observed an unwind shorter than 0.5 seconds; its exact duration is unknown.
  Item replacement cancels without an added recovery timer. When no animation
  graph is available, successful ritual release uses the same 0.3-second fallback.
  With a graph, the actual RitualSpellOut event ends the release hold.
- Refuse new scroll casts whenever the player is not grounded or is swimming.
  The observation specifically tested jumping, not every falling state; applying
  the check to all airborne starts is a conservative extension. Noclip bypasses
  the grounded check for developer verification. Physics is never frozen.
- For automatically calculated scroll charge times, use the greater of stored
  SPIT time and the longest effect's DATA CastingTime. [UESP's SCRL format](https://en.uesp.net/w/index.php?oldid=2334437)
  describes the longest-effect calculation. Retaining a longer authored SPIT
  value is a compatibility choice; conflicting values haven't been tested.
  Fireball becomes 0.5s; Blizzard remains 3s, consistent with qualitative SE
  observations. Animation delays remain separate from charge readiness.
- A concentration-tagged scroll whose available effects are all fire-and-forget
  is treated as a release cast. This explains observed Detect Life (four 15s
  FF effects). Other concentration scrolls retain their existing channeling
  behavior; Telekinesis hasn't been observed. Mixed/missing effect metadata
  preserves the existing cast type pending further evidence.

Precise cancellation timing and broader ritual/airborne coverage remain
uncertain; no deeper game instrumentation is required. General learned-spell
jump/swim restrictions and interruption by blows/shouts remain separate work.
