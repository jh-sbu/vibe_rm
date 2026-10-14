# Quest alias packages

Implemented in `src/ai/alias.rs`. Based on the Creation Kit wiki's *Quest Alias Tab*
(alias packages run on whatever fills the alias while the quest runs, ahead of
the actor's own) and UESP *Mod File Format/QUST* (`ALST` ... `ALPC` ... `ALED`) and
*PACK* (`QNAM` owner quest; `PLDT` / `PTDA` alias kinds).

## What is known

- `vrm-tool alias-packages <data>`: 5,190 alias package slots on 2,583 aliases
  (Travel 1,674, Sandbox 1,010, ForceGreet 311, Follow 169, Escort 35...); 2,741 of
  7,633 packages name an owner quest. Location kind 8 (755 uses) and target kind
  4 (289) are reference aliases.
- Packages of an actor's aliases come first, by quest priority (`DNAM` byte 2),
  then its own; conditions run with the package's quest (so alias-run conditions
  resolve), alias locations and targets resolve through that quest's fills.
- Whereabouts of persistent actors follow alias packages too.

- At the same priority, the alias filled last comes first (`test-mod
  autotest4`): of two quests' aliases forced to one NPC, the second quest's
  package ran whichever went first; of one quest's two aliases forced to it,
  the second forced's (alias 0, forced after alias 1) ran.

## Open questions

- Whether "filled last" is the right reading: in the game's tests start and
  fill order went together, and alias 0 was also the lower id.
