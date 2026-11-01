# Script variable conditions (GetScriptVariable, GetQuestVariable)

Implemented: `GetScriptVariable` (53) in `function_value` (`src/condition/mod.rs`),
tests in `src/condition/variable_tests.rs`. `GetQuestVariable` (79) is left
unsupported (below).

## What is known

- The string parameter of a CTDA is its own CIS1 (parameter 1) or CIS2 (parameter 2)
  subrecord after the CTDA; the CTDA's value for it is a made-up number. UESP Skyrim
  Mod:Mod File Format/CTDA Field (permanent link oldid=2346685, "Field Layout"), and
  Skyrim Mod talk:Mod File Format/CTDA Field (oldid=833409), which shows
  `GetVMQuestVariable` (629) with `::name_var` in CIS2 and incrementing numbers.
- Signatures: `[GetScriptVariable](ObjectReferenceID, Variable Name)` and
  `[GetQuestVariable]([Quest], [Variable] Name)`, UESP Skyrim Mod:Mod File Format/Raw
  Function Data (oldid=3411473); indices in Skyrim Mod:Function Indices (oldid=3566882).
- Vanilla (every plugin `vrm-tool ctda-uses` loads from the install's Data directory):
  no condition calls 53. Calls 79 appear 29 times (INFO, PACK, SCEN),
  always on a quest with a small integer in parameter 2 and no CIS2 subrecord (for
  example `GetQuestVariable(CW, 27)` in PACK 0002EA48, `(DialogueWinterhold, 8)` in
  INFO 0002C6D5). The number names no string in the record. Vanilla 629 (the
  Papyrus form) carries CIS2 names; 79 does not.

## Unconfirmed choices

- 53 is 0 whenever it names a variable (CIS2); no name means the condition
  fails. In the game (`test-mod autotest22`), `GetScriptVariable(Alpha,
  "Marker") == 7` failed run on another actor and on Alpha itself, while
  Alpha's Papyrus script had `Int Marker = 7`, and a missing variable `== 0`
  held. So it doesn't read Papyrus variables; it likely reads Oblivion-era
  script locals (SCRI / SCRV), which Skyrim's data doesn't have. Vanilla
  never calls it.
- 79 stays unsupported: its variable is a number, and nothing in the record, the
  quest's VMAD, or the public sources we checked says which Papyrus variable a number
  names (`setpqv` in Skyrim:Console takes the variable's name). A mapping from quest
  variable numbers to names would need the Creation Kit's own table.
