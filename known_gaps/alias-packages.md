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

## Open questions

- Order among aliases of quests with the same priority (here: by quest and alias
  id), and among several aliases of one quest.
- Aliases are only filled by forced references and unique actors; "find matching
  reference", "created reference", "from event" and location aliases aren't, so
  their packages never apply. A stopped quest's aliases stay filled but are
  ignored.
- Alias location kind 9 (location alias), object id / type targets and
  "Interrupt data" targets aren't resolved (editor location / none).
