# Relationships, factions and followers

Public pages consulted 2026-10-24 (Papyrus Index / community CK wiki text,
summarized), https://papyrus.bellcube.dev/skyrimse/script/ `actor/function/`
`iscommandedactor`, `setdoingfavor` / `isdoingfavor` ("flag this actor as
currently doing a favor for the player"), `hasfamilyrelationship(akOther = None)`,
`hasparentrelationship(akOther)`, `hasassociation(akAssociation, akOther = None)`
(none means "with anyone"), `getfactionreaction(akOther)` (0 neutral, 1 enemy,
2 ally, 3 friend), `ishostiletoactor` (no description), `setoutfit` and
`actorbase/function/setoutfit` ("change the default or sleep outfit for this actor
base").

Implementation (inference where marked):
- `IsCommandedActor`: has a commander (`magic.commanded`: summons-like commanded
  and reanimated actors).
- `SetDoingFavor` / `IsDoingFavor`: a flag, saved as `FAVR`. Nothing asks
  followers for favors yet (ROADMAP), so setting it enters no command mode.
- Family, parent and association queries: the existing authored-`RELA` index
  (`src/relationships.rs`), with a missing argument meaning anyone.
- `GetFactionReaction`: `faction_relation` (strongest combat reaction among the
  two actors' factions; the same faction is allies), as the `GetFactionRelation`
  condition reads it.
- `IsHostileToActor`: the combat `would_attack` test (aggression, faction
  reaction, attacked on sight); false unless both are loaded and alive (inference).
- `ActorBase.SetOutfit(outfit, sleep)`: stored for the NPC record that supplies the
  actors' inventory (`Globals::base_outfits`, saved as `BOFT`). Every actor of it
  without a reference-level outfit that already has an inventory swaps its armor as
  `Actor.SetOutfit` does; actors made afterwards start with it. A reference-level
  outfit wins. Sleep outfits are ignored, as for actors. This settles the
  `known_ambiguities/outfits.md` item about base versus reference.

Tests: synthetic RELA / ASTP / FACT / OTFT records; `FAVR` and `BOFT` payload and
legacy-load tests.
