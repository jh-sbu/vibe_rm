# Bow ammunition selection and crossbow groundwork

`src/ai/archery.rs` checks ammunition compatibility before selecting equipped
arrows, inventory arrows, or an NPC's default iron arrows. Previously, an
equipped bolt bypassed the arrow-only inventory search and could be fired by
a bow. A malformed equipped ammunition record could also suppress usable
arrows. Synthetic-record tests cover both paths without game assets.

## Public evidence

- Local UESP [Skyrim:Ammunition, revision 3607741](https://en.uesp.net/w/index.php?oldid=3607741),
  introduction and Arrows/Bolts sections: bows require arrows and crossbows
  require bolts.
- Local UESP [Skyrim Mod:Mod File Format/AMMO, revision 2645195](https://en.uesp.net/w/index.php?oldid=2645195),
  `DATA` layout: projectile FormID, flags, damage; flag `0x00000004` is
  Non-Bolt. The layout places flags at byte 4 and damage at byte 8 in both
  the documented original and SSE forms.
- Local UESP [Skyrim:Archery, revision 3575828](https://en.uesp.net/w/index.php?oldid=3575828),
  Notes: equipping a bow defaults to the weakest arrows, and the player may
  manually select stronger ones; crossbows have the analogous bolt behavior.
  Ammunition can remain equipped when its weapon is unequipped.

The local index was consulted first. Only public wiki text was used for this
change; no game assets or engine binary data were examined or added.

## Current policy and unresolved behavior

- An equipped, carried, valid arrow takes precedence over the strongest
  carried arrow. This retains the existing selection policy, while excluding
  bolts on every path. If several compatible arrows are marked equipped, the
  first in inventory equipment order wins. That tie rule is a local fallback,
  not verified Skyrim behavior.
- The player's existing fallback picks the strongest carried arrow at shot
  time. UESP's weakest-ammunition auto-equip rule describes weapon equipping,
  a different transition; that auto-equip behavior is not implemented here.
  Exact behavior when compatible ammunition is already equipped, when only
  incompatible ammunition is equipped, or when an arrow stack is exhausted
  still needs public documentation or permitted asset evidence before changing
  the equipment lifecycle. The source-backed weakest-ammunition default remains
  future work, rather than being treated as an unknown rule.
- NPCs without usable arrows retain the existing iron-arrow fallback. A mod
  changing that record into a bolt no longer permits a bow to fire it. The
  general projectile reader still reads both arrows and bolts; the restriction
  belongs to bow selection.
- Crossbow firing, reload animation events and cadence, default NPC bolts, and
  crossbow-specific effects remain roadmap work. No firing or reload timing is
  inferred from the ammunition flag. This change establishes bow compatibility
  only and does not claim to implement crossbow combat.
