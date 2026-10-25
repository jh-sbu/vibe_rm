# Story Manager spell cast events

`src/casting.rs` supplies the current supported `CAST` event's location.
NPC spell cast events remain blocked by the unresolved combat restriction
below; the existence of NPC combat casting is not sufficient evidence to
enable those events.

## Public evidence

- The Creation Kit's [OnStoryCastMagic documentation](https://ck.uesp.net/wiki/OnStoryCastMagic_-_Quest),
  also [indexed by the community Papyrus reference](https://papyrus.bellcube.dev/skyrimse/script/quest/event/onstorycastmagic/),
  lists the caster, spell target, location where the cast happened, and
  spell, in that order. It notes that the target always returns None.
  Accordingly the event uses R1 for the caster, R2 unset, L1 for the cast
  location and F1 for the spell; this follows the existing event member
  convention in `src/story.rs`.
- [Beyond Skyrim's World Interactions guide](https://wiki.beyondskyrim.org/wiki/Arcane_University%3AWorld_Interactions#Story_Manager_Events)
  describes Cast Magic interactions for spells cast on oneself, another
  actor or into the distance. An impact or a successful applied effect is
  therefore not required for this event's location payload.
- In a [2014 modder discussion](https://forums.nexusmods.com/topic/1760533-script-that-senses-when-shout-has-been-used/),
  lofgren reports that the Cast Magic Story Manager event fires only
  outside combat. The discussion concerns detecting player shouts and
  does not establish whether the restriction is on the player, caster,
  Story Manager processing or the particular quest conditions.
- A [2013 modder report](https://www.gamesas.com/starting-quest-through-story-manager-cast-magic-event-t312539.html)
  describes a quest failing to start after a calming spell on a hostile
  actor. It is consistent with a combat restriction, but cannot establish
  one because the quest setup and conditions may explain the failure.

## Implemented scope

- For the existing player ordinary-spell casts with `share >= 1`, L1 is
  resolved at dispatch with `ref_current_location(caster)`. The event
  carries no location when the current cell has none. The player's other
  location-changing and actor-location logic remains responsible for
  identifying the current location.
- From-event location aliases and `OnStoryCastMagic` receive this same
  location through the existing Story Manager pipeline. R2 stays unset,
  including for self spells; the cast event is separate from a hit event.

## Unresolved

- Which casts generate `CAST` for NPCs, including casts made by scripts,
  has not been established. The current NPC AI casts only during combat,
  so enabling its events would conflict with the reported combat
  restriction without further evidence, so NPCs send none.
- The existing player path also emits in combat. It is preserved here;
  reproducing the reported restriction needs evidence about whose combat
  state matters, whether searching counts, and whether suppression happens
  at event generation or in quest conditions.
- Concentration casts currently use fractional landing shares and do not
  emit this event through the existing player path. Public evidence here
  does not settle whether their event occurs at channel start, channel
  end or periodically. Individual damage/healing ticks must not be assumed
  to be independent Story Manager casts.
- Staff enchantments, scrolls, powers and shouts remain outside this
  path's ordinary-spell event policy. The public World Interactions guide
  describes shout reactions and the forum discusses shout events, so the
  current exclusion is a limitation, not a confirmed vanilla rule.

Only public documentation and community reports were consulted for this
change. No game assets or engine executable/binary data were examined or
added. Tests generate synthetic records in a temporary directory.
