# Magic effects' Perk to Apply

Implemented in `src/magic.rs` and `src/perks.rs`. Research used the local
UESP index and public Creation Kit documentation. No engine binary data or
game assets were examined or added for this change. Tests generate records;
their results describe this implementation, not observations in Skyrim SE.

The non-GPU source-overlay regression covers suspension, resumption, duplicate
sources, permanent ranks and NPC exclusion. The ignored Engine integration
test additionally covers parsing, skill XP and explicit perk ownership, but
could not run here: constructing the renderer requires 17 fragment-stage
sampled textures and the available adapter exposes 16. It compiled successfully;
the adapter validation failure occurs before its gameplay assertions.

## Evidence

- [UESP's MGEF format, revision 3507756](https://en.uesp.net/w/index.php?oldid=3507756)
  identifies the `DATA` field at byte `0x88` as a PERK form ID. The reader
  resolves it through the plugin's form-ID mapping and treats absent trailing
  data as no perk, retaining support for short synthetic records.
- The public [Creation Kit Magic Effect documentation](https://ck.uesp.net/w/index.php?section=16&title=Magic_Effect)
  describes Perk to Apply as active for the effect's lifetime and restricted
  to the player. The search engine's indexed page text was available; fetching
  the full page returned HTTP 403. This is documentation, not a local-game
  verification. The implementation follows the player restriction.
- [UESP's Leveling page, revision 3636185](https://en.uesp.net/w/index.php?oldid=3636185)
  documents the Guardian Stone and rested skill-experience bonuses. Existing
  `known_ambiguities/skills.md` describes the Mod Skill Use entry point and
  corresponding spell/perk records.

## Implemented policy

An active, unfinished effect on the player contributes its valid perk at rank
one to `HasPerk` and existing perk entry-point hooks. The contribution is
derived from effects on each query; it does not enter the permanent ownership
cache or script-added/removed perk lists. Condition suspension, dispelling,
expiration and resumption therefore follow the existing active-effect state.
The query does not reevaluate effect conditions, avoiding recursive HasPerk
condition evaluation.

Multiple active effects supplying one perk contribute it once. A permanent
copy also contributes once and retains its authored rank. Ending one source
leaves other active or permanent sources intact. `AddPerk` while an effect
supplies the perk creates a permanent source; `RemovePerk` changes permanent
sources and leaves an active effect's contribution in place. These source
overlap and explicit-removal rules are compatibility policies inferred from
the lifetime description, not behavior measured in Skyrim SE.

## Unresolved

- Temporary perks' ability and quest-stage sections are not executed. The
  documentation does not establish their lifecycle, especially an ability
  which grants the perk that supplies that same ability. `actor_spells` uses
  permanent perk sources to prevent a temporary ability retaining its own
  source indefinitely. Such sections need public evidence or game tests;
  ordinary AddPerk sections retain their existing behavior.
- Rank one, duplicate grants, priority ties against permanent perks, and
  AddPerk/RemovePerk interactions need comparative game tests. The effect
  field supplies no rank. No tests against Skyrim SE were performed.
- The existing ability/effect synchronization interval still determines when
  changed effect conditions suspend or resume a grant. This change does not
  establish Skyrim's exact reevaluation cadence.
- Derived grants are deliberately not saved as permanent perks; they follow
  the active effects native saves keep (`AEFF`, `known_ambiguities/saves.md`).

## Validation limits

The non-GPU regression exercises the production source-overlay helper for
active/suspended/finished effects, overlapping grants, permanent ranks and NPC
exclusion. The synthetic Engine integration test builds but cannot reach its
assertions on the available software adapter: renderer initialization needs
17 sampled textures per fragment stage while the adapter supports 16. Its
real skill-use, explicit AddPerk/RemovePerk and record-decoding checks therefore
remain unverified here; the test is retained as an ignored integration test.
