# Batch 2: five runtime queries

Public Papyrus Index / community CK wiki contracts consulted 2026-10-24:
- https://papyrus.bellcube.dev/skyrimse/script/quest/function/isstopped/ : no longer enabled or running.
- https://papyrus.bellcube.dev/skyrimse/script/game/function/isfasttravelcontrolsenabled/ : script EnableFastTravel gate.
- https://papyrus.bellcube.dev/skyrimse/script/objectreference/function/getkey/ : unlocking key or None.
- https://papyrus.bellcube.dev/skyrimse/script/objectreference/function/getvoicetype/ : actors and talking activators.
- https://papyrus.bellcube.dev/skyrimse/script/cell/function/isattached/ : cell in the loaded area.

Local UESP records consulted first:
- https://en.uesp.net/w/index.php?oldid=2354868 (REFR locks).
- https://en.uesp.net/w/index.php?oldid=3650076 (NPC voice).
- https://en.uesp.net/w/index.php?oldid=2309040 (TACT VNAM voice).

Implementation maps attachment to loaded cells, lock keys to the shared door-lock
holder, actor voices to the existing template-aware voice resolver, and stopped
quests to the existing synchronous quest state. These are integrations with this
runtime, not claims of Skyrim experimental verification. Tests use generated
records; no assets copied or engine binaries inspected.
