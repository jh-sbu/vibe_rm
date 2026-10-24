# Batch 5: actor context

Public Papyrus Index (community CK wiki attribution), consulted 2026-10-24:
- https://papyrus.bellcube.dev/skyrimse/script/actor/function/ischild/
- https://papyrus.bellcube.dev/skyrimse/script/actor/function/isoverencumbered/
- https://papyrus.bellcube.dev/skyrimse/script/actor/function/getcurrentpackage/
- https://papyrus.bellcube.dev/skyrimse/script/actor/function/getdialoguetarget/
- https://papyrus.bellcube.dev/skyrimse/script/objectreference/function/getcurrentscene/

Local UESP first: Carry Weight revision 3630670
(https://en.uesp.net/w/index.php?oldid=3630670), RACE revision 3650080
(https://en.uesp.net/w/index.php?oldid=3650080). Child classification uses the
existing template-aware race resolver and race child flag. Encumbrance initializes
inventory and uses the same capacity/perk calculations as movement. Current
package comes from the loaded actor's selected package or unloaded schedule.
Scene membership uses running scene alias bindings. Player dialogue targets
return None as explicitly documented; an NPC in the conversation targets the
player. Scene dialogue uses its speaking actor's explicit headtracking target;
this is an inference (see ambiguity notes).

Tests use synthetic records. No assets copied or engine binaries inspected.
