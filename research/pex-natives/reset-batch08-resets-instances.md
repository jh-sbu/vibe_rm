# Resets and quest instances

Public pages consulted 2026-10-24 (Papyrus Index / community CK wiki text,
summarized), https://papyrus.bellcube.dev/skyrimse/script/ `objectreference/
function/reset` (back to its original location, inventory reset, an actor
resurrected, "in general tries to get it back to its original state"; may move to
`akTarget` instead, only if loaded; faction and relationship data aren't reset;
actors may need `MoveToMyEditorLocation` after), `cell/function/reset` ("flags
this cell for reset on next load"), `quest/function/reset` ("resets the quest"),
`quest/function/updatecurrentinstanceglobal` (updates the stored value of a
global in the quest's current instance; false when there is no active instance or
the global isn't in the quest's text global list), `quest/function/isstarting` /
`isstopping` (enabled but not running yet / not enabled but still shutting down).
Local UESP [QUST](https://en.uesp.net/w/index.php?oldid=2384538) lists `QTGL`
(text display globals). Vanilla scripts call `Quest.Reset` nowhere
(`vrm-tool pex-calls`: the 81 calls of `Reset` are `Cell.Reset` and
`ObjectReference.Reset`); `UpdateCurrentInstanceGlobal` is called by the Hearthfire
house quests to refresh a displayed count (`<Global=...>` in an objective).

Implementation (`src/resets.rs`; inference where marked). Saved as `RSTC` and `QGLB`:
- `ObjectReference.Reset(akTarget)`: clears what scripts and play changed on the
  reference: where it was moved, enable state, locked state and level, destruction,
  broken joints, behaviour-graph state, open doors, harvest, dropped-weapon charge;
  a container or actor has its inventory rolled again; an actor is resurrected,
  healed and sent to its placed place; a loose object is put back at rest where it
  was placed; an open door is shut. With a loaded `akTarget` it ends up there.
  Factions, relationships, owners and alias fills stay. Scripts hear `OnReset`.
  A reference made while playing is left alone.
- `Cell.Reset`: flags the cell; when it next loads (interior entered, exterior
  cell loaded) every reference in it is restored as above (without `OnReset`, as
  scripts attach after). Actors the cell's schedule moved are left where they are.
- `Quest.Reset`: stopped, with its state back to a fresh quest (stages, objectives,
  texts, log, aliases, flags). Script variables and `OnInit` are left alone: no VM
  helper rebuilds an attached script (ROADMAP).
- `UpdateCurrentInstanceGlobal`: a running quest keeps a copy of each of its
  `QTGL` globals, taken when its text first shows it, used for `<Global=...>` in
  its text; the call takes the global's value now and words the displayed
  objectives again. A stopped or reset quest drops its copies.
- `IsStarting` / `IsStopping`: false; quests start and stop at once here.

Tests: a synthetic cell, container, quest and globals.
